use std::collections::HashMap;
use std::path::PathBuf;

use parse_mediawiki_sql::field_types::PageNamespace;
use parse_mediawiki_sql::iterate_sql_insertions;
use parse_mediawiki_sql::schemas::Redirect;
use parse_mediawiki_sql::utils::memory_map;

pub struct RedirectMap {
    path: PathBuf,
    redirects: HashMap<u32, String>,
}

impl RedirectMap {
    pub fn new(path: PathBuf) -> RedirectMap {
        RedirectMap {
            path,
            redirects: HashMap::new(),
        }
    }

    pub fn parse(&mut self) {
      log::debug!("parsing redirects table");
        let redirects_sql = unsafe { memory_map(&self.path).unwrap() };
        iterate_sql_insertions(&redirects_sql).for_each(
            |Redirect {
                 namespace,
                 title,
                 from,
                 ..
             }| {
                if namespace == PageNamespace(0) {
                    let title = title.0.to_lowercase().replace("_", " ");
                    let from = from.0;
                    self.redirects.insert(from, title);
                }
            },
        );
        log::debug!("parsed {} redirects", self.redirects.len());
    }

    pub fn get(&self, from: u32) -> Option<String> {
        self.redirects.get(&from).cloned()
    }
}
