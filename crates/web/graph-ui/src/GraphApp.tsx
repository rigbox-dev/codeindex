import React, { useEffect, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  BackgroundVariant,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  MarkerType,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { computeForceLayout } from './useForceLayout';
import './graph.css';

interface ApiNode {
  data: { id: string; label: string; kind?: string; file?: string; parent?: string };
}
interface ApiEdge {
  data: { source: string; target: string; kind: string };
}

const KIND_COLORS: Record<string, string> = {
  function: '#3b8bff',
  method: '#d29922',
  struct: '#3fb950',
  class: '#bc8cff',
  interface: '#f85149',
  enum: '#3fb950',
  impl_block: '#8b949e',
  module: '#8b949e',
};

export default function GraphApp() {
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch('/api/graph?limit=200')
      .then(r => r.json())
      .then((data: { nodes: ApiNode[]; edges: ApiEdge[] }) => {
        // Convert API format to React Flow format
        const rfNodes: Node[] = data.nodes.map(n => {
          const isFile = !n.data.kind;
          return {
            id: n.data.id,
            type: isFile ? 'group' : 'default',
            data: { label: n.data.label, kind: n.data.kind || 'file', file: n.data.file },
            position: { x: 0, y: 0 },
            parentId: n.data.parent || undefined,
            extent: n.data.parent ? 'parent' as const : undefined,
            style: isFile
              ? {
                  backgroundColor: 'var(--bg-surface, #161b22)',
                  border: '1px solid var(--border, #30363d)',
                  borderRadius: 10,
                  padding: 20,
                  minWidth: 180,
                  minHeight: 60,
                }
              : {
                  backgroundColor: KIND_COLORS[n.data.kind || ''] || '#8b949e',
                  color: '#fff',
                  borderRadius: 8,
                  padding: '6px 12px',
                  fontSize: 11,
                  fontFamily: 'var(--font-mono, monospace)',
                  fontWeight: 600,
                },
          };
        });

        const rfEdges: Edge[] = data.edges.map((e, i) => ({
          id: `e-${i}`,
          source: e.data.source,
          target: e.data.target,
          data: { kind: e.data.kind },
          style: {
            stroke:
              e.data.kind === 'calls'
                ? 'var(--accent, #58a6ff)'
                : e.data.kind === 'type_reference'
                  ? 'var(--purple, #bc8cff)'
                  : 'var(--text-muted, #8b949e)',
            strokeWidth: 1.5,
          },
          markerEnd: {
            type: MarkerType.ArrowClosed,
            width: 12,
            height: 12,
            color:
              e.data.kind === 'calls'
                ? 'var(--accent, #58a6ff)'
                : 'var(--text-muted, #8b949e)',
          },
          animated: e.data.kind === 'calls',
        }));

        // Apply d3-force layout (skip parent/group nodes)
        const layoutNodes = computeForceLayout(
          rfNodes.filter(n => n.type !== 'group'),
          rfEdges,
        );

        // Merge positions back
        const posMap = new Map(layoutNodes.map(n => [n.id, n.position]));
        const finalNodes = rfNodes.map(n => ({
          ...n,
          position: posMap.get(n.id) || n.position,
        }));

        setNodes(finalNodes);
        setEdges(rfEdges);
        setLoading(false);
      })
      .catch(err => {
        console.error('Failed to load graph:', err);
        setLoading(false);
      });
  }, []);

  if (loading) {
    return (
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          height: '100%',
          color: 'var(--text-muted, #8b949e)',
          background: 'var(--bg-body, #0d1117)',
        }}
      >
        Loading graph...
      </div>
    );
  }

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      fitView
      fitViewOptions={{ padding: 0.3 }}
      minZoom={0.1}
      maxZoom={3}
      proOptions={{ hideAttribution: true }}
    >
      <Background variant={BackgroundVariant.Dots} gap={20} size={1} />
      <Controls showInteractive={false} />
      <MiniMap
        style={{ borderRadius: 8 }}
        maskColor="rgba(0,0,0,0.15)"
      />
    </ReactFlow>
  );
}
