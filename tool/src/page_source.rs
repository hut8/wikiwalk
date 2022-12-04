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

use crate::Vertex;

// WPPageLinkSource is an iterator of vertexes
pub struct WPPageSource {
    sender: Sender<Vertex>,
    source_path: PathBuf,
    pub vertex_count: Arc<AtomicU32>,
}

impl WPPageSource {
    pub fn new(source_path: PathBuf, sender: Sender<Vertex>) -> WPPageSource {
        WPPageSource {
            source_path,
            sender,
            vertex_count: Arc::new(AtomicU32::default()),
        }
    }

    pub fn run(self) -> u32 {
        let insert_count = self.count_vertex_inserts();
        log::debug!("insert count: {}", insert_count);
        let draw_target = ProgressDrawTarget::stderr_with_hz(0.1);
        let progress = indicatif::ProgressBar::new(insert_count as u64);
        progress.set_style(
              ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {human_pos}/{human_len:7} {percent}% {per_sec:5} {eta}").unwrap(),
          );
        progress.set_draw_target(draw_target);

        let page_sql_file = File::open(&self.source_path).expect("open page file");
        let page_sql = BufReader::new(page_sql_file);
        let page_line_iter = page_sql.lines();

        page_line_iter.par_bridge().for_each(|chunk| {
            let line = chunk.expect("read line");
            if !line.starts_with("INSERT ") {
                return;
            }
            let lines = vec![line];
            progress.inc(1);
            let sender = self.sender.clone();
            let vertex_count = self.vertex_count.clone();
            Self::load_vertexes_dump_chunk(lines, sender, vertex_count);
        });
        progress.finish();
        self.vertex_count.load(Ordering::Relaxed)
    }

    pub fn count_vertex_inserts(&self) -> usize {
        log::debug!("counting inserts in page sql");
        let page_sql_file = File::open(&self.source_path).expect("open page file");
        let page_sql = BufReader::new(page_sql_file);
        page_sql
            .lines()
            .filter(|line_res| {
                let line = line_res.as_ref().expect("read line");
                line.starts_with("INSERT ")
            })
            .count()
    }

    fn load_vertexes_dump_chunk(chunk: Vec<String>, sender: Sender<Vertex>, count: Arc<AtomicU32>) {
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::Page,
        };
        let chunk_str = chunk.join("\n");
        let chunk = chunk_str.as_bytes();
        let mut iterator = iterate_sql_insertions(chunk);
        let vertexes = iterator.filter_map(
            |Page {
                 id,
                 namespace,
                 is_redirect,
                 title,
                 ..
             }| {
                if namespace == PageNamespace(0) {
                    Some(Vertex {
                        id: id.0,
                        title: title.0.replace('_', " "),
                        is_redirect,
                    })
                } else {
                    None
                }
            },
        );
        for vertex in vertexes {
            count.fetch_add(1, Ordering::Relaxed);
            sender.send(vertex).expect("send vertex");
        }
    }
}
