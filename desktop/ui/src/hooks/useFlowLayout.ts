import { useCallback, useEffect, useRef } from 'react'
import type { Node, Edge, OnNodesChange } from '@xyflow/react'
import { computeAutoLayout } from '../components/arch/layout'

type SetNodes = (updater: (nodes: Node[]) => Node[]) => void
type SetEdges = (updater: (edges: Edge[]) => Edge[]) => void
type LayoutFn = (nodes: Node[], edges: Edge[]) => Node[]

interface UseFlowLayoutOptions {
  nodes: Node[]
  edges: Edge[]
  setNodes: SetNodes
  setEdges: SetEdges
  onNodesChange: OnNodesChange
  onSavePositions: (pos: Record<string, [number, number]>) => void
  layoutFn?: LayoutFn
  debounceMs?: number
}

/**
 * Handles the generic React Flow layout concerns:
 * – edge handle recalculation after node drag
 * – debounced position persistence after drag
 * – auto-layout via dagre (or a custom layoutFn)
 *
 * All returned callbacks are stable references; callers can pass them
 * directly as ReactFlow event handlers without causing re-renders.
 */
export function useFlowLayout({
  nodes,
  edges,
  setNodes,
  setEdges,
  onNodesChange,
  onSavePositions,
  layoutFn,
  debounceMs = 600,
}: UseFlowLayoutOptions) {
  const nodesRef    = useRef<Node[]>([])
  const saveTimer   = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Stable refs so callbacks never become stale without redefining
  const edgesRef          = useRef(edges)
  const onSaveRef         = useRef(onSavePositions)
  const layoutRef         = useRef<LayoutFn>(layoutFn ?? computeAutoLayout)
  const onNodesChangeRef  = useRef(onNodesChange)

  useEffect(() => { nodesRef.current   = nodes },                        [nodes])
  useEffect(() => { edgesRef.current   = edges },                        [edges])
  useEffect(() => { onSaveRef.current  = onSavePositions },              [onSavePositions])
  useEffect(() => { layoutRef.current  = layoutFn ?? computeAutoLayout }, [layoutFn])
  useEffect(() => { onNodesChangeRef.current = onNodesChange },          [onNodesChange])

  // ── Reassign sourceHandle / targetHandle based on current x-positions ───────
  // Deferred via setTimeout so nodesRef is up-to-date after React commits.

  const recalcEdgeHandles = useCallback(() => {
    setTimeout(() => {
      const posMap = new Map(nodesRef.current.map((n) => [n.id, n.position]))

      setEdges((eds) => {
        type Side = 'left' | 'right'
        type Info = { idx: number; srcSide: Side; tgtSide: Side; srcRelY: number; tgtRelY: number }

        const infos: Info[] = eds.map((edge, idx) => {
          const src = posMap.get(edge.source)
          const tgt = posMap.get(edge.target)
          if (!src || !tgt)
            return { idx, srcSide: 'right' as Side, tgtSide: 'left' as Side, srcRelY: 0, tgtRelY: 0 }
          const goRight = src.x <= tgt.x
          return {
            idx,
            srcSide:  goRight ? 'right' : 'left',
            tgtSide:  goRight ? 'left'  : 'right',
            srcRelY:  tgt.y - src.y,
            tgtRelY:  src.y - tgt.y,
          } as Info
        })

        const srcGroups = new Map<string, Map<Side, Info[]>>()
        const tgtGroups = new Map<string, Map<Side, Info[]>>()

        for (const info of infos) {
          const edge = eds[info.idx]

          if (!srcGroups.has(edge.source)) srcGroups.set(edge.source, new Map())
          const sg = srcGroups.get(edge.source)!
          ;(sg.get(info.srcSide) ?? sg.set(info.srcSide, []).get(info.srcSide)!).push(info)

          if (!tgtGroups.has(edge.target)) tgtGroups.set(edge.target, new Map())
          const tg = tgtGroups.get(edge.target)!
          ;(tg.get(info.tgtSide) ?? tg.set(info.tgtSide, []).get(info.tgtSide)!).push(info)
        }

        const srcHandle = new Map<number, string>()
        const tgtHandle = new Map<number, string>()

        for (const sideMap of srcGroups.values()) {
          for (const [side, list] of sideMap) {
            list.sort((a, b) => a.srcRelY - b.srcRelY)
            list.forEach((info, slot) => srcHandle.set(info.idx, `${side}-${slot % 5}-s`))
          }
        }
        for (const sideMap of tgtGroups.values()) {
          for (const [side, list] of sideMap) {
            list.sort((a, b) => a.tgtRelY - b.tgtRelY)
            list.forEach((info, slot) => tgtHandle.set(info.idx, `${side}-${slot % 5}-t`))
          }
        }

        return eds.map((edge, i) => ({
          ...edge,
          sourceHandle: srcHandle.get(i) ?? edge.sourceHandle,
          targetHandle: tgtHandle.get(i) ?? edge.targetHandle,
        }))
      })
    }, 0)
  }, [setEdges])

  // ── Nodes change: pass-through + debounced position save on drag end ─────────

  const handleNodesChange = useCallback(
    (changes: Parameters<OnNodesChange>[0]) => {
      onNodesChangeRef.current(changes)
      const hasDrag = changes.some((c) => c.type === 'position' && c.dragging === false)
      if (!hasDrag) return
      recalcEdgeHandles()
      if (saveTimer.current) clearTimeout(saveTimer.current)
      saveTimer.current = setTimeout(() => {
        setNodes((cur) => {
          const pos: Record<string, [number, number]> = {}
          for (const n of cur) pos[n.id] = [n.position.x, n.position.y]
          onSaveRef.current(pos)
          return cur
        })
      }, debounceMs)
    },
    [recalcEdgeHandles, setNodes, debounceMs],
  )

  // ── Auto-layout: dagre (or custom fn) + snap + persist ───────────────────────

  const handleAutoLayout = useCallback(() => {
    setNodes((cur) => {
      const laid      = layoutRef.current(cur, edgesRef.current)
      const laidById  = new Map(laid.map((n) => [n.id, n]))
      const snap      = (v: number) => Math.round(v / 20) * 20
      const updated   = cur.map((n) => {
        const l = laidById.get(n.id)
        if (!l) return n
        return { ...n, position: { x: snap(l.position.x), y: snap(l.position.y) } }
      })
      const pos: Record<string, [number, number]> = {}
      for (const n of updated) pos[n.id] = [n.position.x, n.position.y]
      onSaveRef.current(pos)
      return updated
    })
    setTimeout(recalcEdgeHandles, 0)
  }, [setNodes, recalcEdgeHandles])

  return { recalcEdgeHandles, handleNodesChange, handleAutoLayout }
}
