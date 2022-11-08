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
use indicatif::{ProgressDrawTarget, ProgressStyle};
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
        let insert_count = self.count_edge_inserts();
        log::debug!("insert count: {}", insert_count);
        let draw_target = ProgressDrawTarget::stderr_with_hz(0.1);
        let progress = indicatif::ProgressBar::new(insert_count as u64);
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
            let edge_count = self.edge_count.clone();
            Self::load_edges_dump_chunk(lines, sender, edge_count);
        });
        progress.finish();
        self.edge_count.load(Ordering::Relaxed)
    }

    pub fn count_edge_inserts(&self) -> usize {
        log::debug!("counting inserts in pagelinks sql");
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let pagelinks_sql = BufReader::new(pagelinks_sql_file);
        // 56034
        pagelinks_sql
            .lines()
            .filter(|line_res| {
                let line = line_res.as_ref().expect("read line");
                return line.starts_with("INSERT ");
            })
            .count()
    }

    fn load_edges_dump_chunk(
        chunk: Vec<String>,
        sender: Sender<WPPageLink>,
        count: Arc<AtomicU32>,
    ) {
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
                        dest_page_title: title.0.replace("_", " "),
                    })
                } else {
                    None
                }
            },
        );
        for link in links {
            count.fetch_add(1, Ordering::Relaxed);
            sender.send(link).expect("send wppagelink");
        }
    }
}
