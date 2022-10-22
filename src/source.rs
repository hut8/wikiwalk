use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use crossbeam::channel::Sender;
use indicatif::{ProgressDrawTarget, ProgressStyle};
use rayon::prelude::*;

use crate::WPPageLink;

// EdgeSource is an iterator of edges
pub struct WPPageLinkSource {
    sender: Sender<WPPageLink>,
    source_path: PathBuf,
    pub insert_count: usize,
}

impl WPPageLinkSource {
    pub fn new(source_path: PathBuf, sender: Sender<WPPageLink>) -> WPPageLinkSource {
        WPPageLinkSource {
            source_path,
            sender,
            insert_count: 0,
        }
    }

    pub fn run(self) {
        let draw_target = ProgressDrawTarget::stderr_with_hz(0.1);
        let progress = indicatif::ProgressBar::new(self.insert_count as u64);
        progress.set_style(
              ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {human_pos}/{human_len:7} {percent}% {per_sec:5} {eta}").unwrap(),
          );
        progress.set_draw_target(draw_target);

        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let pagelinks_sql = BufReader::new(pagelinks_sql_file);
        let pagelinks_line_iter = pagelinks_sql.lines();

        pagelinks_line_iter.par_bridge().for_each(|chunk| {
            let line = chunk.expect("read line");
            if !line.starts_with("INSERT ") {
                return;
            }
            let lines = vec![line];
            progress.inc(1);
            let sender = self.sender.clone();
            Self::load_edges_dump_chunk(lines, sender);
        });
        progress.finish();
    }

    pub fn count_edge_inserts(&mut self) -> usize {
        if self.insert_count != 0 {
            return self.insert_count;
        }
        log::debug!("counting inserts in pagelinks sql");
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let pagelinks_sql = BufReader::new(pagelinks_sql_file);
        // 56034
        self.insert_count = pagelinks_sql
            .lines()
            .filter(|line_res| {
                let line = line_res.as_ref().expect("read line");
                return line.starts_with("INSERT ");
            })
            .count();
        self.insert_count
    }

    fn load_edges_dump_chunk(chunk: Vec<String>, sender: Sender<WPPageLink>) {
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
                        source_page_id: from.0,
                        dest_page_title: title.0.to_lowercase(),
                    })
                } else {
                    None
                }
            },
        );
        for link in links {
            sender.send(link).expect("send wppagelink");
        }
    }
}
