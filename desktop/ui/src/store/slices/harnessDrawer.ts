import type { StateCreator } from 'zustand'
import type { Session, HarnessReport } from '../../types'
import type { AppStore } from '../types'
import { tauriService } from '../../services/tauri'
import { workspaceFromWorkDir } from '../../lib/sidebarNav'

export interface HarnessDrawerSlice {
  harnessDrawerOpen:  boolean
  harnessSession:     Session | null
  harnessLabel:       string | null
  harnessReport:      HarnessReport | null
  harnessLoading:     boolean
  harnessError:       string | null

  openHarnessDrawer:         (session: Session) => Promise<void>
  openHarnessDrawerForScope: (workspace: string, tenant: string | null, project: string | null, repository: string | null, engine: string) => Promise<void>
  closeHarnessDrawer:        () => void
}

export const createHarnessDrawerSlice: StateCreator<AppStore, [], [], HarnessDrawerSlice> = (set) => ({
  harnessDrawerOpen:  false,
  harnessSession:     null,
  harnessLabel:       null,
  harnessReport:      null,
  harnessLoading:     false,
  harnessError:       null,

  openHarnessDrawer: async (session: Session) => {
    const label = [session.tenant, session.project, session.repository].filter(Boolean).join(' › ') || session.work_dir
    set({ harnessDrawerOpen: true, harnessSession: session, harnessLabel: label, harnessLoading: true, harnessError: null, harnessReport: null })
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

  openHarnessDrawerForScope: async (workspace, tenant, project, repository, engine) => {
    const label = [tenant, project, repository].filter(Boolean).join(' › ') || workspace
    set({ harnessDrawerOpen: true, harnessSession: null, harnessLabel: label, harnessLoading: true, harnessError: null, harnessReport: null })
    try {
      const report = await tauriService.sessionHarness(workspace, tenant, project, repository, engine)
      set({ harnessReport: report, harnessLoading: false })
    } catch (e) {
      set({ harnessError: String(e), harnessLoading: false })
    }
  },

  closeHarnessDrawer: () =>
    set({ harnessDrawerOpen: false, harnessSession: null, harnessLabel: null, harnessReport: null, harnessLoading: false, harnessError: null }),
})
