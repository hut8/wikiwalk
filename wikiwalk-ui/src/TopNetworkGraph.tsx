import { useEffect, useState, FC } from "react";
import { SigmaContainer, useLoadGraph, useSetSettings, useSigma } from "@react-sigma/core";
import { LayoutForceAtlas2Control } from "@react-sigma/layout-forceatlas2";
import EdgeCurveProgram from "@sigma/edge-curve";
import Graph from "graphology";
import { Box, useTheme } from "@mui/material";
import { Edge, Vertex, topGraph } from "./service";
import "@react-sigma/core/lib/style.css";

const randomColor = (top: boolean) =>
    `#${Math.floor(Math.random() * 16777215).toString(16)}${top ? "ff" : "80"}`;

const fetchTopGraph = async (
    setVertexes: React.Dispatch<React.SetStateAction<Vertex[]>>,
    setEdges: React.Dispatch<React.SetStateAction<Edge[]>>
) => {
    const graph = await topGraph();
    const findVertex = (id: string) => graph.vertexes.find((v) => v.id === id);
    for (const vertex of graph.vertexes) {
        vertex.color = randomColor(vertex.top);
        if (vertex.rank) {
            vertex.title = `${vertex.title} (Rank: ${vertex.rank})`;
        }
    }
    setVertexes(graph.vertexes);
    for (const edge of graph.edges) {
        const src = findVertex(edge.source);
        edge.color = src?.color || "grey";
    }
    setEdges(graph.edges);
};

interface GraphLoaderProps {
    vertexes: Vertex[];
    edges: Edge[];
}

const GraphLoader: FC<GraphLoaderProps> = ({ vertexes, edges }) => {
    const loadGraph = useLoadGraph();
    const sigma = useSigma();
    const setSettings = useSetSettings();
    const theme = useTheme();

    // Load graph data
    useEffect(() => {
        const graph = new Graph();

        // Add nodes without pre-calculated positions (let ForceAtlas2 calculate them)
        // Only set label for top nodes
        vertexes.forEach(vertex => {
            const showLabel = vertex.top;
            graph.addNode(vertex.id, {
                label: showLabel ? vertex.title : undefined,
                size: 5, // Slightly larger than 1px for better visibility
                color: vertex.color || '#666',
                rank: vertex.rank || 0,
                top: vertex.top
            });
        });

        // Add edges
        edges.forEach(edge => {
            if (!graph.hasNode(edge.source) || !graph.hasNode(edge.target)) {
                return; // Skip invalid edges
            }
            // Check if edge already exists to avoid duplicates
            if (graph.hasEdge(edge.source, edge.target)) {
                return;
            }
            graph.addEdge(edge.source, edge.target, {
                color: edge.color || '#999',
                size: 2,
                type: 'curved'
            });
        });

        loadGraph(graph);
    }, [loadGraph, vertexes, edges]);

    // Configure Sigma settings with theme-aware label color
    useEffect(() => {
        setSettings({
            renderEdgeLabels: false,
            labelColor: { color: theme.palette.mode === 'dark' ? '#ffffff' : '#000000' },
            labelSize: 14,
            labelWeight: 'normal',
            labelRenderedSizeThreshold: 0,
        });
    }, [setSettings, theme.palette.mode]);

    // Set initial camera position with delay (after layout converges)
    useEffect(() => {
        const timer = setTimeout(() => {
            const camera = sigma.getCamera();
            // Fit view to show all nodes
            camera.animatedReset({ duration: 500 });
            // Then zoom in (initialZoomLevel=2 means ratio=0.5)
            setTimeout(() => {
                camera.setState({ ratio: 0.5 });
            }, 600);
        }, 1000);

        return () => clearTimeout(timer);
    }, [sigma]);

    return (
        <LayoutForceAtlas2Control
            settings={{
                settings: {
                    gravity: 1,
                    scalingRatio: 10,
                    slowDown: 1,
                    barnesHutOptimize: true,
                    barnesHutTheta: 0.5,
                }
            }}
            autoRunFor={2000}
        />
    );
};

export function TopNetworkGraph() {
    const [vertexes, setVertexes] = useState<Vertex[]>([]);
    const [edges, setEdges] = useState<Edge[]>([]);
    const theme = useTheme();

    useEffect(() => {
        fetchTopGraph(setVertexes, setEdges);
    }, []);

    return (
        <>
            <Box sx={{ paddingTop: 1 }}>
                Graph of the connections found between the top 10 ranked pages on English Wikipedia
            </Box>
            <Box sx={{ height: '600px', width: '100%' }}>
                {vertexes.length > 0 && (
                    <SigmaContainer
                        style={{
                            height: '100%',
                            width: '100%',
                            backgroundColor: theme.palette.background.default
                        }}
                        settings={{
                            edgeProgramClasses: {
                                curved: EdgeCurveProgram,
                            },
                            defaultEdgeType: 'curved',
                        }}
                    >
                        <GraphLoader vertexes={vertexes} edges={edges} />
                    </SigmaContainer>
                )}
            </Box>
        </>
    );
}
