#[macro_use]
extern crate diesel;
extern crate dotenv;

pub mod schema;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use schema::{edges, vertexes};
use std::collections::{HashSet, VecDeque};
use std::env;
use std::hash::{Hash, Hasher};
// use std::process::exit;

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

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

fn print_usage(exe: &str) {
    println!("Usage: {} 'Source Article' 'Destination Article'", exe);
}

fn load_vertex(name: &str, conn: &PgConnection) -> QueryResult<Vertex> {
    use crate::vertexes::dsl::*;
    vertexes.filter(title.eq(name)).first::<Vertex>(conn)
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
    //let neighbors = vertexes.fil
    let neighbor_ids: Vec<i32> = edges
        .iter()
        .map(|e| e.dest_vertex_id)
        .filter(|x| !visited_ids.contains(x))
        .collect();
    // Now that we have encountered the neighbors, mark them as visited
    visited_ids.extend(neighbor_ids.iter());
    let neighbors = vertexes
        .filter(id.eq(any(neighbor_ids)))
        .load::<Vertex>(conn)
        .expect("load neighbors");
    println!("neighbors of [{}]: {}", source.title, neighbors.len());
    neighbors
}

fn bfs(source: &Vertex, dest: &Vertex, conn: &PgConnection) {
    //let visited: HashSet<Vertex> = HashSet::from(&[source]);
    //let q: VecDeque<Vertex> = VecDeque::from(&[source]);
    let mut visited_ids: HashSet<i32> = HashSet::new();
    let mut q: VecDeque<Vertex> = VecDeque::new();

    q.push_back(source.clone());
    visited_ids.insert(source.id);

    loop {
        let current = q.pop_back();
        match current {
            Some(v) => {
                let neighbors = load_neighbors(&v, &mut visited_ids, conn);
                q.extend(neighbors);
            }
            None => {
                println!("queue empty");
                break;
            }
        }
    }
}

fn main() {
    println!("Wikipedia Speedrun Computer");
    let args: Vec<String> = env::args().collect();
    let exe = &args[0];

    if args.len() != 3 {
        print_usage(exe);
        return;
    }
    let source_title = &args[1];
    let dest_title = &args[2];

    println!("[{}] → [{}]", source_title, dest_title);

    let conn = establish_connection();

    let source_vertex = load_vertex(source_title, &conn).expect("source not found");
    let dest_vertex = load_vertex(dest_title, &conn).expect("destination not found");
    println!("{:#?}", source_vertex);
    println!("{:#?}", dest_vertex);

    bfs(&source_vertex, &dest_vertex, &conn);
}
