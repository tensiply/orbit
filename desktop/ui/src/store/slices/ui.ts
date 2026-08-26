import type { StateCreator } from 'zustand'
import type { NavView, Theme } from '../../types'
import type { AppStore } from '../types'
import { tauriService } from '../../services/tauri'

// Exported so store.ts can call it from onRehydrateStorage
export function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark')
}

export type FocusedPanel = 'main' | 'drawer'

export interface TabDrawerEntry {
  ptyId: string
  open:  boolean
}

export interface UiSlice {
  navView:              NavView
  theme:                Theme
  sidebarHidden:        boolean
  drawerOpen:           boolean
  drawerTabId:          string | null
  /** Per-tab drawer registry: main tab id → { ptyId, open } */
  tabDrawers:           Record<string, TabDrawerEntry>
  focusedPanel:         FocusedPanel
  sidebarFocused:       boolean
  sidebarSelectedIdx:   number
  selectedWorkspace:    string | null
  previousWorkspace:    string | null
  launchPickerOpen:     boolean
  launchPickerScopePath: string[] | null

  setNavView:            (view: NavView) => void
  toggleTheme:           () => void
  toggleSidebar:         () => void
  toggleDrawer:          () => Promise<void>
  closeDrawer:           () => void
  /** Sync drawerOpen + drawerTabId to the given tab's entry in tabDrawers. */
  syncDrawerToTab:       (tabId: string | null) => void
  /** Close and remove the drawer PTY for a tab (called when closing a tab). */
  closeTabDrawer:        (tabId: string) => Promise<void>
  setFocusedPanel:       (panel: FocusedPanel) => void
  focusSidebar:          () => void
  blurSidebar:           () => void
  setSidebarSelectedIdx: (idx: number) => void
  setSelectedWorkspace:  (ws: string | null) => void
  openLaunchPicker:      (scopePath: string[]) => void
  closeLaunchPicker:     () => void
}

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set, get) => ({
  navView:               'terminal',
  theme:                 'dark',
  sidebarHidden:         false,
  drawerOpen:            false,
  drawerTabId:           null,
  tabDrawers:            {},
  focusedPanel:          'main',
  sidebarFocused:        false,
  sidebarSelectedIdx:    0,
  selectedWorkspace:     null,
  previousWorkspace:     null,
  launchPickerOpen:      false,
  launchPickerScopePath: null,

  setNavView: (navView: NavView) => {
    set({ navView, sidebarFocused: false })
  },

  toggleTheme: () => {
    const next: Theme = get().theme === 'dark' ? 'light' : 'dark'
    applyTheme(next)
    set({ theme: next })
  },

  toggleSidebar: () => set((s) => ({ sidebarHidden: !s.sidebarHidden })),

  toggleDrawer: async () => {
    const activeTabId = get().activeTabId
    if (!activeTabId) return

    const { tabDrawers } = get()
    const existing = tabDrawers[activeTabId]

    if (existing?.open) {
      set((s) => ({
        drawerOpen: false,
        focusedPanel: 'main',
        tabDrawers: { ...s.tabDrawers, [activeTabId]: { ...s.tabDrawers[activeTabId], open: false } },
      }))
      return
    }

    let ptyId = existing?.ptyId
    if (!ptyId) {
      ptyId = await tauriService.ptyOpen(null)
    }
    set((s) => ({
      drawerOpen: true,
      drawerTabId: ptyId!,
      focusedPanel: 'drawer',
      tabDrawers: { ...s.tabDrawers, [activeTabId]: { ptyId: ptyId!, open: true } },
    }))
  },

  closeDrawer: () => {
    const activeTabId = get().activeTabId
    set((s) => ({
      drawerOpen: false,
      focusedPanel: 'main',
      ...(activeTabId && s.tabDrawers[activeTabId]
        ? { tabDrawers: { ...s.tabDrawers, [activeTabId]: { ...s.tabDrawers[activeTabId], open: false } } }
        : {}),
    }))
  },

  syncDrawerToTab: (tabId: string | null) => {
    const { tabDrawers, focusedPanel } = get()
    if (!tabId) {
      set({ drawerOpen: false, drawerTabId: null, focusedPanel: 'main' })
      return
    }
    const d = tabDrawers[tabId]
    set({
      drawerOpen:  d?.open  ?? false,
      drawerTabId: d?.ptyId ?? null,
      ...(!d?.open && focusedPanel === 'drawer' ? { focusedPanel: 'main' } : {}),
    })
  },

  closeTabDrawer: async (tabId: string) => {
    const { tabDrawers } = get()
    const d = tabDrawers[tabId]
    if (!d) return
    await tauriService.ptyClose(d.ptyId).catch(console.error)
    set((s) => {
      const { [tabId]: _removed, ...rest } = s.tabDrawers
      return { tabDrawers: rest }
    })
  },

  setFocusedPanel: (focusedPanel) => set({ focusedPanel }),

  focusSidebar: () => set({ sidebarHidden: false, sidebarFocused: true, sidebarSelectedIdx: 0 }),
  blurSidebar:  () => set({ sidebarFocused: false }),
  setSidebarSelectedIdx: (idx) => set({ sidebarSelectedIdx: idx }),

  setSelectedWorkspace: (ws) => {
    const { selectedWorkspace } = get()
    if (ws === selectedWorkspace) return
    set({ previousWorkspace: selectedWorkspace, selectedWorkspace: ws })
  },

  openLaunchPicker:  (scopePath) => set({ launchPickerOpen: true,  launchPickerScopePath: scopePath }),
  closeLaunchPicker: ()          => set({ launchPickerOpen: false, launchPickerScopePath: null }),
})
