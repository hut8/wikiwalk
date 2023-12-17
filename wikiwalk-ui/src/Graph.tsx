// import { Cosmograph } from '@cosmograph/react'

// export function GraphVisualization ({ nodes, links }) {
//   return (<Cosmograph
//     nodes={nodes}
//     links={links}
//     nodeColor={d => d.color}
//     nodeSize={20}
//     linkWidth={2}
//   />)
// }

export type Vertex = {
  id: string
  label: string
  color: string
}

export type Edge = {
  source: string
  target: string
}

type GraphPayload = {
  vertexes: Vertex[]
  edges: Edge[]
}

import { CosmographProvider, Cosmograph } from '@cosmograph/react'
import React from 'react'

export function TopGraph() {
  const [vertexes, setVertexes] = React.useState<Vertex[]>([]);
  const [edges, setEdges] = React.useState<Edge[]>([]);

  const fetchGraph = async () => {
    const response = await fetch('/top-graph');
    const graph = await response.json() as GraphPayload;
    setVertexes(graph.vertexes);
    setEdges(graph.edges);
  };

  React.useEffect(() => {
    fetchGraph();
  }, []);

  return (
    <CosmographProvider nodes={vertexes} links={edges}>
      <Cosmograph<Vertex,Edge>
        nodeColor={d => d.color}
        nodeSize={20}
        linkWidth={2} />
    </CosmographProvider>
  )
}
