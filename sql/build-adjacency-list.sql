\set FETCH_COUNT 10000
SELECT source_vertex_id, string_agg(dest_vertex_id::text, ',')
     FROM edges
 GROUP BY source_vertex_id;
