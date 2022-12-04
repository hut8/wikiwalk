use sea_orm::{Database, DbConn};

#[rocket::get("/paths/<source_id>/<dest_id>")]
fn paths(source_id: u32, dest_id: u32) -> &'static str {
    "Hello, world!"
}

// API method to search pages based on title
#[rocket::get("/pages?<title>")]
fn pages(title: &str) -> &'static str {
    "Hello, world!"
}

#[rocket::get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[rocket::launch]
async fn rocket() -> _ {
    stderrlog::new()
        .module(module_path!())
        .quiet(false)
        .verbosity(4)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();

    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("speedrun-data");
    // TODO: Env var
    let data_dir = default_data_dir;
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();
    let vertex_al_path = data_dir.join("vertex_al");
    let vertex_ix_path = data_dir.join("vertex_al_ix");
    let db_path = data_dir.join("wikipedia-speedrun.db");
    let conn_str = format!("sqlite:///{}?mode=ro", db_path.to_string_lossy());
    log::debug!("using database: {}", conn_str);
    let db: DbConn = Database::connect(conn_str).await.expect("db connect");

    let mut gdb = GraphDB::new(
      vertex_ix_path.to_str().unwrap(),
      vertex_al_path.to_str().unwrap(),
      db,
  )
  .unwrap();

    rocket::build().mount("/", rocket::routes![index, paths, pages])
}
