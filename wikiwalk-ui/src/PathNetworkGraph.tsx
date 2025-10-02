import { CosmographProvider, Cosmograph } from "@cosmograph/react";
import React from "react";
import { Edge, Vertex, PagePaths, pathsGraph } from "./service";
import { Box } from "@mui/material";

const randomColor = () => {
    // Generate bright colors with good contrast against dark gray background
    const hue = Math.floor(Math.random() * 360);
    const saturation = 70 + Math.floor(Math.random() * 30); // 70-100%
    const lightness = 60 + Math.floor(Math.random() * 20); // 60-80% for brightness
    return `hsl(${hue}, ${saturation}%, ${lightness}%)`;
};

export function PathNetworkGraph({ paths }: { paths: PagePaths }) {
    const [vertexes, setVertexes] = React.useState<Vertex[]>([]);
    const [edges, setEdges] = React.useState<Edge[]>([]);
    const [graphHeight, setGraphHeight] = React.useState<number>(600);
    const containerRef = React.useRef<HTMLDivElement>(null);

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

    React.useEffect(() => {
        const calculateHeight = () => {
            if (containerRef.current) {
                const rect = containerRef.current.getBoundingClientRect();
                const availableHeight = window.innerHeight - rect.top;
                setGraphHeight(Math.max(400, availableHeight - 20)); // Minimum 400px, with 20px padding
            }
        };

        calculateHeight();
        window.addEventListener('resize', calculateHeight);
        return () => window.removeEventListener('resize', calculateHeight);
    }, []);

    return (
        <Box ref={containerRef} sx={{ height: `${graphHeight}px`, width: '100%' }}>
            <CosmographProvider nodes={vertexes} links={edges}>
                <Box sx={{ paddingTop: 1, paddingBottom: 1 }}>
                    Graph visualization of the path connections
                </Box>
                <Cosmograph<Vertex, Edge>
                nodeColor={(d) => d.color || null}
                nodeLabelAccessor={(d) => d.title}
                nodeSize={12}
                nodeLabelColor={() => "white"}
                hoveredNodeLabelColor={() => "white"}
                showTopLabels={true}
                showTopLabelsLimit={20}
                showTopLabelsValueKey="rank"
                showDynamicLabels={true}
                fitViewOnInit={true}
                fitViewDelay={100}
                disableSimulation={true}
                spaceSize={8192}
                curvedLinks={true}
                linkColor={(d) => d.color || null}
                linkWidth={3}
            />
            </CosmographProvider>
        </Box>
    );
}
