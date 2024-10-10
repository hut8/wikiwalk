use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use crossbeam::channel::{Receiver, Sender};
use parse_mediawiki_sql::schemas::LinkTarget;

use crate::DirectLink;

// WPPageLinkSource is an iterator of edges
pub struct WPPageLinkSource {
    sender: Sender<DirectLink>,
    pagelink_source_path: PathBuf,
    linktarget_source_path: PathBuf,
    pub edge_count: Arc<AtomicU32>,
}

impl WPPageLinkSource {
    pub fn new(
        pagelink_source_path: PathBuf,
        linktarget_source_path: PathBuf,
        sender: Sender<DirectLink>,
    ) -> WPPageLinkSource {
        WPPageLinkSource {
            pagelink_source_path,
            linktarget_source_path,
            sender,
            edge_count: Arc::new(AtomicU32::default()),
        }
    }

    // pub async fn count_lines(self) -> usize {
    //     let pagelinks_sql_file = File::open(self.source_path).expect("open pagelinks file");
    //     let total_bytes = pagelinks_sql_file.metadata().expect("get metadata").len() as usize;
    //     let pagelinks_sql = flate2::read::GzDecoder::new(pagelinks_sql_file);
    //     let pagelinks_sql = BufReader::new(pagelinks_sql);
    //     let line_count = pagelinks_sql.lines().count();
    //     log::info!(
    //         "pagelinks line count: {} total bytes: {} bytes per line: {}",
    //         line_count,
    //         total_bytes,
    //         (total_bytes as f64 / line_count as f64).round(),
    //     );
    //     line_count
    // }

    pub async fn run(self) -> u32 {
        log::info!("pagelinks load complete");
        let link_targets = Arc::new(Self::load_link_targets(self.linktarget_source_path.clone()));
        self.load_pagelinks(link_targets);
        self.edge_count.load(Ordering::Relaxed)
    }

    fn load_pagelinks(&self, link_targets: Arc<HashMap<u64, String>>) {
        let pagelinks_sql_file =
            File::open(&self.pagelink_source_path).expect("open pagelinks file");

        let pagelinks_sql_buf = BufReader::new(pagelinks_sql_file);
        let pagelinks_sql = flate2::bufread::GzDecoder::new(pagelinks_sql_buf);
        let pagelinks_reader = BufReader::new(pagelinks_sql);

        let pagelinks_line_iter = pagelinks_reader.lines();

        let (pagelink_chunk_tx, pagelink_chunk_rx): (Sender<String>, Receiver<String>) =
            crossbeam::channel::bounded(1024);

        let num_cpus = num_cpus::get() * 2;
        for _ in 0..num_cpus {
            let chunk_rx = pagelink_chunk_rx.clone();
            let pagelink_sender = self.sender.clone();
            let edge_count = self.edge_count.clone();
            let link_targets = link_targets.clone();
            std::thread::spawn(move || {
                for chunk in chunk_rx {
                    Self::load_pagelinks_dump_chunk(
                        chunk,
                        pagelink_sender.clone(),
                        link_targets.clone(),
                        edge_count.clone(),
                    );
                }
            });
        }

        // let byte_count = AtomicU64::new(0);

        pagelinks_line_iter
            .map(|l| l.expect("read line"))
            .filter(|line| line.starts_with("INSERT "))
            // .map(|line| {
            //     // let read_bytes = pagelinks_byte_counter.count();
            //     let read_bytes = byte_count.fetch_add(line.len() as u64, Ordering::Relaxed);
            //     let elapsed = start.elapsed();
            //     let bytes_per_second = read_bytes as f64 / elapsed.as_secs_f64();
            //     // let total_time_estimate = elapsed.mul_f64(total_bytes as f64 / read_bytes as f64);
            //     // let time_remaining = total_time_estimate - elapsed;
            //     log::info!(
            //         // "pagelinks load progress: {} {}/sec - {}% complete: {} remaining",
            //         "pagelinks load progress: {} {}/sec",
            //         human_bytes::human_bytes(read_bytes as f64),
            //         human_bytes::human_bytes(bytes_per_second),
            //         // 100_f64 * (read_bytes as f64 / total_bytes as f64),
            //         // indicatif::HumanDuration(time_remaining).to_string(),
            //     );
            //     line
            // })
            .for_each(|line| {
                pagelink_chunk_tx.send(line).expect("send chunk");
            });
        drop(pagelink_chunk_tx);
    }

    fn load_link_targets(linktarget_source_path: PathBuf) -> HashMap<u64, String> {
        let linktargets_sql_file =
            File::open(linktarget_source_path).expect("open linktargets file");
        let linktargets_sql_buf = BufReader::new(linktargets_sql_file);
        let linktargets_sql = flate2::bufread::GzDecoder::new(linktargets_sql_buf);
        let linktargets_reader = BufReader::new(linktargets_sql);

        let linktargets_line_iter = linktargets_reader.lines();

        let (chunk_tx, chunk_rx): (Sender<String>, Receiver<String>) =
            crossbeam::channel::bounded(1024);

        let (linktarget_tx, linktarget_rx): (Sender<(u64, String)>, Receiver<(u64, String)>) =
            crossbeam::channel::bounded(1024);

        let num_cpus = num_cpus::get() * 2;
        for _ in 0..num_cpus {
            let chunk_rx = chunk_rx.clone();
            let linktarget_tx = linktarget_tx.clone();
            std::thread::spawn(move || {
                for chunk in chunk_rx {
                    Self::load_link_target_dump_chunk(chunk, linktarget_tx.clone());
                }
            });
        }

        linktargets_line_iter
            .map(|l| l.expect("read line"))
            .filter(|line| line.starts_with("INSERT "))
            .for_each(|line| {
                chunk_tx.send(line).expect("send chunk");
            });
        drop(chunk_tx);

        let mut linktargets = HashMap::new();
        for link in linktarget_rx {
            linktargets.insert(link.0, link.1);
        }
        log::info!("linktargets load complete");
        linktargets
    }

    fn load_link_target_dump_chunk(chunk: String, sender: Sender<(u64, String)>) {
        use parse_mediawiki_sql::{field_types::PageNamespace, iterate_sql_insertions};
        let chunk = chunk.as_bytes();
        let mut sql_iterator = iterate_sql_insertions(chunk);
        let links = sql_iterator.filter_map(
            |LinkTarget {
                 id,
                 namespace,
                 title,
             }| {
                if namespace == PageNamespace(0) {
                    Some((id.0, title.0.replace('_', " ")))
                } else {
                    None
                }
            },
        );
        for link in links {
            sender.send(link.clone()).expect("send link target");
        }
    }

    fn load_pagelinks_dump_chunk(
        chunk: String,
        sender: Sender<DirectLink>,
        link_targets: Arc<HashMap<u64, String>>,
        count: Arc<AtomicU32>,
    ) {
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::PageLink,
        };
        let chunk = chunk.as_bytes();
        let mut sql_iterator = iterate_sql_insertions(chunk);
        let links = sql_iterator.filter_map(
            |PageLink {
                 from,
                 from_namespace,
                 target,
             }| {
                if from_namespace == PageNamespace(0) {
                    let title = link_targets.get(&(target.0)).cloned()?;
                    Some(DirectLink {
                        source_page_id: from.0,
                        dest_page_title: title,
                    })
                } else {
                    None
                }
            },
        );
        for link in links {
            count.fetch_add(1, Ordering::Relaxed);
            sender.send(link.clone()).expect("send wppagelink");
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // parse-mediawiki-sql no longer works with old format
    // #[test]
    // fn parse_pagelink_row_20230920() {
    //     let row = include_bytes!("testdata/pagelinks-row-20230920.sql");
    //     let links = parse_edges_dump_chunk(row);
    //     assert_ne!(links.len(), 0);
    // }
}
