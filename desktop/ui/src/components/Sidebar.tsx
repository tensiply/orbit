import { useEffect, useRef, useState } from 'react'
import { STATUS_COLORS } from '../theme'
import {
  Loader2,
  Terminal, ListChecks, FileText, LayoutGrid, Server, Activity,
  User, BookOpen, Settings, Keyboard, Files, Network,
  ExternalLink, Copy, FolderOpen, Wrench, Bug, Cpu,
  CircleStop, CopyPlus, RefreshCw, Plus, Sun, Moon,
  ChevronLeft, ChevronRight,
  Clipboard, Archive, Trash2, Info, Mail, Send,
} from 'lucide-react'
import {
  ClaudeEngineIcon,
  GeminiEngineIcon,
  OpenCodeEngineIcon,
  DefaultEngineIcon,
} from '../icons'

const ENGINES_MENU = [
  { id: 'claude',   label: 'Claude',   Icon: ClaudeEngineIcon },
  { id: 'gemini',   label: 'Gemini',   Icon: GeminiEngineIcon },
  { id: 'opencode', label: 'OpenCode', Icon: OpenCodeEngineIcon },
]
import { invoke } from '@tauri-apps/api/core'
import { useAppStore, Session, NavView, DocEntry, ScopeTreeWorkspace } from '../store'
import {
  computeSidebarItems,
  getScopeChildren,
  filterSessionsByScope,
  filterDocsByScope,
  workspaceFromWorkDir,
} from '../lib/sidebarNav'
import { cn } from '@/lib/utils'
import { ToggleGroup, ToggleGroupItem } from './ui/toggle-group'
import { Button } from './ui/button'
import { Separator } from './ui/separator'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  DropdownMenuShortcut,
} from './ui/dropdown-menu'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from './ui/context-menu'
import {
  SidebarProvider,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from './ui/sidebar'
import { Kbd } from './ui/kbd'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from './ui/dialog'

const RAIL_W = 52

// ── Rail ──────────────────────────────────────────────────────────────────────

const PANEL_LABELS: Partial<Record<NavView, string>> = {
  terminal:     'Sessions',
  tasks:        'Tasks',
  plans:        'Plans',
  plugins:      'Plugins',
  mcps:         'MCPs',
  activity:     'Activity',
  documents:    'Documents',
  architecture: 'Architecture',
  profile:      'Profile',
  docs:         'Documentation',
}

type RailItem = { label: string; view: NavView; icon: React.ReactNode }

const NAV_ITEMS: RailItem[] = [
  { label: 'Tasks',        view: 'tasks',        icon: <ListChecks /> },
  { label: 'Sessions',     view: 'terminal',     icon: <Terminal /> },
  { label: 'Documents',    view: 'documents',    icon: <Files /> },
  { label: 'Plans',        view: 'plans',        icon: <FileText /> },
  { label: 'Plugins',      view: 'plugins',      icon: <LayoutGrid /> },
  { label: 'MCPs',         view: 'mcps',         icon: <Server /> },
  { label: 'Activity',     view: 'activity',     icon: <Activity /> },
  { label: 'Architecture', view: 'architecture', icon: <Network /> },
]

function RailButton({ item, active, onClick }: { item: RailItem; active: boolean; onClick: () => void }) {
  return (
    <SidebarMenuItem className="w-full flex justify-center">
      <SidebarMenuButton
        isActive={active}
        tooltip={item.label}
        onClick={onClick}
        className="!size-8 !p-0 !justify-center"
      >
        {item.icon}
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}

// ── Settings rail button ───────────────────────────────────────────────────────

function SettingsRailButton() {
  const openShortcuts = useAppStore((s) => s.openShortcuts)
  const openSettings  = useAppStore((s) => s.openSettings)
  const theme         = useAppStore((s) => s.theme)
  const toggleTheme   = useAppStore((s) => s.toggleTheme)
  const blurSidebar   = useAppStore((s) => s.blurSidebar)

  return (
    <SidebarMenuItem className="w-full flex justify-center">
      <DropdownMenu onOpenChange={(open) => { if (open) blurSidebar() }}>
        <DropdownMenuTrigger asChild>
          <SidebarMenuButton tooltip="Settings" className="!size-8 !p-0 !justify-center">
            <Settings />
          </SidebarMenuButton>
        </DropdownMenuTrigger>

        <DropdownMenuContent side="right" align="end" className="w-64">
          <DropdownMenuLabel className="text-[10px] uppercase tracking-wider text-muted-foreground">
            Settings
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem className="text-xs gap-2" onClick={openSettings}>
            <Settings />
            Settings
            <DropdownMenuShortcut className="flex items-center gap-0.5">
              <Kbd className="text-[9px] px-1 py-px border-foreground/20 text-foreground/50">Ctrl</Kbd>
              <span className="text-[9px] text-foreground/25">+</span>
              <Kbd className="text-[9px] px-1 py-px border-foreground/20 text-foreground/50">.</Kbd>
            </DropdownMenuShortcut>
          </DropdownMenuItem>
          <DropdownMenuItem className="text-xs gap-2" onClick={openShortcuts}>
            <Keyboard />
            Keyboard Shortcuts
            <DropdownMenuShortcut className="flex items-center gap-0.5">
              <Kbd className="text-[9px] px-1 py-px border-foreground/20 text-foreground/50">Ctrl</Kbd>
              <span className="text-[9px] text-foreground/25">+</span>
              <Kbd className="text-[9px] px-1 py-px border-foreground/20 text-foreground/50">,</Kbd>
            </DropdownMenuShortcut>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem className="text-xs gap-2" onClick={toggleTheme}>
            {theme === 'dark' ? <Sun /> : <Moon />}
            {theme === 'dark' ? 'Switch to Light' : 'Switch to Dark'}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </SidebarMenuItem>
  )
}

const RING_CLASS = 'ring-1 ring-inset ring-foreground/30'
const LEVEL_LABELS_ALL    = ['workspace', 'tenant', 'project', 'repository'] as const
const LEVEL_LABELS_SCOPED = ['tenant', 'project', 'repository'] as const

// ── ViewModeToggle ────────────────────────────────────────────────────────────

function ViewModeToggle() {
  const scopeViewMode    = useAppStore((s) => s.scopeViewMode)
  const setScopeViewMode = useAppStore((s) => s.setScopeViewMode)
  const loadScopeTree    = useAppStore((s) => s.loadScopeTree)
  const scopeTree        = useAppStore((s) => s.scopeTree)

  return (
    <div className="shrink-0 rounded border border-sidebar-border/50 overflow-hidden">
      <ToggleGroup
        type="single"
        value={scopeViewMode}
        onValueChange={(v) => {
          if (!v) return
          if (v === 'scope') void loadScopeTree()
          setScopeViewMode(v as 'all' | 'scope')
        }}
        className="gap-0"
      >
        <ToggleGroupItem
          value="all"
          className={cn(
            'text-[9px] h-[18px] px-1.5 rounded-none border-r border-sidebar-border/50',
            scopeViewMode === 'all'
              ? 'bg-sidebar-accent text-sidebar-accent-foreground'
              : 'bg-transparent text-sidebar-foreground/30 hover:text-sidebar-foreground/60 hover:bg-sidebar-accent/20',
          )}
        >
          All
        </ToggleGroupItem>
        <ToggleGroupItem
          value="scope"
          className={cn(
            'text-[9px] h-[18px] px-1.5 rounded-none',
            scopeViewMode === 'scope'
              ? 'bg-sidebar-accent text-sidebar-accent-foreground'
              : 'bg-transparent text-sidebar-foreground/30 hover:text-sidebar-foreground/60 hover:bg-sidebar-accent/20',
          )}
        >
          Scope
        </ToggleGroupItem>
      </ToggleGroup>
    </div>
  )
}

// ── ScopeNavigator ────────────────────────────────────────────────────────────

function ScopeNavigator({
  backSelected,
  selectedFolderName,
}: {
  backSelected:       boolean
  selectedFolderName: string | null
}) {
  const scopeTree          = useAppStore((s) => s.scopeTree)
  const scopeTreeLoading   = useAppStore((s) => s.scopeTreeLoading)
  const scopePath          = useAppStore((s) => s.scopePath)
  const navigateIn         = useAppStore((s) => s.navigateScopeIn)
  const navigateOut        = useAppStore((s) => s.navigateScopeOut)
  const selectedWorkspace  = useAppStore((s) => s.selectedWorkspace)
  const launchScopeSession = useAppStore((s) => s.launchScopeSession)
  const blurSidebar        = useAppStore((s) => s.blurSidebar)

  const buttonRefs = useRef<Map<string, HTMLButtonElement>>(new Map())

  useEffect(() => {
    const handler = (e: Event) => {
      const { name } = (e as CustomEvent).detail as { name: string }
      const btn = buttonRefs.current.get(name)
      if (!btn) return
      const rect = btn.getBoundingClientRect()
      btn.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true,
        clientX: rect.left + 8,
        clientY: rect.top + rect.height / 2,
      }))
    }
    window.addEventListener('orbit:open-scope-folder-menu', handler)
    return () => window.removeEventListener('orbit:open-scope-folder-menu', handler)
  }, [])

  const launchFolder = (folderName: string, engine: string) => {
    const fullPath = selectedWorkspace
      ? [selectedWorkspace, ...scopePath, folderName]
      : [...scopePath, folderName]
    blurSidebar()
    void launchScopeSession(fullPath, engine)
  }

  const children    = getScopeChildren(scopeTree, scopePath, selectedWorkspace)
  const levelLabels = selectedWorkspace ? LEVEL_LABELS_SCOPED : LEVEL_LABELS_ALL
  const levelLabel  = levelLabels[scopePath.length] ?? ''

  if (scopeTreeLoading && scopeTree.length === 0) {
    return (
      <div className="flex items-center gap-1.5 px-2 py-2 text-sidebar-foreground/30">
        <Loader2 size={11} className="animate-spin" />
        <span className="text-[10px]">Loading scopes…</span>
      </div>
    )
  }

  if (!scopeTreeLoading && scopeTree.length === 0) {
    return (
      <p className="text-[10px] text-sidebar-foreground/25 px-2 py-1 italic">No scopes found</p>
    )
  }

  return (
    <div data-orbit-zone="orbit.desktop.sidebar.panel.scope-nav" className="mb-1">
      {/* Back / breadcrumb */}
      {scopePath.length > 0 && (
        <button
          onClick={navigateOut}
          className={`flex items-center gap-1 w-full px-2 py-1 mb-1 rounded-md text-[10px] text-sidebar-foreground/40 hover:bg-sidebar-accent/40 hover:text-sidebar-foreground/70 transition-colors ${backSelected ? RING_CLASS : ''}`}
        >
          <ChevronLeft size={10} className="shrink-0" />
          <span className="truncate">{scopePath.join(' › ')}</span>
        </button>
      )}

      {/* Children */}
      {children.length > 0 && (
        <div className="mb-1">
          {scopePath.length > 0 && (
            <div className="px-2 pt-0.5 pb-0.5">
              <span className="text-[9px] text-sidebar-foreground/20 uppercase tracking-wider">
                {levelLabel}
              </span>
            </div>
          )}
          <ul className="space-y-0.5">
            {children.map((name) => {
              const isSelected = selectedFolderName === name
              return (
                <li key={name}>
                  <ContextMenu onOpenChange={(open) => { if (open) blurSidebar() }}>
                    <ContextMenuTrigger asChild>
                      <button
                        ref={(el) => { if (el) buttonRefs.current.set(name, el); else buttonRefs.current.delete(name) }}
                        onClick={() => navigateIn(name)}
                        className={`group flex items-center justify-between w-full px-2 py-1.5 rounded-md text-xs text-sidebar-foreground/55 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors ${isSelected ? RING_CLASS : ''}`}
                      >
                        <span className="truncate">{name}</span>
                        <ChevronRight size={11} className="shrink-0 text-sidebar-foreground/20 group-hover:text-sidebar-accent-foreground/50 transition-colors" />
                      </button>
                    </ContextMenuTrigger>
                    <ContextMenuContent className="w-48 text-xs">
                      <ContextMenuSub>
                        <ContextMenuSubTrigger className="text-xs gap-2">
                          <Plus size={13} />New session with…
                        </ContextMenuSubTrigger>
                        <ContextMenuSubContent className="w-40 text-xs">
                          {ENGINES_MENU.map(({ id, label, Icon }) => (
                            <ContextMenuItem
                              key={id}
                              className="text-xs gap-2"
                              onClick={() => launchFolder(name, id)}
                            >
                              <Icon size={13} />{label}
                            </ContextMenuItem>
                          ))}
                        </ContextMenuSubContent>
                      </ContextMenuSub>
                    </ContextMenuContent>
                  </ContextMenu>
                </li>
              )
            })}
          </ul>
        </div>
      )}

      {scopePath.length > 0 && children.length > 0 && (
        <div className="my-1.5 border-t border-sidebar-border/40" />
      )}
    </div>
  )
}

// ── ArchItemsList ─────────────────────────────────────────────────────────────

function ArchItemsList({
  items,
  sidebarFocused,
  sidebarSelectedIdx,
  onOpen,
}: {
  items: Array<{ type: 'scope-architecture'; workspace: string; tenant: string }>
  sidebarFocused: boolean
  sidebarSelectedIdx: number
  onOpen: (workspace: string, tenant: string) => void
}) {
  const blurSidebar = useAppStore((s) => s.blurSidebar)
  const buttonRefs  = useRef<Map<string, HTMLButtonElement>>(new Map())
  const itemRefs    = useRef<Map<string, HTMLLIElement>>(new Map())

  useEffect(() => {
    const handler = (e: Event) => {
      const { workspace, tenant } = (e as CustomEvent).detail as { workspace: string; tenant: string }
      const key = `${workspace}:${tenant}`
      const btn = buttonRefs.current.get(key)
      if (!btn) return
      const rect = btn.getBoundingClientRect()
      btn.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true,
        clientX: rect.left + 8,
        clientY: rect.top + rect.height / 2,
      }))
    }
    window.addEventListener('orbit:open-arch-item-menu', handler)
    return () => window.removeEventListener('orbit:open-arch-item-menu', handler)
  }, [])

  useEffect(() => {
    if (!sidebarFocused || sidebarSelectedIdx < 0) return
    const item = items[sidebarSelectedIdx]
    if (!item) return
    const key = `${item.workspace}:${item.tenant}`
    itemRefs.current.get(key)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }, [sidebarFocused, sidebarSelectedIdx, items])

  if (items.length === 0) return null

  return (
    <ul className="space-y-0.5">
      {items.map((item, idx) => {
        const key        = `${item.workspace}:${item.tenant}`
        const isSelected = sidebarFocused && idx === sidebarSelectedIdx
        return (
          <li
            key={key}
            ref={(el) => { if (el) itemRefs.current.set(key, el); else itemRefs.current.delete(key) }}
          >
            <ContextMenu onOpenChange={(open) => { if (open) blurSidebar() }}>
              <ContextMenuTrigger asChild>
                <button
                  ref={(el) => { if (el) buttonRefs.current.set(key, el); else buttonRefs.current.delete(key) }}
                  onClick={() => onOpen(item.workspace, item.tenant)}
                  className={`group flex items-center gap-1.5 w-full px-2 py-1.5 rounded-md text-xs text-sidebar-foreground/55 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors ${isSelected ? RING_CLASS : ''}`}
                >
                  <Network size={11} className="shrink-0 text-sidebar-foreground/30 group-hover:text-sidebar-accent-foreground/60" />
                  <span className="truncate font-medium">{item.tenant}</span>
                  <span className="text-[10px] text-sidebar-foreground/30 shrink-0">{item.workspace}</span>
                </button>
              </ContextMenuTrigger>
              <ContextMenuContent className="w-44 text-xs">
                <ContextMenuItem
                  className="text-xs gap-2"
                  onClick={() => onOpen(item.workspace, item.tenant)}
                >
                  <Network size={13} />Open Architecture
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          </li>
        )
      })}
    </ul>
  )
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

export function Sidebar({ width, collapsed }: { width: number; collapsed?: boolean }) {
  const allSessions       = useAppStore((s) => s.sessions)
  const selectedWorkspace = useAppStore((s) => s.selectedWorkspace)
  const sessionsLoading   = useAppStore((s) => s.sessionsLoading)
  const sessions = selectedWorkspace
    ? allSessions.filter((s) => workspaceFromWorkDir(s.work_dir) === selectedWorkspace)
    : allSessions
  const sessionTitles      = useAppStore((s) => s.sessionTitles)
  const blankSessions      = useAppStore((s) => s.blankSessions)
  const navView            = useAppStore((s) => s.navView)
  const openSession        = useAppStore((s) => s.openSession)
  const openShell          = useAppStore((s) => s.openShell)
  const openArchitecture   = useAppStore((s) => s.openArchitecture)
  const setNavView         = useAppStore((s) => s.setNavView)
  const tabs               = useAppStore((s) => s.tabs)
  const activeTabId        = useAppStore((s) => s.activeTabId)
  const killSession        = useAppStore((s) => s.killSession)
  const duplicateSession   = useAppStore((s) => s.duplicateSession)
  const restartSession     = useAppStore((s) => s.restartSession)
  const launchScopeSession = useAppStore((s) => s.launchScopeSession)
  const sidebarFocused     = useAppStore((s) => s.sidebarFocused)
  const sidebarSelectedIdx = useAppStore((s) => s.sidebarSelectedIdx)
  const blurSidebar        = useAppStore((s) => s.blurSidebar)
  const documents          = useAppStore((s) => s.documents)
  const documentsLoading   = useAppStore((s) => s.documentsLoading)
  const fetchDocuments     = useAppStore((s) => s.fetchDocuments)
  const openDocument       = useAppStore((s) => s.openDocument)
  const scopeViewMode      = useAppStore((s) => s.scopeViewMode)
  const scopePath          = useAppStore((s) => s.scopePath)
  const setScopePath       = useAppStore((s) => s.setScopePath)
  const scopeTree          = useAppStore((s) => s.scopeTree)
  const loadScopeTree      = useAppStore((s) => s.loadScopeTree)
  const archHistory        = useAppStore((s) => s.archHistory)

  useEffect(() => {
    if (navView === 'documents') void fetchDocuments()
  }, [navView])

  useEffect(() => {
    if (scopeViewMode === 'scope') void loadScopeTree()
  }, [scopeViewMode])

  // Reset drill-down path when workspace selection changes
  useEffect(() => {
    setScopePath([])
  }, [selectedWorkspace])

  const activeSessionId = tabs.find((t) => t.id === activeTabId)?.sessionId
  const panelLabel      = PANEL_LABELS[navView] ?? navView
  const inScopeMode     = scopeViewMode === 'scope'
  const scopedSessions  = inScopeMode ? filterSessionsByScope(sessions, scopePath, selectedWorkspace) : sessions
  const scopedDocuments = filterDocsByScope(documents, inScopeMode ? scopePath : [], selectedWorkspace)
  const hasScopeFilter  = navView === 'terminal' || navView === 'documents' || navView === 'architecture'
  const scopeLabel      = scopePath.length > 0 ? scopePath[scopePath.length - 1] : null

  // Compute unified items list to derive per-component selection state
  const sidebarItems = hasScopeFilter
    ? computeSidebarItems({ navView, scopeViewMode, scopePath, scopeTree, sessions, documents, selectedWorkspace: selectedWorkspace ?? null, archHistory })
    : []
  const selectedItem = sidebarFocused ? sidebarItems[sidebarSelectedIdx] : null

  // Scope navigator selection
  const backSelected       = selectedItem?.type === 'scope-back'
  const selectedFolderName = selectedItem?.type === 'scope-folder' ? selectedItem.name : null

  // Session/document list: adjust index relative to where sessions/docs start in the flat list
  const sessionOffset      = sidebarItems.findIndex((i) => i.type === 'session')
  const docOffset          = sidebarItems.findIndex((i) => i.type === 'document')
  const sessionRelativeIdx = sessionOffset >= 0 ? sidebarSelectedIdx - sessionOffset : -1
  const docRelativeIdx     = docOffset >= 0 ? sidebarSelectedIdx - docOffset : -1
  const archOffset         = sidebarItems.findIndex((i) => i.type === 'scope-architecture')
  const archRelativeIdx    = archOffset >= 0 ? sidebarSelectedIdx - archOffset : -1

  return (
    <SidebarProvider open={false} onOpenChange={() => {}} className="contents">
      <aside
        data-orbit-zone="orbit.desktop.sidebar"
        style={collapsed ? undefined : { width, minWidth: width }}
        className="flex flex-col shrink-0 select-none bg-sidebar pl-2 pt-0 pb-2"
      >
        {/* Header */}
        <div data-tauri-drag-region data-orbit-zone="orbit.desktop.sidebar.header" className="h-10 flex items-center px-3 shrink-0">
          <span className="text-sm font-semibold text-sidebar-foreground tracking-tight">orbit</span>
        </div>

        {/* Body */}
        <div className="flex flex-1 min-h-0 overflow-hidden">
          {/* Icon rail */}
          <div
            data-orbit-zone="orbit.desktop.sidebar.rail"
            className="flex flex-col items-center py-2 px-1.5 gap-0.5 shrink-0 rounded-2xl ring-1 ring-sidebar-border/80 bg-card"
            style={{ width: RAIL_W }}
          >
            <SidebarMenu className="w-full">
              {NAV_ITEMS.map((item) => (
                <RailButton
                  key={item.view}
                  item={item}
                  active={navView === item.view}
                  onClick={() => setNavView(item.view)}
                />
              ))}
            </SidebarMenu>
            <div className="mt-auto flex flex-col gap-0.5 pt-1 w-full">
              <SidebarMenu className="w-full">
                <RailButton
                  item={{ label: 'Documentation', view: 'docs', icon: <BookOpen /> }}
                  active={navView === 'docs'}
                  onClick={() => setNavView('docs')}
                />
                <Separator className="mx-1 w-auto my-0.5 bg-sidebar-border" />
                <SettingsRailButton />
                <RailButton
                  item={{ label: 'Profile', view: 'profile', icon: <User /> }}
                  active={navView === 'profile'}
                  onClick={() => setNavView('profile')}
                />
              </SidebarMenu>
            </div>
          </div>

          {/* Panel */}
          {!collapsed && (
            <div data-orbit-zone="orbit.desktop.sidebar.panel" className="flex flex-col flex-1 min-h-0 min-w-0 pl-2">
              {/* Panel header */}
              <div data-orbit-zone="orbit.desktop.sidebar.panel.header" className="flex items-center pt-3 pb-1 px-3 shrink-0 gap-2">
                <span className="text-[10px] font-medium text-sidebar-foreground/40 uppercase tracking-wider flex-1">
                  {panelLabel}
                </span>
                {hasScopeFilter && (
                  <ViewModeToggle />
                )}
                {sessionsLoading && navView === 'terminal' && (
                  <Loader2 className="h-3 w-3 animate-spin text-sidebar-foreground/30 shrink-0" />
                )}
                {documentsLoading && navView === 'documents' && (
                  <Loader2 className="h-3 w-3 animate-spin text-sidebar-foreground/30 shrink-0" />
                )}
              </div>

              {/* Panel content */}
              <div data-orbit-zone="orbit.desktop.sidebar.panel.content" className="flex-1 overflow-y-auto overflow-x-hidden min-h-0 px-1.5">
                {navView === 'terminal' && (
                  <>
                    {inScopeMode && (
                      <ScopeNavigator
                        backSelected={backSelected}
                        selectedFolderName={selectedFolderName}
                      />
                    )}
                    <SessionList
                      sessions={scopedSessions}
                      sessionsLoading={sessionsLoading}
                      activeSessionId={activeSessionId}
                      sessionTitles={sessionTitles}
                      blankSessions={blankSessions}
                      sidebarFocused={sidebarFocused}
                      sidebarSelectedIdx={sessionRelativeIdx}
                      onOpen={(s) => { blurSidebar(); openSession(s) }}
                      onKill={(s) => killSession(s)}
                      onDuplicate={(s) => duplicateSession(s)}
                      onRestart={(s) => restartSession(s)}
                      onLaunchWithEngine={(s, engine) => {
                        const ws   = workspaceFromWorkDir(s.work_dir)
                        const path = [ws, s.tenant || null, s.project || null, s.repository || null].filter(Boolean) as string[]
                        blurSidebar()
                        void launchScopeSession(path, engine)
                      }}
                    />
                  </>
                )}

                {navView === 'docs' && <DocsPanel />}
                {navView === 'documents' && (
                  <>
                    {inScopeMode && (
                      <ScopeNavigator
                        backSelected={backSelected}
                        selectedFolderName={selectedFolderName}
                      />
                    )}
                    <DocumentsPanel
                      documents={scopedDocuments}
                      loading={documentsLoading}
                      sidebarFocused={sidebarFocused}
                      sidebarSelectedIdx={docRelativeIdx}
                      onOpen={openDocument}
                    />
                  </>
                )}
                {navView === 'architecture' && (
                  <>
                    {inScopeMode && (
                      <ScopeNavigator
                        backSelected={backSelected}
                        selectedFolderName={selectedFolderName}
                      />
                    )}
                    {!inScopeMode && (
                      <div className="px-2 pt-0.5 pb-0.5">
                        <span className="text-[9px] text-sidebar-foreground/20 uppercase tracking-wider">Recent</span>
                      </div>
                    )}
                    <ArchItemsList
                      items={sidebarItems.filter((i): i is { type: 'scope-architecture'; workspace: string; tenant: string } => i.type === 'scope-architecture')}
                      sidebarFocused={sidebarFocused}
                      sidebarSelectedIdx={archRelativeIdx}
                      onOpen={(ws, t) => { blurSidebar(); openArchitecture(ws, t) }}
                    />
                    {!inScopeMode && sidebarItems.filter((i) => i.type === 'scope-architecture').length === 0 && (
                      <p className="text-[10px] text-sidebar-foreground/25 px-2 pt-1 italic">No architectures opened yet</p>
                    )}
                  </>
                )}
                {navView !== 'terminal' && navView !== 'docs' && navView !== 'documents' && navView !== 'architecture' && (
                  <p className="text-[10px] text-sidebar-foreground/25 px-2 pt-1">Coming soon</p>
                )}
              </div>

            </div>
          )}
        </div>
      </aside>
    </SidebarProvider>
  )
}

// ── Documents panel ───────────────────────────────────────────────────────────

function docScope(doc: DocEntry): string {
  return [doc.tenant, doc.project, doc.repository].filter(Boolean).join(' › ')
}

function docFilename(path: string): string {
  return path.split('/').pop() ?? path
}

const TEXT_FORMATS = new Set(['html', 'csv'])

function fmtTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

function DocumentItem({
  doc,
  isKeySelected,
  onOpen,
  onArchive,
  onDelete,
}: {
  doc: DocEntry
  isKeySelected: boolean
  onOpen: () => void
  onArchive: () => void
  onDelete: () => Promise<void>
}) {
  const itemRef    = useRef<HTMLLIElement>(null)
  const blurSidebar = useAppStore((s) => s.blurSidebar)
  const [showInfo,   setShowInfo]   = useState(false)
  const [showDelete, setShowDelete] = useState(false)
  const [deleting,   setDeleting]   = useState(false)

  useEffect(() => {
    if (isKeySelected && itemRef.current) {
      itemRef.current.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    }
  }, [isKeySelected])

  const scope = docScope(doc)
  const file  = docFilename(doc.output_path)

  async function copyPath() {
    await navigator.clipboard.writeText(doc.output_path)
  }

  async function copyContent() {
    if (TEXT_FORMATS.has(doc.format.toLowerCase())) {
      try {
        const b64 = await invoke<string>('document_read_b64', { path: doc.output_path })
        const text = atob(b64)
        await navigator.clipboard.writeText(text)
        return
      } catch { /* fall through */ }
    }
    await navigator.clipboard.writeText(doc.output_path)
  }

  async function reveal() {
    await invoke('document_reveal', { path: doc.output_path }).catch(() => void 0)
  }

  async function confirmDelete() {
    setDeleting(true)
    try {
      await onDelete()
    } finally {
      setDeleting(false)
      setShowDelete(false)
    }
  }

  return (
    <>
      {/* Info dialog */}
      <Dialog open={showInfo} onOpenChange={setShowInfo}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-sm flex items-center gap-2">
              <span className="text-[10px] uppercase tracking-wider font-medium px-1 py-px rounded bg-muted text-muted-foreground">{doc.format}</span>
              {doc.title}
            </DialogTitle>
            <DialogDescription className="sr-only">Document details</DialogDescription>
          </DialogHeader>
          <div className="space-y-2 text-xs text-foreground/70">
            <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5">
              <span className="text-foreground/40 font-medium">ID</span>
              <span className="font-mono">{doc.id}</span>
              <span className="text-foreground/40 font-medium">Path</span>
              <span className="font-mono break-all text-[10px] leading-snug">{doc.output_path}</span>
              {doc.source_path && (
                <>
                  <span className="text-foreground/40 font-medium">Source</span>
                  <span className="font-mono break-all text-[10px] leading-snug">{doc.source_path}</span>
                </>
              )}
              {doc.template && (
                <>
                  <span className="text-foreground/40 font-medium">Template</span>
                  <span>{doc.template}</span>
                </>
              )}
              {scope && (
                <>
                  <span className="text-foreground/40 font-medium">Scope</span>
                  <span>{scope}</span>
                </>
              )}
              {doc.workspace && (
                <>
                  <span className="text-foreground/40 font-medium">Workspace</span>
                  <span>{doc.workspace}</span>
                </>
              )}
              <span className="text-foreground/40 font-medium">Created</span>
              <span>{fmtTs(doc.created_at)}</span>
              <span className="text-foreground/40 font-medium">Updated</span>
              <span>{fmtTs(doc.updated_at)}</span>
            </div>
          </div>
          <DialogFooter>
            <Button size="sm" variant="outline" onClick={() => setShowInfo(false)}>Close</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete confirmation dialog */}
      <Dialog open={showDelete} onOpenChange={(o) => { if (!deleting) setShowDelete(o) }}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="text-sm">Delete document?</DialogTitle>
            <DialogDescription className="text-xs">
              This will permanently delete <span className="font-medium text-foreground">{doc.title}</span> and its output file. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2">
            <Button size="sm" variant="outline" onClick={() => setShowDelete(false)} disabled={deleting}>Cancel</Button>
            <Button size="sm" variant="destructive" onClick={confirmDelete} disabled={deleting}>
              {deleting ? 'Deleting…' : 'Delete'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <li ref={itemRef}>
        <ContextMenu onOpenChange={(open) => { if (open) blurSidebar() }}>
          <ContextMenuTrigger asChild>
            <button
              onClick={onOpen}
              className={`group w-full min-w-0 py-2 px-2 rounded-md transition-colors text-left text-sidebar-foreground/40 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground ${isKeySelected ? RING_CLASS : ''}`}
            >
              <div className="flex flex-col gap-0.5 min-w-0">
                <div className="flex items-center gap-1.5 min-w-0">
                  <span className="text-[10px] uppercase tracking-wider font-medium px-1 py-px rounded bg-sidebar-foreground/8 text-sidebar-foreground/40 shrink-0">
                    {doc.format}
                  </span>
                  <span className="text-xs font-medium leading-snug truncate text-sidebar-foreground/60 group-hover:text-sidebar-accent-foreground">
                    {doc.title}
                  </span>
                </div>
                <span className="text-[10px] leading-tight truncate text-sidebar-foreground/30 pl-0.5">{file}</span>
                {scope && (
                  <span className="text-[10px] leading-tight truncate text-sidebar-foreground/25 pl-0.5">{scope}</span>
                )}
              </div>
            </button>
          </ContextMenuTrigger>

          <ContextMenuContent className="w-52 text-xs">
            <ContextMenuItem className="text-xs gap-2" onClick={copyPath}>
              <Copy size={13} />Copy path
            </ContextMenuItem>
            <ContextMenuItem className="text-xs gap-2" onClick={copyContent}>
              <Clipboard size={13} />Copy document
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem className="text-xs gap-2" onClick={reveal}>
              <FolderOpen size={13} />Reveal in explorer
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem className="text-xs gap-2" onClick={onArchive}>
              <Archive size={13} />Archive
            </ContextMenuItem>
            <ContextMenuItem
              className="text-xs gap-2 text-destructive focus:text-destructive"
              onClick={() => setShowDelete(true)}
            >
              <Trash2 size={13} />Delete…
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem className="text-xs gap-2" onClick={() => setShowInfo(true)}>
              <Info size={13} />Info
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuSub>
              <ContextMenuSubTrigger className="text-xs gap-2">
                <Send size={13} />Send to
              </ContextMenuSubTrigger>
              <ContextMenuSubContent className="w-40 text-xs">
                <ContextMenuItem className="text-xs gap-2 opacity-40 pointer-events-none" disabled>
                  <Mail size={13} />Email
                </ContextMenuItem>
                <ContextMenuItem className="text-xs gap-2 opacity-40 pointer-events-none" disabled>
                  <FileText size={13} />Orbit
                </ContextMenuItem>
                <ContextMenuItem className="text-xs gap-2 opacity-40 pointer-events-none" disabled>
                  <Network size={13} />Teams
                </ContextMenuItem>
              </ContextMenuSubContent>
            </ContextMenuSub>
          </ContextMenuContent>
        </ContextMenu>
      </li>
    </>
  )
}

function DocumentsPanel({
  documents,
  loading,
  sidebarFocused,
  sidebarSelectedIdx,
  onOpen,
}: {
  documents:          DocEntry[]
  loading:            boolean
  sidebarFocused:     boolean
  sidebarSelectedIdx: number
  onOpen: (doc: DocEntry) => void
}) {
  const archiveDocument = useAppStore((s) => s.archiveDocument)
  const deleteDocument  = useAppStore((s) => s.deleteDocument)

  if (!loading && documents.length === 0) {
    return (
      <p className="text-[10px] text-sidebar-foreground/30 px-2 pt-1 italic">No documents yet</p>
    )
  }

  return (
    <ul className="p-0 w-full space-y-0.5 pt-0.5">
      {documents.map((doc, idx) => (
        <DocumentItem
          key={doc.id}
          doc={doc}
          isKeySelected={sidebarFocused && idx === sidebarSelectedIdx}
          onOpen={() => onOpen(doc)}
          onArchive={() => void archiveDocument(doc.id, doc.workspace)}
          onDelete={() => deleteDocument(doc.id, doc.workspace)}
        />
      ))}
    </ul>
  )
}

// ── Docs panel ────────────────────────────────────────────────────────────────

function DocsPanel() {
  const openUIKit  = useAppStore((s) => s.openUIKit)
  const openColors = useAppStore((s) => s.openColors)
  const openUIMap  = useAppStore((s) => s.openUIMap)
  return (
    <div data-orbit-zone="orbit.desktop.sidebar.panel.docs" className="pt-0.5 space-y-0.5">
      <Button
        variant="ghost"
        onClick={openUIMap}
        className="w-full justify-start h-auto px-2 py-1.5 text-xs text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
      >
        UI Map
      </Button>
      <Button
        variant="ghost"
        onClick={openUIKit}
        className="w-full justify-start h-auto px-2 py-1.5 text-xs text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
      >
        UI Kit
      </Button>
      <Button
        variant="ghost"
        onClick={openColors}
        className="w-full justify-start h-auto px-2 py-1.5 text-xs text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
      >
        Colors
      </Button>
    </div>
  )
}

// ── Session list ──────────────────────────────────────────────────────────────

const MAX_SESSIONS = 10

function SectionLabel({ label }: { label: string }) {
  return (
    <li className="px-2 pt-3 pb-0.5">
      <span className="text-[9px] font-medium uppercase tracking-wider text-sidebar-foreground/30">{label}</span>
    </li>
  )
}

function SessionList({
  sessions,
  sessionsLoading,
  activeSessionId,
  sessionTitles,
  blankSessions,
  sidebarFocused,
  sidebarSelectedIdx,
  onOpen,
  onKill,
  onDuplicate,
  onRestart,
  onLaunchWithEngine,
}: {
  sessions:           import('../types').Session[]
  sessionsLoading:    boolean
  activeSessionId:    string | undefined
  sessionTitles:      Record<string, string>
  blankSessions:      string[]
  sidebarFocused:     boolean
  sidebarSelectedIdx: number
  onOpen:              (s: import('../types').Session) => void
  onKill:              (s: import('../types').Session) => void
  onDuplicate:         (s: import('../types').Session) => void
  onRestart:           (s: import('../types').Session) => void
  onLaunchWithEngine:  (s: import('../types').Session, engine: string) => void
}) {
  const nonHistory   = sessions.filter((s) => !s.is_history)
  const historySlots = Math.max(0, MAX_SESSIONS - nonHistory.length)
  const history      = sessions.filter((s) => !!s.is_history).slice(0, historySlots)
  const flat         = [...nonHistory, ...history]

  const isEmpty    = nonHistory.length === 0 && history.length === 0
  const showLabels = nonHistory.length > 0 && history.length > 0

  return (
    <ul data-orbit-zone="orbit.desktop.sidebar.panel.session-list" className="p-0 w-full space-y-0.5">
      {isEmpty && !sessionsLoading && (
        <li className="px-2 py-1 text-[10px] text-sidebar-foreground/30 italic">No sessions</li>
      )}
      {nonHistory.length > 0 && (
        <>
          {showLabels && <SectionLabel label="Current" />}
          {nonHistory.map((s) => {
            const flatIdx = flat.indexOf(s)
            return (
              <SessionItem
                key={s.id}
                session={s}
                active={s.id === activeSessionId}
                isHistory={false}
                isCurrent={s.id === activeSessionId}
                isKeySelected={sidebarFocused && flatIdx === sidebarSelectedIdx}
                title={sessionTitles[s.id]}
                isBlank={blankSessions.includes(s.id)}
                onOpen={() => onOpen(s)}
                onKill={() => onKill(s)}
                onDuplicate={() => onDuplicate(s)}
                onRestart={() => onRestart(s)}
                onLaunchWithEngine={(engine) => onLaunchWithEngine(s, engine)}
              />
            )
          })}
        </>
      )}
      {history.length > 0 && (
        <>
          {showLabels && <SectionLabel label="History" />}
          {history.map((s) => {
            const flatIdx = flat.indexOf(s)
            return (
              <SessionItem
                key={s.id}
                session={s}
                active={false}
                isHistory={true}
                isCurrent={false}
                isKeySelected={sidebarFocused && flatIdx === sidebarSelectedIdx}
                title={sessionTitles[s.id]}
                isBlank={false}
                onOpen={() => onOpen(s)}
                onKill={() => {}}
                onDuplicate={() => onDuplicate(s)}
                onRestart={() => {}}
                onLaunchWithEngine={(engine) => onLaunchWithEngine(s, engine)}
              />
            )
          })}
        </>
      )}
    </ul>
  )
}

// ── Session item ──────────────────────────────────────────────────────────────

function EngineIcon({ engine, size = 12 }: { engine: string; size?: number }) {
  switch (engine) {
    case 'claude':   return <ClaudeEngineIcon size={size} />
    case 'gemini':   return <GeminiEngineIcon size={size} />
    case 'opencode': return <OpenCodeEngineIcon size={size} />
    default:         return <DefaultEngineIcon size={size} />
  }
}

function relativeTime(unixSecs: number): string {
  const diffSecs = Math.floor(Date.now() / 1000) - unixSecs
  if (diffSecs < 60)    return 'just now'
  if (diffSecs < 3600)  return `${Math.floor(diffSecs / 60)}m ago`
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`
  return `${Math.floor(diffSecs / 86400)}d ago`
}

function scopeParts(session: Session): string[] {
  const parts: string[] = []
  const ws = workspaceFromWorkDir(session.work_dir)
  if (ws)                  parts.push(ws)
  if (session.tenant)      parts.push(session.tenant)
  if (session.project)     parts.push(session.project)
  if (session.repository)  parts.push(session.repository)
  return parts
}

function sessionLabel(session: Session): string {
  if (session.global_mode) return 'Global'
  const parts = scopeParts(session)
  return parts[parts.length - 1] ?? session.id.slice(0, 8)
}

function sessionBreadcrumb(session: Session): string | null {
  if (session.global_mode) return null
  const parts = scopeParts(session)
  if (parts.length <= 1) return null
  return parts.slice(0, -1).join(' › ')
}

function sessionScopeCrumb(session: Session): string | null {
  if (session.global_mode) return null
  const parts: string[] = []
  if (session.tenant)     parts.push(session.tenant)
  if (session.project)    parts.push(session.project)
  if (session.repository) parts.push(session.repository)
  return parts.length > 0 ? parts.join(' › ') : null
}

function SessionItem({
  session,
  active,
  isHistory,
  isCurrent,
  isKeySelected,
  title,
  isBlank,
  onOpen,
  onKill,
  onDuplicate,
  onRestart,
  onLaunchWithEngine,
}: {
  session:            Session
  active:             boolean
  isHistory:          boolean
  isCurrent:          boolean
  isKeySelected:      boolean
  title:              string | undefined
  isBlank:            boolean
  onOpen:             () => void
  onKill:             () => void
  onDuplicate:        () => void
  onRestart:          () => void
  onLaunchWithEngine: (engine: string) => void
}) {
  const blurSidebar = useAppStore((s) => s.blurSidebar)
  const itemRef     = useRef<HTMLLIElement>(null)
  const buttonRef   = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (isKeySelected && itemRef.current) {
      itemRef.current.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    }
  }, [isKeySelected])

  useEffect(() => {
    const handler = (e: Event) => {
      const { sessionId } = (e as CustomEvent).detail as { sessionId: string }
      if (sessionId !== session.id || !buttonRef.current) return
      const rect = buttonRef.current.getBoundingClientRect()
      buttonRef.current.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true,
        clientX: rect.left + 8,
        clientY: rect.top + rect.height / 2,
      }))
    }
    window.addEventListener('orbit:open-session-menu', handler)
    return () => window.removeEventListener('orbit:open-session-menu', handler)
  }, [session.id])

  const engine          = session.engine.toLowerCase()
  const label           = (isBlank && !title) ? '' : (title ?? sessionLabel(session))
  const crumb           = (title || (isBlank && !title)) ? sessionScopeCrumb(session) : sessionBreadcrumb(session)
  const timeStr         = relativeTime(session.started_at)
  const effectiveStatus = !isHistory ? (session.status ?? 'ready') : undefined

  const rowClass = isCurrent
    ? 'bg-sidebar-accent text-sidebar-accent-foreground'
    : isHistory
      ? 'text-sidebar-foreground/25 hover:bg-sidebar-accent/40 hover:text-sidebar-accent-foreground/50'
      : 'text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground'

  const copyToClipboard = (text: string) => void navigator.clipboard.writeText(text)

  return (
    <li ref={itemRef}>
      <ContextMenu onOpenChange={(open) => { if (open) blurSidebar() }}>
        <ContextMenuContent className="w-56 text-xs">
          <ContextMenuItem className="text-xs gap-2" onClick={onOpen}>
            <ExternalLink size={13} />Open session
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuSub>
            <ContextMenuSubTrigger className="text-xs gap-2">
              <Plus size={13} />New session with…
            </ContextMenuSubTrigger>
            <ContextMenuSubContent className="w-40 text-xs">
              {ENGINES_MENU.map(({ id, label, Icon }) => (
                <ContextMenuItem
                  key={id}
                  className="text-xs gap-2"
                  onClick={() => onLaunchWithEngine(id)}
                >
                  <Icon size={13} />{label}
                </ContextMenuItem>
              ))}
            </ContextMenuSubContent>
          </ContextMenuSub>
          {!isHistory && (
            <>
              <ContextMenuSeparator />
              <ContextMenuItem className="text-xs gap-2" onClick={onDuplicate}>
                <CopyPlus size={13} />Duplicate session
              </ContextMenuItem>
              <ContextMenuItem className="text-xs gap-2" onClick={onRestart}>
                <RefreshCw size={13} />Restart session
              </ContextMenuItem>
              <ContextMenuItem
                className="text-xs gap-2 text-destructive focus:text-destructive"
                onClick={onKill}
              >
                <CircleStop size={13} />End session
              </ContextMenuItem>
            </>
          )}
          <ContextMenuSeparator />
          <ContextMenuItem className="text-xs gap-2" onClick={() => copyToClipboard(session.id)}>
            <Copy size={13} />Copy session ID
          </ContextMenuItem>
          <ContextMenuItem className="text-xs gap-2" onClick={() => copyToClipboard(session.work_dir)}>
            <FolderOpen size={13} />Copy working directory
          </ContextMenuItem>
          <ContextMenuItem className="text-xs gap-2" onClick={() => copyToClipboard(engine)}>
            <Cpu size={13} />Copy engine name
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuSub>
            <ContextMenuSubTrigger className="text-xs gap-2 text-muted-foreground">
              <Wrench size={13} />Development
            </ContextMenuSubTrigger>
            <ContextMenuSubContent className="w-48 text-xs">
              <ContextMenuItem
                className="text-xs gap-2"
                onClick={() => void invoke('open_devtools').catch(() => void 0)}
              >
                <Bug size={13} />Open DevTools
              </ContextMenuItem>
              <ContextMenuItem className="text-xs gap-2" onClick={() => window.location.reload()}>
                <Wrench size={13} />Reload UI
              </ContextMenuItem>
            </ContextMenuSubContent>
          </ContextMenuSub>
        </ContextMenuContent>

        <ContextMenuTrigger asChild>
          <button
            ref={buttonRef}
            data-orbit-zone="orbit.desktop.sidebar.panel.session-item"
            onClick={onOpen}
            className={`group relative w-full min-w-0 py-2 px-2 rounded-lg transition-colors text-left ${rowClass} ${isKeySelected ? RING_CLASS : ''}`}
          >
            <span
              className="absolute pointer-events-none select-none text-sidebar-foreground"
              style={{ top: 6, right: 6, opacity: isHistory ? 0.04 : 0.07 }}
            >
              <EngineIcon engine={engine} size={26} />
            </span>
            <div className="flex flex-col gap-0.5 min-w-0 pr-7">
              <div className="flex items-center gap-1.5 min-w-0">
                <div className="relative flex items-center justify-center shrink-0">
                  {effectiveStatus === 'working' && (
                    <span
                      className="absolute w-4 h-4 rounded-full animate-ping"
                      style={{ backgroundColor: STATUS_COLORS.working, opacity: 0.35 }}
                    />
                  )}
                  <span
                    className="w-2.5 h-2.5 rounded-full relative"
                    style={isHistory
                      ? { border: '1.5px solid currentColor' }
                      : { backgroundColor: effectiveStatus === 'working' ? STATUS_COLORS.working : STATUS_COLORS.active }
                    }
                  />
                </div>
                <span className={`text-xs font-medium leading-snug truncate ${isCurrent ? 'text-sidebar-accent-foreground' : ''}`}>
                  {label}
                </span>
              </div>
              {crumb && (
                <span className={`text-[10px] leading-tight truncate mt-1 ${isHistory ? 'text-sidebar-foreground/20' : 'text-sidebar-foreground/35'}`}>
                  {crumb}
                </span>
              )}
              <span className={`text-[10px] leading-tight ${isHistory ? 'text-sidebar-foreground/15' : 'text-sidebar-foreground/25'}`}>
                {engine} · {timeStr}
              </span>
            </div>
          </button>
        </ContextMenuTrigger>
      </ContextMenu>
    </li>
  )
}
