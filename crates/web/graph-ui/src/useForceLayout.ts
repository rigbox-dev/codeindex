import { forceSimulation, forceLink, forceManyBody, forceX, forceY, forceCollide } from 'd3-force';
import type { Node, Edge } from '@xyflow/react';

interface SimNode {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

interface SimLink {
  source: string;
  target: string;
}

export function computeForceLayout(nodes: Node[], edges: Edge[]): Node[] {
  if (nodes.length === 0) return nodes;

  const simNodes: SimNode[] = nodes.map(n => ({
    id: n.id,
    x: Math.random() * 800,
    y: Math.random() * 600,
    width: 160,
    height: 40,
  }));

  const nodeIds = new Set(nodes.map(n => n.id));
  const simLinks: SimLink[] = edges
    .filter(e => nodeIds.has(e.source as string) && nodeIds.has(e.target as string))
    .map(e => ({ source: e.source as string, target: e.target as string }));

  const sim = forceSimulation(simNodes as any)
    .force('link', forceLink(simLinks as any).id((d: any) => d.id).distance(200).strength(0.15))
    .force('charge', forceManyBody().strength(-600))
    .force('x', forceX(0).strength(0.05))
    .force('y', forceY(0).strength(0.05))
    .force('collide', forceCollide().radius(80).strength(0.7))
    .stop();

  for (let i = 0; i < 300; i++) sim.tick();

  const posMap = new Map(simNodes.map(n => [n.id, { x: n.x || 0, y: n.y || 0 }]));

  return nodes.map(n => ({
    ...n,
    position: posMap.get(n.id) || n.position,
  }));
}
