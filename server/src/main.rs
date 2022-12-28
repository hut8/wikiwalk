use std::path::PathBuf;
use std::time::Instant;

use actix_web::{get, web, App, HttpServer, Responder};

use fern::colors::{Color, ColoredLevelConfig};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use wikipedia_speedrun::{schema, GraphDB};

use actix_web_static_files::ResourceFiles;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

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

#[derive(Deserialize)]
struct PathParams {
    source_id: u32,
    dest_id: u32,
}

#[get("/paths/{source_id}/{dest_id}")]
async fn paths(
    path: web::Path<PathParams>,
    gdb: web::Data<GraphDB>,
) -> actix_web::Result<impl Responder> {
    let source_id = path.source_id;
    let dest_id = path.dest_id;
    let timestamp = chrono::Utc::now();
    let start_time = Instant::now();
    let paths = match fetch_cache(source_id, dest_id, &gdb).await {
        Some(paths) => {
            log::debug!("fetched cached entry for {source_id} - {dest_id}");
            paths
        }
        None => gdb.bfs(source_id, dest_id).await,
    };
    let elapsed = start_time.elapsed();
    let count = paths.len();
    let lengths = paths.iter().map(|p| p.len());
    let degrees = lengths.max();
    let search = schema::search::ActiveModel {
        source_page_id: Set(source_id as i32),
        target_page_id: Set(dest_id as i32),
        timestamp: Set(timestamp.to_string()),
        duration: Set(elapsed.as_secs_f64()),
        ..Default::default()
    };
    search
        .insert(&gdb.master_db)
        .await
        .expect("insert log record");

    Ok(web::Json(PathData {
        paths,
        count,
        degrees,
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use actix_web::middleware::Logger;
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
    // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::White)
        .debug(Color::White)
    // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);
    let colors_level = colors_line.clone().info(Color::Green);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{target}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        .level(log::LevelFilter::Debug)
        .level_for("sqlx", log::LevelFilter::Warn)
        .chain(std::io::stdout())
        .apply()
        .expect("initialize logs");
    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("speedrun-data");
    let data_dir = match std::env::var("DATA_ROOT").ok() {
        Some(data_dir_str) => PathBuf::from(data_dir_str),
        None => default_data_dir,
    };
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let port = u16::from_str_radix(&port, 10).expect("parse port");
    let bind_addr = std::env::var("ADDRESS").unwrap_or_else(|_| "localhost".to_string());

    let gdb = GraphDB::new("current".into(), &data_dir).await.unwrap();
    let gdb_data = web::Data::new(gdb);

    tokio::spawn(async move { tls::launch_tls_redirect().await });

    HttpServer::new(move || {
        let generated = generate();
        App::new()
            .wrap(Logger::default())
            .app_data(gdb_data.clone())
            .service(paths)
            .service(ResourceFiles::new("/", generated))
    })
    .bind((bind_addr, port))?
    .run()
    .await?;

    Ok(())
}
