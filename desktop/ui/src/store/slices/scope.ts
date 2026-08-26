import type { StateCreator } from 'zustand'
import type { AppStore } from '../types'
import type { ScopeTreeWorkspace } from '../../types'
import { tauriService } from '../../services/tauri'

export interface ScopeSlice {
  scopeTree:        ScopeTreeWorkspace[]
  scopeTreeLoading: boolean
  scopePath:        string[]
  scopeViewMode:    'all' | 'scope'

  loadScopeTree:   () => Promise<void>
  navigateScopeIn: (name: string) => void
  navigateScopeOut: () => void
  setScopeViewMode: (mode: 'all' | 'scope') => void
  setScopePath:    (path: string[]) => void
}

export const createScopeSlice: StateCreator<AppStore, [], [], ScopeSlice> = (set, get) => ({
  scopeTree:        [],
  scopeTreeLoading: false,
  scopePath:        [],
  scopeViewMode:    'all',

  loadScopeTree: async () => {
    if (get().scopeTreeLoading) return
    set({ scopeTreeLoading: true })
    try {
      const tree = await tauriService.scopeTree()
      set({ scopeTree: tree, scopeTreeLoading: false })
    } catch {
      set({ scopeTreeLoading: false })
    }
  },

  navigateScopeIn:  (name) => set((s) => ({ scopePath: [...s.scopePath, name] })),
  navigateScopeOut: ()     => set((s) => ({ scopePath: s.scopePath.slice(0, -1) })),
  setScopeViewMode: (mode) => set({ scopeViewMode: mode }),
  setScopePath:     (path) => set({ scopePath: path }),
})
