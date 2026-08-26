import { memo, useState, useRef } from 'react'
import { EdgeLabelRenderer, BaseEdge, useReactFlow, type EdgeProps } from '@xyflow/react'

// ── Path builder ──────────────────────────────────────────────────────────────

const CORNER_R = 6
const SNAP = 20

function snapGrid(v: number) { return Math.round(v / SNAP) * SNAP }

// Build an orthogonal SVG path through pts with rounded corners
function buildPath(pts: [number, number][], r: number): string {
  if (pts.length < 2) return ''
  let d = `M ${pts[0][0]} ${pts[0][1]}`
  for (let i = 1; i < pts.length - 1; i++) {
    const [px, py] = pts[i - 1]
    const [ex, ey] = pts[i]
    const [nx, ny] = pts[i + 1]
    const dx1 = ex - px, dy1 = ey - py
    const dx2 = nx - ex, dy2 = ny - ey
    const len1 = Math.sqrt(dx1 * dx1 + dy1 * dy1)
    const len2 = Math.sqrt(dx2 * dx2 + dy2 * dy2)
    if (len1 < 0.5 || len2 < 0.5) continue
    const rad = Math.min(r, len1 / 2, len2 / 2)
    const bx = ex - (dx1 / len1) * rad, by = ey - (dy1 / len1) * rad
    const ax = ex + (dx2 / len2) * rad, ay = ey + (dy2 / len2) * rad
    d += ` L ${bx.toFixed(1)} ${by.toFixed(1)} Q ${ex.toFixed(1)} ${ey.toFixed(1)} ${ax.toFixed(1)} ${ay.toFixed(1)}`
  }
  const [lx, ly] = pts[pts.length - 1]
  return d + ` L ${lx.toFixed(1)} ${ly.toFixed(1)}`
}

// ── Route data ────────────────────────────────────────────────────────────────

export interface EdgeRoute {
  // 3-segment (L-shape): single vertical at cx
  cx?: number
  // 5-segment (S-shape): two verticals at cx1/cx2 with midY horizontal between them
  cx1?: number
  cx2?: number
  midY?: number
}

// ── Component ─────────────────────────────────────────────────────────────────

export const ArchEdge = memo(function ArchEdge({
  id,
  sourceX, sourceY,
  targetX, targetY,
  sourceHandleId,
  data, style, markerEnd,
}: EdgeProps) {
  const { setEdges, getViewport } = useReactFlow()
  const [hovered, setHovered] = useState(false)
  const draggingRef = useRef(false)

  // Always derive direction from actual positions — never from props.sourcePosition
  // which can be stale before recalcEdgeHandles fires, causing backward-loop paths.
  const goRight = sourceX <= targetX
  const route = (data?.route ?? {}) as EdgeRoute
  const srcSlot = parseInt(sourceHandleId?.split('-')[1] ?? '0') || 0

  // Default elbow x: slot-offset from midpoint, clamped away from target
  const rawMidX = (sourceX + targetX) / 2
  const defaultCx = goRight
    ? Math.min(rawMidX + srcSlot * 15, targetX - 20)
    : Math.max(rawMidX - srcSlot * 15, targetX + 20)

  const hasMidY = route.midY != null
  const cx = route.cx ?? defaultCx
  const cx1 = route.cx1 ?? defaultCx
  const cx2 = route.cx2 ?? defaultCx
  const midY = route.midY ?? (sourceY + targetY) / 2

  const pts: [number, number][] = hasMidY
    ? [[sourceX, sourceY], [cx1, sourceY], [cx1, midY], [cx2, midY], [cx2, targetY], [targetX, targetY]]
    : [[sourceX, sourceY], [cx, sourceY], [cx, targetY], [targetX, targetY]]

  const pathD = buildPath(pts, CORNER_R)

  // ── Generic drag starter ───────────────────────────────────────────────────

  const startDrag = (
    e: React.PointerEvent<HTMLElement>,
    initVal: number,
    axis: 'x' | 'y',
    applyDelta: (snappedNewVal: number) => EdgeRoute,
  ) => {
    e.stopPropagation()
    e.preventDefault()
    draggingRef.current = true
    setHovered(true)

    const { zoom } = getViewport()
    const startClient = axis === 'x' ? e.clientX : e.clientY

    const onMove = (me: PointerEvent) => {
      const clientPos = axis === 'x' ? me.clientX : me.clientY
      const newVal = snapGrid(initVal + (clientPos - startClient) / zoom)
      setEdges((eds) => eds.map((edge) => {
        if (edge.id !== id) return edge
        return { ...edge, data: { ...(edge.data ?? {}), route: applyDelta(newVal) } }
      }))
    }

    const onUp = () => {
      draggingRef.current = false
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }

  // Reset all manual routing (double-click any handle)
  const resetRoute = () => {
    setEdges((eds) => eds.map((e) =>
      e.id === id ? { ...e, data: { ...(e.data ?? {}), route: {} } } : e,
    ))
  }

  const stopHover = () => { if (!draggingRef.current) setHovered(false) }

  // ── Handles ────────────────────────────────────────────────────────────────

  interface Handle {
    x: number; y: number
    cursor: 'ew-resize' | 'ns-resize'
    title: string
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => void
  }

  const handles: Handle[] = []

  if (!hasMidY) {
    // ── 3-segment handles ──────────────────────────────────────────────────

    // Vertical segment only (L/R drag). The two horizontal segments that
    // exit/approach the nodes are intentionally not draggable.
    handles.push({
      x: cx, y: (sourceY + targetY) / 2,
      cursor: 'ew-resize', title: 'Drag to move vertical segment',
      onPointerDown: (e) => startDrag(e, cx, 'x', (v) => ({ ...route, cx: v })),
    })
  } else {
    // ── 5-segment handles ──────────────────────────────────────────────────

    // Left vertical (L/R drag)
    handles.push({
      x: cx1, y: (sourceY + midY) / 2,
      cursor: 'ew-resize', title: 'Drag to move vertical segment',
      onPointerDown: (e) => startDrag(e, cx1, 'x', (v) => ({ ...route, cx1: v })),
    })

    // Middle horizontal (U/D drag)
    handles.push({
      x: (cx1 + cx2) / 2, y: midY,
      cursor: 'ns-resize', title: 'Drag to move horizontal segment',
      onPointerDown: (e) => startDrag(e, midY, 'y', (v) => ({ ...route, midY: v })),
    })

    // Right vertical (L/R drag)
    handles.push({
      x: cx2, y: (midY + targetY) / 2,
      cursor: 'ew-resize', title: 'Drag to move vertical segment',
      onPointerDown: (e) => startDrag(e, cx2, 'x', (v) => ({ ...route, cx2: v })),
    })
  }

  return (
    <>
      <BaseEdge
        path={pathD}
        style={style}
        markerEnd={markerEnd}
        interactionWidth={0}
      />
      {/* Wide transparent stroke for hover detection */}
      <path
        d={pathD}
        fill="none"
        stroke="transparent"
        strokeWidth={16}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={stopHover}
        style={{ cursor: 'default' }}
      />
      <EdgeLabelRenderer>
        {handles.map((h, i) => (
          <div
            key={i}
            className="nodrag nopan absolute"
            style={{
              transform: `translate(-50%, -50%) translate(${h.x}px, ${h.y}px)`,
              cursor: h.cursor,
              width:  h.cursor === 'ew-resize' ? 4 : 16,
              height: h.cursor === 'ew-resize' ? 16 : 4,
              background: 'var(--color-primary, #6366f1)',
              borderRadius: 99,
              boxShadow: '0 1px 4px rgba(0,0,0,0.35)',
              opacity: hovered ? 1 : 0,
              pointerEvents: hovered ? 'all' : 'none',
              transition: 'opacity 0.12s',
              zIndex: 10,
            }}
            onMouseEnter={() => setHovered(true)}
            onMouseLeave={stopHover}
            onPointerDown={h.onPointerDown}
            onDoubleClick={resetRoute}
            title={h.title}
          />
        ))}
      </EdgeLabelRenderer>
    </>
  )
})
