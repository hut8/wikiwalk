use std::{collections::{HashMap, HashSet}, fs::File};

use itertools::Itertools;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use wikiwalk::{schema::prelude::Vertex, GraphDB};

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

pub async fn generate_sub_graph(sink_path: &std::path::Path, db: &sea_orm::DatabaseConnection, gdb: &GraphDB) {
    log::info!("sitemap: finding top pages");
    let top_page_ids = crate::api::top_page_ids().await;
    log::info!("sitemap: found {} valid top pages", top_page_ids.len());
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

    // Compute all of the paths between all those pairs
    for (source, target) in pairs {
        let pair_paths = gdb.bfs(source, target).await;
        if pair_paths.is_empty() {
            log::warn!("no path found between {} and {}", source, target);
            continue;
        }
        page_id_set.extend(pair_paths.iter().flatten().cloned());
        for path in pair_paths {
            paths.push(path);
        }
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

    let gd = GraphData { vertexes, edges };
    let topgraph_file = File::create(sink_path).expect("create topgraph file");
    serde_json::to_writer_pretty(topgraph_file, &gd).expect("write topgraph file");
}
