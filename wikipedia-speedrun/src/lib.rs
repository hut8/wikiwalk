use std::{fs::File, hash::Hash, hash::Hasher, path::PathBuf, time::Instant};

use memmap2::MmapOptions;
use sea_orm::{
    sea_query::TableCreateStatement, ActiveModelTrait, ColumnTrait, ConnectionTrait, Database,
    DbBackend, DbConn, EntityTrait, QueryFilter, Schema, Set, SqlxSqliteConnector,
};
use serde::Serialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
    SqlitePool,
};

pub mod bfs;
pub mod dump;
pub mod edge_db;
pub mod redirect;
pub mod schema;

#[derive(Clone, Debug, Serialize)]
pub struct Vertex {
    pub id: u32,
    pub title: String,
    pub is_redirect: bool,
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

#[derive(PartialEq, Eq, Debug, Copy, Clone, Default)]
#[repr(align(8))]
pub struct Edge {
    pub source_vertex_id: u32,
    pub dest_vertex_id: u32,
}

impl Edge {
    pub fn from_bytes(buf: &[u8]) -> Edge {
        let mut val = Edge::default();
        let source_ptr = buf.as_ptr() as *const Edge;
        let dest_ptr = &mut val as *mut Edge;
        unsafe {
            *dest_ptr = *source_ptr;
        }
        val
    }
}

pub struct Link<'a> {
    pub source: &'a Vertex,
    pub dest: &'a Vertex,
}

pub struct GraphDB {
    pub master_db: DbConn,
    pub graph_db: DbConn,
    pub edge_db: edge_db::EdgeDB,
}

impl GraphDB {
    pub async fn new(
        dump_date: String,
        root_data_dir: &PathBuf,
    ) -> Result<GraphDB, std::io::Error> {
        let master_db_path = root_data_dir.join("master.db");
        let master_conn_str = format!("sqlite:///{}?mode=rwc", master_db_path.to_string_lossy());
        Self::create_master_db(&master_db_path).await;
        let master_db: DbConn = Database::connect(master_conn_str)
            .await
            .expect("master db connect");

        let data_dir = root_data_dir.join(dump_date);

        let graph_db_path = data_dir.join("graph.db");
        let graph_db_conn_str = format!("sqlite:///{}?mode=rw", graph_db_path.to_string_lossy());
        let graph_db: DbConn = Database::connect(graph_db_conn_str)
            .await
            .expect("graph db connect");

        let path_ix = data_dir.join("vertex-al-ix");
        let path_al = data_dir.join("vertex-al");
        let file_ix = File::open(path_ix)?;
        let file_al = File::open(path_al)?;
        let mmap_ix = unsafe { MmapOptions::new().map(&file_ix)? };
        let mmap_al = unsafe { MmapOptions::new().map(&file_al)? };
        let edge_db = edge_db::EdgeDB::new(mmap_al, mmap_ix);
        Ok(GraphDB {
            edge_db,
            graph_db,
            master_db,
        })
    }

    async fn create_master_db(db_path: &PathBuf) {
        let opts = SqliteConnectOptions::new()
            .synchronous(SqliteSynchronous::Off)
            .journal_mode(SqliteJournalMode::Memory)
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await.expect("db connect");
        let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
        let schema = Schema::new(DbBackend::Sqlite);
        let mut create_stmt: TableCreateStatement =
            schema.create_table_from_entity(schema::search::Entity);
        create_stmt.if_not_exists();
        db.execute(db.get_database_backend().build(&create_stmt))
            .await
            .expect("create table");
    }

    pub async fn find_vertex_by_title(&mut self, title: String) -> Option<Vertex> {
        let canon_title = title.replace('_', " ");
        log::debug!("loading vertex: {}", canon_title);
        let vertex_model = schema::vertex::Entity::find()
            .filter(schema::vertex::Column::Title.eq(title))
            .one(&self.graph_db)
            .await
            .expect("find vertex by title");
        match vertex_model {
            Some(v) => Some(Vertex {
                id: v.id,
                title: v.title,
                is_redirect: v.is_redirect,
            }),
            None => None,
        }
    }

    pub async fn find_vertex_by_id(&self, id: u32) -> Option<Vertex> {
        let vertex_model = schema::vertex::Entity::find_by_id(id)
            .one(&self.graph_db)
            .await
            .expect("find vertex by title");
        match vertex_model {
            Some(v) => Some(Vertex {
                id: v.id,
                title: v.title,
                is_redirect: v.is_redirect,
            }),
            None => None,
        }
    }

    pub async fn bfs(&self, src: u32, dest: u32) -> Vec<Vec<u32>> {
        let start_time = Instant::now();
        let timestamp = chrono::Utc::now();
        let paths = bfs::breadth_first_search(src, dest, &self.edge_db);
        let elapsed = start_time.elapsed();
        let paths_ser = serde_json::ser::to_string(&paths).expect("serialize paths");
        let path_entity = schema::path::ActiveModel {
            source_page_id: Set(src as i32),
            target_page_id: Set(dest as i32),
            timestamp: Set(timestamp.to_string()),
            duration: Set(elapsed.as_secs_f64()),
            path_data: Set(paths_ser),
            ..Default::default()
        };
        path_entity
            .insert(&self.graph_db)
            .await
            .expect("insert path record");
        paths
    }
}
