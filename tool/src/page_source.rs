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
        log::info!("loading page file from {:?}", &self.source_path);
        let page_sql_file = File::open(&self.source_path).unwrap();
        let page_sql = flate2::read::GzDecoder::new(page_sql_file);
        let page_sql = BufReader::new(page_sql);
        let page_line_iter = page_sql.lines();
        page_line_iter.par_bridge().for_each(|chunk| {
            let line = chunk.expect("read line");
            if !line.starts_with("INSERT ") {
                return;
            }
            let lines = vec![line];
            let sender = self.sender.clone();
            let vertex_count = self.vertex_count.clone();
            Self::load_vertexes_dump_chunk(lines, sender, vertex_count);
        });
        log::info!("page file load complete");
        self.vertex_count.load(Ordering::Relaxed)
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
