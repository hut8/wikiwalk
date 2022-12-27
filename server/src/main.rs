use std::path::PathBuf;

use rocket::serde::Serialize;
use rocket::State;
use rocket::{fs::FileServer, serde::json::Json};

use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use wikipedia_speedrun::{schema, GraphDB, Vertex};

mod tls;

#[derive(Serialize)]
struct PathData {
    paths: Vec<Vec<u32>>,
    count: usize,
    degrees: Option<usize>,
}

async fn fetch_cache(source_id: u32, dest_id: u32, gdb: &GraphDB) -> Option<Vec<Vec<u32>>> {
    let path = schema::path::Entity::find()
        .filter(
            Condition::all()
                .add(schema::path::Column::SourcePageId.eq(source_id))
                .add(schema::path::Column::TargetPageId.eq(dest_id)),
        )
        .one(&gdb.graph_db)
        .await
        .expect("find cached path");

    match path {
        Some(path) => serde_json::from_str(&path.path_data).ok(),
        None => None,
    }
}

#[rocket::get("/paths/<source_id>/<dest_id>")]
async fn paths(source_id: u32, dest_id: u32, gdb: &State<GraphDB>) -> Json<PathData> {
    let paths = match fetch_cache(source_id, dest_id, gdb).await {
        Some(paths) => {
            log::debug!("fetched cached entry for {source_id} - {dest_id}");
            paths
        }
        None => gdb.bfs(source_id, dest_id).await,
    };
    let count = paths.len();
    let lengths = paths.iter().map(|p| p.len());
    let degrees = lengths.max();
    Json(PathData {
        paths,
        count,
        degrees,
    })
}

// API method to search pages based on title
#[rocket::get("/pages?<title>")]
async fn pages(title: &str, gdb: &State<GraphDB>) -> Json<Vec<Vertex>> {
    let vertex_models = schema::vertex::Entity::find()
        .filter(schema::vertex::Column::Title.starts_with(title))
        .all(&gdb.graph_db)
        .await
        .expect("find vertex by title");
    Json(
        vertex_models
            .iter()
            .map(|v| Vertex {
                id: v.id,
                is_redirect: v.is_redirect,
                title: v.title.clone(),
            })
            .collect(),
    )
}

#[rocket::main]
async fn main() {
    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("speedrun-data");
    let data_dir = match std::env::var("DATA_ROOT").ok() {
        Some(data_dir_str) => PathBuf::from(data_dir_str),
        None => default_data_dir,
    };
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();

    let static_root = std::env::var("STATIC_ROOT").unwrap_or_else(|_| "../ui/dist".into());

    let gdb = GraphDB::new("current".into(), &data_dir).await.unwrap();

    tokio::spawn(async move { tls::launch_tls_redirect().await });

    let _ = rocket::build()
        .manage(gdb)
        .mount("/", FileServer::from(static_root))
        .mount("/", rocket::routes![paths, pages])
        .launch()
        .await;
}
