import type { StateCreator } from 'zustand'
import type { Session, HarnessReport } from '../../types'
import type { AppStore } from '../types'
import { tauriService } from '../../services/tauri'
import { workspaceFromWorkDir } from '../../lib/sidebarNav'

export interface HarnessDrawerSlice {
  harnessDrawerOpen:  boolean
  harnessSession:     Session | null
  harnessReport:      HarnessReport | null
  harnessLoading:     boolean
  harnessError:       string | null

  openHarnessDrawer:  (session: Session) => Promise<void>
  closeHarnessDrawer: () => void
}

export const createHarnessDrawerSlice: StateCreator<AppStore, [], [], HarnessDrawerSlice> = (set) => ({
  harnessDrawerOpen:  false,
  harnessSession:     null,
  harnessReport:      null,
  harnessLoading:     false,
  harnessError:       null,

  openHarnessDrawer: async (session: Session) => {
    set({ harnessDrawerOpen: true, harnessSession: session, harnessLoading: true, harnessError: null, harnessReport: null })
    try {
      const workspace = workspaceFromWorkDir(session.work_dir)
      const report = await tauriService.sessionHarness(
        workspace,
        session.tenant || null,
        session.project || null,
        session.repository || null,
        session.engine,
      )
      set({ harnessReport: report, harnessLoading: false })
    } catch (e) {
      set({ harnessError: String(e), harnessLoading: false })
    }
  },

  closeHarnessDrawer: () =>
    set({ harnessDrawerOpen: false, harnessSession: null, harnessReport: null, harnessLoading: false, harnessError: null }),
})
