#[macro_use]
extern crate diesel;
extern crate dotenv;

pub mod schema;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use schema::{edges, vertexes};
use std::env;

#[derive(Identifiable, Queryable, PartialEq, Debug)]
#[table_name = "vertexes"]
// If table name is not specified, diesel pluralizes to vertexs
pub struct Vertex {
    pub id: i32,
    pub title: String,
}

#[derive(Identifiable, Queryable, PartialEq, Debug)]
#[primary_key(source_vertex_id, dest_vertex_id)]
pub struct Edge {
    pub source_vertex_id: i32,
    pub dest_vertex_id: i32,
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

    let source_vertex = load_vertex(source_title, &conn);
    let dest_vertex = load_vertex(dest_title, &conn);
    println!("{:#?}", source_vertex);
    println!("{:#?}", dest_vertex);
}
