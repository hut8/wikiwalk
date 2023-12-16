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
struct GraphData {
    vertexes: Vec<String>,
    edges: Vec<GraphEdge>,
}

pub async fn generate_sub_graph(
    sink_path: &std::path::Path,
    db: &sea_orm::DatabaseConnection,
) {
    log::info!("top graph: finding top pages");
    let top_page_ids = crate::api::top_page_ids(Some(100)).await;
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
    let mut page_id_set = HashSet::new();

    let mut paths = Vec::new();

    let (paths_tx, paths_rx) = crossbeam::channel::unbounded();
    let (queries_tx, queries_rx) = crossbeam::channel::unbounded();

    let computer_procs = (0..num_cpus::get()).map(|_| {
        let queries_rx = queries_rx.clone();
        let paths_tx = paths_tx.clone();
        tokio::spawn(async move {
            compute_graphs(queries_rx, paths_tx).await;
        })
    }).collect_vec();
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

    // page id -> page title
    let vertex_data: HashMap<u32, String> = Vertex::find()
        .select_only()
        .filter(wikiwalk::schema::vertex::Column::Id.is_in(page_id_set.clone()))
        .column(wikiwalk::schema::vertex::Column::Id)
        .column(wikiwalk::schema::vertex::Column::Title)
        .into_tuple()
        .all(db)
        .await
        .expect("query vertexes")
        .into_iter()
        .map(|(id, title)| (id, title))
        .collect();

    let mut edges = Vec::new();
    // let mut paths = Vec::new();
    for path in paths {
        for (source, target) in path.iter().tuple_windows() {
            let source_title = vertex_data.get(source).expect("source title");
            let target_title = vertex_data.get(target).expect("target title");
            edges.push(GraphEdge {
                source: source_title.to_string(),
                target: target_title.to_string(),
            });
        }
    }

    let vertexes: Vec<String> = vertex_data.values().cloned().collect();
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
        let pair_paths = gdb.bfs(source, target).await;
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
