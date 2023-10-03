use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use crossbeam::channel::Sender;
use itertools::Itertools;
use rayon::prelude::*;

use crate::WPPageLink;

// WPPageLinkSource is an iterator of edges
pub struct WPPageLinkSource {
    sender: Sender<WPPageLink>,
    source_path: PathBuf,
    pub edge_count: Arc<AtomicU32>,
}

impl WPPageLinkSource {
    pub fn new(source_path: PathBuf, sender: Sender<WPPageLink>) -> WPPageLinkSource {
        WPPageLinkSource {
            source_path,
            sender,
            edge_count: Arc::new(AtomicU32::default()),
        }
    }

    pub fn run(self) -> u32 {
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let pagelinks_sql = flate2::read::GzDecoder::new(pagelinks_sql_file);
        let pagelinks_sql = BufReader::new(pagelinks_sql);
        let pagelinks_line_iter = pagelinks_sql.lines();
        log::info!("loading pagelinks");
        pagelinks_line_iter.par_bridge().for_each(|chunk| {
            let line = chunk.expect("read line");
            if !line.starts_with("INSERT ") {
                log::debug!(
                    "skipping pagelinks line which does not start with INSERT: {}",
                    line
                );
                return;
            }
            let lines = vec![line];
            let sender = self.sender.clone();
            let edge_count = self.edge_count.clone();
            Self::load_edges_dump_chunk(lines, sender, edge_count);
        });
        log::info!("pagelinks load complete");
        self.edge_count.load(Ordering::Relaxed)
    }

    fn load_edges_dump_chunk(
        chunk: Vec<String>,
        sender: Sender<WPPageLink>,
        count: Arc<AtomicU32>,
    ) {
        let chunk_str = chunk.join("\n");
        let chunk = chunk_str.as_bytes();
        let links = parse_edges_dump_chunk(chunk);
        log::debug!(
            "sending {} links from chunk of size: {}",
            links.len(),
            chunk.len()
        );
        for link in links {
            count.fetch_add(1, Ordering::Relaxed);
            sender.send(link).expect("send wppagelink");
        }
    }
}

fn parse_edges_dump_chunk(chunk: &[u8]) -> Vec<WPPageLink> {
    use parse_mediawiki_sql::{
        field_types::PageNamespace, iterate_sql_insertions, schemas::PageLink,
    };
    let mut sql_iterator = iterate_sql_insertions(chunk);
    sql_iterator
        .filter_map(
            |PageLink {
                 from,
                 from_namespace,
                 namespace,
                 title,
             }| {
                if from_namespace == PageNamespace(0) && namespace == PageNamespace(0) {
                    Some(WPPageLink {
                        source_page_id: from.0,
                        dest_page_title: title.0.replace('_', " "),
                    })
                } else {
                    None
                }
            },
        )
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_pagelink_row_20230920() {
        let row = include_bytes!("testdata/pagelinks-row-20230920.sql");
        let links = parse_edges_dump_chunk(row);
        assert_ne!(links.len(), 0);
    }
    #[test]
    fn parse_pagelink_row_20231001() {
        let row = include_bytes!("testdata/pagelinks-row-20231001.sql");
        let links = parse_edges_dump_chunk(row);
        assert_ne!(links.len(), 0);
    }
}
