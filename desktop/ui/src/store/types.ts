import type { TabsSlice } from './slices/tabs'
import type { SessionsSlice } from './slices/sessions'
import type { UiSlice } from './slices/ui'
import type { ShortcutsSlice } from './slices/shortcuts'
import type { SettingsSlice } from './slices/settings'
import type { DocumentsSlice } from './slices/documents'
import type { ScopeSlice } from './slices/scope'
import type { ArchDrawerSlice } from './slices/archDrawer'

export type AppStore = TabsSlice & SessionsSlice & UiSlice & ShortcutsSlice & SettingsSlice & DocumentsSlice & ScopeSlice & ArchDrawerSlice
