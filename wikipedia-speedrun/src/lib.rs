use std::{fs::File, hash::Hash, hash::Hasher, time::Instant};

use memmap2::MmapOptions;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, EntityTrait, QueryFilter, Set};
use serde::Serialize;

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
    pub fn new(
        path_ix: &str,
        path_al: &str,
        graph_db: DbConn,
        master_db: DbConn,
    ) -> Result<GraphDB, std::io::Error> {
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
        let search = schema::search::ActiveModel {
            source_page_id: Set(src as i32),
            target_page_id: Set(dest as i32),
            timestamp: Set(timestamp.to_string()),
            duration: Set(elapsed.as_secs_f64()),
            ..Default::default()
        };
        search
            .insert(&self.master_db)
            .await
            .expect("insert log record");
        println!("\nelapsed time: {} seconds", elapsed.as_secs());
        paths
    }
}
