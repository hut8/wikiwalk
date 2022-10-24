use std::collections::HashMap;
use std::path::PathBuf;

use itertools::Itertools;
use parse_mediawiki_sql::field_types::PageNamespace;
use parse_mediawiki_sql::iterate_sql_insertions;
use parse_mediawiki_sql::schemas::Redirect;
use parse_mediawiki_sql::utils::memory_map;
use sea_orm::{ColumnTrait, DbConn, EntityTrait, QueryFilter};

use crate::schema;

pub struct RedirectMap {
    path: PathBuf,
    redirects: HashMap<u32, u32>,
}

impl RedirectMap {
    pub fn new(path: PathBuf) -> RedirectMap {
        RedirectMap {
            path,
            redirects: HashMap::new(),
        }
    }

    pub async fn parse(&mut self, db: DbConn) {
        log::debug!("parsing redirects table");
        let redirects_sql = unsafe { memory_map(&self.path).unwrap() };
        let mut redirect_iter = iterate_sql_insertions(&redirects_sql);
        let redirect_iter = redirect_iter.filter_map(
            |Redirect {
                 namespace,
                 title,
                 from,
                 ..
             }| {
                if namespace == PageNamespace(0) {
                    let title = title.0.replace("_", " ");
                    let from = from.0;
                    Some((from, title))
                } else {
                    None
                }
            },
        );
        // resolves redirect destination (which is a title) to the actual page
        // this operation is chunked to minimize separate SQL queries
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
                  self.redirects.insert(redir.0, *dest_id);
                }
                None => {
                  log::debug!("page title: {} in redirects has no page entry", redir.1);
                }
              }
            }
        }

        log::debug!("parsed {} redirects", self.redirects.len());
    }

    pub fn get(&self, from: u32) -> Option<u32> {
        self.redirects.get(&from).cloned()
    }
}
