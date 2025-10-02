import { CosmographProvider, Cosmograph } from "@cosmograph/react";
import React from "react";
import { Edge, Vertex, PagePaths, pathsGraph } from "./service";
import { Box } from "@mui/material";

const randomColor = () =>
    `#${Math.floor(Math.random() * 16777215).toString(16)}ff`;

export function PathNetworkGraph({ paths }: { paths: PagePaths }) {
    const [vertexes, setVertexes] = React.useState<Vertex[]>([]);
    const [edges, setEdges] = React.useState<Edge[]>([]);

    React.useEffect(() => {
        const graph = pathsGraph(paths);

        // Color each vertex
        for (const vertex of graph.vertexes) {
            vertex.color = randomColor();
        }
        setVertexes(graph.vertexes);

        // Color each edge based on its source vertex
        const findVertex = (id: string) => graph.vertexes.find((v) => v.id === id);
        for (const edge of graph.edges) {
            const src = findVertex(edge.source);
            edge.color = src?.color || "grey";
        }
        setEdges(graph.edges);
    }, [paths]);

    return (
        <Box sx={{ height: '80vh', maxHeight: '80vh', width: '100%' }}>
            <CosmographProvider nodes={vertexes} links={edges}>
                <Box sx={{ paddingTop: 1, paddingBottom: 1 }}>
                    Graph visualization of the path connections
                </Box>
                <Cosmograph<Vertex, Edge>
                nodeColor={(d) => d.color || null}
                nodeLabelAccessor={(d) => d.title}
                nodeSize={1}
                nodeLabelColor={() => "white"}
                hoveredNodeLabelColor={() => "white"}
                showTopLabels={false}
                showDynamicLabels={true}
                fitViewDelay={1000}
                disableSimulation={false}
                spaceSize={4096}
                curvedLinks={true}
                linkColor={(d) => d.color || null}
                linkWidth={2}
                initialZoomLevel={2}
            />
            </CosmographProvider>
        </Box>
    );
}
