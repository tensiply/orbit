import { memo } from 'react'
import { Handle, Position, type NodeProps, useConnection, useEdges, useNodeId } from '@xyflow/react'
import { cn } from '@/lib/utils'
import type { ArchEntityDto } from '../../types'

// ── Kind config ───────────────────────────────────────────────────────────────

const KIND_STYLE: Record<string, { header: string; border: string; dot: string }> = {
  Services:       { header: 'bg-blue-100/80    dark:bg-blue-900/60',       border: 'border-blue-400    dark:border-blue-600',     dot: 'bg-blue-500    dark:bg-blue-400'    },
  Databases:      { header: 'bg-emerald-100/80 dark:bg-emerald-900/60',    border: 'border-emerald-400 dark:border-emerald-600',   dot: 'bg-emerald-500 dark:bg-emerald-400' },
  Integrations:   { header: 'bg-purple-100/80  dark:bg-purple-900/60',     border: 'border-purple-400  dark:border-purple-600',    dot: 'bg-purple-500  dark:bg-purple-400'  },
  Infrastructure: { header: 'bg-orange-100/80  dark:bg-orange-900/60',     border: 'border-orange-400  dark:border-orange-600',    dot: 'bg-orange-500  dark:bg-orange-400'  },
  APIs:           { header: 'bg-cyan-100/80    dark:bg-cyan-900/60',       border: 'border-cyan-400    dark:border-cyan-600',     dot: 'bg-cyan-500    dark:bg-cyan-400'    },
  Pipelines:      { header: 'bg-amber-100/80   dark:bg-amber-900/60',      border: 'border-amber-400   dark:border-amber-600',    dot: 'bg-amber-500   dark:bg-amber-400'   },
  Secrets:        { header: 'bg-red-100/80     dark:bg-red-900/60',        border: 'border-red-400     dark:border-red-600',      dot: 'bg-red-500     dark:bg-red-400'     },
  IAM:            { header: 'bg-pink-100/80    dark:bg-pink-900/60',       border: 'border-pink-400    dark:border-pink-600',     dot: 'bg-pink-500    dark:bg-pink-400'    },
  Teams:          { header: 'bg-zinc-200/80    dark:bg-zinc-800/60',       border: 'border-zinc-400    dark:border-zinc-600',     dot: 'bg-zinc-500    dark:bg-zinc-400'    },
}

// Within Services, tags override the default blue: frontend=blue, backend=red, interfaces=purple
const SERVICE_TAG_STYLE: Record<string, { header: string; border: string; dot: string }> = {
  frontend:   { header: 'bg-blue-100/80   dark:bg-blue-900/60',   border: 'border-blue-400   dark:border-blue-600',   dot: 'bg-blue-500   dark:bg-blue-400'   },
  backend:    { header: 'bg-red-100/80    dark:bg-red-900/60',    border: 'border-red-400    dark:border-red-600',    dot: 'bg-red-500    dark:bg-red-400'    },
  interfaces: { header: 'bg-purple-100/80 dark:bg-purple-900/60', border: 'border-purple-400 dark:border-purple-600', dot: 'bg-purple-500 dark:bg-purple-400' },
}

function resolveStyle(data: ArchEntityDto) {
  if (data.kind === 'Services') {
    for (const tag of data.tags ?? []) {
      if (SERVICE_TAG_STYLE[tag]) return SERVICE_TAG_STYLE[tag]
    }
  }
  return KIND_STYLE[data.kind] ?? KIND_STYLE['Services']
}

const CRITICALITY_DOT: Record<string, string> = {
  critical: 'bg-red-500    dark:bg-red-400',
  high:     'bg-amber-500  dark:bg-amber-400',
  medium:   'bg-cyan-500   dark:bg-cyan-400',
  low:      'bg-zinc-400   dark:bg-zinc-500',
}

const LIFECYCLE_TEXT: Record<string, string> = {
  production:  'text-emerald-600 dark:text-emerald-400',
  development: 'text-sky-600     dark:text-sky-400',
  deprecated:  'text-zinc-500    line-through',
  planned:     'text-purple-600  dark:text-purple-400',
}

// 5 handles per side, evenly spaced at 1/6 … 5/6 of the node height
const HANDLE_SLOTS = [0, 1, 2, 3, 4] as const
const HANDLE_COUNT = HANDLE_SLOTS.length

// ── Component ─────────────────────────────────────────────────────────────────

export const EntityNode = memo(function EntityNode({ data: rawData, selected }: NodeProps) {
  const data    = rawData as unknown as ArchEntityDto
  const style   = resolveStyle(data)
  const critCls = data.criticality ? (CRITICALITY_DOT[data.criticality] ?? 'bg-zinc-400 dark:bg-zinc-500') : null
  const lfCls   = data.lifecycle   ? (LIFECYCLE_TEXT[data.lifecycle]   ?? 'text-muted-foreground') : 'text-muted-foreground'

  // Show handles while a connection drag is in progress OR if already attached
  const { inProgress: isConnecting } = useConnection()
  const nodeId       = useNodeId()
  const edges        = useEdges()

  // A slot is "connected" if any of its four handle IDs appears in the edge list.
  // IDs: left-N-s (source), left-N-t (target), right-N-s (source), right-N-t (target)
  const connectedSlots = new Set<string>() // e.g. "left-2", "right-0"
  for (const e of edges) {
    const add = (h: string | null | undefined) => {
      if (!h) return
      // Strip trailing -s / -t to get the slot key
      const slot = h.replace(/-(s|t)$/, '')
      connectedSlots.add(slot)
    }
    if (e.source === nodeId) add(e.sourceHandle)
    if (e.target === nodeId) add(e.targetHandle)
  }

  const slotStyle = (slotKey: string, i: number): React.CSSProperties => {
    const visible = isConnecting || connectedSlots.has(slotKey)
    return {
      top:           `${(i + 1) * 100 / (HANDLE_COUNT + 1)}%`,
      opacity:       visible ? 1 : 0,
      pointerEvents: visible ? 'all' : 'none',
      transition:    'opacity 0.12s',
    }
  }

  const handleCls = '!w-2 !h-2 !bg-foreground/25 !border-foreground/20 hover:!bg-primary hover:!border-primary'

  return (
    <div
      className={cn(
        'w-56 rounded-lg border bg-card shadow-lg transition-all select-none',
        selected ? 'ring-2 ring-foreground/25' : 'ring-0',
        style.border,
      )}
    >
      {/* Header */}
      <div className={cn(
        'flex items-center gap-1.5 px-2.5 py-1.5 rounded-t-lg border-b',
        style.header,
        style.border,
      )}>
        {critCls && <span className={cn('w-1.5 h-1.5 rounded-full shrink-0', critCls)} />}
        <span className="text-[10px] font-medium text-foreground/70 dark:text-white/70 uppercase tracking-wider truncate flex-1">
          {data.kind}
        </span>
        {data.criticality && (
          <span className="text-[9px] text-foreground/40 dark:text-white/40">{data.criticality}</span>
        )}
      </div>

      {/* Body */}
      <div className="px-2.5 py-2">
        <p className="text-xs font-semibold text-foreground leading-tight truncate" title={data.name}>
          {data.name}
        </p>
        <p className="text-[10px] text-muted-foreground font-mono truncate mt-0.5" title={data.id}>
          {data.id}
        </p>
      </div>

      {/* Footer */}
      {data.lifecycle && (
        <div className={cn('px-2.5 pb-1.5 text-[10px]', lfCls)}>
          {data.lifecycle}
        </div>
      )}

      {/* Each slot has 2 handles: one source-typed, one target-typed.
          ReactFlow looks up handles by type when rendering edge paths,
          so both types must exist at every physical position. */}
      {HANDLE_SLOTS.map((i) => (
        <span key={`left-${i}`}>
          <Handle id={`left-${i}-t`} type="target" position={Position.Left} style={slotStyle(`left-${i}`, i)} className={handleCls} />
          <Handle id={`left-${i}-s`} type="source" position={Position.Left} style={slotStyle(`left-${i}`, i)} className={handleCls} />
        </span>
      ))}
      {HANDLE_SLOTS.map((i) => (
        <span key={`right-${i}`}>
          <Handle id={`right-${i}-s`} type="source" position={Position.Right} style={slotStyle(`right-${i}`, i)} className={handleCls} />
          <Handle id={`right-${i}-t`} type="target" position={Position.Right} style={slotStyle(`right-${i}`, i)} className={handleCls} />
        </span>
      ))}
    </div>
  )
})
