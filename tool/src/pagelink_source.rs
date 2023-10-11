use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    sync::{
        atomic::{self, AtomicU32, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

use crossbeam::channel::Sender;
use futures::{stream, StreamExt};

use crate::WPPageLink;

// WPPageLinkSource is an iterator of edges
pub struct WPPageLinkSource {
    sender: Sender<WPPageLink>,
    source_path: PathBuf,
    pub edge_count: Arc<AtomicU32>,
}

struct ByteReadCounter<R> {
    count: atomic::AtomicUsize,
    inner: RwLock<R>,
}

impl<R> ByteReadCounter<R>
where
    R: Read + Sized + Send + Sync,
{
    fn new(inner: R) -> Self {
        Self {
            count: atomic::AtomicUsize::new(0),
            inner: RwLock::new(inner),
        }
    }
}

impl<R> Read for &ByteReadCounter<R>
where
    R: Read + Sized + Send + Sync,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self.inner.write().unwrap();
        let read = guard.read(buf)?;
        self.count.fetch_add(read, Ordering::Relaxed);
        Ok(read)
    }
}

impl<R: Read + Send + Sync> ByteReadCounter<R> {
    fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

impl WPPageLinkSource {
    pub fn new(source_path: PathBuf, sender: Sender<WPPageLink>) -> WPPageLinkSource {
        WPPageLinkSource {
            source_path,
            sender,
            edge_count: Arc::new(AtomicU32::default()),
        }
    }

    pub async fn count_lines(self) -> usize {
        let pagelinks_sql_file = File::open(self.source_path).expect("open pagelinks file");
        let total_bytes = pagelinks_sql_file.metadata().expect("get metadata").len() as usize;
        let pagelinks_sql = flate2::read::GzDecoder::new(pagelinks_sql_file);
        let pagelinks_sql = BufReader::new(pagelinks_sql);
        let line_count = pagelinks_sql.lines().count();
        log::info!(
            "pagelinks line count: {} total bytes: {} bytes per line: {}",
            line_count,
            total_bytes,
            (total_bytes as f64 / line_count as f64).round(),
        );
        line_count
    }

    pub async fn run(self) -> u32 {
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        //let total_bytes = pagelinks_sql_file.metadata().expect("get metadata").len() as usize;
        // let pagelinks_lock = Arc::new(pagelinks_sql_file);

        // let pagelinks_byte_counter = Arc::new(ByteReadCounter::new(pagelinks_lock));

        // let pagelinks_sql = flate2::read::GzDecoder::new(pagelinks_byte_counter.as_ref());
        let pagelinks_sql_buf = BufReader::new(pagelinks_sql_file);
        let pagelinks_sql = flate2::bufread::GzDecoder::new(pagelinks_sql_buf);
        let reader = BufReader::new(pagelinks_sql);

        let pagelinks_line_iter = reader.lines();
        let start = Instant::now();

        let (chunk_tx, chunk_rx): (Sender<String>, Receiver<String>) =
            crossbeam::channel::bounded(1024);
        let num_cpus = num_cpus::get() * 2;
        for i in 0..num_cpus {
            let chunk_rx = chunk_rx.clone();
            let sender = self.sender.clone();
            let edge_count = self.edge_count.clone();
            std::thread::spawn(move || {
                for chunk in chunk_rx {
                    log::info!("pagelinks load chunk: proc {}: {} bytes", i, chunk.len());
                    Self::load_edges_dump_chunk(chunk, sender.clone(), edge_count.clone());
                }
            });
        }

        let byte_count = AtomicU64::new(0);

        pagelinks_line_iter
            .map(|l| l.expect("read line"))
            .filter(|line| line.starts_with("INSERT "))
            .map(|line| {
                // let read_bytes = pagelinks_byte_counter.count();
                let read_bytes = byte_count.fetch_add(line.len() as u64, Ordering::Relaxed);
                let elapsed = start.elapsed();

                let bytes_per_second = read_bytes as f64 / elapsed.as_secs_f64();
                //                let total_time_estimate = elapsed.mul_f64(total_bytes as f64 / read_bytes as f64);
                // let time_remaining = total_time_estimate - elapsed;
                log::info!(
                    // "pagelinks load progress: {} {}/sec - {}% complete: {} remaining",
                    "pagelinks load progress: {} {}/sec",
                    human_bytes::human_bytes(read_bytes as f64),
                    human_bytes::human_bytes(bytes_per_second),
                    // 100_f64 * (read_bytes as f64 / total_bytes as f64),
                    // indicatif::HumanDuration(time_remaining).to_string(),
                );
                line
            })
            .for_each(|line| {
                chunk_tx.send(line).expect("send chunk");
            });
        drop(chunk_tx);

        log::info!("pagelinks load complete");
        self.edge_count.load(Ordering::Relaxed)
    }

    fn load_edges_dump_chunk(chunk: String, sender: Sender<WPPageLink>, count: Arc<AtomicU32>) {
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::PageLink,
        };
        let chunk = chunk.as_bytes();
        let mut sql_iterator = iterate_sql_insertions(chunk);
        let links = sql_iterator.filter_map(
            |PageLink {
                 from,
                 from_namespace,
                 namespace,
                 title,
                 target: _, // Note: target is always None in current dump as of 20231001
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
