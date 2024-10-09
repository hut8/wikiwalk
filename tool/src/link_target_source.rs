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

// WPLinkTargetSource is a simple loader for the link target file.
pub struct WPLinkTargetSource {}

pub struct WPLinkTarget {
    pub id: u64,
    pub title: String,
}

impl WPLinkTargetSource {
    pub fn run(&mut self, source_path: &str) {
        log::info!("loading link target file");
        let (sender, receiver) = crossbeam::channel::bounded(1000);
        let source_path = source_path.to_string();
        let join_handle = std::thread::spawn(move || Self::run_producers(&source_path, sender));

        join_handle.join().expect("join producer thread");
        log::info!("link target load complete");
    }

    fn run_producers(source_path: &str, sender: Sender<WPLinkTarget>) {
        let sql_file = File::open(&source_path).expect("open page file");
        let link_target_sql = flate2::read::GzDecoder::new(sql_file);
        let link_target_sql = BufReader::new(link_target_sql);
        let line_iter = link_target_sql.lines();
        line_iter.par_bridge().for_each(|chunk| {
            let line = chunk.expect("read line");
            if !line.starts_with("INSERT ") {
                return;
            }
            let lines = vec![line];
            let sender = sender.clone();
            Self::load_link_target_dump_chunk(lines, sender);
        });
    }

    fn load_link_target_dump_chunk(chunk: Vec<String>, sender: Sender<WPLinkTarget>) {
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::LinkTarget,
        };
        let chunk_str = chunk.join("\n");
        let chunk = chunk_str.as_bytes();
        let mut iterator = iterate_sql_insertions(chunk);
        let vertexes = iterator.filter_map(
            |LinkTarget {
                 id,
                 namespace,
                 title,
                 ..
             }| {
                if namespace == PageNamespace(0) {
                    Some(WPLinkTarget {
                        id: id.0,
                        title: title.0.replace('_', " "),
                    })
                } else {
                    None
                }
            },
        );
        for vertex in vertexes {
            sender.send(vertex).expect("send vertex");
        }
    }
}
