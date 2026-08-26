import type { StateCreator } from 'zustand'
import type { Tab, Session } from '../../types'
import { tauriService } from '../../services/tauri'
import { sendTerminalCmd } from '../../lib/terminalBus'
import type { AppStore } from '../types'

function workspaceFromWorkDir(workDir: string): string | null {
  const parts = workDir.split('/').filter(Boolean)
  return parts.length >= 3 && parts[0] === 'home' ? parts[2] : null
}

export interface TabsSlice {
  tabs: Tab[]
  activeTabId: string | null
  archHistory: Array<{ workspace: string; tenant: string }>

  openShell: () => Promise<void>
  openSession: (session: Session) => Promise<void>
  openShortcuts: () => void
  openUIKit: () => void
  openColors: () => void
  openUIMap: () => void
  openSettings: () => void
  openArchitecture: (workspace: string, tenant: string) => void
  closeTab: (tabId: string) => Promise<void>
  setActiveTab: (tabId: string) => void
  killSession: (session: Session) => Promise<void>
  duplicateSession: (session: Session) => Promise<void>
  restartSession: (session: Session) => Promise<void>
  launchScopeSession: (scopePath: string[], engine?: string) => Promise<void>
}

export const createTabsSlice: StateCreator<AppStore, [], [], TabsSlice> = (set, get) => {
  // Sync drawer state whenever the active tab changes.
  const syncActive = (tabId: string | null) => get().syncDrawerToTab(tabId)

  return {
    tabs: [],
    activeTabId: null,
    archHistory: [],

    openShell: async () => {
      const tabId = await tauriService.ptyOpen(null)
      const tab: Tab = { id: tabId, title: 'shell', type: 'terminal' }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tabId, navView: 'terminal' }))
      syncActive(tabId)
    },

    openSession: async (session: Session) => {
      const existing = get().tabs.find((t) => t.sessionId === session.id)
      if (existing) {
        localStorage.setItem('orbit-last-session', session.id)
        set({ activeTabId: existing.id, navView: 'terminal', focusedPanel: 'main' })
        syncActive(existing.id)
        // Force DOM focus on the terminal — panel may already have been 'main'
        // so the panelFocused effect in TerminalPane wouldn't re-fire without this.
        requestAnimationFrame(() => sendTerminalCmd(existing.id, 'focus'))
        return
      }

      const label = session.repository || session.project || session.tenant

      if (session.is_history) {
        // History sessions have a dead tmux session — re-launch orbit at the same scope.
        const launched = await tauriService.sessionLaunch({
          workspace:  workspaceFromWorkDir(session.work_dir),
          tenant:     session.tenant     || null,
          project:    session.project    || null,
          repository: session.repository || null,
          engine:     session.engine,
          new_session: true,
        })
        get().markSessionBlank(launched.session_id)
        const tabId = await tauriService.ptyOpen(launched.tmux_name)
        const tab: Tab = {
          id:          tabId,
          title:       label,
          type:        'terminal',
          sessionId:   launched.session_id,
          tmuxSession: launched.tmux_name,
        }
        localStorage.setItem('orbit-last-session', launched.session_id)
        set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tabId, navView: 'terminal', focusedPanel: 'main' }))
        syncActive(tabId)
        return
      }

      const tabId = await tauriService.ptyOpen(session.tmux_session ?? null)
      const tab: Tab = {
        id:          tabId,
        title:       label,
        type:        'terminal',
        sessionId:   session.id,
        tmuxSession: session.tmux_session,
      }
      localStorage.setItem('orbit-last-session', session.id)
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tabId, navView: 'terminal', focusedPanel: 'main' }))
      syncActive(tabId)
    },

    openShortcuts: () => {
      const existing = get().tabs.find((t) => t.type === 'shortcuts')
      if (existing) { set({ activeTabId: existing.id }); syncActive(existing.id); return }
      const tab: Tab = { id: 'shortcuts', title: 'Shortcuts', type: 'shortcuts' }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: 'shortcuts' }))
      syncActive('shortcuts')
    },

    openUIKit: () => {
      const existing = get().tabs.find((t) => t.type === 'uikit')
      if (existing) { set({ activeTabId: existing.id }); syncActive(existing.id); return }
      const tab: Tab = { id: 'uikit', title: 'UI Kit', type: 'uikit' }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: 'uikit' }))
      syncActive('uikit')
    },

    openColors: () => {
      const existing = get().tabs.find((t) => t.type === 'colors')
      if (existing) { set({ activeTabId: existing.id }); syncActive(existing.id); return }
      const tab: Tab = { id: 'colors', title: 'Colors', type: 'colors' }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: 'colors' }))
      syncActive('colors')
    },

    openUIMap: () => {
      const existing = get().tabs.find((t) => t.type === 'ui-map')
      if (existing) { set({ activeTabId: existing.id }); syncActive(existing.id); return }
      const tab: Tab = { id: 'ui-map', title: 'UI Map', type: 'ui-map' }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: 'ui-map' }))
      syncActive('ui-map')
    },

    openSettings: () => {
      const existing = get().tabs.find((t) => t.type === 'settings')
      if (existing) { set({ activeTabId: existing.id }); syncActive(existing.id); return }
      const tab: Tab = { id: 'settings', title: 'Settings', type: 'settings' }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: 'settings' }))
      syncActive('settings')
    },

    openArchitecture: (workspace: string, tenant: string) => {
      const tabId = `arch-${workspace}-${tenant}`
      const existing = get().tabs.find((t) => t.id === tabId)
      const prev = get().archHistory.filter((h) => !(h.workspace === workspace && h.tenant === tenant))
      const archHistory = [{ workspace, tenant }, ...prev].slice(0, 10)
      if (existing) {
        set({ activeTabId: tabId, navView: 'architecture', archHistory })
        syncActive(tabId)
        return
      }
      const tab: Tab = {
        id: tabId,
        title: `${tenant} architecture`,
        type: 'architecture',
        archWorkspace: workspace,
        archTenant: tenant,
      }
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tabId, navView: 'architecture', archHistory }))
      syncActive(tabId)
    },

    closeTab: async (tabId: string) => {
      const tab = get().tabs.find((t) => t.id === tabId)
      if (!tab) return
      if (tab.type === 'terminal' || tab.type == null) {
        await tauriService.ptyClose(tabId).catch(console.error)
      }
      await get().closeTabDrawer(tabId)
      set((state) => {
        const tabs = state.tabs.filter((t) => t.id !== tabId)
        let activeTabId = state.activeTabId
        if (activeTabId === tabId) {
          const idx = state.tabs.findIndex((t) => t.id === tabId)
          activeTabId = tabs[Math.max(0, idx - 1)]?.id ?? null
        }
        return { tabs, activeTabId }
      })
      syncActive(get().activeTabId)
    },

    setActiveTab: (tabId: string) => {
      const tab = get().tabs.find((t) => t.id === tabId)
      if (tab?.sessionId) {
        localStorage.setItem('orbit-last-session', tab.sessionId)
      }
      const navView =
        tab?.type === 'document'     ? 'documents' as const :
        tab?.type === 'architecture' ? 'architecture' as const :
        tab?.type === 'settings'     ? 'settings' as const :
                                       'terminal' as const
      set({ activeTabId: tabId, navView })
      syncActive(tabId)
    },

    killSession: async (session: Session) => {
      await tauriService.sessionKill(session.id).catch(console.error)
      const tab = get().tabs.find((t) => t.sessionId === session.id)
      if (tab) await get().closeTab(tab.id)
      await get().refreshSessions()
    },

    duplicateSession: async (session: Session) => {
      const ws = workspaceFromWorkDir(session.work_dir)
      const launched = await tauriService.sessionLaunch({
        workspace:   ws,
        tenant:      session.tenant     || null,
        project:     session.project    || null,
        repository:  session.repository || null,
        engine:      session.engine,
        new_session: true,
      })
      const label = session.repository || session.project || session.tenant || 'shell'
      const tabId = await tauriService.ptyOpen(launched.tmux_name)
      const tab: Tab = {
        id:          tabId,
        title:       label,
        type:        'terminal',
        sessionId:   launched.session_id,
        tmuxSession: launched.tmux_name,
      }
      localStorage.setItem('orbit-last-session', launched.session_id)
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tabId, navView: 'terminal' }))
      syncActive(tabId)
      await get().refreshSessions()
    },

    launchScopeSession: async (scopePath: string[], engine = 'claude') => {
      const [workspace, tenant, project, repository] = scopePath
      const launched = await tauriService.sessionLaunch({
        workspace:   workspace   ?? null,
        tenant:      tenant      ?? null,
        project:     project     ?? null,
        repository:  repository  ?? null,
        engine,
        new_session: true,
      })
      const label = repository || project || tenant || workspace || 'shell'
      get().markSessionBlank(launched.session_id)
      const tabId = await tauriService.ptyOpen(launched.tmux_name)
      const tab: Tab = {
        id:          tabId,
        title:       label,
        type:        'terminal',
        sessionId:   launched.session_id,
        tmuxSession: launched.tmux_name,
      }
      localStorage.setItem('orbit-last-session', launched.session_id)
      set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tabId, navView: 'terminal', focusedPanel: 'main' }))
      syncActive(tabId)
      await get().refreshSessions()
    },

    restartSession: async (session: Session) => {
      await tauriService.sessionKill(session.id).catch(console.error)
      const tab = get().tabs.find((t) => t.sessionId === session.id)
      if (tab) {
        await tauriService.ptyClose(tab.id).catch(console.error)
        set((state) => {
          const tabs = state.tabs.filter((t) => t.id !== tab.id)
          const activeTabId = state.activeTabId === tab.id
            ? (tabs[0]?.id ?? null)
            : state.activeTabId
          return { tabs, activeTabId }
        })
        syncActive(get().activeTabId)
      }
      const ws = workspaceFromWorkDir(session.work_dir)
      const launched = await tauriService.sessionLaunch({
        workspace:   ws,
        tenant:      session.tenant     || null,
        project:     session.project    || null,
        repository:  session.repository || null,
        engine:      session.engine,
        new_session: true,
      })
      const label = session.repository || session.project || session.tenant || 'shell'
      const tabId = await tauriService.ptyOpen(launched.tmux_name)
      const newTab: Tab = {
        id:          tabId,
        title:       label,
        type:        'terminal',
        sessionId:   launched.session_id,
        tmuxSession: launched.tmux_name,
      }
      localStorage.setItem('orbit-last-session', launched.session_id)
      set((state) => ({ tabs: [...state.tabs, newTab], activeTabId: tabId, navView: 'terminal' }))
      syncActive(tabId)
      await get().refreshSessions()
    },
  }
}
