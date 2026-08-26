import dagre from '@dagrejs/dagre'
import type { Node, Edge } from '@xyflow/react'
import type { ArchEntityDto } from '../../types'

export const NODE_W = 224
export const NODE_H = 90

const H_GAP = 40
const V_GAP = 40
const SECTION_GAP = 80
const ISOLATED_COLS = 5

function applyDagreLayout(nodes: Node[], edges: Edge[]): Node[] {
  const g = new dagre.graphlib.Graph()
  g.setDefaultEdgeLabel(() => ({}))
  g.setGraph({ rankdir: 'LR', ranksep: 100, nodesep: 60 })

  for (const n of nodes) g.setNode(n.id, { width: NODE_W, height: NODE_H })
  for (const e of edges) g.setEdge(e.source, e.target)

  dagre.layout(g)

  return nodes.map((n) => {
    const pos = g.node(n.id)
    if (!pos) return n
    return { ...n, position: { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 } }
  })
}

/**
 * Full auto-layout: dagre (LR) for connected components, grid-by-kind for
 * isolated nodes placed below the connected section.
 */
export function computeAutoLayout(nodes: Node[], edges: Edge[]): Node[] {
  if (nodes.length === 0) return nodes

  const nodeIds = new Set(nodes.map((n) => n.id))
  const validEdges = edges.filter((e) => nodeIds.has(e.source) && nodeIds.has(e.target))

  const connectedIds = new Set<string>()
  for (const e of validEdges) { connectedIds.add(e.source); connectedIds.add(e.target) }

  const connectedNodes = nodes.filter((n) => connectedIds.has(n.id))
  const isolatedNodes  = nodes.filter((n) => !connectedIds.has(n.id))

  const result: Node[] = []
  let maxY = 0

  if (connectedNodes.length > 0) {
    const laid = applyDagreLayout(connectedNodes, validEdges)
    result.push(...laid)
    maxY = Math.max(...laid.map((n) => n.position.y + NODE_H))
  }

  if (isolatedNodes.length > 0) {
    const startY = connectedNodes.length > 0 ? maxY + SECTION_GAP : 0

    // Group isolated nodes by kind, preserving KIND_ORDER
    const KIND_ORDER = ['Services', 'Databases', 'Integrations', 'Infrastructure', 'APIs', 'Pipelines', 'Secrets', 'IAM', 'Teams']
    const byKind: Record<string, Node[]> = {}
    for (const n of isolatedNodes) {
      const kind = ((n.data as unknown as ArchEntityDto)?.kind) ?? 'Other'
      ;(byKind[kind] ??= []).push(n)
    }

    const orderedKinds = [
      ...KIND_ORDER.filter((k) => byKind[k]),
      ...Object.keys(byKind).filter((k) => !KIND_ORDER.includes(k)),
    ]

    let y = startY
    for (const kind of orderedKinds) {
      const kindNodes = byKind[kind]
      let col = 0
      let rowY = y
      for (const n of kindNodes) {
        if (col > 0 && col % ISOLATED_COLS === 0) {
          col = 0
          rowY += NODE_H + V_GAP
        }
        result.push({ ...n, position: { x: col * (NODE_W + H_GAP), y: rowY } })
        col++
      }
      y = rowY + NODE_H + V_GAP
    }
  }

  return result
}

export function entitiesToFlow(
  entities: ArchEntityDto[],
  savedPositions: Record<string, [number, number]>,
): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = entities.map((e) => {
    const saved = savedPositions[e.id]
    return {
      id: e.id,
      type: 'entity',
      position: saved ? { x: saved[0], y: saved[1] } : { x: 0, y: 0 },
      data: e as unknown as Record<string, unknown>,
    }
  })

  // Track how many edges already exit/enter each node to assign handle slots
  const sourceSlots: Record<string, number> = {}
  const targetSlots: Record<string, number> = {}

  const edges: Edge[] = []
  for (const e of entities) {
    for (const target of e.connections) {
      const si = sourceSlots[e.id]      ?? 0
      const ti = targetSlots[target]    ?? 0
      sourceSlots[e.id]   = si + 1
      targetSlots[target] = ti + 1

      edges.push({
        id:           `${e.id}->${target}`,
        source:       e.id,
        target,
        sourceHandle: `right-${si % 5}-s`,
        targetHandle: `left-${ti % 5}-t`,
        type:         'arch',
        animated:     false,
        style:        { strokeWidth: 1.5, borderRadius: 4 },
      })
    }
  }

  const needsLayout = nodes.some((n) => !savedPositions[n.id])
  if (needsLayout) {
    const laid = computeAutoLayout(nodes, edges)
    return {
      nodes: laid.map((n) => {
        const saved = savedPositions[n.id]
        return saved ? { ...n, position: { x: saved[0], y: saved[1] } } : n
      }),
      edges,
    }
  }

  return { nodes, edges }
}
