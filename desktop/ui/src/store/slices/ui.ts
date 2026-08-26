import type { StateCreator } from 'zustand'
import type { NavView, Theme } from '../../types'
import type { AppStore } from '../types'
import { tauriService } from '../../services/tauri'

// Exported so store.ts can call it from onRehydrateStorage
export function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark')
}

export type FocusedPanel = 'main' | 'drawer'

export interface UiSlice {
  navView:              NavView
  theme:                Theme
  sidebarHidden:        boolean
  drawerOpen:           boolean
  drawerTabId:          string | null
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
    const { drawerOpen, drawerTabId } = get()
    if (drawerOpen) {
      set({ drawerOpen: false, focusedPanel: 'main' })
      return
    }
    let tabId = drawerTabId
    if (!tabId) {
      tabId = await tauriService.ptyOpen(null)
      set({ drawerTabId: tabId })
    }
    set({ drawerOpen: true, focusedPanel: 'drawer' })
  },

  closeDrawer: () => set({ drawerOpen: false, focusedPanel: 'main' }),

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
