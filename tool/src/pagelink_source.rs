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

    pub async fn run(self) -> u32 {
        let pagelinks_sql_file = File::open(&self.source_path).expect("open pagelinks file");
        let total_bytes = pagelinks_sql_file.metadata().expect("get metadata").len() as usize;
        let pagelinks_lock = Arc::new(pagelinks_sql_file);
        // let file_cell = RefCell::new(pagelinks_sql_file);
        // let file_lock = Arc::new(RwLock::new(file_cell));
        let pagelinks_byte_counter = Arc::new(ByteReadCounter::new(pagelinks_lock));
        let pagelinks_sql = flate2::read::GzDecoder::new(pagelinks_byte_counter.as_ref());
        let pagelinks_sql = BufReader::new(pagelinks_sql);
        let pagelinks_line_iter = pagelinks_sql.lines();
        let start = Instant::now();
        let lines_iter = pagelinks_line_iter
            .map(|l| l.expect("read line"))
            .filter(|line| line.starts_with("INSERT "))
            .map(|line| {
                let read_bytes = pagelinks_byte_counter.count();
                let elapsed = start.elapsed();
                let bytes_per_second = read_bytes as f64 / elapsed.as_secs_f64();
                let total_time_estimate = elapsed.mul_f64(total_bytes as f64 / read_bytes as f64);
                let time_remaining = total_time_estimate - elapsed;
                log::info!(
                    "pagelinks load progress: {} {}/sec - {}% complete: {} remaining",
                    human_bytes::human_bytes(read_bytes as f64),
                    human_bytes::human_bytes(bytes_per_second),
                    100_f64 * (read_bytes as f64 / total_bytes as f64),
                    indicatif::HumanDuration(time_remaining).to_string(),
                );
                line
            });
        stream::iter(lines_iter).for_each_concurrent(num_cpus::get(), |line| {
            let sender = self.sender.clone();
            let edge_count = self.edge_count.clone();
            async {
                Self::load_edges_dump_chunk(line, sender, edge_count);
            }
        }).await;

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
