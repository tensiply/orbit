import { invoke } from '@tauri-apps/api/core'
import type {
  Session, LaunchScope, LaunchedInfo, WorkspaceInfo, ScopeTreeWorkspace,
  ArchCatalogDto, ArchEntityDto, SaveEntityArgs, ArchLayout, ArchRoutes,
  HarnessReport,
} from '../types'

export const tauriService = {
  ptyOpen: (tmuxSession: string | null): Promise<string> =>
    invoke('pty_open', { tmuxSession }),

  ptyClose: (tabId: string): Promise<void> =>
    invoke('pty_close', { tabId }),

  ptyWrite: (tabId: string, data: string): Promise<void> =>
    invoke('pty_write', { tabId, data }),

  ptyResize: (tabId: string, cols: number, rows: number): Promise<void> =>
    invoke('pty_resize', { tabId, cols, rows }),

  listSessions: (): Promise<Session[]> =>
    invoke('session_list'),

  sessionHarness: (
    workspace: string | null,
    tenant: string | null,
    project: string | null,
    repository: string | null,
    engine: string,
  ): Promise<HarnessReport> =>
    invoke('session_harness', { workspace, tenant, project, repository, engine }),

  sessionKill: (id: string): Promise<void> =>
    invoke('session_kill', { id }),

  getSessionTitle: (workDir: string, startedAt: number): Promise<string | null> =>
    invoke('get_session_title', { workDir, startedAt }),

  sessionLaunch: (scope: LaunchScope): Promise<LaunchedInfo> =>
    invoke('session_launch', { scope }),

  listWorkspaces: (): Promise<WorkspaceInfo[]> =>
    invoke('workspace_list'),

  scopeTree: (): Promise<ScopeTreeWorkspace[]> =>
    invoke('scope_tree'),

  architectureLoad: (workspace: string, tenant: string): Promise<ArchCatalogDto> =>
    invoke('architecture_load', { workspace, tenant }),

  architectureSaveEntity: (args: SaveEntityArgs): Promise<ArchEntityDto> =>
    invoke('architecture_save_entity', { args }),

  architectureDeleteEntity: (workspace: string, tenant: string, kindFolder: string, id: string): Promise<void> =>
    invoke('architecture_delete_entity', { workspace, tenant, kindFolder, id }),

  architectureLoadLayout: (catalogPath: string): Promise<ArchLayout> =>
    invoke('architecture_load_layout', { catalogPath }),

  architectureSaveLayout: (catalogPath: string, positions: ArchLayout): Promise<void> =>
    invoke('architecture_save_layout', { catalogPath, positions }),

  architectureLoadRoutes: (catalogPath: string): Promise<ArchRoutes> =>
    invoke('architecture_load_routes', { catalogPath }),

  architectureSaveRoutes: (catalogPath: string, routes: ArchRoutes): Promise<void> =>
    invoke('architecture_save_routes', { catalogPath, routes }),
}
