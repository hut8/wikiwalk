#!/usr/bin/bash
DATA_DIR='/srv/storage/data'

mysql wiki -e <<EOF
 SELECT page_id, page_title
 FROM vertexes ORDER BY
 page_id INTO OUTFILE "$DATA_DIR/vertexes";
EOF

mysql wiki -e <<EOF
 SELECT source_page_id, dest_page_id
 FROM edges ORDER BY
 source_page_id INTO OUTFILE "$DATA_DIR/edges";
EOF

cd "$DATA_DIR"
xz vertexes
xz edges

pv vertexes.xz |
    xzcat |
    psql --dbname=wiki \
         --command='COPY vertexes (id, title) FROM STDIN'
pv edges.xz |
    xzcat |
    psql --dbname=wiki \
         --command='COPY edges (source_vertex_id, dest_vertex_id) FROM STDIN'
