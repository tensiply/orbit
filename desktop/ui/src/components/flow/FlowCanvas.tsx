import '@xyflow/react/dist/style.css'
import { AlertTriangle } from 'lucide-react'
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  type Node,
  type Edge,
  type Connection,
  type NodeTypes,
  type EdgeTypes,
  type OnNodesChange,
  type OnEdgesChange,
  type DefaultEdgeOptions,
  ConnectionLineType,
  ConnectionMode,
} from '@xyflow/react'

// ── Props ─────────────────────────────────────────────────────────────────────

export interface FlowCanvasProps {
  nodes: Node[]
  edges: Edge[]
  nodeTypes: NodeTypes
  edgeTypes?: EdgeTypes
  defaultEdgeOptions?: DefaultEdgeOptions

  onNodesChange: OnNodesChange
  onEdgesChange: OnEdgesChange
  onConnect?: (conn: Connection) => void
  onEdgesDelete?: (edges: Edge[]) => void
  onReconnectStart?: () => void
  onReconnect?: (oldEdge: Edge, newConn: Connection) => void
  onReconnectEnd?: (e: MouseEvent | TouchEvent, edge: Edge) => void
  onNodeClick?: (e: React.MouseEvent, node: Node) => void
  onNodeContextMenu?: (e: React.MouseEvent, node: Node) => void
  onPaneClick?: () => void
  onMove?: () => void

  /** Rendered inside the top header bar. Receives the full flex row — use
   *  fragments with their own flex/gap/grow classes. */
  toolbar?: React.ReactNode

  /** Rendered inside the filter strip below the header.
   *  Pass undefined to hide the strip entirely. */
  filterBar?: React.ReactNode

  /** Show a centered loading spinner. */
  loading?: boolean

  /** Show a centered error message. */
  error?: string | null

  /** Shown when nodes is empty and loading/error are both falsy. */
  empty?: React.ReactNode

  theme?: 'dark' | 'light' | 'system'

  /** Per-node color for the MiniMap. Omit to use the default gray. */
  miniMapNodeColor?: (node: Node) => string
}

// ── Component ─────────────────────────────────────────────────────────────────

export function FlowCanvas({
  nodes,
  edges,
  nodeTypes,
  edgeTypes,
  defaultEdgeOptions,
  onNodesChange,
  onEdgesChange,
  onConnect,
  onEdgesDelete,
  onReconnectStart,
  onReconnect,
  onReconnectEnd,
  onNodeClick,
  onNodeContextMenu,
  onPaneClick,
  onMove,
  toolbar,
  filterBar,
  loading,
  error,
  empty,
  theme = 'dark',
  miniMapNodeColor,
}: FlowCanvasProps) {
  const showEmpty = !loading && !error && nodes.length === 0 && empty != null

  return (
    <div className="flex flex-col h-full bg-card text-sidebar-foreground text-sm overflow-hidden">

      {/* Header bar — always visible; content injected via toolbar slot */}
      {toolbar != null && (
        <div className="flex items-center gap-3 px-4 h-8 shrink-0 border-b border-sidebar-border/40 bg-card">
          {toolbar}
        </div>
      )}

      {/* Filter strip — hidden when filterBar is undefined */}
      {filterBar != null && (
        <div className="flex items-center px-3 h-8 shrink-0 border-b border-sidebar-border/20 bg-card overflow-x-auto no-scrollbar">
          {filterBar}
        </div>
      )}

      {/* Body: loading / error / empty / canvas */}
      {loading ? (
        <div className="flex-1 flex items-center justify-center text-muted-foreground text-xs">
          Loading…
        </div>
      ) : error ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 text-xs">
          <AlertTriangle size={16} className="text-amber-500 dark:text-amber-400" />
          <span className="text-muted-foreground">{error}</span>
        </div>
      ) : showEmpty ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 text-xs text-muted-foreground">
          {empty}
        </div>
      ) : (
        <div className="flex-1 min-h-0 border-x-4 border-b-4 border-card rounded-b-2xl overflow-hidden">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onEdgesDelete={onEdgesDelete}
            onReconnectStart={onReconnectStart}
            onReconnect={onReconnect}
            onReconnectEnd={onReconnectEnd}
            onNodeClick={onNodeClick}
            onNodeContextMenu={onNodeContextMenu}
            onPaneClick={onPaneClick}
            onMove={onMove}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            defaultEdgeOptions={defaultEdgeOptions}
            connectionMode={ConnectionMode.Loose}
            connectionLineType={ConnectionLineType.SmoothStep}
            colorMode={theme}
            snapToGrid
            snapGrid={[20, 20]}
            fitView
            fitViewOptions={{ padding: 0.15 }}
            minZoom={0.2}
            maxZoom={2}
            deleteKeyCode="Delete"
            proOptions={{ hideAttribution: true }}
            style={{ '--xy-background-color': 'var(--color-background)' } as React.CSSProperties}
          >
            <Background
              variant={BackgroundVariant.Dots}
              gap={20}
              size={1.5}
              color={theme === 'dark' ? 'rgba(255,255,255,0.09)' : 'rgba(0,0,0,0.08)'}
            />
            <Controls
              showInteractive={false}
              className="!rounded-lg !overflow-hidden [&>button]:!bg-card [&>button]:!border-card-border [&>button]:!text-foreground/20 [&>button:hover]:!bg-sidebar-accent [&>button:hover]:!text-sidebar-accent-foreground"
            />
            <MiniMap
              nodeColor={miniMapNodeColor}
              maskColor="rgba(0,0,0,0.9)"
              style={{ '--xy-minimap-background-color': 'var(--color-card-border)' } as React.CSSProperties}
              className="!bg-card-border !border-card-border !rounded-lg !overflow-hidden"
            />
          </ReactFlow>
        </div>
      )}
    </div>
  )
}
