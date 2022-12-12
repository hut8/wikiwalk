use std::path::PathBuf;

use rocket::{serde::json::Json, fs::FileServer};
use rocket::State;

use sea_orm::{ColumnTrait, Database, DbConn, EntityTrait, QueryFilter};
use wikipedia_speedrun::{schema, GraphDB, Vertex};

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
        .all(&gdb.db)
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

#[rocket::launch]
async fn rocket() -> _ {
    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("speedrun-data");
    let data_dir = match std::env::var("DATA_ROOT").ok() {
      Some(data_dir_str) => PathBuf::from(data_dir_str),
      None => default_data_dir
    };
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();
    let vertex_al_path = data_dir.join("vertex_al");
    let vertex_ix_path = data_dir.join("vertex_al_ix");
    let db_path = data_dir.join("wikipedia-speedrun.db");
    let conn_str = format!("sqlite:///{}?mode=ro", db_path.to_string_lossy());
    log::debug!("using database: {}", conn_str);
    let db: DbConn = Database::connect(conn_str).await.expect("db connect");
    let static_root = std::env::var("STATIC_ROOT").unwrap_or("../ui/dist".into());

    let gdb = GraphDB::new(
        vertex_ix_path.to_str().unwrap(),
        vertex_al_path.to_str().unwrap(),
        db,
    )
    .unwrap();

    rocket::build()
        .manage(gdb)
        .mount("/", FileServer::from(static_root))
        .mount("/", rocket::routes![paths, pages])
}
