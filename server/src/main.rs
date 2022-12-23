use std::path::PathBuf;

use rocket::State;
use rocket::{fs::FileServer, serde::json::Json};

use sea_orm::{ColumnTrait, Database, DbConn, EntityTrait, QueryFilter};
use wikipedia_speedrun::{schema, GraphDB, Vertex};

mod tls;

#[rocket::get("/paths/<source_id>/<dest_id>")]
fn paths(source_id: u32, dest_id: u32, gdb: &State<GraphDB>) -> Json<Vec<Vec<u32>>> {
    let paths = gdb.bfs(source_id, dest_id);
    Json(paths)
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
    let vertex_al_path = data_dir.join("vertex_al");
    let vertex_ix_path = data_dir.join("vertex_al_ix");
    let graph_db_path = data_dir.join("graph.db");
    let conn_str = format!("sqlite:///{}?mode=ro", graph_db_path.to_string_lossy());
    let graph_db: DbConn = Database::connect(conn_str).await.expect("graph db connect");
    let master_db_path = data_dir.join("master.db");
    let master_conn_str = format!("sqlite:///{}?mode=rwc", master_db_path.to_string_lossy());
    let master_db: DbConn = Database::connect(master_conn_str).await.expect("master db connect");
    let static_root = std::env::var("STATIC_ROOT").unwrap_or("../ui/dist".into());

    let gdb = GraphDB::new(
        vertex_ix_path.to_str().unwrap(),
        vertex_al_path.to_str().unwrap(),
        graph_db,
        master_db,
    )
    .unwrap();

    tokio::spawn(async move {
        tls::launch_tls_redirect().await
    });

    let _ = rocket::build()
        .manage(gdb)
        .mount("/", FileServer::from(static_root))
        .mount("/", rocket::routes![paths, pages])
        .launch()
        .await;
}
