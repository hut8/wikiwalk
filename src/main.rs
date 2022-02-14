#[macro_use]
extern crate diesel;
extern crate dotenv;

pub mod schema;

use clap::Parser;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dirs;
use dotenv::dotenv;
use memmap2::MmapOptions;
use schema::{edges, vertexes};
use spinners::{Spinner, Spinners};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::prelude::*;
use std::io::Write;
use std::path::Path;

#[derive(Identifiable, Queryable, Debug, Clone)]
#[table_name = "vertexes"]
// If table name is not specified, diesel pluralizes to vertexs
pub struct Vertex {
    pub id: i32,
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

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations, Copy, Clone)]
#[primary_key(source_vertex_id, dest_vertex_id)]
#[belongs_to(Vertex, foreign_key = "source_vertex_id")]
pub struct Edge {
    pub source_vertex_id: i32,
    pub dest_vertex_id: i32,
}

pub struct Link<'a> {
    pub source: &'a Vertex,
    pub dest: &'a Vertex,
}

/// Writes appropriate null-terminated list of 4-byte values to al_file
/// Each 4-byte value is a LE representation
pub fn build_adjacency_list(al_file: &mut File, vertex_id: i32, conn: &PgConnection) -> u64 {
    use crate::edges::dsl::*;
    let edge_ids: Vec<i32> = edges
        .filter(source_vertex_id.eq(vertex_id))
        .select(dest_vertex_id)
        .load::<i32>(conn)
        .unwrap();
    if edge_ids.len() == 0 {
        // No outgoing edges or no such vertex
        return 0;
    }

    // Position at which we are writing the thing.
    let al_position = al_file.stream_position().unwrap();
    println!(
        "writing vertex {} list with {} edges {}",
        vertex_id,
        edge_ids.len(),
        al_position
    );
    for neighbor in edge_ids.iter() {
        let neighbor_bytes = neighbor.to_le_bytes();
        al_file.write(&neighbor_bytes).unwrap();
    }
    // Null terminator
    al_file.write(&(0i32).to_le_bytes()).unwrap();
    al_position
}

// vertex_al_ix format: array of i64
// each element is indexed by page id
// each value is the offset into the vertex_al file
//      where that vertex's adjacency list is found
pub fn build_database(conn: &PgConnection) {
    use crate::edges::dsl::*;
    use crate::vertexes::dsl::*;
    use diesel::dsl::max;
    let max_page_id: i32 = vertexes
        .select(max(id))
        .get_result::<Option<i32>>(conn)
        .expect("could not find max id")
        .unwrap();
    let home = dirs::home_dir().unwrap();
    let data_dir = home.join("wpsr");
    std::fs::create_dir_all(&data_dir).unwrap();

    let vertex_al_ix_path = data_dir.join("vertex_al_ix");
    let vertex_al_path = data_dir.join("vertex_al");
    if vertex_al_path.exists() && vertex_al_ix_path.exists() {
        println!("vertex_al and vertex_al_ix exist... skipping");
        return;
    }

    let mut vertex_al_ix_file = match File::create(&vertex_al_ix_path) {
        Err(why) => panic!("couldn't create {:?}: {}", vertex_al_ix_path, why),
        Ok(file) => file,
    };
    let mut vertex_al_file = match File::create(&vertex_al_path) {
        Err(why) => panic!("couldn't create {:?}: {}", vertex_al_path, why),
        Ok(file) => file,
    };

    println!(
        "building vertex_al_ix at {} - {} vertexes",
        vertex_al_ix_path.to_str().unwrap(),
        max_page_id
    );

    for n in 0..max_page_id {
        let vertex_al_offset: u64 = build_adjacency_list(&mut vertex_al_file, n, conn);
        vertex_al_ix_file
            .write(&vertex_al_offset.to_le_bytes())
            .unwrap();
    }
}

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

fn load_vertex(name: &str, conn: &PgConnection) -> QueryResult<Vertex> {
    use crate::vertexes::dsl::*;
    vertexes.filter(title.ilike(name)).first::<Vertex>(conn)
}

pub struct GraphDB {
    pub mmap_ix: memmap2::Mmap,
    pub mmap_al: memmap2::Mmap,
    pub visited_ids: HashSet<u32>,
    pub parents: HashMap<u32, u32>,
    pub q: VecDeque<u32>,
}

impl GraphDB {
    pub fn new(path_ix: &str, path_al: &str) -> Result<GraphDB, std::io::Error> {
        let file_ix = File::open(path_ix)?;
        let file_al = File::open(path_al)?;
        let mmap_ix = unsafe { MmapOptions::new().map(&file_ix)? };
        let mmap_al = unsafe { MmapOptions::new().map(&file_al)? };
        let visited_ids = HashSet::new();
        let parents = HashMap::new();
        let mut q: VecDeque<u32> = VecDeque::new();
        Ok(GraphDB {
            mmap_ix,
            mmap_al,
            visited_ids,
            parents,
            q,
        })
    }

    pub fn load_neighbors(&self, vertex_id: u32) -> Vec<u32> {
        let mut neighbors: Vec<u32> = Vec::new();
        let ix_position: usize = ((u32::BITS / 8) * vertex_id) as usize;
        let mut buf: [u8; 8] = [0; 8];
        buf.copy_from_slice(&self.mmap_ix[ix_position..ix_position + 8]);
        let mut al_offset: usize = u64::from_be_bytes(buf) as usize;
        let mut vbuf: [u8; 4] = [0; 4];
        loop {
            vbuf.copy_from_slice(&self.mmap_al[al_offset..al_offset + 4]);
            let i: u32 = u32::from_be_bytes(vbuf);
            if i == 0 {
                break;
            }
            neighbors.push(i);
            al_offset += 8;
        }
        neighbors
    }
}

fn load_neighbors(
    source: &Vertex,
    visited_ids: &mut HashSet<i32>,
    conn: &PgConnection,
) -> Vec<Vertex> {
    use crate::vertexes::dsl::*;
    use diesel::dsl::any;
    let edges = Edge::belonging_to(source)
        .load::<Edge>(conn)
        .expect("load edges");
    let neighbor_ids: Vec<i32> = edges
        .iter()
        .map(|e| e.dest_vertex_id)
        .filter(|x| !visited_ids.contains(x))
        .collect();
    let neighbors = vertexes
        .filter(id.eq(any(neighbor_ids)))
        .load::<Vertex>(conn)
        .expect("load neighbors");
    neighbors
}

fn build_path<'a>(
    source: &'a Vertex,
    dest: &'a Vertex,
    parents: &'a HashMap<i32, i32>,
    graph: &'a HashMap<i32, Vertex>,
) -> Vec<Vertex> {
    let mut path: Vec<Vertex> = Vec::new();
    let mut current = dest;
    loop {
        path.push(current.clone());
        if current.id == source.id {
            break;
        }
        let next_id = parents
            .get(&current.id)
            .expect(&format!("parent not recorded for {:#?}", current));
        current = graph.get(&next_id).unwrap();
    }
    path.reverse();
    path
}

fn format_path(vertexes: Vec<Vertex>) -> String {
    let titles: Vec<String> = vertexes.into_iter().map(|v| v.title).collect();
    titles.join(" → ")
}

/// Breadth First Search from source to dest
fn bfs(source: &Vertex, dest: &Vertex, verbose: bool, conn: &PgConnection) {
    let mut visited_ids: HashSet<i32> = HashSet::new();
    let mut q: VecDeque<i32> = VecDeque::new();
    // parents = vertex id -> which vertex id came before
    let mut parents: HashMap<i32, i32> = HashMap::new();
    let mut graph: HashMap<i32, Vertex> = HashMap::new();

    graph.insert(source.id, source.clone());
    graph.insert(dest.id, dest.clone());

    q.push_back(source.id);
    visited_ids.insert(source.id);

    let sp = Spinner::new(&Spinners::Dots9, "Computing path".into());

    let mut visited_count = 0;

    loop {
        let current = q.pop_front();
        match current {
            Some(vertex_id) => {
                visited_count += 1;
                if verbose {
                    println!(
                        "{} → ...",
                        format_path(build_path(
                            source,
                            graph.get(&vertex_id).unwrap(),
                            &parents,
                            &graph
                        ))
                    );
                } else {
                    sp.message(format!(
                        "Computing path - visited {} pages, queue size {}",
                        visited_count,
                        q.len()
                    ));
                }
                if dest.id == vertex_id {
                    let path = build_path(source, dest, &parents, &graph);
                    println!("\npath: {}", format_path(path));
                    break;
                }
                let neighbors =
                    load_neighbors(graph.get(&vertex_id).unwrap(), &mut visited_ids, conn);
                for n in &neighbors {
                    parents.insert(n.id, vertex_id);
                    visited_ids.insert(n.id);
                    graph.insert(n.id, n.clone());
                    q.push_back(n.id);
                }
            }
            None => {
                println!("No path from {} to {}", source.title, dest.title);
                break;
            }
        }
    }

    sp.stop();
}

/// CLI Options
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Source article
    source: String,
    /// Destination article
    destination: String,
    /// Verbose output
    #[clap(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    let source_title = cli.source.replace(" ", "_");
    let dest_title = cli.destination.replace(" ", "_");

    println!("[{}] → [{}]", source_title, dest_title);

    let conn = establish_connection();

    // build_database(&conn);

    let home_dir = dirs::home_dir().unwrap();
    let data_dir = home_dir.join("wpsr");
    let vertex_al_path = data_dir.join("vertex_al");
    let vertex_ix_path = data_dir.join("vertex_ix");

    let mut graphdb = GraphDB::new(
        vertex_ix_path.to_str().unwrap(),
        vertex_al_path.to_str().unwrap(),
    )
    .unwrap();

    let source_vertex = load_vertex(&source_title, &conn).expect("source not found");
    let dest_vertex = load_vertex(&dest_title, &conn).expect("destination not found");

    graphdb.bfs(source_vertex.id as u32, dest_vertex.id as u32);

    bfs(&source_vertex, &dest_vertex, cli.verbose, &conn);
}
