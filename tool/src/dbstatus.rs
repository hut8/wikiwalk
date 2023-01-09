use std::{path::PathBuf, thread, fs::File};

use memmap2::Mmap;
use sha3::{Digest, Sha3_256};
use wikipedia_speedrun::paths::{DumpPaths, DBPaths};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct DBStatus {
    pub dump_date: Option<String>,
    pub wp_page_hash: Option<Vec<u8>>,
    pub wp_pagelinks_hash: Option<Vec<u8>>,
    pub edges_resolved: Option<bool>,
    pub edges_sorted: Option<bool>,
    pub vertex_al_hash: Option<Vec<u8>>,
    pub vertex_al_ix_hash: Option<Vec<u8>>,
    pub build_complete: Option<bool>,
    #[serde(skip)]
    pub status_path: Option<PathBuf>,
}

impl DBStatus {
    pub fn compute(dump_paths: DumpPaths, db_paths: DBPaths) -> DBStatus {
        let dump_paths_t = dump_paths.clone();
        let wp_page_hash_thread = thread::spawn(move || Self::hash_file(dump_paths_t.page()));
        let dump_paths_t = dump_paths;
        let wp_pagelinks_hash_thread = thread::spawn(move || Self::hash_file(dump_paths_t.pagelinks()));
        let db_paths_t = db_paths.clone();
        let vertex_al_hash_thread = thread::spawn(move || Self::hash_file(db_paths_t.path_vertex_al()));
        let db_paths_t = db_paths;
        let vertex_al_ix_hash_thread = thread::spawn(move || Self::hash_file(db_paths_t.path_vertex_al_ix()));
        let wp_page_hash = wp_page_hash_thread.join().unwrap();
        let wp_pagelinks_hash = wp_pagelinks_hash_thread.join().unwrap();
        let vertex_al_hash = vertex_al_hash_thread.join().unwrap();
        let vertex_al_ix_hash = vertex_al_ix_hash_thread.join().unwrap();

        DBStatus {
            wp_page_hash,
            wp_pagelinks_hash,
            vertex_al_hash,
            vertex_al_ix_hash,
            edges_resolved: None,
            edges_sorted: None,
            build_complete: None,
            status_path: None,
            dump_date: None,
        }
    }

    pub fn load(status_path: PathBuf) -> DBStatus {
        match File::open(&status_path) {
            Ok(file) => {
                let mut val: DBStatus = serde_json::from_reader(file).unwrap();
                val.status_path = Some(status_path);
                val
            }
            Err(_) => DBStatus {
                wp_page_hash: None,
                wp_pagelinks_hash: None,
                build_complete: None,
                edges_resolved: None,
                edges_sorted: None,
                status_path: Some(status_path),
                dump_date: None,
                vertex_al_hash: None,
                vertex_al_ix_hash: None,
            },
        }
    }

    pub fn save(&self) {
        let sink = File::create(self.status_path.as_ref().unwrap()).unwrap();
        serde_json::to_writer_pretty(&sink, self).unwrap();
    }

    // Hash the last megabyte of the file
    fn hash_file(path: PathBuf) -> Option<Vec<u8>> {
        match File::open(path) {
            Ok(source) => {
                let source = unsafe { Mmap::map(&source).unwrap() };
                let mut hasher = Sha3_256::new();
                let max_tail_size: usize = 1024 * 1024;
                let tail_size = source.len().min(max_tail_size);
                let tail = (source.len() - tail_size)..source.len() - 1;
                hasher.update(&source[tail]);
                Some(hasher.finalize().to_vec())
            }
            Err(_) => None,
        }
    }
}
