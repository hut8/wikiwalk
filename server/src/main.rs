use std::path::PathBuf;

use rocket::serde::Serialize;
use rocket::State;
use rocket::{fs::FileServer, serde::json::Json};

use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DbBackend, DbConn, EntityTrait, QueryFilter, Schema,
};
use wikipedia_speedrun::{schema, GraphDB, Vertex};

mod tls;

#[derive(Serialize)]
struct PathData {
    paths: Vec<Vec<u32>>,
    count: usize,
    degrees: Option<usize>,
}

#[rocket::get("/paths/<source_id>/<dest_id>")]
async fn paths(source_id: u32, dest_id: u32, gdb: &State<GraphDB>) -> Json<PathData> {
    let paths = gdb.bfs(source_id, dest_id).await;
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

async fn make_database(db: &DbConn) {
    let schema = Schema::new(DbBackend::Sqlite);
    let mut create_stmt = schema.create_table_from_entity(schema::search::Entity);
    let stmt = create_stmt.if_not_exists();

    db.execute(db.get_database_backend().build(stmt))
        .await
        .expect("create table");
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
    let graph_db_path = data_dir.join("graph.db");
    let conn_str = format!("sqlite:///{}?mode=ro", graph_db_path.to_string_lossy());
    let graph_db: DbConn = Database::connect(conn_str).await.expect("graph db connect");
    let master_db_path = data_dir.join("master.db");
    let master_conn_str = format!("sqlite:///{}?mode=rwc", master_db_path.to_string_lossy());
    let master_db: DbConn = Database::connect(master_conn_str)
        .await
        .expect("master db connect");
    make_database(&master_db).await;

    let static_root = std::env::var("STATIC_ROOT").unwrap_or_else(|_| "../ui/dist".into());

    let gdb = GraphDB::new(
        "current".into(),
        &data_dir,
        graph_db,
        master_db,
    )
    .unwrap();

    tokio::spawn(async move { tls::launch_tls_redirect().await });

    let _ = rocket::build()
        .manage(gdb)
        .mount("/", FileServer::from(static_root))
        .mount("/", rocket::routes![paths, pages])
        .launch()
        .await;
}
