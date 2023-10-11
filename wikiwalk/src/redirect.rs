use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::RwLock;
use std::{collections::HashMap, fs::File};

use futures::stream::StreamExt;
use itertools::Itertools;
use parse_mediawiki_sql::field_types::PageNamespace;
use parse_mediawiki_sql::iterate_sql_insertions;
use parse_mediawiki_sql::schemas::Redirect;
use sea_orm::{ColumnTrait, DbConn, EntityTrait, QueryFilter};

use crate::schema;

pub struct RedirectMap {
    path: PathBuf,
    redirects: RwLock<RefCell<HashMap<u32, u32>>>,
}

impl RedirectMap {
    pub fn new(path: PathBuf) -> RedirectMap {
        RedirectMap {
            path,
            redirects: RwLock::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn len(&self) -> usize {
        self.redirects.read().unwrap().take().len()
    }

    pub fn is_empty(&self) -> bool {
        self.redirects.read().unwrap().take().is_empty()
    }

    pub async fn parse_line(
        db: DbConn,
        line: String,
        tx: std::sync::mpsc::Sender<HashMap<u32, u32>>,
    ) {
        let mut redirect_iter = iterate_sql_insertions(line.as_bytes());
        let redirect_iter = redirect_iter.filter_map(
            |Redirect {
                 namespace,
                 title,
                 from,
                 ..
             }| {
                if namespace == PageNamespace(0) {
                    let title = title.0.replace('_', " ");
                    let from = from.0;
                    Some((from, title))
                } else {
                    None
                }
            },
        );
        // resolves redirect destination (which is a title) to the actual page
        // this operation is chunked to minimize separate SQL queries
        let mut redirects = HashMap::new();
        for chunk in redirect_iter.chunks(32760).into_iter() {
            let chunk_lookup: HashMap<String, u32> = HashMap::new();
            let chunk_vec: Vec<(u32, String)> = chunk.into_iter().collect();
            let titles = chunk_vec.iter().map(|f| f.1.clone()).unique();
            let vertexes = schema::vertex::Entity::find()
                .filter(schema::vertex::Column::Title.is_in(titles))
                .all(&db)
                .await
                .expect("query vertexes by title");
            let chunk_lookup = vertexes.into_iter().fold(chunk_lookup, |mut accum, elm| {
                accum.insert(elm.title, elm.id);
                accum
            });

            for redir in chunk_vec {
                let dest = chunk_lookup.get(&redir.1);
                match dest {
                    Some(dest_id) => {
                        redirects.insert(redir.0, *dest_id);
                    }
                    None => {
                        log::warn!("page title: {} in redirects has no page entry", redir.1);
                    }
                }
            }
        }
        tx.send(redirects).expect("send redirects");
    }

    pub async fn parse(&self, db: DbConn) -> HashMap<u32, u32> {
        log::info!("parsing redirects table at {}", &self.path.display());

        let redirect_sql_file = File::open(&self.path).expect("open redirects file");
        let redirect_sql = flate2::read::GzDecoder::new(redirect_sql_file);
        let redirect_sql = BufReader::new(redirect_sql);
        let redirect_line_iter = redirect_sql.lines();

        let (tx, rx) = std::sync::mpsc::channel::<HashMap<u32, u32>>();
        let reducer = tokio::task::spawn_blocking(move || Self::reduce(rx));
        let lines = redirect_line_iter
            .map(|l| l.expect("read line"))
            .filter(|l| l.starts_with("INSERT "));
        futures::stream::iter(lines)
            .for_each_concurrent(16, |line| {
                let tx = tx.clone();
                let db = db.clone();
                Self::parse_line(db, line, tx)
            })
            .await;
        drop(tx);

        reducer.await.expect("join reducer")
    }

    fn reduce(rx: std::sync::mpsc::Receiver<HashMap<u32, u32>>) -> HashMap<u32, u32> {
        let mut reduced: HashMap<u32, u32> = HashMap::new();
        for chunk in rx.iter() {
            log::info!("received chunk of {} redirects", chunk.len());
            reduced.extend(chunk);
        }
        log::info!("reduced to {} redirects", reduced.len());
        reduced
    }

    pub fn get(&self, from: u32) -> Option<u32> {
        let guard = self.redirects.read().unwrap();
        guard.take().get(&from).cloned()
    }
}
