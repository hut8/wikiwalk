use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use crossbeam::channel::{Receiver, Sender};
use itertools::Itertools;

use crate::WPPageLink;

// EdgeSource is an iterator of edges
pub struct WPPageLinkSource {
    sender: Sender<WPPageLink>,
    pub receiver: Receiver<WPPageLink>,
    source_path: PathBuf,
}

impl WPPageLinkSource {
    pub fn new(source_path: PathBuf) -> WPPageLinkSource {
        let (sender, receiver) = crossbeam::channel::bounded(16535);

        WPPageLinkSource {
            source_path,
            sender,
            receiver,
        }
    }

    pub fn run(&mut self) {
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let pagelinks_sql = BufReader::new(pagelinks_sql_file);
        let pagelinks_line_iter = pagelinks_sql.lines();
        let pagelinks_line_iter = pagelinks_line_iter
            .skip_while(|line| !line.as_ref().expect("read preamble").starts_with("INSERT"))
            .into_iter();

        let chunks = pagelinks_line_iter.chunks(1);

        rayon::scope(move |s| {
            for chunk in chunks.into_iter() {
                let line_iter = chunk.map(|l| l.expect("read line"));
                let lines = line_iter.collect_vec();
                let sender = self.sender.clone();
                s.spawn(move|_| Self::load_edges_dump_chunk(lines, sender));
            }
        });
    }

    pub fn count_edge_inserts(&self) -> usize {
        log::debug!("counting inserts in pagelinks sql");
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let pagelinks_sql = BufReader::new(pagelinks_sql_file);
        // 56034
        return pagelinks_sql
            .lines()
            .filter(|line_res| {
                let line = line_res.as_ref().expect("read line");
                return line.starts_with("INSERT ");
            })
            .count();
    }

    fn load_edges_dump_chunk(chunk: Vec<String>, sender: Sender<WPPageLink>) {
        log::debug!("load_edges_dump_chunk: load chunk of {}", chunk.len());
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::PageLink,
        };
        let chunk_str = chunk.join("\n");
        let chunk = chunk_str.as_bytes();
        let mut sql_iterator = iterate_sql_insertions(&chunk);
        let links = sql_iterator.filter_map(
            |PageLink {
                 from,
                 from_namespace,
                 namespace,
                 title,
             }| {
                if from_namespace == PageNamespace(0) && namespace == PageNamespace(0) {
                    Some(WPPageLink {
                        source_page_id: from,
                        dest_page_title: title,
                    })
                } else {
                    None
                }
            },
        );
        for link in links {
            sender.send(link);
        }
    }
}
