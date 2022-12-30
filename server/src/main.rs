use std::fs::File;
use std::time::Instant;
use std::{io::BufReader, path::PathBuf};

use actix_web::{get, web, App, HttpServer, Responder};
use actix_web_lab::{header::StrictTransportSecurity, middleware::RedirectHttps};

use fern::colors::{Color, ColoredLevelConfig};

use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, read_one, Item};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use wikipedia_speedrun::{schema, GraphDB};

use actix_web_static_files::ResourceFiles;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

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
    let default_data_dir = home_dir.join("data").join("speedrun-data");
    let data_dir = match std::env::var("DATA_ROOT").ok() {
        Some(data_dir_str) => PathBuf::from(data_dir_str),
        None => default_data_dir,
    };
    log::debug!("using data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).unwrap();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let port = port.parse::<u16>().expect("parse port");
    let bind_addr = std::env::var("ADDRESS").unwrap_or_else(|_| "localhost".to_string());
    let cert_path = std::env::var("TLS_CERT").ok();
    let key_path = std::env::var("TLS_KEY").ok();
    let enable_https = matches!((&cert_path, &key_path), (Some(_), Some(_)));

    let gdb = GraphDB::new("current".into(), &data_dir).await.unwrap();
    let gdb_data = web::Data::new(gdb);

    let mut server = HttpServer::new(move || {
        let generated = generate();
        App::new()
            .wrap(Logger::default())
            .wrap(actix_web::middleware::Condition::new(
                enable_https,
                RedirectHttps::with_hsts(StrictTransportSecurity::default()),
            ))
            .app_data(gdb_data.clone())
            .service(paths)
            .service(ResourceFiles::new("/", generated))
    });

    if let (Some(cert_path), Some(key_path)) = (cert_path, key_path) {
        // enable TLS
        log::info!("enabling tls");
        log::info!("tls cert={cert_path}");
        log::info!("tls key={key_path}");
        let tls_config = load_rustls_config(&cert_path, &key_path);
        server = server.bind_rustls((bind_addr.clone(), port), tls_config).expect("unable to create tls listener");
        // enable port 80 -> 443 redirects
        server = server.bind((bind_addr.clone(), 80))?;
    } else {
        server = server.bind((bind_addr.clone(), port))?;
    }

    server.run().await?;

    Ok(())
}

fn load_rustls_config(cert_path: &str, key_path: &str) -> rustls::ServerConfig {
    // init server config builder with safe defaults
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth();

    // load TLS key/cert files
    let cert_file = &mut BufReader::new(File::open(cert_path).unwrap());
    let key_file = &mut BufReader::new(File::open(key_path).unwrap());

    let key = match read_one(key_file).unwrap().expect("did not find key in key file") {
        Item::ECKey(key) => PrivateKey(key),
        _ => {
            log::error!("expected to find valid key in keyfile at {key_path}");
            std::process::exit(1);
        }
    };

    // convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();

    config.with_single_cert(cert_chain, key).unwrap()
}
