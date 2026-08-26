import { useEffect } from 'react'
import { useAppStore } from '../store'
import { sendTerminalCmd } from '../lib/terminalBus'
import {
  computeSidebarItems,
  workspaceFromWorkDir,
} from '../lib/sidebarNav'

type ParsedKeys = {
  ctrl: boolean
  meta: boolean
  alt: boolean
  shift: boolean
  key: string
} | null

function parseShortcutKeys(keys: string): ParsedKeys {
  if (!keys) return null
  const parts = keys.split('+').filter(Boolean)
  const MODS  = new Set(['Ctrl', '⌘', 'Alt', '⇧'])
  const ctrl  = parts.includes('Ctrl')
  const meta  = parts.includes('⌘')
  const alt   = parts.includes('Alt')
  const shift = parts.includes('⇧')
  const keyParts = parts.filter((p) => !MODS.has(p))
  if (keyParts.length !== 1) return null
  return { ctrl, meta, alt, shift, key: keyParts[0] }
}

function matchesEvent(e: KeyboardEvent, parsed: ParsedKeys): boolean {
  if (!parsed) return false
  if (e.ctrlKey  !== parsed.ctrl)  return false
  if (e.metaKey  !== parsed.meta)  return false
  if (e.altKey   !== parsed.alt)   return false
  if (e.shiftKey !== parsed.shift) return false
  const evKey = e.key.length === 1 ? e.key.toUpperCase() : e.key
  const scKey = parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key
  return evKey === scKey
}

export function useGlobalShortcuts() {
  const shortcuts           = useAppStore((s) => s.shortcuts)
  const tabs                = useAppStore((s) => s.tabs)
  const activeTabId         = useAppStore((s) => s.activeTabId)
  const allSessions         = useAppStore((s) => s.sessions)
  const selectedWorkspace   = useAppStore((s) => s.selectedWorkspace)
  const openShell           = useAppStore((s) => s.openShell)
  const closeTab            = useAppStore((s) => s.closeTab)
  const setActiveTab        = useAppStore((s) => s.setActiveTab)
  const openShortcuts       = useAppStore((s) => s.openShortcuts)
  const openSettings        = useAppStore((s) => s.openSettings)
  const toggleTheme         = useAppStore((s) => s.toggleTheme)
  const toggleSidebar       = useAppStore((s) => s.toggleSidebar)
  const setNavView          = useAppStore((s) => s.setNavView)
  const toggleDrawer        = useAppStore((s) => s.toggleDrawer)
  const restartSession      = useAppStore((s) => s.restartSession)
  const duplicateSession    = useAppStore((s) => s.duplicateSession)
  const killSession         = useAppStore((s) => s.killSession)
  const openSession         = useAppStore((s) => s.openSession)
  const drawerOpen          = useAppStore((s) => s.drawerOpen)
  const focusedPanel        = useAppStore((s) => s.focusedPanel)
  const closeDrawer         = useAppStore((s) => s.closeDrawer)
  const setFocusedPanel     = useAppStore((s) => s.setFocusedPanel)
  const sidebarFocused      = useAppStore((s) => s.sidebarFocused)
  const sidebarSelectedIdx  = useAppStore((s) => s.sidebarSelectedIdx)
  const focusSidebar        = useAppStore((s) => s.focusSidebar)
  const blurSidebar         = useAppStore((s) => s.blurSidebar)
  const setSidebarSelectedIdx = useAppStore((s) => s.setSidebarSelectedIdx)
  const drawerTabId           = useAppStore((s) => s.drawerTabId)
  const navView               = useAppStore((s) => s.navView)
  const documents             = useAppStore((s) => s.documents)
  const openDocument          = useAppStore((s) => s.openDocument)
  const registeredWorkspaces  = useAppStore((s) => s.registeredWorkspaces)
  const setSelectedWorkspace  = useAppStore((s) => s.setSelectedWorkspace)
  const scopeViewMode         = useAppStore((s) => s.scopeViewMode)
  const scopePath             = useAppStore((s) => s.scopePath)
  const scopeTree             = useAppStore((s) => s.scopeTree)
  const setScopeViewMode      = useAppStore((s) => s.setScopeViewMode)
  const navigateScopeIn       = useAppStore((s) => s.navigateScopeIn)
  const navigateScopeOut      = useAppStore((s) => s.navigateScopeOut)
  const loadScopeTree         = useAppStore((s) => s.loadScopeTree)
  const openLaunchPicker      = useAppStore((s) => s.openLaunchPicker)
  const openArchitecture      = useAppStore((s) => s.openArchitecture)
  const archHistory           = useAppStore((s) => s.archHistory)

  useEffect(() => {
    const parsed = shortcuts.map((s) => ({ ...s, parsed: parseShortcutKeys(s.keys) }))

    const sessions = selectedWorkspace
      ? allSessions.filter((s) => workspaceFromWorkDir(s.work_dir) === selectedWorkspace)
      : allSessions

    // Open workspaces with active sessions, in registry order
    const openWorkspaces = registeredWorkspaces.filter((ws) =>
      allSessions.some((s) => !s.is_history && workspaceFromWorkDir(s.work_dir) === ws.name),
    )

    // Unified sidebar items (only meaningful for terminal/documents views)
    const sidebarItems = computeSidebarItems({
      navView,
      scopeViewMode,
      scopePath,
      scopeTree,
      sessions,
      documents,
      selectedWorkspace: selectedWorkspace ?? null,
      archHistory,
    })

    const onKeyDown = (e: KeyboardEvent) => {
      // Alt+W — open workspace menu
      if (e.altKey && !e.ctrlKey && !e.metaKey && e.key.toLowerCase() === 'w') {
        e.preventDefault()
        window.dispatchEvent(new CustomEvent('orbit:open-workspace-menu'))
        return
      }

      // Alt+S — toggle All/Scope view mode and focus first item
      if (e.altKey && !e.ctrlKey && !e.metaKey && e.key.toLowerCase() === 's') {
        const hasScopeFilter = navView === 'terminal' || navView === 'documents' || navView === 'architecture'
        if (hasScopeFilter) {
          e.preventDefault()
          const next: 'all' | 'scope' = scopeViewMode === 'all' ? 'scope' : 'all'
          setScopeViewMode(next)
          if (next === 'scope') void loadScopeTree()
          setSidebarSelectedIdx(0)
          ;(document.activeElement as HTMLElement)?.blur()
          focusSidebar()
        }
        return
      }

      // Alt+1..9 — switch to open workspace by index
      if (e.altKey && !e.ctrlKey && !e.metaKey && e.key >= '1' && e.key <= '9') {
        const idx = parseInt(e.key, 10) - 1
        const ws  = openWorkspaces[idx]
        if (ws) {
          e.preventDefault()
          setSelectedWorkspace(ws.name)
        }
        return
      }

      // Ctrl+Enter in sidebar — open context menu for the focused item
      if (sidebarFocused && e.ctrlKey && !e.metaKey && !e.altKey && e.key === 'Enter') {
        const hasScopeFilter = navView === 'terminal' || navView === 'documents' || navView === 'architecture'
        if (hasScopeFilter) {
          e.preventDefault()
          e.stopPropagation()
          const item = sidebarItems[sidebarSelectedIdx]
          if (item?.type === 'session') {
            window.dispatchEvent(new CustomEvent('orbit:open-session-menu', { detail: { sessionId: item.session.id } }))
          } else if (item?.type === 'scope-folder') {
            window.dispatchEvent(new CustomEvent('orbit:open-scope-folder-menu', { detail: { name: item.name } }))
          } else if (item?.type === 'scope-architecture') {
            window.dispatchEvent(new CustomEvent('orbit:open-arch-item-menu', { detail: { workspace: item.workspace, tenant: item.tenant } }))
          }
          return
        }
      }

      // ArrowDown activates sidebar focus when not already focused (documents, terminal, architecture).
      // Skip when xterm has DOM focus — the terminal must receive the key (history navigation, etc.)
      if (!sidebarFocused && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && e.key === 'ArrowDown') {
        const terminalHasFocus = !!(document.activeElement?.closest('.xterm'))
        if (!terminalHasFocus) {
          const hasScopeFilter = navView === 'terminal' || navView === 'documents' || navView === 'architecture'
          if (hasScopeFilter && sidebarItems.length > 0) {
            e.preventDefault()
            e.stopPropagation()
            setSidebarSelectedIdx(0)
            focusSidebar()
          }
        }
        return
      }

      // Sidebar navigation — active when sidebar has keyboard focus
      if (sidebarFocused && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
        const len = sidebarItems.length

        if (e.key === 'ArrowDown') {
          e.preventDefault()
          e.stopPropagation()
          if (len > 0) setSidebarSelectedIdx((sidebarSelectedIdx + 1) % len)
          return
        }

        if (e.key === 'ArrowUp') {
          e.preventDefault()
          e.stopPropagation()
          if (len > 0) setSidebarSelectedIdx((sidebarSelectedIdx - 1 + len) % len)
          return
        }

        if (e.key === 'ArrowRight') {
          e.preventDefault()
          e.stopPropagation()
          const item = sidebarItems[sidebarSelectedIdx]
          if (item?.type === 'scope-folder') {
            navigateScopeIn(item.name)
            setSidebarSelectedIdx(0)
          }
          return
        }

        if (e.key === 'ArrowLeft') {
          e.preventDefault()
          e.stopPropagation()
          if (scopePath.length > 0) {
            navigateScopeOut()
            setSidebarSelectedIdx(0)
          }
          return
        }

        if (e.key === 'Enter') {
          e.preventDefault()
          e.stopPropagation()
          const item = sidebarItems[sidebarSelectedIdx]
          if (!item) return
          switch (item.type) {
            case 'scope-back':
              navigateScopeOut()
              setSidebarSelectedIdx(0)
              break
            case 'scope-folder':
              navigateScopeIn(item.name)
              setSidebarSelectedIdx(0)
              break
            case 'session':
              void openSession(item.session)
              blurSidebar()
              break
            case 'document':
              openDocument(item.doc)
              blurSidebar()
              break
            case 'scope-architecture':
              openArchitecture(item.workspace, item.tenant)
              blurSidebar()
              break
          }
          return
        }

        if (e.key === 'Escape') {
          e.preventDefault()
          e.stopPropagation()
          blurSidebar()
          const refocusId = focusedPanel === 'drawer' ? drawerTabId : activeTabId
          if (refocusId) sendTerminalCmd(refocusId, 'focus')
          return
        }
      }

      for (const s of parsed) {
        if (!matchesEvent(e, s.parsed)) continue
        e.preventDefault()
        switch (s.action) {
          case 'new_shell':
            void openShell()
            break
          case 'close_tab':
            if (focusedPanel === 'drawer' && drawerOpen) closeDrawer()
            else if (activeTabId) void closeTab(activeTabId)
            break
          case 'next_tab': {
            if (tabs.length > 1) {
              const idx  = tabs.findIndex((t) => t.id === activeTabId)
              const next = tabs[(idx + 1) % tabs.length]
              if (next) setActiveTab(next.id)
            }
            break
          }
          case 'prev_tab': {
            if (tabs.length > 1) {
              const idx  = tabs.findIndex((t) => t.id === activeTabId)
              const prev = tabs[(idx - 1 + tabs.length) % tabs.length]
              if (prev) setActiveTab(prev.id)
            }
            break
          }
          case 'toggle_drawer':
            void toggleDrawer()
            break
          case 'restart_session': {
            const activeTab = tabs.find((t) => t.id === activeTabId)
            const session   = allSessions.find((s) => s.id === activeTab?.sessionId)
            if (session) void restartSession(session)
            break
          }
          case 'duplicate_session': {
            const activeTab = tabs.find((t) => t.id === activeTabId)
            const session   = allSessions.find((s) => s.id === activeTab?.sessionId)
            if (session) void duplicateSession(session)
            break
          }
          case 'end_session': {
            const activeTab = tabs.find((t) => t.id === activeTabId)
            const session   = allSessions.find((s) => s.id === activeTab?.sessionId)
            if (session) void killSession(session)
            break
          }
          case 'focus_next_panel':
            if (drawerOpen) setFocusedPanel('drawer')
            break
          case 'focus_prev_panel':
            setFocusedPanel('main')
            break
          case 'clear':
            if (activeTabId) sendTerminalCmd(activeTabId, 'clear')
            break
          case 'scroll_up':
            if (activeTabId) sendTerminalCmd(activeTabId, 'scroll_up')
            break
          case 'scroll_down':
            if (activeTabId) sendTerminalCmd(activeTabId, 'scroll_down')
            break
          case 'toggle_sidebar':
            toggleSidebar()
            break
          case 'open_shortcuts':
            openShortcuts()
            break
          case 'open_settings':
            openSettings()
            break
          case 'toggle_theme':
            toggleTheme()
            break
          case 'nav_sessions':
            setNavView('terminal')
            break
          case 'nav_documents':
            setNavView('documents')
            break
          case 'focus_sessions':
            if (sidebarFocused) {
              blurSidebar()
              const refocusId = focusedPanel === 'drawer' ? drawerTabId : activeTabId
              if (refocusId) sendTerminalCmd(refocusId, 'focus')
            } else {
              // Compute terminal items to find the active session's position
              const terminalItems = computeSidebarItems({
                navView: 'terminal',
                scopeViewMode,
                scopePath,
                scopeTree,
                sessions,
                documents,
                selectedWorkspace: selectedWorkspace ?? null,
                archHistory,
              })
              const activeSessionId = tabs.find((t) => t.id === activeTabId)?.sessionId
              const activeIdx = activeSessionId
                ? terminalItems.findIndex((item) => item.type === 'session' && item.session.id === activeSessionId)
                : -1
              setNavView('terminal')
              setSidebarSelectedIdx(activeIdx >= 0 ? activeIdx : 0)
              ;(document.activeElement as HTMLElement)?.blur()
              focusSidebar()
            }
            break
          default:
            break
        }
        return
      }
    }

    // Capture phase: intercept before xterm.js or child elements consume the event.
    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [shortcuts, tabs, activeTabId, allSessions, selectedWorkspace,
      openShell, closeTab, setActiveTab,
      openShortcuts, openSettings, toggleTheme, toggleSidebar, setNavView,
      toggleDrawer, restartSession, duplicateSession, killSession, openSession,
      drawerOpen, focusedPanel, closeDrawer, setFocusedPanel,
      sidebarFocused, sidebarSelectedIdx, focusSidebar, blurSidebar, setSidebarSelectedIdx,
      drawerTabId, navView, documents, openDocument,
      registeredWorkspaces, setSelectedWorkspace,
      scopeViewMode, scopePath, scopeTree,
      setScopeViewMode, navigateScopeIn, navigateScopeOut, loadScopeTree,
      openLaunchPicker, openArchitecture])
}
