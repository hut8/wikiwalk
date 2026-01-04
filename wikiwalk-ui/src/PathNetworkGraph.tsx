import { useEffect, useState, useRef, FC } from "react";
import { SigmaContainer, useLoadGraph, useSetSettings, useSigma } from "@react-sigma/core";
import EdgeCurveProgram from "@sigma/edge-curve";
import Graph from "graphology";
import { Box } from "@mui/material";
import { Edge, Vertex, PagePaths, pathsGraph } from "./service";
import "@react-sigma/core/lib/style.css";

const randomColor = () => {
    // Generate bright colors with good contrast against dark gray background
    const hue = Math.floor(Math.random() * 360);
    const saturation = 70 + Math.floor(Math.random() * 30); // 70-100%
    const lightness = 60 + Math.floor(Math.random() * 20); // 60-80% for brightness
    return `hsl(${hue}, ${saturation}%, ${lightness}%)`;
};

interface GraphLoaderProps {
    vertexes: Vertex[];
    edges: Edge[];
}

const GraphLoader: FC<GraphLoaderProps> = ({ vertexes, edges }) => {
    const loadGraph = useLoadGraph();
    const sigma = useSigma();
    const setSettings = useSetSettings();

    // Load graph data
    useEffect(() => {
        const graph = new Graph();

        // Add nodes with pre-calculated positions
        // Only set label for top 20 by rank
        vertexes.forEach(vertex => {
            const showLabel = vertex.rank && vertex.rank > 0 && vertex.rank <= 20;
            graph.addNode(vertex.id, {
                label: showLabel ? vertex.title : undefined,
                x: vertex.x || 0,
                y: vertex.y || 0,
                size: 15,
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
            graph.addEdge(edge.source, edge.target, {
                color: edge.color || '#999',
                size: 4,
                type: 'curved'
            });
        });

        loadGraph(graph);
    }, [loadGraph, vertexes, edges]);

    // Configure Sigma settings
    useEffect(() => {
        setSettings({
            renderEdgeLabels: false,
            labelColor: { color: '#ffffff' },
            labelSize: 14,
            labelWeight: 'normal',
            labelRenderedSizeThreshold: 0,
        });
    }, [setSettings]);

    // Set initial camera position
    useEffect(() => {
        const camera = sigma.getCamera();
        // Cosmograph initialZoomLevel of 0.8 corresponds to ratio of 1/0.8 = 1.25
        camera.setState({ ratio: 1.25 });
    }, [sigma]);

    return null;
};

export function PathNetworkGraph({ paths }: { paths: PagePaths }) {
    const [vertexes, setVertexes] = useState<Vertex[]>([]);
    const [edges, setEdges] = useState<Edge[]>([]);
    const [graphHeight, setGraphHeight] = useState<number>(600);
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
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

    useEffect(() => {
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
        <Box ref={containerRef} sx={{ height: `${graphHeight}px`, width: '100%', marginBottom: '20px' }}>
            {vertexes.length > 0 && (
                <SigmaContainer
                    style={{ height: '100%', width: '100%' }}
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
    );
}
