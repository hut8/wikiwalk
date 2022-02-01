table! {
    edges (source_vertex_id, dest_vertex_id) {
        source_vertex_id -> Int4,
        dest_vertex_id -> Int4,
    }
}

table! {
    vertexes (id) {
        id -> Int4,
        title -> Varchar,
    }
}

allow_tables_to_appear_in_same_query!(
    edges,
    vertexes,
);
