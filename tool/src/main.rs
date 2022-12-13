use clap::{Parser, Subcommand};
use crossbeam::channel::Receiver;
use itertools::Itertools;
use memmap2::{Mmap, MmapMut};
use memory_stats::memory_stats;
use rayon::slice::ParallelSliceMut;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Index, Table, TableCreateStatement};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseBackend, DbBackend, DbConn, DeriveColumn,
    EntityTrait, EnumIter, QueryFilter, QuerySelect, Schema, Set, SqlxSqliteConnector, Statement,
    TransactionTrait,
};
use sha3::{Digest, Sha3_256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::hash::Hash;
use std::io::Write;
use std::io::{prelude::*, BufWriter};
use std::path::PathBuf;
use std::thread;
use wikipedia_speedrun::redirect::RedirectMap;
use wikipedia_speedrun::{edge_db, redirect, schema, Edge, GraphDB, Vertex};

mod page_source;
mod pagelink_source;

/// Intermediate type of only fields necessary to create an Edge
#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub struct WPPageLink {
    pub source_page_id: u32,
    pub dest_page_title: String,
}

enum EdgeSort {
    Incoming,
    Outgoing,
}

// flat file database for sorting/aggregating edges
struct EdgeProcDB {
    /// directory containing raw edge list and both sorted files
    root_path: PathBuf,
    writer: BufWriter<File>,
    fail_writer: BufWriter<File>,
    unflushed_inserts: usize,
}

impl EdgeProcDB {
    pub fn new(path: PathBuf) -> EdgeProcDB {
        std::fs::create_dir_all(&path).expect("create edge db directory");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path.join("edges"))
            .expect("open edge proc db file");
        let fail_log = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path.join("edges_fail.csv"))
            .expect("open edge proc db fail file");
        EdgeProcDB {
            root_path: path,
            writer: BufWriter::new(file),
            unflushed_inserts: 0,
            fail_writer: BufWriter::new(fail_log),
        }
    }

    pub fn truncate(self) -> Self {
        let mut file = self.writer.into_inner().unwrap();
        file.set_len(0).unwrap();
        file.rewind().unwrap();
        let mut fail_file = self.fail_writer.into_inner().unwrap();
        fail_file.set_len(0).unwrap();
        fail_file.rewind().unwrap();

        EdgeProcDB {
            root_path: self.root_path,
            writer: BufWriter::new(file),
            unflushed_inserts: 0,
            fail_writer: BufWriter::new(fail_file),
        }
    }

    pub fn write_edge(&mut self, edge: &Edge) {
        let edge_ptr = edge as *const Edge as *const u8;
        let edge_slice =
            unsafe { std::slice::from_raw_parts(edge_ptr, std::mem::size_of::<Edge>()) };
        self.writer.write_all(edge_slice).expect("write edge");
        self.unflushed_inserts += 1;
        if self.unflushed_inserts % 1024 == 0 {
            self.unflushed_inserts = 0;
            self.writer.flush().expect("flush edge proc db");
        }
    }

    pub fn write_fail(&mut self, source_vertex_id: u32, dest_page_title: String) {
        let line = format!("{source_vertex_id},{dest_page_title}\n");
        self.fail_writer
            .write_all(line.as_bytes())
            .expect("write edge fail");
    }

    fn sort_basename(sort_by: &EdgeSort) -> String {
        format!(
            "edges-{}",
            match sort_by {
                EdgeSort::Incoming => "incoming",
                EdgeSort::Outgoing => "outgoing",
            }
        )
    }

    fn open_sort_file(&self, sort_by: &EdgeSort) -> Mmap {
        let basename = Self::sort_basename(sort_by);
        let path = &self.root_path.join(basename);
        let source_file = OpenOptions::new()
            .read(true)
            .open(path)
            .expect("open edge db sort file as source");
        let map = unsafe { Mmap::map(&source_file).expect("mmap edge sort file") };
        Self::configure_mmap(&map);
        map
    }

    #[cfg(unix)]
    fn configure_mmap(mmap: &Mmap) {
        mmap.advise(memmap2::Advice::Sequential)
            .expect("set madvice sequential");
    }

    #[cfg(windows)]
    /// configure_mmap is a nop in Windows
    fn configure_mmap(_mmap: &Mmap) {}

    fn make_sort_file(&self, sort_by: &EdgeSort) -> (MmapMut, File) {
        let sink_basename = Self::sort_basename(sort_by);
        let sink_path = &self.root_path.join(sink_basename);
        std::fs::copy(self.root_path.join("edges"), sink_path).expect("copy file for sort");

        let sink_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(sink_path)
            .expect("open edge db sort file as sink");
        let map = unsafe { MmapMut::map_mut(&sink_file).expect("mmap edge sort file") };
        (map, sink_file)
    }

    pub fn write_sorted_by(&mut self, sort_by: EdgeSort) {
        let (mut sink, sink_file) = self.make_sort_file(&sort_by);

        log::debug!(
            "sorting edge db for direction: {}",
            match sort_by {
                EdgeSort::Incoming => "incoming",
                EdgeSort::Outgoing => "outgoing",
            }
        );
        let slice = &mut sink[..];
        let sink_byte_len = slice.len();
        let edges_ptr = slice.as_mut_ptr() as *mut Edge;
        let edges_len = sink_byte_len / std::mem::size_of::<Edge>();
        let edges = unsafe { std::slice::from_raw_parts_mut(edges_ptr, edges_len) };
        let sink_edge_len = edges.len();
        log::debug!("sink byte len={}", sink_byte_len);
        log::debug!("size of edge={}", std::mem::size_of::<Edge>());
        log::debug!("edge count={}", sink_edge_len);

        edges.par_sort_unstable_by(|x, y| match sort_by {
            EdgeSort::Incoming => x.dest_vertex_id.cmp(&y.dest_vertex_id),
            EdgeSort::Outgoing => x.source_vertex_id.cmp(&y.source_vertex_id),
        });
        sink.flush().expect("sink flush");
        drop(sink_file);
    }

    pub fn flush(&mut self) {
        self.writer.flush().expect("flush edge db");
    }

    pub fn iter(&self, max_page_id: u32) -> AdjacencySetIterator {
        let outgoing_source = self.open_sort_file(&EdgeSort::Outgoing);
        let incoming_source = self.open_sort_file(&EdgeSort::Incoming);

        AdjacencySetIterator {
            outgoing_source,
            incoming_source,
            incoming_i: 0,
            outgoing_i: 0,
            vertex_id: 0,
            max_page_id,
        }
    }
}

// AdjacencySet is an AdjacencyList combined with its vertex
struct AdjacencySet {
    adjacency_list: wikipedia_speedrun::edge_db::AdjacencyList,
}

struct AdjacencySetIterator {
    incoming_source: Mmap,
    outgoing_source: Mmap,
    incoming_i: usize,
    outgoing_i: usize,
    vertex_id: u32,
    max_page_id: u32,
}

impl Iterator for AdjacencySetIterator {
    type Item = AdjacencySet;

    // iterates over range of 0..max_page_id,
    // combining data in incoming_source and outgoing_source
    // into adjacency lists
    fn next(&mut self) -> Option<Self::Item> {
        // are we done yet?
        if self.vertex_id > self.max_page_id {
            log::debug!(
                "adjacency set iter: done after {} iterations",
                self.max_page_id
            );
            return None;
        }

        let mut val = AdjacencySet {
            adjacency_list: edge_db::AdjacencyList::default(),
        };

        // put in all the outgoing edges
        // outgoing source is sorted by source vertex id
        loop {
            let outgoing_offset: usize = self.outgoing_i * std::mem::size_of::<Edge>();
            if outgoing_offset >= self.outgoing_source.len() {
                break;
            }

            let current_edge: Edge = Edge::from_bytes(
                &self.outgoing_source
                    [outgoing_offset..outgoing_offset + std::mem::size_of::<Edge>()],
            );

            if current_edge.source_vertex_id > self.vertex_id {
                break;
            }
            if current_edge.source_vertex_id < self.vertex_id {
                panic!("current edge source vertex id={} is before current vertex id={}; edge was missed",
            current_edge.source_vertex_id, self.vertex_id);
            }

            if current_edge.dest_vertex_id > self.max_page_id {
                panic!(
                    "destination vertex id for edge: {:#?} is greater than max page id {}",
                    current_edge, self.max_page_id
                );
            }
            val.adjacency_list
                .outgoing
                .push(current_edge.dest_vertex_id);
            self.outgoing_i += 1;
        }

        // put in all the incoming edges
        // incoming source is sorted by destination vertex id
        loop {
            let incoming_offset: usize = self.incoming_i * std::mem::size_of::<Edge>();
            if incoming_offset >= self.incoming_source.len() {
                break;
            }

            let current_edge: Edge = Edge::from_bytes(
                &self.incoming_source
                    [incoming_offset..incoming_offset + std::mem::size_of::<Edge>()],
            );

            if current_edge.dest_vertex_id > self.vertex_id {
                break;
            }

            if current_edge.dest_vertex_id < self.vertex_id {
                panic!("current edge dest vertex id={} is before current vertex id={}; edge was missed",
              current_edge.dest_vertex_id, self.vertex_id);
            }

            if current_edge.source_vertex_id > self.max_page_id {
                panic!(
                    "source vertex id for edge: {:#?} is greater than max page id {}",
                    current_edge, self.max_page_id
                );
            }

            val.adjacency_list
                .incoming
                .push(current_edge.source_vertex_id);
            self.incoming_i += 1;
        }

        self.vertex_id += 1;

        Some(val)
    }
}

struct GraphDBBuilder {
    // inputs
    pub page_path: PathBuf,
    pub pagelinks_path: PathBuf,
    pub redirects_path: PathBuf,

    // outputs
    pub ix_path: PathBuf,
    pub al_path: PathBuf,

    // process directory
    process_path: PathBuf,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum QueryAs {
    MaxVertexId,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct DBStatus {
    wp_page_hash: Option<Vec<u8>>,
    wp_pagelinks_hash: Option<Vec<u8>>,
    edges_resolved: Option<bool>,
    edges_sorted: Option<bool>,
    build_complete: Option<bool>,
    #[serde(skip)]
    status_path: Option<PathBuf>,
}

impl DBStatus {
    pub fn compute(page_path: PathBuf, pagelinks_path: PathBuf) -> DBStatus {
        let wp_page_hash_thread = thread::spawn(|| Self::hash_file(page_path));
        let wp_pagelinks_hash_thread = thread::spawn(|| Self::hash_file(pagelinks_path));
        let wp_page_hash = Some(wp_page_hash_thread.join().unwrap());
        let wp_pagelinks_hash = Some(wp_pagelinks_hash_thread.join().unwrap());
        DBStatus {
            wp_page_hash,
            wp_pagelinks_hash,
            edges_resolved: None,
            edges_sorted: None,
            build_complete: None,
            status_path: None,
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
            },
        }
    }

    pub fn save(&self) {
        let sink = File::create(self.status_path.as_ref().unwrap()).unwrap();
        serde_json::to_writer_pretty(&sink, self).unwrap();
    }

    fn hash_file(path: PathBuf) -> Vec<u8> {
        let source = File::open(path).unwrap();
        let source = unsafe { Mmap::map(&source).unwrap() };
        let mut hasher = Sha3_256::new();
        let max_tail_size: usize = 1024 * 1024;
        let tail_size = source.len().min(max_tail_size);
        let tail = (source.len() - tail_size)..source.len() - 1;
        hasher.update(&source[tail]);
        hasher.finalize().to_vec()
    }
}

impl GraphDBBuilder {
    pub fn new(
        page: PathBuf,
        pagelinks: PathBuf,
        redirects_path: PathBuf,
        ix_path: PathBuf,
        al_path: PathBuf,
        process_path: PathBuf,
    ) -> GraphDBBuilder {
        GraphDBBuilder {
            page_path: page,
            pagelinks_path: pagelinks,
            ix_path,
            al_path,
            process_path,
            redirects_path,
        }
    }

    /// load vertexes from page.sql and put them in a sqlite file
    pub async fn build_database(&mut self) {
        let db_status_path = self.process_path.join("status.json");

        log::debug!("computing current and finished state of data files");
        let mut db_status = DBStatus::load(db_status_path.clone());
        let db_status_complete =
            DBStatus::compute(self.page_path.clone(), self.pagelinks_path.clone());

        // adjust status if pagelinks hash is mismatched
        let pagelink_data_changed = match db_status.wp_pagelinks_hash.as_ref() {
            Some(hash) => hash != db_status_complete.wp_pagelinks_hash.as_ref().unwrap(),
            None => true,
        };
        if pagelink_data_changed {
            log::info!("wp_pagelinks_hash mismatch; will recompute all edge data");
            db_status.build_complete = Some(false);
            db_status.edges_resolved = Some(false);
            db_status.edges_sorted = Some(false);
        }

        let db_path = self.process_path.join("wikipedia-speedrun.db");
        let conn_str = format!("sqlite:///{}?mode=rwc", db_path.to_string_lossy());
        log::debug!("using database: {}", conn_str);
        let opts = SqliteConnectOptions::new()
            .synchronous(SqliteSynchronous::Off)
            .journal_mode(SqliteJournalMode::Memory)
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await.expect("db connect");
        let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);

        let need_vertexes = match &db_status.wp_page_hash {
            Some(stat_hash) => db_status_complete
                .wp_page_hash
                .as_ref()
                .map(|complete_hash| complete_hash != stat_hash)
                .unwrap(),
            None => true,
        };

        if need_vertexes {
            log::info!("loading page.sql");
            self.load_vertexes_dump(db.clone()).await;
            self.create_vertex_title_ix(&db).await;
            db_status.wp_page_hash = db_status_complete.wp_page_hash;
            db_status.save();
        } else {
            log::info!(
                "skipping page.sql load due to match on hash: {}",
                hex::encode(db_status.wp_page_hash.as_ref().unwrap())
            );
        }

        log::debug!("finding max index");

        let max_page_id: u32 = schema::vertex::Entity::find()
            .select_only()
            .column_as(schema::vertex::Column::Id.max(), QueryAs::MaxVertexId)
            .into_values::<_, QueryAs>()
            .one(&db)
            .await
            .expect("query max id")
            .unwrap();

        log::debug!("building edge map");

        let mut edge_db = self
            .load_edges_dump(self.pagelinks_path.clone(), db, &mut db_status)
            .await;

        let needs_sort = match db_status.edges_sorted {
            Some(sorted) => !sorted,
            None => true,
        };

        if needs_sort {
            log::debug!("writing sorted outgoing edges");
            edge_db.write_sorted_by(EdgeSort::Outgoing);
            log::debug!("writing sorted incoming edges");
            edge_db.write_sorted_by(EdgeSort::Incoming);
            db_status.wp_pagelinks_hash = db_status_complete.wp_pagelinks_hash;
            db_status.edges_sorted = Some(true);
            db_status.save();
        } else {
            log::debug!("edges already sorted");
        }

        log::debug!(
            "building al [{}] and ix [{}] - {} vertexes",
            self.al_path.to_str().unwrap(),
            self.ix_path.to_str().unwrap(),
            max_page_id,
        );

        let edge_iter = edge_db.iter(max_page_id);
        let ix_file = match File::create(&self.ix_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &self.ix_path, why),
            Ok(file) => file,
        };
        let al_file = match File::create(&self.al_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &self.al_path, why),
            Ok(file) => file,
        };
        let mut ix_writer = BufWriter::new(ix_file);
        let mut al_writer = BufWriter::new(al_file);

        for adjacency_set in edge_iter {
            // log::debug!(
            //     "adjacencies for: {}\toutgoing: [{}] incoming: [{}]",
            //     adjacency_set.vertex_id,
            //     adjacency_set.adjacency_list.outgoing.iter().join(" "),
            //     adjacency_set.adjacency_list.incoming.iter().join(" "),
            // );
            let vertex_al_offset: u64 = self.write_adjacency_set(&adjacency_set, &mut al_writer);
            ix_writer
                .write_all(&vertex_al_offset.to_le_bytes())
                .unwrap();
        }
        db_status.build_complete = Some(true);
        db_status.save();
        log::info!("database build complete");
    }

    async fn resolve_edges(
        rx: Receiver<WPPageLink>,
        edge_db: &mut EdgeProcDB,
        db: DbConn,
        redirects: &mut RedirectMap,
    ) -> (u32, u32) {
        // look up and write in chunks
        let mut received_count = 0u32;
        let mut hit_count = 0u32;
        for page_link_chunk in &rx.iter().chunks(32760) {
            let page_links: Vec<WPPageLink> = page_link_chunk.collect();
            received_count += page_links.len() as u32;
            let mut title_map = HashMap::new();
            let titles = page_links
                .iter()
                .map(|l| l.dest_page_title.clone())
                .into_iter()
                .unique();
            let vertexes = schema::vertex::Entity::find()
                .filter(schema::vertex::Column::Title.is_in(titles))
                .all(&db)
                .await
                .expect("query vertexes by title");
            for v in vertexes {
                hit_count += 1;
                if v.is_redirect {
                    // in this case, "v" is a redirect. The destination of the redirect
                    // is in the redirects table, which is loaded into the RedirectMap.
                    // this will make it appear that our current vertex (by title) maps
                    // to the page ID of the destination of the redirect
                    match redirects.get(v.id) {
                        Some(dest) => {
                            title_map.insert(v.title, dest);
                        }
                        None => {
                            log::debug!("tried to resolve redirect for page: [{}: {}] but no entry was in redirects",
                    v.id, v.title);
                        }
                    }
                    continue;
                }
                title_map.insert(v.title, v.id);
            }

            for link in page_links {
                if let Some(dest) = title_map.get(&link.dest_page_title) {
                    let edge = Edge {
                        source_vertex_id: link.source_page_id,
                        dest_vertex_id: *dest,
                    };
                    edge_db.write_edge(&edge);
                } else {
                    edge_db.write_fail(link.source_page_id, link.dest_page_title);
                }
            }
        }
        (received_count, hit_count)
    }

    // load edges from the pagelinks.sql dump
    async fn load_edges_dump(
        &self,
        path: PathBuf,
        db: DbConn,
        db_status: &mut DBStatus,
    ) -> EdgeProcDB {
        let edge_db = EdgeProcDB::new(self.process_path.join("edge-db"));

        if let Some(resolved) = db_status.edges_resolved {
            if resolved {
                log::debug!("edges already resolved; returning");
                return edge_db;
            }
        }

        let mut redirects = redirect::RedirectMap::new(self.redirects_path.clone());
        redirects.parse(db.clone()).await;

        log::debug!("loading edges dump");
        let (pagelink_tx, pagelink_rx) = crossbeam::channel::bounded(32);
        let pagelink_source = pagelink_source::WPPageLinkSource::new(path, pagelink_tx);

        log::debug!("truncating edge db");
        let mut edge_db = edge_db.truncate();

        log::debug!("spawning pagelink source");
        let pagelink_thread = thread::spawn(move || pagelink_source.run());

        log::debug!("spawning edge resolver");
        let (resolved_total_count, resolved_hit_count) =
            Self::resolve_edges(pagelink_rx, &mut edge_db, db, &mut redirects).await;
        log::debug!(
            "edge resolver: received {} pagelinks and resolved {}",
            resolved_total_count,
            resolved_hit_count
        );

        log::debug!("joining pagelink count thread");
        let pagelink_count = pagelink_thread.join().unwrap();
        log::debug!("pagelink count = {}", pagelink_count);

        log::debug!("\nflushing edge database");
        edge_db.flush();

        db_status.edges_resolved = Some(true);
        db_status.save();

        edge_db
    }

    // load vertexes from the pages.sql dump
    async fn load_vertexes_dump(&mut self, db: DbConn) {
        let stmt = Table::drop()
            .table(schema::vertex::Entity.table_ref())
            .to_owned();
        let _ = db.execute(db.get_database_backend().build(&stmt)).await;
        self.create_vertex_table(&db).await;

        if let Some(usage) = memory_stats() {
            println!("Current physical memory usage: {}", usage.physical_mem);
            println!("Current virtual memory usage: {}", usage.virtual_mem);
        } else {
            println!("Couldn't get the current memory usage :(");
        }

        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA synchronous = OFF".to_owned(),
        ))
        .await
        .expect("set sync pragma");

        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA journal_mode = MEMORY".to_owned(),
        ))
        .await
        .expect("set journal_mode pragma");

        let txn = db.begin().await.expect("start transaction");

        let (vertex_tx, vertex_rx) = crossbeam::channel::bounded(32);
        let page_source = page_source::WPPageSource::new(self.page_path.clone(), vertex_tx);
        log::debug!("spawning page source thread");
        let page_thread = thread::spawn(move || page_source.run());

        for v in vertex_rx {
            let vertex_model = schema::vertex::ActiveModel {
                title: Set(v.title),
                id: Set(v.id),
                is_redirect: Set(v.is_redirect),
            };
            schema::vertex::Entity::insert(vertex_model)
                .exec(&txn)
                .await
                .expect("insert vertex");
        }
        txn.commit().await.expect("commit");
        log::debug!("commited vertex sqlite inserts");
        let page_count = page_thread.join().expect("join page thread");
        log::debug!("page count: {}", page_count);
    }

    pub async fn create_vertex_title_ix(&self, db: &DbConn) {
        log::debug!("vertex table: creating title index");
        let stmt = Index::create()
            .name("vertex-title-ix")
            .table(schema::vertex::Entity)
            .col(schema::vertex::Column::Title)
            .if_not_exists()
            .to_owned();
        let stmt = db.get_database_backend().build(&stmt);
        db.execute(stmt).await.expect("vertex title index");
        log::debug!("vertex table: title index created");
    }

    pub async fn create_vertex_table(&self, db: &DbConn) {
        let schema = Schema::new(DbBackend::Sqlite);
        let create_stmt: TableCreateStatement =
            schema.create_table_from_entity(schema::vertex::Entity);
        db.execute(db.get_database_backend().build(&create_stmt))
            .await
            .expect("create table");
    }

    /// Writes appropriate null-terminated list of 4-byte values to al_file
    /// Each 4-byte value is a LE representation
    pub fn write_adjacency_set(
        &mut self,
        adjacency_set: &AdjacencySet,
        al_writer: &mut BufWriter<File>,
    ) -> u64 {
        if adjacency_set.adjacency_list.is_empty() {
            // No outgoing edges or no such vertex
            return 0;
        }

        // Position at which we are writing the thing.
        let al_position = al_writer.stream_position().unwrap();
        // log::debug!(
        //     "writing vertex {} list with {} edges {}",
        //     vertex_id,
        //     edge_ids.len(),
        //     al_position
        // );
        al_writer
            .write_all(&(0xCAFECAFE_u32).to_le_bytes())
            .unwrap();

        // outgoing edges
        for neighbor in adjacency_set.adjacency_list.outgoing.iter() {
            let neighbor_bytes = neighbor.to_le_bytes();
            al_writer.write_all(&neighbor_bytes).unwrap();
        }
        // Null terminator
        al_writer.write_all(&(0u32).to_le_bytes()).unwrap();

        // incoming edges
        for neighbor in adjacency_set.adjacency_list.incoming.iter() {
            let neighbor_bytes = neighbor.to_le_bytes();
            al_writer.write_all(&neighbor_bytes).unwrap();
        }
        // Null terminator
        al_writer.write_all(&(0u32).to_le_bytes()).unwrap();

        al_position
    }
}

fn format_path(vertexes: Vec<Vertex>) -> String {
    let titles: Vec<String> = vertexes.into_iter().map(|v| v.title).collect();
    titles.join(" → ")
}

/// CLI Options
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Verbose output
    #[clap(short, long)]
    verbose: bool,
    /// Path to database
    #[clap(short, long)]
    data_path: Option<PathBuf>,
    /// Command to execute
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build the database from a MediaWiki Dump
    /// https://dumps.wikimedia.org/enwiki/latest/
    Build {
        /// Path to page.sql
        #[clap(long)]
        page: PathBuf,
        /// Path to pagelinks.sql
        #[clap(long)]
        pagelinks: PathBuf,
        /// Path to redirects.sql
        #[clap(long)]
        redirects: PathBuf,
    },
    /// Find the shortest path
    Run {
        /// Source article
        source: String,
        /// Destination article
        destination: String,
    },
    /// Query a page
    Query {
        /// Article to query
        target: String,
    },
}

#[tokio::main]
async fn main() {
    stderrlog::new()
        .module(module_path!())
        .quiet(false)
        .verbosity(4)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();

    log::info!("Wikipedia Speedrun");
    let cli = Cli::parse();

    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("speedrun-data");
    let data_dir = cli.data_path.unwrap_or(default_data_dir);
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();
    let vertex_al_path = data_dir.join("vertex_al");
    let vertex_ix_path = data_dir.join("vertex_al_ix");
    let db_path = data_dir.join("wikipedia-speedrun.db");
    let conn_str = format!("sqlite:///{}?mode=ro", db_path.to_string_lossy());
    log::debug!("using database: {}", conn_str);

    // directory used for processing import
    match cli.command {
        Command::Build {
            page,
            pagelinks,
            redirects,
        } => {
            log::info!("building database");
            let mut gddb = GraphDBBuilder::new(
                page,
                pagelinks,
                redirects,
                vertex_ix_path,
                vertex_al_path,
                data_dir,
            );
            gddb.build_database().await;
        }
        Command::Run {
            source,
            destination,
        } => {
            let db: DbConn = Database::connect(conn_str).await.expect("db connect");

            log::info!("computing path");
            let mut gdb = GraphDB::new(
                vertex_ix_path.to_str().unwrap(),
                vertex_al_path.to_str().unwrap(),
                db,
            )
            .unwrap();
            let source_title = source.replace('_', " ");
            let dest_title = destination.replace('_', " ");

            log::info!("speedrun: [{}] → [{}]", source_title, dest_title);

            let source_vertex = gdb
                .find_vertex_by_title(source_title)
                .await
                .expect("source not found");
            let dest_vertex = gdb
                .find_vertex_by_title(dest_title)
                .await
                .expect("destination not found");

            log::info!("speedrun: [{:#?}] → [{:#?}]", source_vertex, dest_vertex);

            let paths = gdb.bfs(source_vertex.id, dest_vertex.id);
            if paths.is_empty() {
                println!("\nno path found");
                return;
            }
            for path in paths {
                let vertex_path = path.into_iter().map(|vid| gdb.find_vertex_by_id(vid));
                let vertex_path = futures::future::join_all(vertex_path)
                    .await
                    .into_iter()
                    .map(|v| v.expect("vertex not found"))
                    .collect();
                let formatted_path = format_path(vertex_path);
                println!("{formatted_path}");
            }
        }
        Command::Query { target } => {
            let target = target.replace('_', " ");
            log::info!("querying target: {}", target);
            let db: DbConn = Database::connect(conn_str).await.expect("db connect");
            let mut gdb = GraphDB::new(
                vertex_ix_path.to_str().unwrap(),
                vertex_al_path.to_str().unwrap(),
                db,
            )
            .unwrap();
            let vertex = gdb
                .find_vertex_by_title(target)
                .await
                .expect("find vertex by title");
            log::info!("vertex:\n{:#?}", vertex);
            let al = gdb.edge_db.read_edges(vertex.id);
            log::info!("incoming edges:");
            for vid in al.incoming.iter() {
                let v = gdb.find_vertex_by_id(*vid).await;
                match v {
                    Some(v) => println!("\t{:09}\t{}", v.id, v.title),
                    None => log::error!("vertex id {} not found!", vid),
                }
            }
            log::info!("outgoing edges:");
            for vid in al.outgoing.iter() {
                let v = gdb.find_vertex_by_id(*vid).await;
                match v {
                    Some(v) => println!("\t{:09}\t{}", v.id, v.title),
                    None => log::error!("vertex id {} not found!", vid),
                }
            }
        }
    }
}
