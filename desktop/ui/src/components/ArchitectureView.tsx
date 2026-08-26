import '@xyflow/react/dist/style.css'
import { ARCH_KIND_COLORS } from '../theme'
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  useNodesState,
  useEdgesState,
  addEdge,
  reconnectEdge,
  type Node,
  type Edge,
  type Connection,
  type NodeTypes,
  type EdgeTypes,
} from '@xyflow/react'
import { RefreshCw, Plus, AlertTriangle, FolderOpen, Network, LayoutGrid, Pencil, Save, Loader2, Search, X } from 'lucide-react'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { cn } from '@/lib/utils'
import { tauriService } from '../services/tauri'
import type { ArchEntityDto, ArchLayout, ArchRoutes } from '../types'
import { useAppStore } from '../store'
import { EntityNode } from './arch/EntityNode'
import { ArchEdge, type EdgeRoute } from './arch/ArchEdge'
import { entitiesToFlow } from './arch/layout'
import { FlowCanvas } from './flow/FlowCanvas'
import { useFlowLayout } from '../hooks/useFlowLayout'

// ── Route helpers ─────────────────────────────────────────────────────────────

function edgesToRoutes(edges: Edge[]): ArchRoutes {
  const map: ArchRoutes = {}
  for (const e of edges) {
    const r = (e.data as Record<string, unknown> | undefined)?.route as EdgeRoute | undefined
    if (r && Object.keys(r).length > 0) map[e.id] = r as Record<string, number>
  }
  return map
}

// ── Broken-connection helper ──────────────────────────────────────────────────

interface BrokenEdge { source: string; target: string }

function findBrokenEdges(entities: ArchEntityDto[]): BrokenEdge[] {
  const ids = new Set(entities.map((e) => e.id))
  const broken: BrokenEdge[] = []
  for (const entity of entities) {
    for (const conn of entity.connections) {
      if (!ids.has(conn)) broken.push({ source: entity.id, target: conn })
    }
  }
  return broken
}

// ── Relative time ─────────────────────────────────────────────────────────────

function relativeTime(date: Date): string {
  const secs = Math.floor((Date.now() - date.getTime()) / 1000)
  if (secs < 5)    return 'just now'
  if (secs < 60)   return `${secs}s ago`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`
  return `${Math.floor(secs / 3600)}h ago`
}

// ── Node / edge types (defined outside component to avoid re-renders) ─────────

const NODE_TYPES: NodeTypes = { entity: EntityNode as never }
const EDGE_TYPES: EdgeTypes = { arch: ArchEdge as never }
const DEFAULT_EDGE_OPTIONS = { type: 'arch' }

// ── Kind filter config ────────────────────────────────────────────────────────

const KIND_ORDER = [
  'Frontend', 'Backend', 'Interfaces',
  'Databases', 'Integrations', 'Infrastructure',
  'APIs', 'Pipelines', 'Secrets', 'IAM', 'Teams',
]

const KIND_CHIP: Record<string, string> = {
  Frontend:       'bg-blue-100    text-blue-700    hover:bg-blue-200    dark:bg-blue-900/50    dark:text-blue-300    dark:hover:bg-blue-800/60',
  Backend:        'bg-red-100     text-red-700     hover:bg-red-200     dark:bg-red-900/50     dark:text-red-300     dark:hover:bg-red-800/60',
  Interfaces:     'bg-purple-100  text-purple-700  hover:bg-purple-200  dark:bg-purple-900/50  dark:text-purple-300  dark:hover:bg-purple-800/60',
  Databases:      'bg-emerald-100 text-emerald-700 hover:bg-emerald-200 dark:bg-emerald-900/50 dark:text-emerald-300 dark:hover:bg-emerald-800/60',
  Integrations:   'bg-purple-100  text-purple-700  hover:bg-purple-200  dark:bg-purple-900/50  dark:text-purple-300  dark:hover:bg-purple-800/60',
  Infrastructure: 'bg-orange-100  text-orange-700  hover:bg-orange-200  dark:bg-orange-900/50  dark:text-orange-300  dark:hover:bg-orange-800/60',
  APIs:           'bg-cyan-100    text-cyan-700    hover:bg-cyan-200    dark:bg-cyan-900/50    dark:text-cyan-300    dark:hover:bg-cyan-800/60',
  Pipelines:      'bg-amber-100   text-amber-700   hover:bg-amber-200   dark:bg-amber-900/50   dark:text-amber-300   dark:hover:bg-amber-800/60',
  Secrets:        'bg-red-100     text-red-700     hover:bg-red-200     dark:bg-red-900/50     dark:text-red-300     dark:hover:bg-red-800/60',
  IAM:            'bg-pink-100    text-pink-700    hover:bg-pink-200    dark:bg-pink-900/50    dark:text-pink-300    dark:hover:bg-pink-800/60',
  Teams:          'bg-zinc-200    text-zinc-600    hover:bg-zinc-300    dark:bg-zinc-800/50    dark:text-zinc-300    dark:hover:bg-zinc-700/60',
}

const ENV_ORDER = ['production', 'staging', 'development', 'deprecated']
const ENV_LABEL: Record<string, string> = {
  production: 'prod', staging: 'staging', development: 'dev', deprecated: 'depr',
}
const ENV_ACTIVE: Record<string, string> = {
  production:  'bg-rose-100  text-rose-700  dark:bg-rose-900/60  dark:text-rose-300',
  staging:     'bg-amber-100 text-amber-700 dark:bg-amber-900/60 dark:text-amber-300',
  development: 'bg-blue-100  text-blue-700  dark:bg-blue-900/60  dark:text-blue-300',
  deprecated:  'bg-zinc-200  text-zinc-600  dark:bg-zinc-800     dark:text-zinc-400',
}

// ── Environment presence helper ───────────────────────────────────────────────

function isPresentInEnv(d: ArchEntityDto, env: string): boolean {
  if (d.environments.length > 0) return d.environments.includes(env)
  return (d.lifecycle ?? '').toLowerCase() === env
}

function effectiveKind(d: ArchEntityDto): string {
  if (d.kind === 'Services') {
    if (d.tags?.includes('frontend'))   return 'Frontend'
    if (d.tags?.includes('backend'))    return 'Backend'
    if (d.tags?.includes('interfaces')) return 'Interfaces'
  }
  return d.kind
}

// ── Main component ────────────────────────────────────────────────────────────

interface Props {
  workspace: string
  tenant: string
}

export function ArchitectureView({ workspace, tenant }: Props) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([])
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([])

  const [allEntities,  setAllEntities]  = useState<ArchEntityDto[]>([])
  const [catalogPath,  setCatalogPath]  = useState('')
  const [loading,      setLoading]      = useState(true)
  const [loadError,    setLoadError]    = useState<string | null>(null)
  const [parseErrors,  setParseErrors]  = useState<[string, string][]>([])
  const [selectedKind, setSelectedKind] = useState<string | null>(null)
  const [selectedEnv,  setSelectedEnv]  = useState<string | null>(null)
  const [ctxMenu,       setCtxMenu]       = useState<{ x: number; y: number; entity: ArchEntityDto } | null>(null)
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null)
  const [refreshing,   setRefreshing]   = useState(false)
  const [saving,       setSaving]       = useState(false)
  const [brokenEdges,  setBrokenEdges]  = useState<BrokenEdge[]>([])
  const [lastSaved,    setLastSaved]    = useState<Date | null>(null)
  const [,             setTick]         = useState(0)
  const [searchQuery,  setSearchQuery]  = useState('')
  const searchRef = useRef<HTMLInputElement>(null)

  const theme                    = useAppStore((s) => s.theme)
  const openArchDrawer           = useAppStore((s) => s.openArchDrawer)
  const openArchDetail           = useAppStore((s) => s.openArchDetail)
  const closeArchDrawer          = useAppStore((s) => s.closeArchDrawer)
  const setArchDrawerAllEntities = useAppStore((s) => s.setArchDrawerAllEntities)

  // Stable ref for catalog path so position-save callbacks don't go stale
  const catalogPathRef  = useRef(catalogPath)
  const savedPositions  = useRef<ArchLayout>({})

  useEffect(() => { catalogPathRef.current = catalogPath }, [catalogPath])

  // Ctrl+F → focus search input
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'f') {
        e.preventDefault()
        searchRef.current?.focus()
        searchRef.current?.select()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  // Refresh "X ago" label every 30s while there's a saved timestamp
  useEffect(() => {
    if (!lastSaved) return
    const id = setInterval(() => setTick((t) => t + 1), 30_000)
    return () => clearInterval(id)
  }, [lastSaved])

  // ── Layout hook ───────────────────────────────────────────────────────────

  const onSavePositions = useCallback((pos: Record<string, [number, number]>) => {
    savedPositions.current = pos as ArchLayout
    void tauriService.architectureSaveLayout(catalogPathRef.current, pos as ArchLayout)
      .then(() => setLastSaved(new Date()))
  }, [])

  const { recalcEdgeHandles, handleNodesChange, handleAutoLayout } = useFlowLayout({
    nodes, edges, setNodes, setEdges, onNodesChange, onSavePositions,
  })

  // ── Load catalog (full reset) ─────────────────────────────────────────────

  const load = useCallback(async () => {
    setLoading(true); setLoadError(null); setSelectedKind(null); setSelectedEnv(null); setSearchQuery('')
    try {
      const catalog = await tauriService.architectureLoad(workspace, tenant)
      const [layout, savedRoutes] = await Promise.all([
        tauriService.architectureLoadLayout(catalog.catalog_path).catch(() => ({}) as ArchLayout),
        tauriService.architectureLoadRoutes(catalog.catalog_path).catch(() => ({}) as ArchRoutes),
      ])

      setCatalogPath(catalog.catalog_path)
      setAllEntities(catalog.entities)
      setArchDrawerAllEntities(catalog.entities)
      setParseErrors(catalog.errors)
      setBrokenEdges(findBrokenEdges(catalog.entities))
      savedPositions.current = layout

      const { nodes: n, edges: e } = entitiesToFlow(catalog.entities, layout)
      const edgesWithRoutes = e.map((edge) => {
        const r = savedRoutes[edge.id]
        return r ? { ...edge, data: { ...(edge.data ?? {}), route: r } } : edge
      })
      setNodes(n)
      setEdges(edgesWithRoutes)
      setTimeout(recalcEdgeHandles, 50)
    } catch (e) {
      setLoadError(String(e))
    } finally {
      setLoading(false)
    }
  }, [workspace, tenant, recalcEdgeHandles])

  useEffect(() => { void load() }, [load])

  // Close arch drawer when leaving this scope
  useEffect(() => {
    return () => { closeArchDrawer() }
  }, [workspace, tenant])

  // ── Listen to arch-save / arch-delete events from ArchEditDrawer ──────────

  useEffect(() => {
    const onSave = (e: Event) => {
      const { entity, isNew } = (e as CustomEvent<{ entity: ArchEntityDto; isNew: boolean }>).detail
      if (isNew) {
        const newNode: Node = {
          id: entity.id,
          type: 'entity',
          position: { x: Math.random() * 400, y: Math.random() * 300 },
          data: entity as unknown as Record<string, unknown>,
        }
        setNodes((n) => [...n, newNode])
        setAllEntities((all) => {
          const updated = [...all, entity]
          setArchDrawerAllEntities(updated)
          return updated
        })
      } else {
        setNodes((prev) =>
          prev.map((n) =>
            n.id === entity.id ? { ...n, data: entity as unknown as Record<string, unknown> } : n,
          ),
        )
        setAllEntities((prev) => {
          const updated = prev.map((e) => (e.id === entity.id ? entity : e))
          setArchDrawerAllEntities(updated)
          return updated
        })
      }
    }

    const onDelete = (e: Event) => {
      const { entity } = (e as CustomEvent<{ entity: ArchEntityDto }>).detail
      setNodes((n) => n.filter((node) => node.id !== entity.id))
      setEdges((edges) => edges.filter((edge) => edge.source !== entity.id && edge.target !== entity.id))
      setAllEntities((all) => {
        const updated = all.filter((e) => e.id !== entity.id)
        setArchDrawerAllEntities(updated)
        return updated
      })
    }

    document.addEventListener('orbit:arch-save', onSave)
    document.addEventListener('orbit:arch-delete', onDelete)
    return () => {
      document.removeEventListener('orbit:arch-save', onSave)
      document.removeEventListener('orbit:arch-delete', onDelete)
    }
  }, [setArchDrawerAllEntities])

  // Persist edge routes to disk whenever they change (debounced)
  const routeSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => {
    if (!catalogPath) return
    if (routeSaveTimer.current) clearTimeout(routeSaveTimer.current)
    routeSaveTimer.current = setTimeout(() => {
      void tauriService.architectureSaveRoutes(catalogPath, edgesToRoutes(edges))
        .then(() => setLastSaved(new Date()))
    }, 400)
  }, [edges, catalogPath])

  // ── Refresh: reload entities from disk, keep current node positions ──────

  const handleRefresh = useCallback(async () => {
    setRefreshing(true)
    try {
      const catalog = await tauriService.architectureLoad(workspace, tenant)
      const savedRoutes = await tauriService.architectureLoadRoutes(catalog.catalog_path).catch(() => ({}) as ArchRoutes)
      setCatalogPath(catalog.catalog_path)
      setAllEntities(catalog.entities)
      setArchDrawerAllEntities(catalog.entities)
      setParseErrors(catalog.errors)
      setBrokenEdges(findBrokenEdges(catalog.entities))

      setNodes((curNodes) => {
        const pos: ArchLayout = {}
        for (const n of curNodes) pos[n.id] = [n.position.x, n.position.y]
        const { nodes: newNodes, edges: newEdges } = entitiesToFlow(catalog.entities, pos)
        const edgesWithRoutes = newEdges.map((edge) => {
          const r = savedRoutes[edge.id]
          return r ? { ...edge, data: { ...(edge.data ?? {}), route: r } } : edge
        })
        setEdges(edgesWithRoutes)
        return newNodes
      })

      setTimeout(recalcEdgeHandles, 50)
    } catch (e) {
      setLoadError(String(e))
    } finally {
      setRefreshing(false)
    }
  }, [workspace, tenant, recalcEdgeHandles])

  // ── Save: persist current layout immediately ──────────────────────────────

  const handleSave = useCallback(() => {
    setSaving(true)
    setNodes((cur) => {
      const pos: ArchLayout = {}
      for (const n of cur) pos[n.id] = [n.position.x, n.position.y]
      savedPositions.current = pos
      tauriService.architectureSaveLayout(catalogPathRef.current, pos)
        .then(() => setLastSaved(new Date()))
        .finally(() => setSaving(false))
      return cur
    })
  }, [])

  // ── Edge: connect ─────────────────────────────────────────────────────────

  const handleConnect = useCallback(
    (conn: Connection) => {
      if (!conn.source || !conn.target) return
      setEdges((eds) =>
        addEdge({ ...conn, animated: false, type: 'arch', style: { strokeWidth: 1.5 } }, eds),
      )
      setAllEntities((prev) => {
        const source = prev.find((e) => e.id === conn.source)
        if (!source || source.connections.includes(conn.target!)) return prev
        const updated = { ...source, connections: [...source.connections, conn.target!] }
        void tauriService.architectureSaveEntity({
          workspace, tenant,
          kind_folder: source.kind_folder,
          id: source.id, name: source.name,
          description: source.description, criticality: source.criticality,
          lifecycle: source.lifecycle, owner: source.owner, team: source.team,
          tags: source.tags, connections: updated.connections,
          notes: source.notes, last_updated: source.last_updated,
        })
        return prev.map((e) => (e.id === updated.id ? updated : e))
      })
    },
    [workspace, tenant],
  )

  // ── Edge: delete ──────────────────────────────────────────────────────────

  const handleEdgesDelete = useCallback(
    (deleted: Edge[]) => {
      setAllEntities((prev) => {
        let next = prev
        for (const edge of deleted) {
          const source = next.find((e) => e.id === edge.source)
          if (!source) continue
          const updated = { ...source, connections: source.connections.filter((c) => c !== edge.target) }
          next = next.map((e) => (e.id === updated.id ? updated : e))
          void tauriService.architectureSaveEntity({
            workspace, tenant,
            kind_folder: source.kind_folder,
            id: source.id, name: source.name,
            description: source.description, criticality: source.criticality,
            lifecycle: source.lifecycle, owner: source.owner, team: source.team,
            tags: source.tags, connections: updated.connections,
            notes: source.notes, last_updated: source.last_updated,
          })
        }
        return next
      })
    },
    [workspace, tenant],
  )

  // ── Edge: reconnect ───────────────────────────────────────────────────────

  const reconnectOk = useRef(false)

  const handleReconnectStart = useCallback(() => {
    reconnectOk.current = false
  }, [])

  const handleReconnect = useCallback(
    (oldEdge: Edge, newConn: Connection) => {
      reconnectOk.current = true
      setEdges((eds) => reconnectEdge(oldEdge, newConn, eds))
      if (!newConn.source || !newConn.target) return

      setAllEntities((prev) => {
        let next = prev

        if (oldEdge.source !== newConn.source) {
          const oldSrc = next.find((e) => e.id === oldEdge.source)
          if (oldSrc) {
            const updated = { ...oldSrc, connections: oldSrc.connections.filter((c) => c !== oldEdge.target) }
            next = next.map((e) => (e.id === updated.id ? updated : e))
            void tauriService.architectureSaveEntity({
              workspace, tenant,
              kind_folder: oldSrc.kind_folder,
              id: oldSrc.id, name: oldSrc.name,
              description: oldSrc.description, criticality: oldSrc.criticality,
              lifecycle: oldSrc.lifecycle, owner: oldSrc.owner, team: oldSrc.team,
              tags: oldSrc.tags, connections: updated.connections,
              notes: oldSrc.notes, last_updated: oldSrc.last_updated,
            })
          }
        }

        const newSrc = next.find((e) => e.id === newConn.source)
        if (newSrc) {
          const connections = newSrc.connections
            .filter((c) => !(oldEdge.source === newConn.source && c === oldEdge.target))
          if (!connections.includes(newConn.target!)) connections.push(newConn.target!)
          const updated = { ...newSrc, connections }
          next = next.map((e) => (e.id === updated.id ? updated : e))
          void tauriService.architectureSaveEntity({
            workspace, tenant,
            kind_folder: newSrc.kind_folder,
            id: newSrc.id, name: newSrc.name,
            description: newSrc.description, criticality: newSrc.criticality,
            lifecycle: newSrc.lifecycle, owner: newSrc.owner, team: newSrc.team,
            tags: newSrc.tags, connections: updated.connections,
            notes: newSrc.notes, last_updated: newSrc.last_updated,
          })
        }

        return next
      })
    },
    [workspace, tenant],
  )

  const handleReconnectEnd = useCallback(
    (_: MouseEvent | TouchEvent, edge: Edge) => {
      if (!reconnectOk.current) {
        setEdges((eds) => eds.filter((e) => e.id !== edge.id))
        setAllEntities((prev) => {
          const source = prev.find((e) => e.id === edge.source)
          if (!source) return prev
          const updated = { ...source, connections: source.connections.filter((c) => c !== edge.target) }
          void tauriService.architectureSaveEntity({
            workspace, tenant,
            kind_folder: source.kind_folder,
            id: source.id, name: source.name,
            description: source.description, criticality: source.criticality,
            lifecycle: source.lifecycle, owner: source.owner, team: source.team,
            tags: source.tags, connections: updated.connections,
            notes: source.notes, last_updated: source.last_updated,
          })
          return prev.map((e) => (e.id === updated.id ? updated : e))
        })
      }
      reconnectOk.current = false
    },
    [workspace, tenant],
  )

  // ── Node: context menu / click ────────────────────────────────────────────

  const handleNodeContextMenu = useCallback((e: React.MouseEvent, node: Node) => {
    e.preventDefault()
    const entity = node.data as unknown as ArchEntityDto
    setCtxMenu({ x: e.clientX, y: e.clientY, entity })
  }, [])

  const closeCtxMenu = useCallback(() => setCtxMenu(null), [])

  const handleNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    const entity = node.data as unknown as ArchEntityDto
    setFocusedNodeId((prev) => {
      if (prev === node.id) {
        closeArchDrawer()
        return null
      }
      openArchDetail(entity, workspace, tenant, allEntities)
      return node.id
    })
  }, [workspace, tenant, allEntities, openArchDetail, closeArchDrawer])

  // ── Derived: visible nodes / edges with kind + env-lens + focus dimming ─────

  const nodeData = (n: Node) => n.data as unknown as ArchEntityDto

  const q = searchQuery.trim().toLowerCase()

  // Kind + search filter (env handled separately as a "lens", not a dim filter)
  const matchingNodeIds = new Set(
    nodes
      .filter((n) => {
        const d = nodeData(n)
        const kindMatch   = !selectedKind || effectiveKind(d) === selectedKind
        const searchMatch = !q ||
          d.name.toLowerCase().includes(q) ||
          (d.description ?? '').toLowerCase().includes(q) ||
          d.id.toLowerCase().includes(q)
        return kindMatch && searchMatch
      })
      .map((n) => n.id),
  )

  // Env lens: nodes absent from the selected environment
  const absentFromEnvIds: Set<string> = selectedEnv
    ? new Set(nodes.filter((n) => !isPresentInEnv(nodeData(n), selectedEnv)).map((n) => n.id))
    : new Set()

  const anyFilter = !!(selectedKind || q)

  const connectedNodeIds: Set<string> = focusedNodeId
    ? new Set([
        focusedNodeId,
        ...edges
          .filter((e) => e.source === focusedNodeId || e.target === focusedNodeId)
          .flatMap((e) => [e.source, e.target]),
      ])
    : new Set()

  const visibleNodes = nodes.map((n) => {
    const envAbsent    = absentFromEnvIds.has(n.id)
    const filterDimmed = anyFilter && !matchingNodeIds.has(n.id)
    const focusDimmed  = !!focusedNodeId && !connectedNodeIds.has(n.id)

    let opacity = 1
    if (envAbsent) opacity = 0.06
    else if (filterDimmed || focusDimmed) opacity = 0.15

    return {
      ...n,
      style: {
        ...n.style,
        opacity,
        transition: 'opacity 0.15s',
        ...(envAbsent ? { pointerEvents: 'none' as const } : {}),
      },
    }
  })

  const visibleEdges = edges.map((e) => {
    const sourceAbsent = absentFromEnvIds.has(e.source)
    const targetAbsent = absentFromEnvIds.has(e.target)
    const bothAbsent   = sourceAbsent && targetAbsent
    const isBroken     = !!(selectedEnv && (sourceAbsent || targetAbsent))

    const filterDimmed = anyFilter && (!matchingNodeIds.has(e.source) || !matchingNodeIds.has(e.target))
    const lit          = focusedNodeId && (e.source === focusedNodeId || e.target === focusedNodeId)
    const focusDimmed  = focusedNodeId && !lit

    if (bothAbsent) {
      return { ...e, style: { ...e.style, opacity: 0, transition: 'opacity 0.15s' } }
    }

    if (isBroken) {
      return {
        ...e,
        style: {
          ...e.style,
          strokeWidth: 1.5,
          stroke: 'hsl(38 92% 50%)',
          opacity: 0.55,
          strokeDasharray: '5 4',
          transition: 'opacity 0.15s, stroke 0.15s',
        },
      }
    }

    const dimmed = filterDimmed || focusDimmed
    return {
      ...e,
      style: {
        ...e.style,
        strokeWidth: lit ? 2.5 : 1.5,
        stroke:      lit ? 'hsl(217 91% 60%)' : undefined,
        opacity:     dimmed ? 0.15 : 1,
        transition:  'opacity 0.15s, stroke 0.15s, stroke-width 0.15s',
      },
    }
  })

  // ── Derived: counts for filter chips ─────────────────────────────────────

  const kindCounts = allEntities.reduce<Record<string, number>>((acc, e) => {
    const k = effectiveKind(e); acc[k] = (acc[k] ?? 0) + 1; return acc
  }, {})

  // Collect all unique env keys across entities; env counts = entities present in that env
  const envValues = [...new Set(
    allEntities.flatMap((e) =>
      e.environments.length > 0
        ? e.environments
        : e.lifecycle ? [e.lifecycle.toLowerCase()] : [],
    )
  )].sort((a, b) => {
    const ai = ENV_ORDER.indexOf(a), bi = ENV_ORDER.indexOf(b)
    if (ai !== -1 && bi !== -1) return ai - bi
    if (ai !== -1) return -1
    if (bi !== -1) return 1
    return a.localeCompare(b)
  })

  const envCounts = allEntities.reduce<Record<string, number>>((acc, e) => {
    const envs = e.environments.length > 0
      ? e.environments
      : e.lifecycle ? [e.lifecycle.toLowerCase()] : []
    for (const env of envs) acc[env] = (acc[env] ?? 0) + 1
    return acc
  }, {})

  // ── Slots: toolbar + filterBar + empty ───────────────────────────────────

  const toolbar = (
    <>
      <span className="text-foreground/25 shrink-0">
        <Network size={16} />
      </span>

      <div className="flex items-center gap-0.5 flex-1 min-w-0 overflow-hidden">
        {[workspace, tenant].map((part, i) => (
          <span key={i} className="flex items-center gap-0.5 shrink-0">
            {i > 0 && <span className="text-[10px] text-foreground/18 mx-0.5">›</span>}
            <span className={`text-[10px] font-medium ${i === 1 ? 'text-foreground/55' : 'text-foreground/28'}`}>
              {part}
            </span>
          </span>
        ))}
      </div>

      <div className="flex items-center gap-1 shrink-0">
        {lastSaved && (
          <span className="text-[10px] text-foreground/25 tabular-nums select-none">
            {relativeTime(lastSaved)}
          </span>
        )}
        {brokenEdges.length > 0 && (
          <span
            title={`${brokenEdges.length} broken connection${brokenEdges.length > 1 ? 's' : ''}: ${brokenEdges.map((b) => `${b.source} → ${b.target}`).join(', ')}`}
            className="flex items-center gap-1 h-6 px-2 rounded-md border border-amber-400/50 bg-amber-50/80 dark:bg-amber-900/30 text-[10px] text-amber-600 dark:text-amber-400 cursor-default"
          >
            <AlertTriangle size={10} />{brokenEdges.length}
          </span>
        )}
        <button
          onClick={() => openArchDrawer('new', workspace, tenant, allEntities)}
          title="Add entity"
          className="flex items-center gap-1 h-6 px-2 rounded-md border border-sidebar-border/40 text-[10px] text-foreground/50 hover:text-foreground/80 hover:bg-sidebar-accent/40 hover:border-sidebar-border/70 transition-colors"
        >
          <Plus size={10} />Add
        </button>
        {allEntities.length > 0 && (
          <button
            onClick={handleAutoLayout}
            title="Re-layout all nodes"
            className="flex items-center justify-center h-6 w-6 rounded-md border border-sidebar-border/40 text-foreground/40 hover:text-foreground/80 hover:bg-sidebar-accent/40 hover:border-sidebar-border/70 transition-colors"
          >
            <LayoutGrid size={10} />
          </button>
        )}
        <button
          onClick={handleSave}
          title="Save current layout"
          disabled={saving}
          className="flex items-center justify-center h-6 w-6 rounded-md border border-sidebar-border/40 text-foreground/40 hover:text-foreground/80 hover:bg-sidebar-accent/40 hover:border-sidebar-border/70 transition-colors disabled:opacity-30"
        >
          {saving
            ? <Loader2 size={10} className="animate-spin" />
            : <Save size={10} />
          }
        </button>
        <button
          onClick={() => void handleRefresh()}
          title="Reload entities and recalculate edges"
          disabled={refreshing}
          className="flex items-center justify-center h-6 w-6 rounded-md border border-sidebar-border/40 text-foreground/40 hover:text-foreground/80 hover:bg-sidebar-accent/40 hover:border-sidebar-border/70 transition-colors disabled:opacity-30"
        >
          <RefreshCw size={10} className={refreshing ? 'animate-spin' : ''} />
        </button>
      </div>
    </>
  )

  const filterBar = allEntities.length > 0 ? (
    <>
      {/* Kind filter — left */}
      <div className="shrink-0 rounded border border-sidebar-border/35 overflow-hidden">
        <ToggleGroup
          type="single"
          value={selectedKind ?? ''}
          onValueChange={(v) => setSelectedKind(v || null)}
          className="gap-0"
        >
          <ToggleGroupItem
            value=""
            className={cn(
              'text-[10px] h-[22px] px-2.5 rounded-none border-r border-sidebar-border/35 gap-1',
              !selectedKind
                ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                : 'bg-transparent text-foreground/30 hover:text-foreground/60 hover:bg-sidebar-accent/20',
            )}
          >
            All <span className="opacity-50">{allEntities.length}</span>
          </ToggleGroupItem>
          {KIND_ORDER.filter((k) => (kindCounts[k] ?? 0) > 0).map((k) => (
            <ToggleGroupItem
              key={k}
              value={k}
              className={cn(
                'text-[10px] h-[22px] px-2.5 rounded-none border-r last:border-r-0 border-sidebar-border/35 gap-1',
                selectedKind === k
                  ? KIND_CHIP[k]
                  : 'bg-transparent text-foreground/30 hover:text-foreground/60 hover:bg-sidebar-accent/20',
              )}
            >
              {k} <span className="opacity-50">{kindCounts[k]}</span>
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>

      <div className="flex-1" />

      {/* Search + env filter — right */}
      <div className="flex items-center gap-2 shrink-0">
        <div className="relative flex items-center">
          <Search size={10} className="absolute left-2 text-foreground/30 pointer-events-none" />
          <input
            ref={searchRef}
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Escape' && setSearchQuery('')}
            placeholder="search..."
            className="h-[22px] pl-5.5 pr-5 rounded border border-sidebar-border/35 bg-transparent text-[10px] text-foreground/70 placeholder:text-foreground/25 focus:outline-none focus:border-sidebar-border/70 w-28 focus:w-44 transition-[width] duration-150"
          />
          {searchQuery && (
            <button
              onClick={() => { setSearchQuery(''); searchRef.current?.focus() }}
              className="absolute right-1.5 text-foreground/30 hover:text-foreground/70 transition-colors"
            >
              <X size={9} />
            </button>
          )}
        </div>

        {envValues.length > 0 && (
          <>
            <div className="w-px h-3.5 bg-sidebar-border/40 shrink-0" />
            <div className="shrink-0 rounded border border-sidebar-border/35 overflow-hidden">
              <ToggleGroup
                type="single"
                value={selectedEnv ?? ''}
                onValueChange={(v) => setSelectedEnv(v || null)}
                className="gap-0"
              >
                <ToggleGroupItem
                  value=""
                  className={cn(
                    'text-[10px] h-[22px] px-2.5 rounded-none border-r border-sidebar-border/35',
                    !selectedEnv
                      ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                      : 'bg-transparent text-foreground/30 hover:text-foreground/60 hover:bg-sidebar-accent/20',
                  )}
                >
                  all
                </ToggleGroupItem>
                {envValues.map((env) => (
                  <ToggleGroupItem
                    key={env}
                    value={env}
                    className={cn(
                      'text-[10px] h-[22px] px-2.5 rounded-none border-r last:border-r-0 border-sidebar-border/35 gap-1',
                      selectedEnv === env
                        ? (ENV_ACTIVE[env] ?? 'bg-sidebar-accent text-sidebar-accent-foreground')
                        : 'bg-transparent text-foreground/30 hover:text-foreground/60 hover:bg-sidebar-accent/20',
                    )}
                  >
                    {ENV_LABEL[env] ?? env} <span className="opacity-50">{envCounts[env] ?? 0}</span>
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            </div>
          </>
        )}
      </div>
    </>
  ) : undefined

  const emptyState = (
    <>
      <FolderOpen size={16} />
      <span>No catalog found</span>
      <span className="text-muted-foreground/60">{catalogPath}</span>
      <button
        onClick={() => openArchDrawer('new', workspace, tenant, allEntities)}
        className="mt-2 flex items-center gap-1 text-[10px] px-3 py-1.5 rounded bg-card text-foreground/80 hover:bg-accent transition-colors"
      >
        <Plus size={11} />Add first entity
      </button>
    </>
  )

  const miniMapNodeColor = (n: Node) => {
    const d = n.data as unknown as ArchEntityDto | undefined
    if (!d) return ARCH_KIND_COLORS._default
    if (d.kind === 'Services') {
      if (d.tags?.includes('frontend'))   return '#3b82f6'
      if (d.tags?.includes('backend'))    return '#ef4444'
      if (d.tags?.includes('interfaces')) return '#8b5cf6'
    }
    return ARCH_KIND_COLORS[d.kind] ?? ARCH_KIND_COLORS._default
  }

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <>
      <FlowCanvas
        nodes={visibleNodes}
        edges={visibleEdges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        defaultEdgeOptions={DEFAULT_EDGE_OPTIONS}
        onNodesChange={handleNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={handleConnect}
        onEdgesDelete={handleEdgesDelete}
        onReconnectStart={handleReconnectStart}
        onReconnect={handleReconnect}
        onReconnectEnd={handleReconnectEnd}
        onNodeClick={handleNodeClick}
        onNodeContextMenu={handleNodeContextMenu}
        onPaneClick={() => { setFocusedNodeId(null); closeCtxMenu(); closeArchDrawer() }}
        onMove={closeCtxMenu}
        toolbar={toolbar}
        filterBar={filterBar}
        loading={loading && nodes.length === 0}
        error={loadError}
        empty={emptyState}
        theme={theme}
        miniMapNodeColor={miniMapNodeColor}
      />

      {ctxMenu && (
        <div
          style={{ position: 'fixed', left: ctxMenu.x, top: ctxMenu.y, zIndex: 50 }}
          className="min-w-[140px] rounded-md border border-border bg-popover shadow-md py-1 text-xs"
          onMouseLeave={closeCtxMenu}
        >
          <button
            className="w-full flex items-center gap-2 px-3 py-1.5 text-left hover:bg-accent transition-colors"
            onClick={() => {
              openArchDrawer(ctxMenu.entity, workspace, tenant, allEntities)
              closeCtxMenu()
            }}
          >
            <Pencil size={12} className="text-muted-foreground" />
            Edit
          </button>
        </div>
      )}
    </>
  )
}
