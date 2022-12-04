use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use crate::edge_db::EdgeDB;

struct NeighborList {
    data: HashMap<u32, Vec<u32>>,
}

impl NeighborList {
    fn new() -> Self {
        NeighborList {
            data: HashMap::new(),
        }
    }

    fn record(&mut self, vertex_id: u32, parent_id: u32) {
        self.data
            .entry(vertex_id)
            .and_modify(|v| v.push(parent_id))
            .or_insert_with(|| vec![parent_id]);
    }

    fn neighbors(&self, vertex_id: u32) -> Vec<u32> {
        self.data
            .get(&vertex_id)
            .map_or_else(std::vec::Vec::new, |v| v.to_owned())
    }

    fn all(&self) -> Vec<u32> {
        self.data.keys().cloned().collect_vec()
    }

    fn contains(&self, id: u32) -> bool {
        self.data.contains_key(&id)
    }

    fn has_some(&self) -> bool {
        !self.data.is_empty()
    }

    fn move_in(&mut self, from: &mut NeighborList) {
        for (k, v) in from.data.drain() {
            self.data
                .entry(k)
                .and_modify(|own_vals| own_vals.extend(v.clone()))
                .or_insert_with(|| v.clone());
        }
        assert!(from.data.is_empty());
    }

    fn intersection(x: &NeighborList, y: &NeighborList) -> Vec<u32> {
        let x = HashSet::<_>::from_iter(x.data.keys().copied());
        let y = HashSet::<_>::from_iter(y.data.keys().copied());
        let inter = HashSet::intersection(&x, &y);
        inter.copied().collect_vec()
    }
}

enum EdgeDirection {
    Incoming,
    Outgoing,
}

fn count_edges(vertexes: &Vec<u32>, direction: EdgeDirection, edge_db: &EdgeDB) -> usize {
    let mut count = 0;
    for v in vertexes {
        let al = edge_db.read_edges(*v);
        let al_count = match direction {
            EdgeDirection::Incoming => al.incoming.len(),
            EdgeDirection::Outgoing => al.outgoing.len(),
        };
        count += al_count;
    }
    count
}

pub fn breadth_first_search(
    source_vertex_id: u32,
    dest_vertex_id: u32,
    edge_db: &EdgeDB,
) -> Vec<Vec<u32>> {
    let mut paths: Vec<Vec<u32>> = Vec::new();
    if source_vertex_id == dest_vertex_id {
        paths.push(vec![source_vertex_id]);
        return paths;
    }

    // The unvisited dictionaries are a mapping from page ID to a list of that page's parents' IDs.
    let mut unvisited_forward = NeighborList::new();
    let mut unvisited_backward = NeighborList::new();

    // 0 is a magic value meaning "no parent"
    unvisited_forward.record(source_vertex_id, 0);
    unvisited_backward.record(dest_vertex_id, 0);

    // The visited dictionaries are a mapping from page ID to a list of that page's parents' IDs.
    let mut visited_forward = NeighborList::new();
    let mut visited_backward = NeighborList::new();

    //let mut forward_depth = 0;
    //let mut backward_depth = 0;

    // iterate furiously
    while paths.is_empty() && (unvisited_forward.has_some() && unvisited_backward.has_some()) {
        let forward_links_count =
            count_edges(&unvisited_forward.all(), EdgeDirection::Outgoing, edge_db);
        let backward_links_count =
            count_edges(&unvisited_backward.all(), EdgeDirection::Incoming, edge_db);

        if forward_links_count < backward_links_count {
            log::debug!("running forward bfs");
            // forward BFS
            //forward_depth += 1;

            // Remember the currently unvisited forward pages.
            let outgoing_visit_q = unvisited_forward.all();

            // Mark all of the unvisited forward pages as visited.
            // Clear unvisited forward
            visited_forward.move_in(&mut unvisited_forward);

            // Fetch the pages which can be reached from the currently unvisited forward pages.
            for current_source_id in outgoing_visit_q {
                let outgoing_edges = edge_db.read_edges(current_source_id).outgoing;
                for target_id in outgoing_edges {
                    //log::debug!("visiting outgoing page {}", target_id);
                    // If we have not yet visited this target, mark it as unvisited.
                    if !visited_forward.contains(target_id) {
                        unvisited_forward.record(target_id, current_source_id);
                    }
                }
            }
        } else {
            log::debug!("running backward bfs");
            // backward BFS
            //backward_depth += 1;

            // Remember the currently unvisited backward pages.
            let incoming_visit_q = unvisited_backward.all();

            // Mark all of the unvisited backward pages as visited.
            // Clear unvisited backward
            visited_backward.move_in(&mut unvisited_backward);

            // Fetch the pages which can be reached from the currently unvisited backward pages.
            for current_target_id in incoming_visit_q {
                let incoming_edges = edge_db.read_edges(current_target_id).incoming;
                for source_id in incoming_edges {
                    // log::debug!("visiting incoming page {}", source_id);
                    // If the source page has not been visited, mark it as unvisited.
                    if !visited_backward.contains(source_id) {
                        unvisited_backward.record(source_id, current_target_id);
                    }
                }
            }
        }

        // #---  CHECK FOR PATH COMPLETION  ---#
        // # The search is complete if any of the pages are in both unvisited backward and unvisited, so
        // # find the resulting paths.
        let intersection = NeighborList::intersection(&unvisited_forward, &unvisited_backward);
        for page_id in intersection {
            let paths_from_source =
                render_paths(unvisited_forward.neighbors(page_id), &visited_forward);
            let paths_from_target =
                render_paths(unvisited_backward.neighbors(page_id), &visited_backward);
            for path_from_source in paths_from_source {
                // TODO: Fix this clone.
                let paths_from_target = paths_from_target.clone();
                for path_from_target in paths_from_target {
                    let current_path = path_from_source
                        .clone()
                        .into_iter()
                        .chain(std::iter::once(page_id))
                        .chain(path_from_target.into_iter().rev())
                        .collect_vec();
                    if !paths.contains(&current_path) {
                        paths.push(current_path);
                    } else {
                        log::warn!("paths already contains just-computed path");
                    }
                }
            }
        }
    }

    paths
}

fn render_paths(ids: Vec<u32>, visited: &NeighborList) -> Vec<Vec<u32>> {
    let mut paths = Vec::new();
    for id in ids {
        if id == 0 {
            return vec![vec![]];
        }

        let current_paths = render_paths(visited.neighbors(id), visited);
        for current_path in current_paths {
            let mut new_path = current_path.clone();
            new_path.push(id);
            paths.push(new_path);
        }
    }
    paths
}
