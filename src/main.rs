#[macro_use]
extern crate diesel;
extern crate dotenv;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env;

#[derive(Queryable)]
pub struct Vertex {
    pub id: u32,
    pub title: String,
}

#[derive(Queryable)]
pub struct Edge {
    pub source_vertex_id: u32,
    pub dest_vertex_id: u32,
}

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

fn page_id_by_name(name: &str) -> Option<u64> {
    None
}

fn print_usage(exe: &str) {
    println!("Usage: {} 'Source Article' 'Destination Article'", exe);
}

fn main() {
    println!("Wikipedia Speedrun Computer");
    let args: Vec<String> = env::args().collect();
    let exe = &args[0];

    if args.len() != 3 {
        print_usage(exe);
        return;
    }
    let origin_title = &args[1];
    let dest_title = &args[2];

    println!("[{}] → [{}]", origin_title, dest_title);

    let conn = establish_connection();
}
