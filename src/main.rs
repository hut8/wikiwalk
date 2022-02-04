#[macro_use]
extern crate diesel;
extern crate dotenv;

pub mod schema;

use clap::Parser;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use schema::{edges, vertexes};
use std::collections::{HashMap, HashSet, VecDeque};
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

fn load_vertex(name: &str, conn: &PgConnection) -> QueryResult<Vertex> {
    use crate::vertexes::dsl::*;
    vertexes.filter(title.ilike(name)).first::<Vertex>(conn)
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
    neighbors
}

fn build_path<'a>(
    source: &'a Vertex,
    dest: &'a Vertex,
    parents: &'a HashMap<Vertex, Vertex>,
) -> Vec<Vertex> {
    let mut path: Vec<Vertex> = Vec::new();
    let mut current = dest;
    loop {
        path.push(current.clone());
        if current.id == source.id {
            break;
        }
        current = parents
            .get(&current)
            .expect(&format!("parent not recorded for {:#?}", current));
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
    let mut q: VecDeque<Vertex> = VecDeque::new();
    // parents - vertex -> which vertex came before
    let mut parents: HashMap<Vertex, Vertex> = HashMap::new();

    q.push_back(source.clone());
    visited_ids.insert(source.id);

    loop {
        let current = q.pop_front();
        match current {
            Some(v) => {
                if verbose {
                    println!("{} → ...", format_path(build_path(source, &v, &parents)));
                }
                // FIXME: Compare references better
                if dest.id == v.id {
                    let path = build_path(source, dest, &parents);
                    println!("path: {}", format_path(path));
                    break;
                }
                let neighbors = load_neighbors(&v, &mut visited_ids, conn);
                for n in &neighbors {
                    // TODO visited_ids should be populated here; not in load_neighbors
                    parents.insert(n.clone(), v.clone());
                }
                q.extend(neighbors);
            }
            None => {
                println!("No path from {} to {}", source.title, dest.title);
                break;
            }
        }
    }
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

    let source_vertex = load_vertex(&source_title, &conn).expect("source not found");
    let dest_vertex = load_vertex(&dest_title, &conn).expect("destination not found");
    println!("{:#?}", source_vertex);
    println!("{:#?}", dest_vertex);

    bfs(&source_vertex, &dest_vertex, cli.verbose, &conn);
}
