use clap::{Parser, Subcommand};
use dirs;
use futures::channel::mpsc::{self, UnboundedSender};
use futures::stream::{self, StreamExt};
use indicatif::ProgressStyle;
use memmap2::{Mmap, MmapOptions};
use memory_stats::memory_stats;
use parse_mediawiki_sql::schemas::Page;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Table, TableCreateStatement};
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
use std::io::prelude::*;
use std::io::Write;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

mod dump;
mod schema;

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

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Edge {
    pub source_vertex_id: u32,
    pub dest_vertex_id: u32,
}

pub struct Link<'a> {
    pub source: &'a Vertex,
    pub dest: &'a Vertex,
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

        let max_page_id = schema::vertex::Entity::find()
            .select_only()
            .column_as(schema::vertex::Column::Id.max(), QueryAs::MaxVertexId)
            .into_values::<_, QueryAs>()
            .one(&db)
            .await
            .expect("query max id")
            .unwrap();

        log::debug!("building edge map");
        let edge_map = self.build_edge_map(db.clone()).await;

        log::debug!(
            "building al [{}] and ix [{}] - {} vertexes",
            self.al_path.to_str().unwrap(),
            self.ix_path.to_str().unwrap(),
            max_page_id,
        );

        for n in 0..max_page_id {
            let vertex_al_offset: u64 = self.build_adjacency_list(n, &edge_map);
            self.ix_file.write(&vertex_al_offset.to_le_bytes()).unwrap();
            if n % 1000 == 0 {
                log::debug!("-> wrote {} entries", n);
            }
        }
        self.ix_file.flush().unwrap();
        self.al_file.flush().unwrap();
        log::info!("database build complete")
    }

    // builds outgoing edges
    async fn build_edge_map(&self, db: DbConn) -> HashMap<u32, Vec<u32>> {
        let mut m = HashMap::new();
        log::debug!("loading edges from dump");
        let edges = Self::load_edges_dump(&self.pagelinks_path, db);
        log::debug!("building edge map");
        for edge in edges.await.iter() {
            if !m.contains_key(&edge.source_vertex_id) {
                m.insert(edge.source_vertex_id, vec![]);
            }
            m.get_mut(&edge.source_vertex_id)
                .unwrap()
                .push(edge.dest_vertex_id);
        }
        m
    }

    // load edges from the pagelinks.sql dump
    async fn load_edges_dump(path: &PathBuf, db: DbConn) -> Vec<Edge> {
        use parse_mediawiki_sql::utils::memory_map;
        let pagelinks_sql = unsafe { Arc::new(memory_map(path).unwrap()) };

        let chunks = dump::dump_chunks(&pagelinks_sql);
        // let mut threads = Vec::new();
        let (tx, rx) = mpsc::unbounded::<Edge>();

        for chunk in chunks.into_iter() {
            log::debug!(
                "spawning thread for edge loading: {} -> {}",
                chunk.start,
                chunk.end
            );
            let pagelinks_ref = Arc::clone(&pagelinks_sql);
            let db = db.clone();
            let tx = tx.clone();
            let t =
                thread::spawn(move || Self::load_edges_dump_chunk(pagelinks_ref, chunk, db, tx));
            //threads.push(t);
        }

        // let mut links = Vec::new();
        // for t in threads.into_iter() {
        //     let mut res = t.join().unwrap().await;
        //     log::debug!("joined edge loading thread");
        //     links.append(&mut res);
        // }
        let links = rx.collect::<Vec<Edge>>().await;
        log::info!("loaded pagelinks");
        links
    }

    async fn load_edges_dump_chunk(
        sql_map: Arc<Mmap>,
        range: Range<usize>,
        db: DbConn,
        tx: UnboundedSender<Edge>,
    ) {
        let chunk: &[u8] = &sql_map[range];
        use parse_mediawiki_sql::{
            field_types::PageNamespace, iterate_sql_insertions, schemas::PageLink,
        };

        let mut sql_iterator = iterate_sql_insertions(&chunk);
        let links = sql_iterator.filter_map(
            |PageLink {
                 from,
                 from_namespace,
                 namespace,
                 title,
             }| {
                if from_namespace == PageNamespace(0) && namespace == PageNamespace(0) {
                    Some((from, title))
                } else {
                    None
                }
            },
        );
        let links_stream = futures::stream::iter(links);

        let query_stream = links_stream
            .for_each(|pl| {
                let db = db.clone();
                let tx = tx.clone();
                async move {
                    let res = schema::vertex::Entity::find()
                        .filter(schema::vertex::Column::Title.eq(pl.1 .0))
                        .one(&db)
                        .await
                        .expect("query vertex by title");
                    match res {
                        Some(dest) => {
                            let edge = Edge {
                                source_vertex_id: pl.0 .0,
                                dest_vertex_id: dest.id,
                            };
                            tx.unbounded_send(edge).expect("transmit edge");
                        }
                        _ => (),
                    };
                }
            })
            .await;
    }

    // load vertexes from the pages.sql dump
    async fn load_vertexes_dump(&mut self, db: DbConn) {
        use parse_mediawiki_sql::utils::memory_map;
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
                        title: title.0,
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
    pub fn build_adjacency_list(
        &mut self,
        vertex_id: u32,
        edge_map: &HashMap<u32, Vec<u32>>,
    ) -> u64 {
        let edge_ids = edge_map.get(&vertex_id).unwrap();
        if edge_ids.len() == 0 {
            // No outgoing edges or no such vertex
            return 0;
        }

        // Position at which we are writing the thing.
        let al_position = self.al_file.stream_position().unwrap();
        log::debug!(
            "writing vertex {} list with {} edges {}",
            vertex_id,
            edge_ids.len(),
            al_position
        );
        for neighbor in edge_ids.iter() {
            let neighbor_bytes = neighbor.to_le_bytes();
            self.al_file.write(&neighbor_bytes).unwrap();
        }
        // Null terminator
        self.al_file.write(&(0i32).to_le_bytes()).unwrap();
        al_position
    }
}

pub struct GraphDB {
    pub mmap_ix: memmap2::Mmap,
    pub mmap_al: memmap2::Mmap,
    pub db: DbConn,
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
        Ok(GraphDB {
            mmap_ix,
            mmap_al,
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

    fn check_al(&mut self) {
        let mut buf: [u8; 4] = [0; 4];
        buf.copy_from_slice(&self.mmap_al[0..4]);
        let magic: u32 = u32::from_be_bytes(buf);
        assert!(magic == 1337);
    }

    fn check_ix(&mut self) {
        // read index file and ensure that all 64-bit entries
        // point to within range
        let max_sz: u64 = (self.mmap_al.len() - 4) as u64;
        let mut buf: [u8; 8] = [0; 8];
        let mut position: usize = 0;
        while position <= (self.mmap_ix.len() - 8) {
            buf.copy_from_slice(&self.mmap_ix[position..position + 8]);
            let value: u64 = u64::from_be_bytes(buf);
            if value > max_sz {
                let msg = format!(
                    "check_ix: at index file: {}, got pointer to {} in AL file (maximum: {})",
                    position, value, max_sz
                );
                panic!("{}", msg);
            }
            position += 8;
        }
    }

    fn check_db(&mut self) {
        self.check_al();
        println!("checking index file");
        self.check_ix();
        println!("done");
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
        self.check_db();
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
        let mut neighbors: Vec<u32> = Vec::new();
        let ix_position: usize = ((u64::BITS / 8) * vertex_id) as usize;
        // println!(
        //     "load_neighbors for {} from ix position: {}",
        //     vertex_id, ix_position
        // );
        let mut buf: [u8; 8] = [0; 8];
        buf.copy_from_slice(&self.mmap_ix[ix_position..ix_position + 8]);
        // println!("buf from ix = {:?}", buf);
        let mut al_offset: usize = u64::from_be_bytes(buf) as usize;
        if al_offset == 0 {
            // println!("vertex {} has no neighbors", vertex_id);
            return neighbors;
        }
        let mut vbuf: [u8; 4] = [0; 4];
        loop {
            // println!("looking at al_offset = {}", al_offset);
            vbuf.copy_from_slice(&self.mmap_al[al_offset..al_offset + 4]);
            // println!("vbuf from al = {:?}", vbuf);
            let i: u32 = u32::from_be_bytes(vbuf);
            // println!("vbuf -> int = {:?}", i);
            if i == 0 {
                break;
            }
            neighbors.push(i);
            al_offset += 4;
        }
        neighbors
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
