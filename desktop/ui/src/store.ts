import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import { createTabsSlice } from './store/slices/tabs'
import { createSessionsSlice } from './store/slices/sessions'
import { createUiSlice, applyTheme } from './store/slices/ui'
import { createShortcutsSlice } from './store/slices/shortcuts'
import { createSettingsSlice } from './store/slices/settings'
import { createDocumentsSlice } from './store/slices/documents'
import { createScopeSlice } from './store/slices/scope'
import { createArchDrawerSlice } from './store/slices/archDrawer'
import type { AppStore } from './store/types'

export const useAppStore = create<AppStore>()(
  persist(
    (...a) => ({
      ...createTabsSlice(...a),
      ...createSessionsSlice(...a),
      ...createUiSlice(...a),
      ...createShortcutsSlice(...a),
      ...createSettingsSlice(...a),
      ...createDocumentsSlice(...a),
      ...createScopeSlice(...a),
      ...createArchDrawerSlice(...a),
    }),
    {
      name: 'orbit-ui-prefs',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        theme:             state.theme,
        selectedWorkspace: state.selectedWorkspace,
        previousWorkspace: state.previousWorkspace,
        scopeViewMode:     state.scopeViewMode,
        archHistory:       state.archHistory,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) return
        applyTheme(state.theme)
      },
    },
  ),
)

// Re-export types so existing component imports continue to work
export type { AppStore } from './store/types'
export type { Session, Tab, NavView, Theme, Shortcut, ShortcutCategory, Setting, SettingCategory, DocEntry, WorkspaceInfo, ScopeTreeWorkspace } from './types'
