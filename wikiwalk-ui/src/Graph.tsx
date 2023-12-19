import { CosmographProvider, Cosmograph } from '@cosmograph/react'
import React from 'react'
import { Edge, Vertex, topGraph } from './service';

const randomColor = (top: boolean) => `#${Math.floor(Math.random() * 16777215).toString(16)}${top ? 'ff' : '80'}`;

const fetchGraph = async (
  setVertexes: React.Dispatch<React.SetStateAction<Vertex[]>>,
  setEdges: React.Dispatch<React.SetStateAction<Edge[]>>
) => {
  const graph = await topGraph();
  const findVertex = (id: string) => graph.vertexes.find(v => v.id === id);
  for (const vertex of graph.vertexes) {
    vertex.color = randomColor(vertex.top);
  }
  setVertexes(graph.vertexes);
  for (const edge of graph.edges) {
    const src = findVertex(edge.source);
    edge.color = src?.color || 'grey';
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
        style={{ height: '60vh', width: '80vw' }}
        nodeColor={d => d.color || null}
        nodeLabelAccessor={d => d.title}
        nodeSize={2}
        nodeLabelColor={d => d.top ? 'white' : 'grey'}
        hoveredNodeLabelColor={d => d.top ? 'white' : 'grey'}
        showTopLabelsValueKey='top'
        simulationDecay={2000}
        simulationRepulsion={.8}
        curvedLinks={true}
        linkColor={d => d.color || null}
        linkWidth={2} />
    </CosmographProvider>
  )
}
