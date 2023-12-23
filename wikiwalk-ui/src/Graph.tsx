import { CosmographProvider, Cosmograph } from "@cosmograph/react";
import React from "react";
import { Edge, Vertex, topGraph } from "./service";

const randomColor = (top: boolean) =>
  `#${Math.floor(Math.random() * 16777215).toString(16)}${top ? "ff" : "80"}`;

const fetchGraph = async (
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

export function TopGraph() {
  const [vertexes, setVertexes] = React.useState<Vertex[]>([]);
  const [edges, setEdges] = React.useState<Edge[]>([]);

  React.useEffect(() => {
    fetchGraph(setVertexes, setEdges);
  }, []);

  return (
    <CosmographProvider nodes={vertexes} links={edges}>
      <Cosmograph<Vertex, Edge>
        // style={{ height: '100%', width: '100%' }}
        nodeColor={(d) => d.color || null}
        nodeLabelAccessor={(d) => d.title}
        nodeSize={2}
        nodeLabelColor={(d) => (d.top ? "white" : "grey")}
        hoveredNodeLabelColor={(d) => (d.top ? "white" : "grey")}
        showTopLabels={true}
        showTopLabelsValueKey="rank"
        showDynamicLabels={false}
        // Controls the friction coefficient, affecting how much nodes slow down over time.
        // Higher values result in slower movement and longer simulation time, lower values allow faster movement and quicker convergence.
        // Range: 0.8-1.0 Default: 0.85
        simulationFriction={0.9}
        // Controls the force simulation decay coefficient.
        // Higher values make the simulation "cool down" slower.
        // Increase for a longer-lasting simulation, decrease for a faster decay.
        // Range: 100-10000 Default: 1000
        simulationDecay={800}
        simulationRepulsion={1.9}
        simulationLinkSpring={1.5} // Default 1
        simulationLinkDistance={3} // Default 2
        // Centering:
        simulationGravity={0.1} // Default 0
        simulationCenter={0.4} // Default 0
        // Linking:
        curvedLinks={true}
        linkColor={(d) => d.color || null}
        linkWidth={2}
        initialZoomLevel={3}
      />
    </CosmographProvider>
  );
}
