use std::{
    collections::{HashMap, HashSet},
    fs::File,
};

use itertools::Itertools;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use wikiwalk::{paths::Paths, schema::prelude::Vertex, GraphDB};

#[derive(Serialize)]
struct GraphEdge {
    source: String,
    target: String,
}

#[derive(Serialize)]
struct GraphVertex {
    id: String,
    title: String,
    color: String,
}

#[derive(Serialize)]
struct GraphData {
    vertexes: Vec<GraphVertex>,
    edges: Vec<GraphEdge>,
}

pub async fn generate_sub_graph(sink_path: &std::path::Path, db: &sea_orm::DatabaseConnection) {
    log::info!("top graph: finding top pages");
    let top_page_ids = crate::api::top_page_ids(Some(10)).await;
    log::info!("top graph: found {} valid top pages", top_page_ids.len());
    let page_ids: Vec<u32> = Vertex::find()
        .select_only()
        .filter(wikiwalk::schema::vertex::Column::Id.is_in(top_page_ids.clone()))
        .column(wikiwalk::schema::vertex::Column::Id)
        .order_by(wikiwalk::schema::vertex::Column::Id, sea_orm::Order::Asc)
        .into_tuple()
        .all(db)
        .await
        .expect("query vertexes");
    let missing_page_ids: Vec<u32> = top_page_ids
        .clone()
        .into_iter()
        .filter(|page_id| !page_ids.contains(page_id))
        .collect();
    log::info!(
        "sitemap: found {} valid top pages in db (missing {:?})",
        page_ids.len(),
        missing_page_ids
    );
    let sources = page_ids.clone();
    let targets = page_ids.clone();
    let pairs = sources
        .into_iter()
        .cartesian_product(targets.into_iter())
        .filter(|(source, target)| *source != *target)
        .collect::<Vec<(u32, u32)>>();
    log::info!("sitemap: found {} pairs", pairs.len());

    // page_id_set is the set of all page ids that we encounter in the paths between the top pages
    let top_page_id_set: HashSet<u32> = top_page_ids.into_iter().collect();
    let mut page_id_set = HashSet::new();

    let mut paths = Vec::new();

    let (paths_tx, paths_rx) = crossbeam::channel::unbounded();
    let (queries_tx, queries_rx) = crossbeam::channel::unbounded();

    let computer_procs = (0..num_cpus::get())
        .map(|_| {
            let queries_rx = queries_rx.clone();
            let paths_tx = paths_tx.clone();
            tokio::spawn(async move {
                compute_graphs(queries_rx, paths_tx).await;
            })
        })
        .collect_vec();
    drop(paths_tx);

    for (source, target) in pairs {
        let tx = queries_tx.clone();
        tx.send((source, target)).expect("send query");
    }
    drop(queries_tx);
    drop(queries_rx);

    futures::future::join_all(computer_procs).await;

    log::info!("top graph: waiting for paths");
    for path in paths_rx {
        for page_id in path.iter().flatten() {
            page_id_set.insert(*page_id);
        }
        paths.extend(path);
    }

    log::info!("top graph: resolving {} total vertexes", page_id_set.len());

    let page_ids = page_id_set.into_iter().collect_vec();
    let resolve_futures = page_ids.chunks(1000).map(|chunk| resolve_pages(chunk, db));

    // page id -> page title
    let page_id_title_maps = futures::future::join_all(resolve_futures).await;
    let vertex_data = page_id_title_maps
        .into_iter()
        .reduce(|mut acc, e| {
            for ele in e {
                acc.insert(ele.0, ele.1.to_string());
            }
            acc
        })
        .expect("vertex data present");
    log::info!("top graph: resolved {} pages", vertex_data.len());

    log::info!("top graph: building edge list");
    let mut edges = Vec::new();
    for path in paths {
        for (source, target) in path.iter().tuple_windows() {
            edges.push(GraphEdge {
                source: source.to_string(),
                target: target.to_string(),
            });
        }
    }

    log::info!("top graph: building vertex list");
    let vertexes: Vec<GraphVertex> = vertex_data
        .into_iter()
        .map(|(id, title)| GraphVertex {
            id: id.to_string(),
            color: if top_page_id_set.contains(&id) {
                "red".to_string()
            } else {
                "blue".to_string()
            },
            title,
        })
        .collect();

    log::info!("top graph: writing graph data to file");
    let graph_data = GraphData { vertexes, edges };
    let topgraph_file = File::create(sink_path).expect("create topgraph file");
    serde_json::to_writer_pretty(topgraph_file, &graph_data).expect("write topgraph file");
}

async fn compute_graphs(
    queries_rx: crossbeam::channel::Receiver<(u32, u32)>,
    results_tx: crossbeam::channel::Sender<Vec<Vec<u32>>>,
) {
    let data_root = Paths::new().base;
    let gdb = GraphDB::new("current".to_string(), &data_root)
        .await
        .expect("create graph db");
    for (source, target) in queries_rx {
        let start_time = std::time::Instant::now();
        let pair_paths = gdb.bfs(source, target, false).await;
        let elapsed = start_time.elapsed();
        log::info!(
            "found {} paths between {} and {} in {} seconds",
            pair_paths.len(),
            source,
            target,
            elapsed.as_secs_f32(),
        );
        results_tx.send(pair_paths).expect("send result");
    }
}

async fn resolve_pages(page_ids: &[u32], db: &sea_orm::DatabaseConnection) -> HashMap<u32, String> {
    let predicate = page_ids.to_owned();
    let vertex_data: HashMap<u32, String> = Vertex::find()
        .select_only()
        .filter(wikiwalk::schema::vertex::Column::Id.is_in(predicate.clone()))
        .column(wikiwalk::schema::vertex::Column::Id)
        .column(wikiwalk::schema::vertex::Column::Title)
        .into_tuple()
        .all(db)
        .await
        .expect("query vertexes")
        .into_iter()
        .map(|(id, title)| (id, title))
        .collect();
    vertex_data
}
