use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::{collections::HashMap, fs::File};

use futures::stream::StreamExt;
use itertools::Itertools;
use parse_mediawiki_sql::field_types::PageNamespace;
use parse_mediawiki_sql::iterate_sql_insertions;
use parse_mediawiki_sql::schemas::Redirect;
use sea_orm::{ColumnTrait, DbConn, EntityTrait, QueryFilter};

use crate::schema;

pub struct RedirectMapBuilder {
    path: PathBuf,
    map_file: Arc<RwLock<RedirectMapFile>>,
}

impl RedirectMapBuilder {
    pub fn new(path: PathBuf, map_file: Arc<RwLock<RedirectMapFile>>) -> RedirectMapBuilder {
        RedirectMapBuilder { path, map_file }
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

    pub async fn build(&self, db: DbConn) -> u32 {
        log::info!("parsing redirects table at {}", &self.path.display());

        let redirect_sql_file = File::open(&self.path).expect("open redirects file");
        let redirect_sql = flate2::read::GzDecoder::new(redirect_sql_file);
        let redirect_sql = BufReader::new(redirect_sql);
        let redirect_line_iter = redirect_sql.lines();

        let (tx, rx) = std::sync::mpsc::channel::<HashMap<u32, u32>>();
        let map_file = self.map_file.clone();
        let reducer = tokio::task::spawn_blocking(move || Self::reduce(rx, map_file));
        let lines = redirect_line_iter
            .map(|l| l.expect("read line"))
            .filter(|l| l.starts_with("INSERT "));
        futures::stream::iter(lines)
            .for_each_concurrent(num_cpus::get(), |line| {
                let tx = tx.clone();
                let db = db.clone();
                Self::parse_line(db, line, tx)
            })
            .await;
        drop(tx);

        reducer.await.expect("join reducer")
    }

    fn reduce(
        rx: std::sync::mpsc::Receiver<HashMap<u32, u32>>,
        map_file: Arc<RwLock<RedirectMapFile>>,
    ) -> u32 {
        let mut count = 0_u32;
        let mut write_guard = map_file.as_ref().write().unwrap();
        for chunk in rx.iter() {
            for (from, to) in chunk {
                write_guard.set(from, to);
                count += 1;
            }
        }
        log::info!("reduced to {} redirects", count);
        count
    }
}

pub struct RedirectMapFile {
    map: memmap2::MmapMut,
}

impl RedirectMapFile {
    pub fn new(path: PathBuf, max_page_id: u32) -> anyhow::Result<RedirectMapFile> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .expect("open edge proc db file");
        let file_size: u64 = max_page_id as u64 * 4;
        file.set_len(file_size)?;

        let map = unsafe { memmap2::MmapMut::map_mut(&file).expect("map anon") };
        Ok(RedirectMapFile { map })
    }

    pub fn get(&self, from: u32) -> Option<u32> {
        let u32_size = std::mem::size_of::<u32>();
        let offset = (from as usize) * u32_size;
        let to = u32::from_le_bytes(self.map[offset..offset + u32_size].try_into().unwrap());
        if to == 0 {
            None
        } else {
            Some(to)
        }
    }

    pub fn set(&mut self, from: u32, to: u32) {
        let u32_size = std::mem::size_of::<u32>();
        let offset = (from as usize) * u32_size;
        if offset + u32_size > self.map.len() {
            log::error!(
                "will not set redirect for {} to {}: out of bounds!",
                from,
                to
            );
            return;
        }
        self.map[offset..offset + u32_size].copy_from_slice(&to.to_le_bytes());
    }
}
