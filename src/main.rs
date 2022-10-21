use bincode::config::{BigEndian, Fixint};
use bincode::error::DecodeError;
use clap::{Parser, Subcommand};
use crossbeam::channel::Receiver;
use dirs;
use indicatif::{ProgressDrawTarget, ProgressStyle};
use itertools::Itertools;
use memmap2::{Mmap, MmapOptions};
use memory_stats::memory_stats;
use parse_mediawiki_sql::schemas::Page;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Index, Table, TableCreateStatement};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseBackend, DbBackend, DbConn, DeriveColumn,
    EntityTrait, EnumIter, QueryFilter, QuerySelect, Schema, Set, SqlxSqliteConnector, Statement,
    TransactionTrait,
};
use spinners::{Spinner, Spinners};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::io::{prelude::*, BufReader, BufWriter};
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

mod dump;
mod edge_db;
mod schema;
mod source;
#[derive(Clone, Debug)]
pub struct Vertex {
    pub id: u32,
    pub title: String,
}

impl Hash for Vertex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for Vertex {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Vertex {}

#[derive(PartialEq, Debug, Copy, Clone, bincode::Decode, bincode::Encode)]
pub struct Edge {
    pub source_vertex_id: u32,
    pub dest_vertex_id: u32,
}

pub struct Link<'a> {
    pub source: &'a Vertex,
    pub dest: &'a Vertex,
}

/// Intermediate type of only fields necessary to create an Edge
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
    path: PathBuf,
    writer: BufWriter<File>,
    bc_config: bincode::config::Configuration<BigEndian, Fixint>,
    pub edges: Vec<Edge>,
    unflushed_inserts: usize,
}

impl EdgeProcDB {
    pub fn new(path: PathBuf) -> EdgeProcDB {
        let file = File::create(&path).expect("open edge proc db");
        EdgeProcDB {
            path,
            writer: BufWriter::new(file),
            bc_config: Self::bincode_config(),
            edges: Vec::new(),
            unflushed_inserts: 0,
        }
    }

    #[inline]
    fn bincode_config() -> bincode::config::Configuration<BigEndian, Fixint> {
        bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding()
            .with_no_limit()
    }

    pub fn write_edge(&mut self, edge: &Edge) {
        bincode::encode_into_std_write(edge, &mut self.writer, self.bc_config).expect("write edge");
        self.unflushed_inserts += 1;
        if self.unflushed_inserts % 1024 == 0 {
            self.unflushed_inserts = 0;
            self.writer.flush().expect("flush edge proc db");
        }
    }

    fn sort_basename(sort_by: &EdgeSort) -> String {
        format!(
            "-sort-{}",
            match sort_by {
                EdgeSort::Incoming => "incoming",
                EdgeSort::Outgoing => "outgoing",
            }
        )
    }

    fn sort_file(&self, sort_by: &EdgeSort) -> File {
        let sink_basename = Self::sort_basename(&sort_by);
        File::create(self.path.join(sink_basename)).expect("open edge db sort sink")
    }

    pub fn write_sorted_by(&mut self, sort_by: EdgeSort) {
        let source = File::open(&self.path).expect("open edge db sort source");
        let sink_basename = Self::sort_basename(&sort_by);
        let sink = File::create(self.path.join(sink_basename)).expect("open edge db sort sink");
        let mut source = BufReader::new(source);
        let mut sink = BufWriter::new(sink);
        log::debug!("loading edge db for sort");
        loop {
            let res = bincode::decode_from_reader::<Edge, _, _>(&mut source, self.bc_config);
            match res {
                Ok(edge) => {
                    self.edges.push(edge);
                }
                Err(err) => match err {
                    DecodeError::Io { inner, .. } => match inner.kind() {
                        std::io::ErrorKind::UnexpectedEof => {
                            // eof = we're done
                            break;
                        }
                        _ => {
                            panic!("unexpected io error during decode: {:#?}", inner);
                        }
                    },
                    _ => {
                        panic!("decode error: {:#?}", err);
                    }
                },
            }
        }
        log::debug!("sorting edge db");
        self.edges.sort_unstable_by(|x, y| match sort_by {
            EdgeSort::Incoming => x.dest_vertex_id.cmp(&y.dest_vertex_id),
            EdgeSort::Outgoing => x.source_vertex_id.cmp(&y.source_vertex_id),
        });
        for edge in &self.edges {
            bincode::encode_into_std_write(edge, &mut sink, self.bc_config).expect("write edge");
        }
    }

    pub fn flush(&mut self) {
        self.writer.flush().expect("flush edge db");
    }

    pub fn iter(&self, max_page_id: u32) -> AdjacencySetIterator {
        let outgoing_source = self.sort_file(&EdgeSort::Outgoing);
        let incoming_source = self.sort_file(&EdgeSort::Incoming);

        let outgoing_source = unsafe { Mmap::map(&outgoing_source).unwrap() };
        let incoming_source = unsafe { Mmap::map(&incoming_source).unwrap() };
        AdjacencySetIterator {
            outgoing_source,
            incoming_source,
            incoming_i: 0,
            outgoing_i: 0,
            vertex_id: 0,
            max_page_id,
            bc_config: self.bc_config,
        }
    }
}

// AdjacencySet is an AdjacencyList combined with its vertex
struct AdjacencySet {
    vertex_id: u32,
    adjacency_list: edge_db::AdjacencyList,
}

struct AdjacencySetIterator {
    incoming_source: Mmap,
    outgoing_source: Mmap,
    incoming_i: usize,
    outgoing_i: usize,
    vertex_id: u32,
    max_page_id: u32,
    bc_config: bincode::config::Configuration<BigEndian, Fixint>,
}

impl Iterator for AdjacencySetIterator {
    type Item = AdjacencySet;

    // iterates over range of 0..max_page_id,
    // combining data in incoming_source and outgoing_source
    // into adjacency lists
    fn next(&mut self) -> Option<Self::Item> {
        // are we done yet?
        if self.vertex_id > self.max_page_id {
            return None;
        }

        let mut val = AdjacencySet {
            vertex_id: self.vertex_id,
            adjacency_list: edge_db::AdjacencyList::default(),
        };

        // put in all the outgoing edges
        loop {
            let outgoing_offset: usize = self.outgoing_i * (u32::BITS as usize / 8) * 2;

            if outgoing_offset >= self.outgoing_source.len() {
                break;
            }

            let current_edge: Edge = bincode::decode_from_slice(
                &self.outgoing_source[outgoing_offset..],
                self.bc_config,
            )
            .unwrap()
            .0;

            if current_edge.source_vertex_id > self.vertex_id {
                break;
            }
            if current_edge.source_vertex_id < self.vertex_id {
                self.outgoing_i += 1;
                continue;
            }
            val.adjacency_list
                .outgoing
                .push(current_edge.dest_vertex_id);
        }

        // put in all the incoming edges
        loop {
            let incoming_offset: usize = self.incoming_i * (u32::BITS as usize / 8) * 2;

            if incoming_offset >= self.incoming_source.len() {
                break;
            }

            let current_edge: Edge = bincode::decode_from_slice(
                &self.incoming_source[incoming_offset..],
                self.bc_config,
            )
            .unwrap()
            .0;

            if current_edge.dest_vertex_id > self.vertex_id {
                break;
            }

            // necessary at the beginning before hitting first vertex
            if current_edge.dest_vertex_id < self.vertex_id {
                self.incoming_i += 1;
                continue;
            }

            val.adjacency_list
                .outgoing
                .push(current_edge.source_vertex_id);
        }

        self.vertex_id += 1;

        Some(val)
    }
}

struct GraphDBBuilder {
    // inputs
    pub page_path: PathBuf,
    pub pagelinks_path: PathBuf,
    // outputs
    pub ix_path: PathBuf,
    pub al_path: PathBuf,
    al_file: File,
    ix_file: File,

    // process directory
    process_path: PathBuf,

    // approximate size
    vertex_count: u32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum QueryAs {
    MaxVertexId,
}

impl GraphDBBuilder {
    pub fn new(
        page: PathBuf,
        pagelinks: PathBuf,
        ix_path: PathBuf,
        al_path: PathBuf,
        process_path: PathBuf,
    ) -> GraphDBBuilder {
        let ix_file = match File::create(&ix_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &ix_path, why),
            Ok(file) => file,
        };
        let al_file = match File::create(&al_path) {
            Err(why) => panic!("couldn't create {:?}: {}", &al_path, why),
            Ok(file) => file,
        };

        let vertex_count = 6546390;
        GraphDBBuilder {
            page_path: page,
            pagelinks_path: pagelinks,
            ix_path,
            al_path,
            al_file,
            ix_file,
            process_path,
            vertex_count,
        }
    }

    /// load vertexes from page.sql and put them in a sqlite file
    pub async fn build_database(&mut self) {
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

        log::info!("loading page.sql");
        self.load_vertexes_dump(db.clone()).await;
        self.create_vertex_title_ix(&db).await;

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

        let mut edge_db = self.load_edges_dump(self.pagelinks_path.clone(), db).await;
        log::debug!("writing sorted outgoing edges");
        edge_db.write_sorted_by(EdgeSort::Outgoing);
        log::debug!("writing sorted incoming edges");
        edge_db.write_sorted_by(EdgeSort::Incoming);

        log::debug!(
            "building al [{}] and ix [{}] - {} vertexes",
            self.al_path.to_str().unwrap(),
            self.ix_path.to_str().unwrap(),
            max_page_id,
        );

        let edge_iter = edge_db.iter(max_page_id);

        for adjacency_set in edge_iter {
            // log::debug!("adjacencies for: {}", adjacency_set.source_vertex_id);
            let vertex_al_offset: u64 = self.write_adjacency_set(&adjacency_set);
            self.ix_file.write(&vertex_al_offset.to_le_bytes()).unwrap();
            if adjacency_set.vertex_id % 1000 == 0 {
                log::debug!("-> wrote {} entries", adjacency_set.vertex_id);
            }
        }
        self.ix_file.flush().unwrap();
        self.al_file.flush().unwrap();
        log::info!("database build complete");
    }

    async fn resolve_edges(rx: Receiver<WPPageLink>, edge_db: &mut EdgeProcDB, db: DbConn) {
        // look up and write in chunks
        for page_link_chunk in &rx.iter().chunks(32760) {
            let page_links: Vec<WPPageLink> = page_link_chunk.collect();

            let mut title_map = HashMap::new();
            let titles = page_links.iter().map(|l| l.dest_page_title.clone());
            let vertexes = schema::vertex::Entity::find()
                .filter(schema::vertex::Column::Title.is_in(titles))
                .all(&db)
                .await
                .expect("query vertexes by title");
            for v in vertexes {
                title_map.insert(v.title, v.id);
            }

            for link in page_links {
                if let Some(dest) = title_map.get(&link.dest_page_title) {
                    let edge = Edge {
                        source_vertex_id: link.source_page_id,
                        dest_vertex_id: *dest,
                    };
                    edge_db.write_edge(&edge);
                }
            }
        }
    }

    // load edges from the pagelinks.sql dump
    async fn load_edges_dump(&self, path: PathBuf, db: DbConn) -> EdgeProcDB {
        let mut edge_db = EdgeProcDB::new(self.process_path.join("edge-db"));
        let (pagelink_tx, pagelink_rx) = crossbeam::channel::bounded(32);

        let mut pagelink_source = source::WPPageLinkSource::new(path, pagelink_tx);

        pagelink_source.count_edge_inserts();
        log::debug!("spawning pagelink source");
        thread::spawn(move || pagelink_source.run());

        log::debug!("spawning edge resolver");
        Self::resolve_edges(pagelink_rx, &mut edge_db, db).await;

        log::debug!("edge resolver returned");

        log::debug!("\nflushing edge database");
        edge_db.flush();
        edge_db
    }

    // load vertexes from the pages.sql dump
    async fn load_vertexes_dump(&mut self, db: DbConn) {
        use parse_mediawiki_sql::utils::memory_map;

        // If everything is already imported, skip importation
        // Otherwise, drop the table if it exists, then create it
        let count = schema::vertex::Entity::find().count(&db).await;
        match count {
            Ok(count) => {
                log::debug!("rows present: {}", count);
                if count == self.vertex_count as usize {
                    log::debug!("all rows already present");
                    return;
                }
                log::debug!("wrong row count; expected {}", self.vertex_count);
                let stmt = Table::drop()
                    .table(schema::vertex::Entity.table_ref())
                    .to_owned();
                db.execute(db.get_database_backend().build(&stmt))
                    .await
                    .expect("drop table");
                self.create_vertex_table(&db).await;
            }
            Err(_) => {
                self.create_vertex_table(&db).await;
            }
        }

        let draw_target = ProgressDrawTarget::stderr_with_hz(0.1);
        let progress = indicatif::ProgressBar::new(self.vertex_count.into());
        progress.set_style(
            ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {human_pos}/{human_len:7} {percent}% {per_sec:5} {eta}").unwrap(),
        );
        progress.set_draw_target(draw_target);

        if let Some(usage) = memory_stats() {
            println!("Current physical memory usage: {}", usage.physical_mem);
            println!("Current virtual memory usage: {}", usage.virtual_mem);
        } else {
            println!("Couldn't get the current memory usage :(");
        }

        let page_sql = unsafe { memory_map(&self.page_path).unwrap() };

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

        use parse_mediawiki_sql::{field_types::PageNamespace, iterate_sql_insertions};
        let txn = db.begin().await.expect("start transaction");

        let mut iterator = iterate_sql_insertions(&page_sql);
        let vertexes = iterator.filter_map(
            |Page {
                 id,
                 namespace,
                 is_redirect,
                 title,
                 ..
             }| {
                if namespace == PageNamespace(0) && !is_redirect {
                    Some(Vertex {
                        id: id.0,
                        title: title.0.to_lowercase().replace("_", " "),
                    })
                } else {
                    None
                }
            },
        );

        for v in vertexes {
            let vertex_model = schema::vertex::ActiveModel {
                title: Set(v.title),
                ..Default::default()
            };
            schema::vertex::Entity::insert(vertex_model)
                .exec(&txn)
                .await
                .expect("insert vertex");
            progress.inc(1);
        }
        txn.commit().await.expect("commit");
        progress.finish();
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
    pub fn write_adjacency_set(&mut self, adjacency_set: &AdjacencySet) -> u64 {
        if adjacency_set.adjacency_list.is_empty() {
            // No outgoing edges or no such vertex
            return 0;
        }

        // Position at which we are writing the thing.
        let al_position = self.al_file.stream_position().unwrap();
        // log::debug!(
        //     "writing vertex {} list with {} edges {}",
        //     vertex_id,
        //     edge_ids.len(),
        //     al_position
        // );
        self.al_file.write(&(0xCAFECAFE_u32).to_le_bytes()).unwrap();

        // outgoing edges
        for neighbor in adjacency_set.adjacency_list.outgoing.iter() {
            let neighbor_bytes = neighbor.to_le_bytes();
            self.al_file.write(&neighbor_bytes).unwrap();
        }
        // Null terminator
        self.al_file.write(&(0u32).to_le_bytes()).unwrap();

        // incoming edges
        for neighbor in adjacency_set.adjacency_list.incoming.iter() {
            let neighbor_bytes = neighbor.to_le_bytes();
            self.al_file.write(&neighbor_bytes).unwrap();
        }
        // Null terminator
        self.al_file.write(&(0u32).to_le_bytes()).unwrap();

        al_position
    }
}

pub struct GraphDB {
    pub db: DbConn,
    pub edge_db: edge_db::EdgeDB,
    pub visited_ids: HashSet<u32>,
    pub parents: HashMap<u32, u32>,
    pub q: VecDeque<u32>,
}

impl GraphDB {
    pub fn new(path_ix: &str, path_al: &str, db: DbConn) -> Result<GraphDB, std::io::Error> {
        let file_ix = File::open(path_ix)?;
        let file_al = File::open(path_al)?;
        let mmap_ix = unsafe { MmapOptions::new().map(&file_ix)? };
        let mmap_al = unsafe { MmapOptions::new().map(&file_al)? };
        let visited_ids = HashSet::new();
        let parents = HashMap::new();
        let q: VecDeque<u32> = VecDeque::new();
        let edge_db = edge_db::EdgeDB::new(mmap_al, mmap_ix);
        Ok(GraphDB {
            edge_db,
            db,
            visited_ids,
            parents,
            q,
        })
    }

    pub async fn find_vertex_by_title(&mut self, title: String) -> Option<Vertex> {
        let canon_title = title.to_lowercase();
        log::debug!("loading vertex: {}", canon_title);
        let vertex_model = schema::vertex::Entity::find()
            .filter(schema::vertex::Column::Title.eq(title))
            .one(&self.db)
            .await
            .expect("find vertex by title");
        match vertex_model {
            Some(v) => Some(Vertex {
                id: v.id,
                title: v.title,
            }),
            None => None,
        }
    }

    pub async fn find_vertex_by_id(&self, id: u32) -> Option<Vertex> {
        log::debug!("loading vertex: id={}", id);
        let vertex_model = schema::vertex::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .expect("find vertex by title");
        match vertex_model {
            Some(v) => Some(Vertex {
                id: v.id,
                title: v.title,
            }),
            None => None,
        }
    }

    fn build_path(&self, source: u32, dest: u32) -> Vec<u32> {
        let mut path: Vec<u32> = Vec::new();
        let mut current = dest;
        loop {
            path.push(current);
            if current == source {
                break;
            }
            current = *self
                .parents
                .get(&current)
                .expect(&format!("parent not recorded for {:#?}", current));
        }
        path.reverse();
        path
    }

    pub fn bfs(&mut self, src: u32, dest: u32) -> Option<Vec<u32>> {
        self.edge_db.check_db();
        let mut sp = Spinner::new(Spinners::Dots9, "Computing path".into());

        let start_time = Instant::now();
        self.q.push_back(src);
        loop {
            match self.q.pop_front() {
                Some(current) => {
                    if current == dest {
                        sp.stop_with_message(format!(
                            "Computed path - visited {} pages",
                            self.visited_ids.len()
                        ));
                        let path = self.build_path(src, dest);
                        let elapsed = start_time.elapsed();
                        println!("\nelapsed time: {} seconds", elapsed.as_secs());
                        return Some(path);
                    }
                    let neighbors = self.load_neighbors(current);
                    let next_neighbors: Vec<u32> = neighbors
                        .into_iter()
                        .filter(|x| !self.visited_ids.contains(x))
                        .collect();
                    for &n in next_neighbors.iter() {
                        self.parents.insert(n, current);
                        self.visited_ids.insert(n);
                        self.q.push_back(n);
                    }
                }
                None => {
                    sp.stop();
                    let elapsed = start_time.elapsed();
                    println!("\nelapsed time: {} seconds", elapsed.as_secs());
                    return None;
                }
            }
        }
    }

    pub fn load_neighbors(&self, vertex_id: u32) -> Vec<u32> {
        // println!(
        //     "load_neighbors for {}",
        //     vertex_id
        // );
        self.edge_db.read_edges(vertex_id).outgoing
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
    },
    /// Find the shortest path
    Run {
        /// Source article
        source: String,
        /// Destination article
        destination: String,
    },
}

#[tokio::main]
async fn main() {
    stderrlog::new()
        .module(module_path!())
        .quiet(false)
        .verbosity(4)
        //    .timestamp(ts)
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

    // directory used for processing import
    match cli.command {
        Command::Build { page, pagelinks } => {
            log::info!("building database");
            let mut gddb =
                GraphDBBuilder::new(page, pagelinks, vertex_ix_path, vertex_al_path, data_dir);
            gddb.build_database().await;
        }
        Command::Run {
            source,
            destination,
        } => {
            let db_path = data_dir.join("wikipedia-speedrun.db");
            let conn_str = format!("sqlite:///{}?mode=ro", db_path.to_string_lossy());
            log::debug!("using database: {}", conn_str);
            let db: DbConn = Database::connect(conn_str).await.expect("db connect");

            log::info!("computing path");
            let mut gdb = GraphDB::new(
                vertex_ix_path.to_str().unwrap(),
                vertex_al_path.to_str().unwrap(),
                db,
            )
            .unwrap();
            let source_title = source.replace(" ", "_").to_lowercase();
            let dest_title = destination.replace(" ", "_").to_lowercase();

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

            match gdb.bfs(source_vertex.id as u32, dest_vertex.id as u32) {
                Some(path) => {
                    let vertex_path = path.into_iter().map(|vid| gdb.find_vertex_by_id(vid));
                    let vertex_path = futures::future::join_all(vertex_path)
                        .await
                        .into_iter()
                        .map(|v| v.expect("vertex not found"))
                        .collect();
                    let formatted_path = format_path(vertex_path);
                    println!("\n{}", formatted_path);
                }
                None => {
                    println!("\nno path found");
                }
            }
        }
    }
}
