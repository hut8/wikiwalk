use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use actix_cors::Cors;
use actix_files as fs;
use actix_web::middleware::Logger;
use actix_web::{get, guard, web, App, Error, HttpResponse, HttpServer, Responder};
use actix_web_lab::{header::StrictTransportSecurity, middleware::RedirectHttps};

use fern::colors::{Color, ColoredLevelConfig};

use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use static_files::Resource;
use wikiwalk::paths::Paths;
use wikiwalk::{schema, GraphDB};

use actix_web_static_files::ResourceFiles;
use chrono::NaiveDate;
use wikiwalk::dbstatus::DBStatus;

mod content_negotiation;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[derive(Serialize)]
struct PathData {
    paths: Vec<Vec<u32>>,
    count: usize,
    degrees: Option<usize>,
    duration: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseStatus {
    date: Option<NaiveDate>,
    vertex_count: u32,
    edge_count: u32,
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

#[get("/status")]
async fn status(db_status: web::Data<DBStatus>) -> actix_web::Result<impl Responder> {
    Ok(web::Json(DatabaseStatus {
        date: db_status.dump_date(),
        edge_count: db_status.edge_count,
        vertex_count: db_status.vertex_count,
    }))
}

#[get("/sitemaps/{filename}")]
async fn sitemap(params: web::Path<String>) -> Result<fs::NamedFile, Error> {
    let filename = params.into_inner();
    let sitemaps_path = Paths::new().db_paths("current").sitemaps_path();
    let sitemap_path = sitemaps_path.join(filename);
    let file = fs::NamedFile::open(sitemap_path)?;
    Ok(file.use_last_modified(true))
}

// SPA Route
async fn serve_ui_paths(
    path: web::Path<PathParams>,
    gdb: web::Data<GraphDB>,
    statics: web::Data<HashMap<&str, Resource>>,
) -> actix_web::Result<impl Responder> {
    let source_id = path.source_id;
    let dest_id = path.dest_id;
    // find vertexes to avoid soft 404
    let source_vertex = schema::vertex::Entity::find_by_id(source_id)
        .one(&gdb.graph_db)
        .await
        .expect("query source vertex");
    let dest_vertex = schema::vertex::Entity::find_by_id(dest_id)
        .one(&gdb.graph_db)
        .await
        .expect("query destination vertex");
    if source_vertex.is_none() || dest_vertex.is_none() {
        // TODO: Make a real 404 page
        return Ok(HttpResponse::NotFound().body("404 Source or destination page not found"));
    }
    let content = statics.get("index.html").expect("index.html resource");
    Ok(HttpResponse::Ok().body(content.data))
}

#[get("/top-graph")]
async fn serve_top_graph() -> actix_web::Result<impl Responder> {
    let top_graph_path = Paths::new().db_paths("current").topgraph_path();
    let source = fs::NamedFile::open(top_graph_path).map_err(|e| {
        log::error!("failed to open top graph: {:?}", e);
        e
    })?;
    Ok(source)
}

async fn serve_paths(
    path: web::Path<PathParams>,
    gdb: web::Data<GraphDB>,
) -> actix_web::Result<impl Responder> {
    let source_id = path.source_id;
    let dest_id = path.dest_id;
    log::info!("finding paths from {source_id} to {dest_id}");
    let timestamp = chrono::Utc::now();
    let start_time = Instant::now();
    let paths = match fetch_cache(source_id, dest_id, &gdb).await {
        Some(paths) => {
            log::debug!("fetched cached entry for {source_id} - {dest_id}");
            paths
        }
        None => gdb.bfs(source_id, dest_id, true).await,
    };
    let elapsed = start_time.elapsed();
    let count = paths.len();
    let lengths = paths.iter().map(|p| p.len());
    let degrees = lengths.max().map(|i| i - 1);
    let search = schema::search::ActiveModel {
        source_page_id: Set(source_id as i32),
        target_page_id: Set(dest_id as i32),
        timestamp: Set(timestamp.to_string()),
        duration: Set(elapsed.as_millis() as i64),
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
        duration: elapsed.as_millis(),
    }))
}

#[derive(Debug)]
enum Environment {
    Development,
    Production,
}

impl Environment {
    fn current() -> Environment {
        match std::env::var("WIKIWALK_ENV").as_deref() {
            Ok("production") => Environment::Production,
            _ => Environment::Development,
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let sentry_dsn = match Environment::current() {
      Environment::Development => "",
      Environment::Production => "https://2377de2b8f109351d1d4d349e0f152e0@o4506004333199360.ingest.sentry.io/4506004334510080",
    };
    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::White)
        .debug(Color::White)
        // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);
    let colors_level = colors_line.info(Color::Green);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{target}] [{level}{color_line}] {message}\x1B[0m",
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
        .level_for("h2", log::LevelFilter::Warn)
        .level_for("rustls", log::LevelFilter::Error)
        .chain(std::io::stdout())
        .apply()
        .expect("initialize logs");

    let home_dir = dirs::home_dir().unwrap();
    let default_data_dir = home_dir.join("data").join("wikiwalk");
    let data_dir = match std::env::var("DATA_ROOT").ok() {
        Some(data_dir_str) => PathBuf::from(data_dir_str),
        None => default_data_dir,
    };
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();

    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let port = port.parse::<u16>().expect("parse port");
    let bind_addr = std::env::var("ADDRESS").unwrap_or_else(|_| "localhost".to_string());
    log::info!("binding to {}:{}", bind_addr, port);
    let cert_path = std::env::var("TLS_CERT").ok();
    let key_path = std::env::var("TLS_KEY").ok();
    let well_known_path = std::env::var("WELL_KNOWN_ROOT").ok();
    let enable_https = matches!((&cert_path, &key_path), (Some(_), Some(_)));

    let db_paths = Paths::new().db_paths("current");
    let db_status = DBStatus::load(db_paths.db_status_path());
    let db_status_data = web::Data::new(db_status);

    let gdb = GraphDB::new("current".into(), &data_dir).await.unwrap();
    // Sanity check database quickly
    if let Err(err) = gdb.edge_db.check_sizes() {
        log::error!("database sanity check failed: {:?}", err);
        std::process::exit(1);
    }
    let gdb_data = web::Data::new(gdb);

    let mut server = HttpServer::new(move || {
        let generated = generate();
        let generated_data = web::Data::new(generate());
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET"])
            .allow_any_header()
            .max_age(3600);
        let app = App::new()
            .wrap(sentry_actix::Sentry::new())
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(actix_web::middleware::Condition::new(
                enable_https,
                RedirectHttps::with_hsts(StrictTransportSecurity::default()),
            ))
            .app_data(gdb_data.clone())
            .app_data(db_status_data.clone())
            .app_data(generated_data.clone())
            .route(
                "/paths/{source_id}/{dest_id}",
                web::route()
                    .guard(guard::Get())
                    .guard(content_negotiation::accept_json_guard)
                    .to(serve_paths),
            )
            .route(
                "/paths/{source_id}/{dest_id}",
                web::route()
                    .guard(guard::Get())
                    .guard(content_negotiation::accept_html_guard)
                    .to(serve_ui_paths),
            )
            .service(serve_top_graph)
            .service(status)
            .service(sitemap);
        match &well_known_path {
            Some(well_known_path) => {
                // optionally add .well-known static files path so that lego can do HTTP acme challenge on port 80
                app.service(
                    actix_files::Files::new("/.well-known", well_known_path).use_hidden_files(),
                )
            }
            None => app,
        }
        .service(ResourceFiles::new("/", generated))
    });

    server = server.bind((bind_addr.clone(), port))?;

    server.run().await?;

    Ok(())
}
