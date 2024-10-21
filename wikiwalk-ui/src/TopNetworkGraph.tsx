import { CosmographProvider, Cosmograph } from "@cosmograph/react";
import React from "react";
import { Edge, Vertex, topGraph } from "./service";
import { Box } from "@mui/material";

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

export function TopNetworkGraph() {
    const [vertexes, setVertexes] = React.useState<Vertex[]>([]);
    const [edges, setEdges] = React.useState<Edge[]>([]);

    React.useEffect(() => {
        fetchTopGraph(setVertexes, setEdges);
    }, []);

    return (
        <CosmographProvider nodes={vertexes} links={edges}>
            <Box sx={{ paddingTop: 1 }}>
                Graph of the connections found between the top 10 ranked pages on English Wikipedia
            </Box>
            <Cosmograph<Vertex, Edge>
                // style={{ height: '100%', width: '100%' }}
                nodeColor={(d) => d.color || null}
                nodeLabelAccessor={(d) => d.title}
                nodeSize={1}
                nodeLabelColor={(d) => (d.top ? "white" : "grey")}
                hoveredNodeLabelColor={(d) => (d.top ? "white" : "grey")}
                showTopLabels={true}
                showTopLabelsLimit={10}
                showTopLabelsValueKey="rank"
                showDynamicLabels={false}
                fitViewDelay={1000}
                disableSimulation={false}

                // Controls the friction coefficient, affecting how much nodes slow down over time.
                // Higher values result in slower movement and longer simulation time, lower values allow faster movement and quicker convergence.
                // Range: 0.8-1.0 Default: 0.85
                // simulationFriction={0.9}

                // Controls the force simulation decay coefficient.
                // Higher values make the simulation "cool down" slower.
                // Increase for a longer-lasting simulation, decrease for a faster decay.
                // Range: 100-10000 Default: 1000
                // simulationDecay={10000}
                // simulationRepulsion={1.9}
                // Adjusts the link spring force coefficient, determining the strength of attraction between connected nodes.
                // Increase for stronger attraction, decrease for weaker attraction.
                // Range: 0.0-2.0 Default: 1.0
                //        simulationLinkSpring={1.5}
                // Defines the minimum distance between linked nodes, affecting their positioning.
                // Increase for more spacing, decrease for closer proximity.
                // Range: 1-20	Default: 2
                //      simulationLinkDistance={7}
                // Adjusts the gravity force coefficient, determining how much nodes are attracted towards the center of the graph.
                // Increase for stronger gravitational pull towards the center, decrease for weaker attraction.
                // Range: 0.0-1.0	Default: 0
                //    simulationGravity={0.00}
                // Changes the centering force coefficient, pulling nodes towards the center of the graph.
                // Increase for more centered nodes, decrease for less centralization.
                // Range: 0.0-1.0 Default: 0
                //  simulationCenter={0.00}
                // Sets the repulsion force coefficient from the mouse cursor. Activates the repulsion force when the right mouse button is pressed.
                // Increase for stronger repulsion from the cursor click, decrease for weaker repulsion.
                // Range: 0.0-5.0	Default: 2.0
                //simulationRepulsionFromMouse={0.05}

                spaceSize={8192}

                // Linking:
                curvedLinks={true}
                linkColor={(d) => d.color || null}
                linkWidth={2}
                initialZoomLevel={2}
            />
        </CosmographProvider>
    );
}
