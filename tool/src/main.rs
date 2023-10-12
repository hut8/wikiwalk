use std::collections::HashMap;
use std::fs::{canonicalize, read_dir, symlink_metadata, DirEntry, File};
use std::hash::Hash;
use std::io::Write;
use std::io::{prelude::*, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::{process, thread};

use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use crossbeam::channel::{Receiver, Sender};
use edge_db_builder::{AdjacencySet, EdgeProcDB};
use itertools::Itertools;
use memory_stats::memory_stats;
use reqwest::Client;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Index, Table, TableCreateStatement};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DbBackend, DbConn, DeriveColumn, EntityTrait,
    EnumIter, QueryFilter, QuerySelect, Schema, Set, SqlxSqliteConnector, Statement,
    TransactionTrait,
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::SqlitePool;

use fetch::DumpStatus;
use wikiwalk::dbstatus::DBStatus;
use wikiwalk::paths::{DBPaths, Paths};
use wikiwalk::redirect::{RedirectMapBuilder, RedirectMapFile};
use wikiwalk::schema::vertex::Model;
use wikiwalk::{schema, Edge, GraphDB, Vertex};

mod api;
mod edge_db_builder;
mod fetch;
mod page_source;
mod pagelink_source;
#[cfg(feature = "google-cloud-storage")]
mod push;
mod sitemap;

/// Intermediate type of only fields necessary to create an Edge
#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub struct WPPageLink {
    pub source_page_id: u32,
    pub dest_page_title: String,
}

pub enum EdgeSort {
    Incoming,
    Outgoing,
}

struct GraphDBBuilder {
    // inputs
    pub dump_date: String,
    pub page_path: PathBuf,
    pub pagelinks_path: PathBuf,
    pub redirects_path: PathBuf,

    pub paths: Paths,
    pub db_paths: DBPaths,

    // outputs
    pub ix_path: PathBuf,
    pub al_path: PathBuf,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum QueryAs {
    MaxVertexId,
}

struct BatchLookup {
    edges: Vec<Edge>,
    title_map: HashMap<String, u32>,
    redirect_failures: Vec<Model>,
}

impl GraphDBBuilder {
    pub fn new(dump_date: String, root_data_dir: &Path) -> GraphDBBuilder {
        let paths = Paths::with_base(root_data_dir);
        let dump_paths = paths.dump_paths(&dump_date);
        let page_path = dump_paths.page();
        let redirects_path = dump_paths.redirect();
        let pagelinks_path = dump_paths.pagelinks();

        let db_paths = paths.db_paths(&dump_date);
        let ix_path = db_paths.vertex_al_ix_path();
        let al_path = db_paths.vertex_al_path();

        GraphDBBuilder {
            page_path,
            pagelinks_path,
            ix_path,
            al_path,
            redirects_path,
            dump_date,
            paths,
            db_paths,
        }
    }

    /// load vertexes from page.sql and put them in a sqlite file
    /// then process edges into memory-mapped flat-file database
    pub async fn build_database(&mut self) -> anyhow::Result<()> {
        let db_status_path = self.db_paths.db_status_path();
        let mut db_status = DBStatus::load(db_status_path.clone());

        if !db_status.dump_date_str.is_empty() && db_status.dump_date_str != self.dump_date {
            log::error!("for build of {dump_date}, db status file indicates dump date is {status_file_date}",
              dump_date=self.dump_date,
              status_file_date=db_status.dump_date_str,
            )
        }
        db_status.dump_date_str = self.dump_date.clone();

        if db_status.build_complete {
            self.create_current_symlink();
            self.clean_old_databases();
            log::info!("skipping build: db status file indicates complete");
            return Ok(());
        }

        self.db_paths.ensure_exists().expect("db path exists");

        let db_path = self.db_paths.graph_db();
        let conn_str = format!("sqlite:///{}?mode=rwc", db_path.to_string_lossy());
        log::debug!("using database: {}", conn_str);
        let opts = SqliteConnectOptions::new()
            .synchronous(SqliteSynchronous::Off)
            .journal_mode(SqliteJournalMode::Memory)
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await.expect("db connect");
        let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);

        if !db_status.vertexes_loaded {
            log::info!("loading page.sql");
            db_status.vertex_count = self.load_vertexes_dump(db.clone()).await;
            self.create_vertex_title_ix(&db).await;
            db_status.vertexes_loaded = true;
            db_status.save();
        } else {
            log::info!("skipping page.sql load: build status indicates vertexes were loaded");
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

        log::info!("max page id: {}", max_page_id);
        log::debug!("building edge map");

        let mut edge_proc_db = self
            .load_edges_dump(self.pagelinks_path.clone(), db, max_page_id, &mut db_status)
            .await;

        if !db_status.edges_sorted {
            log::debug!("making edge sort files");
            edge_proc_db.make_sort_files();
            log::debug!("writing sorted outgoing edges");
            edge_proc_db.write_sorted_by(EdgeSort::Outgoing);
            log::debug!("writing sorted incoming edges");
            edge_proc_db.write_sorted_by(EdgeSort::Incoming);
            db_status.edges_sorted = true;
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

        let edge_iter = edge_proc_db.iter(max_page_id);
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
            let vertex_al_offset: u64 = self.write_adjacency_set(&adjacency_set, &mut al_writer);
            ix_writer
                .write_all(&vertex_al_offset.to_le_bytes())
                .unwrap();
        }

        edge_proc_db.destroy();

        db_status.build_complete = true;
        db_status.save();

        self.create_current_symlink();
        self.clean_old_databases();

        log::info!("database build complete");
        Ok(())
    }

    fn clean_old_databases(&self) {
        // Identify the very necessary "current" data symlinks's target
        let current_data_dir = self.paths.base.join("current");
        let md = match symlink_metadata(&current_data_dir) {
            Ok(md) => md,
            Err(e) => {
                log::info!(
                    "unable to clean old databases: current data directory: {p} points to non-existent target: {e}",
                    p = &current_data_dir.display(),
                    e = e
                );
                return;
            }
        };

        if !md.is_symlink() {
            log::warn!(
                "unable to clean old databases: current data directory: {p} points to non-symlink",
                p = &current_data_dir.display()
            );
        }
        let current_data_abs = canonicalize(&current_data_dir).expect("canonicalize symlink");
        log::debug!(
            "cleaning: current data directory: {p} points to {t}",
            p = &current_data_dir.display(),
            t = current_data_abs.display()
        );
        // Exclude important directory entries in order to avoid deleting the current data or other files
        let filter_trash = |p: DirEntry| {
            // Exclude non-directories
            if !p
                .file_type()
                .expect("file type of dirent in base path")
                .is_dir()
            {
                log::debug!(
                    "cleaning: skipping non-directory: {p}",
                    p = p.path().display()
                );
                return None;
            }
            // Exclude the important symlink (probably redundant)
            if p.file_name() == "current" {
                log::debug!("cleaning: skipping current symlink");
                return None;
            }
            // We have a directory. Is it a database directory?
            // Database directories are YYYYMMDD
            match NaiveDate::parse_from_str(
                p.file_name().to_str().expect("convert os str to str"),
                "%Y%m%d",
            ) {
                Err(_) => {
                    log::debug!(
                        "cleaning: skipping directory not matching YYYYMMDD: {p}",
                        p = p.path().display()
                    );
                    None
                }
                Ok(database_date) => {
                    log::debug!(
                        "cleaning: evaluating candidate for date: {d}",
                        d = database_date.format("%Y-%m-%d")
                    );
                    let canon_p = canonicalize(p.path()).expect("canonicalize data path");
                    if canon_p == current_data_abs {
                        return None;
                    }
                    Some(p)
                }
            }
        };

        read_dir(&self.paths.base)
            .expect("read base path")
            .filter_map(|p| p.ok())
            .filter_map(filter_trash)
            .for_each(|trash_path| {
                log::info!(
                    "cleaning: removing old database: {p}",
                    p = trash_path.path().display()
                );
                std::fs::remove_dir_all(trash_path.path()).expect("remove old database");
            });
    }

    fn create_current_symlink(&self) {
        // symlink that which was just built from the "current" link
        let current_data_dir = self.paths.base.join("current");
        // sanity check: if it's not a symlink, we have problems
        if let Ok(md) = symlink_metadata(&current_data_dir) {
            if md.is_symlink() {
                std::fs::remove_file(&current_data_dir).expect("remove old symlink");
            } else {
                log::warn!(
                    "current data directory: {p} points to non-symlink",
                    p = current_data_dir.display()
                );
            }
        }
        symlink::symlink_dir(&self.db_paths.base, &current_data_dir)
            .expect("symlink current directory");
    }

    async fn resolve_edges(
        rx: Receiver<WPPageLink>,
        tx: Sender<Edge>,
        redirects: Arc<RwLock<RedirectMapFile>>,
        db: DbConn,
    ) {
        let mut cache: lrumap::LruHashMap<String, u32> = lrumap::LruHashMap::new(100000);
        let mut lookup_q: Vec<WPPageLink> = Vec::new();

        // looping over every relevant entry in the pagelinks table,
        // we want to send as many edges as we possibly can resolve. Not all pagelink table entries
        // will be resolved, because:
        // 1. The link may point to a page that doesn't exist (very common)
        // 2. The link may point to a page that is a redirect (according to the page table),
        //    but the redirect destination is not present in the redirects table (less common)
        for pl in rx {
            // Check LRU cache first (thread-local)
            // TODO: make it shared? Might cause excessive contention
            // TODO: Find hit rate. Is it even worth it?
            if let Some(dest_vertex_id) = cache.get_without_update(&pl.dest_page_title) {
                let edge = Edge {
                    source_vertex_id: pl.source_page_id,
                    dest_vertex_id: *dest_vertex_id,
                };
                tx.send(edge).expect("send edge");
                continue;
            }

            // We don't want to look up every page that's passed in individually. SQLite
            // is much more effective if we put it in batches. The maximum number of placeholders
            // seems to be roughly 2^15 (signed 16-bit integer) minus a few, so we'll
            // batch our lookups until we hit that point.
            lookup_q.push(pl);
            if lookup_q.len() > 32000 {
                let pending_q = lookup_q.clone();
                lookup_q.clear();
                let batch_lookup =
                    Self::lookup_batch(pending_q, db.clone(), redirects.clone()).await;
                for (title, id) in batch_lookup.title_map {
                    cache.push(title, id);
                }
                for failure in batch_lookup.redirect_failures {
                    log::debug!(
                        "redirect failure: {} marked as redirect but no entry in redirects table",
                        failure.title
                    );
                }
                for edge in batch_lookup.edges {
                    tx.send(edge).expect("send edge");
                }
            }
        }

        // send the last batch now that we've read all the pagelinks
        let batch_lookup = Self::lookup_batch(lookup_q, db.clone(), redirects.clone()).await;
        // don't bother caching; cache is about to be dropped
        // TODO: Send redirect failures
        for edge in batch_lookup.edges {
            tx.send(edge).expect("send edge");
        }
    }

    async fn lookup_batch(
        q: Vec<WPPageLink>,
        db: DbConn,
        redirects: Arc<RwLock<RedirectMapFile>>,
    ) -> BatchLookup {
        let mut title_map: HashMap<String, u32> = HashMap::new();
        let titles = q.iter().map(|l| l.dest_page_title.clone()).unique();
        let vertexes = schema::vertex::Entity::find()
            .filter(schema::vertex::Column::Title.is_in(titles))
            .all(&db)
            .await
            .expect("query vertexes by title");

        let mut redirect_failures = Vec::new();
        for v in vertexes {
            if v.is_redirect {
                // in this case, "v" is a redirect. The destination of the redirect
                // is in the redirects table, which is loaded into the RedirectMap.
                // this will make it appear that our current vertex (by title) maps
                // to the page ID of the destination of the redirect
                let redirects = redirects.read().unwrap();
                match redirects.get(v.id) {
                    Some(dest) => {
                        title_map.insert(v.title, dest);
                    }
                    None => {
                        redirect_failures.push(v);
                    }
                }
                continue;
            }
            title_map.insert(v.title, v.id);
        }
        let mut edges = Vec::new();
        for pl in q {
            if let Some(dest) = title_map.get(&pl.dest_page_title).copied() {
                let edge = Edge {
                    source_vertex_id: pl.source_page_id,
                    dest_vertex_id: dest,
                };
                edges.push(edge);
            }
            // FIXME: If title not found, send to the "fail log" thing
        }

        BatchLookup {
            edges,
            title_map,
            redirect_failures,
        }
    }

    async fn write_edges(rx: Receiver<Edge>, edge_db: &mut EdgeProcDB) -> u32 {
        let mut count = 0;
        // look up and write in chunks
        for link in rx {
            edge_db.write_edge(&link);
            count += 1;
        }
        count
    }

    // load edges from the pagelinks.sql dump
    async fn load_edges_dump(
        &self,
        path: PathBuf,
        db: DbConn,
        max_page_id: u32,
        db_status: &mut DBStatus,
    ) -> EdgeProcDB {
        let edge_db_build = EdgeProcDB::new(self.db_paths.base.join("edge-db"));

        if db_status.edges_resolved {
            log::debug!("edges already resolved; returning");
            return edge_db_build;
        }

        let redirects_map_file = Arc::new(std::sync::RwLock::new(
            RedirectMapFile::new(self.db_paths.redirects_path(), max_page_id).unwrap(),
        ));

        if !db_status.redirects_resolved {
            log::info!(
                "loading redirects from {} to {}",
                self.redirects_path.clone().display(),
                self.db_paths.redirects_path().display(),
            );
            let start = Instant::now();
            let redirect_builder =
                RedirectMapBuilder::new(self.redirects_path.clone(), redirects_map_file.clone());
            let redirect_count = redirect_builder.build(db.clone()).await;
            let elapsed = start.elapsed();
            log::info!(
                "loaded {} redirects in {} seconds ({}/sec)",
                redirect_count,
                elapsed.as_secs(),
                (f64::from(redirect_count) / elapsed.as_secs_f64()).round() as u64,
            );
            db_status.redirects_resolved = true;
            db_status.save();
        } else {
            log::info!("skipping redirects: build status indicates redirects were loaded");
        }

        log::debug!("loading edges dump");
        let (pagelink_tx, pagelink_rx) = crossbeam::channel::bounded(4096);
        let pagelink_source = pagelink_source::WPPageLinkSource::new(path, pagelink_tx);

        log::debug!("truncating edge db");
        let mut edge_db = edge_db_build.truncate();

        log::debug!("spawning pagelink source");
        let pagelink_thread = tokio::spawn(pagelink_source.run());

        log::debug!("spawning edge resolver");
        let (edge_tx, edge_rx) = crossbeam::channel::bounded(2048);
        for _ in 0..num_cpus::get() {
            let redirects_map_file = redirects_map_file.clone();
            let pagelink_rx = pagelink_rx.clone();
            let edge_tx = edge_tx.clone();
            let db = db.clone();
            thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(Self::resolve_edges(
                    pagelink_rx,
                    edge_tx,
                    redirects_map_file,
                    db,
                ));
                log::info!("edge resolver thread exiting");
            });
        }

        log::debug!("running edge writer");
        let edge_write_count = Self::write_edges(edge_rx, &mut edge_db).await;
        log::debug!("edge resolver: wrote {} edges", edge_write_count,);

        log::debug!("joining pagelink thread");
        let edge_count = pagelink_thread.await.expect("join pagelink thread");
        log::debug!("pagelink count = {}", edge_count);
        db_status.edge_count = edge_count;

        log::debug!("flushing edge database");
        edge_db.flush();

        db_status.edges_resolved = true;
        db_status.save();

        edge_db
    }

    // load vertexes from the pages.sql dump
    async fn load_vertexes_dump(&mut self, db: DbConn) -> u32 {
        let stmt = Table::drop()
            .table(schema::vertex::Entity.table_ref())
            .to_owned();
        let _ = db.execute(db.get_database_backend().build(&stmt)).await;
        self.create_vertex_table(&db).await;
        self.create_paths_table(&db).await;

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

        let (vertex_tx, vertex_rx) = crossbeam::channel::unbounded(); //  crossbeam::channel::bounded(64);
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
        page_count
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

    pub async fn create_paths_table(&self, db: &DbConn) {
        let schema = Schema::new(DbBackend::Sqlite);
        let mut create_stmt = schema.create_table_from_entity(schema::path::Entity);
        let stmt = create_stmt.if_not_exists();

        db.execute(db.get_database_backend().build(stmt))
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
        /// Dump date to import
        #[clap(long)]
        dump_date: String,
        /// Push files to Google Cloud after build
        push: bool,
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
    /// Find latest dump
    FindLatest {
        #[clap(long)]
        /// Display date
        date: bool,
        /// Display URLs
        #[clap(long)]
        urls: bool,
        /// Use URLs relative to the host root
        #[clap(long)]
        relative: bool,
    },
    /// Fetch latest dumps
    Fetch {
        #[clap(long)]
        dump_date: Option<String>,
    },
    /// Fetch latest dump and import it
    Pull {
        #[clap(long)]
        push: bool,
    },
    /// Build Sitemap
    Sitemap,
}

async fn run_build(data_dir: &Path, dump_date: &str) -> anyhow::Result<()> {
    let mut gddb = GraphDBBuilder::new(dump_date.to_owned(), data_dir);
    log::info!("cleaning old databases");
    gddb.clean_old_databases();
    log::info!("building database");
    gddb.build_database().await
}

async fn run_fetch(dump_dir: &Path, dump_date: Option<DumpStatus>) -> anyhow::Result<()> {
    let latest_dump = match dump_date {
        Some(dump_status) => dump_status,
        None => match fetch::find_latest().await {
            None => {
                log::error!("[pull] found no recent dumps");
                process::exit(1);
            }
            Some(x) => x,
        },
    };
    fetch::fetch_dump(dump_dir, latest_dump).await?;
    Ok(())
}

async fn run_compute(data_dir: &Path, source: String, destination: String) {
    log::info!("computing path");
    let mut gdb = GraphDB::new("current".into(), data_dir).await.unwrap();
    let source_title = source.replace('_', " ");
    let dest_title = destination.replace('_', " ");

    log::info!("wikiwalk: [{}] → [{}]", source_title, dest_title);

    let source_vertex = gdb
        .find_vertex_by_title(source_title)
        .await
        .expect("source not found");
    let dest_vertex = gdb
        .find_vertex_by_title(dest_title)
        .await
        .expect("destination not found");

    log::info!("wikiwalk: [{:#?}] → [{:#?}]", source_vertex, dest_vertex);

    let paths = gdb.bfs(source_vertex.id, dest_vertex.id).await;
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

async fn run_query(data_dir: &Path, target: String) {
    let target = target.replace('_', " ");
    log::info!("querying target: {}", target);
    let mut gdb = GraphDB::new("current".into(), data_dir).await.unwrap();
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

async fn run_sitemap() {
    let current_db_paths = Paths::new().db_paths("current");
    let db_path = current_db_paths.graph_db();
    let conn_str = format!("sqlite:///{}?mode=rwc", db_path.to_string_lossy());
    log::debug!("building sitemap using database: {}", conn_str);
    let opts = SqliteConnectOptions::new()
        .synchronous(SqliteSynchronous::Off)
        .journal_mode(SqliteJournalMode::Memory)
        .filename(&db_path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await.expect("db connect");
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    let sitemaps_path = current_db_paths.sitemaps_path();
    sitemap::make_sitemap(&db, &sitemaps_path).await;
}

async fn run_find_latest(urls: bool, relative: bool, date: bool) {
    let latest_dump = fetch::find_latest().await;
    match latest_dump {
        None => {
            log::error!("found no recent dumps");
            process::exit(1);
        }
        Some(status) => {
            if date {
                println!("{}", status.dump_date);
                return;
            }
            if urls {
                status.jobs.all().into_iter().for_each(|job| {
                    job.files.iter().for_each(|(_file, info)| {
                        let u = if relative {
                            info.url.clone()
                        } else {
                            fetch::absolute_dump_url(&info.url)
                        };
                        println!("{}", u);
                    });
                });
                return;
            }
            println!(
                "{}",
                serde_json::to_value(status).expect("serialize dump status")
            );
        }
    }
}

async fn run_pull(dump_dir: &Path, data_dir: &Path, push: bool) {
    let latest_dump = {
        match fetch::find_latest().await {
            None => {
                log::error!("[pull] found no recent dumps");
                process::exit(1);
            }
            Some(x) => x,
        }
    };
    let current_path = Paths::new().db_paths("current");
    let db_status = DBStatus::load(current_path.db_status_path());

    let db_dump_date = &db_status.dump_date_str;
    let latest_dump_date = &latest_dump.dump_date;

    if db_dump_date == latest_dump_date {
        log::info!("[pull] database dump date {db_dump_date} is already the latest",);
        process::exit(0);
    }
    log::info!(
      "[pull] database dump date {db_dump_date} is older than latest dump date: {latest_dump_date} - will fetch and build"
    );

    run_fetch(dump_dir, Some(latest_dump.clone()))
        .await
        .expect("fetch dump");
    log::info!("fetched data from {latest_dump_date}",);
    if let Err(err) = run_build(data_dir, latest_dump_date).await {
        log::error!("build failed: {:#?}", err);
        process::exit(1);
    }
    log::info!("built database from {latest_dump_date}. cleaning dump directory.");
    fetch::clean_dump_dir(dump_dir);
    log::info!("building sitemap");
    run_sitemap().await;

    #[cfg(feature = "google-cloud-storage")]
    if push {
        log::info!("pushing built files");
        if let Err(err) = push::push_built_files(current_path).await {
            log::error!("push failed: {:#?}", err);
            process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() {
    stderrlog::new()
        .module(module_path!())
        .module("wikiwalk")
        .module("wikiwalk::graphdb")
        .module("wikiwalk::fetch")
        .module("wikiwalk::redirect")
        .module("wikiwalk::push")
        .module("wikiwalk::sitemap")
        .module("wikiwalk::page_source")
        .module("wikiwalk::pagelink_source")
        .module("wikiwalk::edge_db_builder")
        .show_module_names(true)
        .quiet(false)
        .verbosity(3)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();

    log::info!("WikiWalk: {}", module_path!());
    let cli = Cli::parse();

    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("wikiwalk");
    let env_data_dir: Option<PathBuf> = std::env::var("DATA_ROOT").ok().map(PathBuf::from);
    let data_dir = cli.data_path.or(env_data_dir).unwrap_or(default_data_dir);
    let dump_dir = data_dir.join("dumps");
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();

    match cli.command {
        Command::Build { dump_date, push } => {
            run_build(&data_dir, &dump_date).await.unwrap();
            #[cfg(feature = "google-cloud-storage")]
            if push {
                log::info!("pushing built files");
                if let Err(err) = push::push_built_files(Paths::new().db_paths("current")).await {
                    log::error!("push failed: {:#?}", err);
                    process::exit(1);
                }
            }
        }
        Command::Run {
            source,
            destination,
        } => {
            run_compute(&data_dir, source, destination).await;
        }
        Command::Query { target } => {
            run_query(&data_dir, target).await;
        }
        Command::Fetch { dump_date } => {
            let dump_status = match dump_date {
                Some(date) => fetch::fetch_dump_status_for_date(&Client::default(), &date)
                    .await
                    .map_or_else(
                        |err| {
                            log::error!("fetch dump status for specified date failed: {:#?}", err);
                            process::exit(1);
                        },
                        Some,
                    ),
                None => None,
            };
            run_fetch(&dump_dir, dump_status)
                .await
                .expect("fetch failed");
        }
        Command::FindLatest {
            urls,
            relative,
            date,
        } => {
            run_find_latest(urls, relative, date).await;
        }
        Command::Pull { push } => {
            run_pull(&dump_dir, &data_dir, push).await;
        }
        Command::Sitemap => {
            run_sitemap().await;
        }
    }
}
