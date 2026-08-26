import type { StateCreator } from 'zustand'
import type { Session, WorkspaceInfo } from '../../types'
import { tauriService } from '../../services/tauri'
import type { AppStore } from '../types'

const SESSIONS_CACHE_KEY = 'orbit-sessions-cache'
const TITLES_CACHE_KEY   = 'orbit-titles-cache'

function loadCached<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : null
  } catch {
    return null
  }
}

export interface SessionsSlice {
  sessions: Session[]
  sessionsLoading: boolean
  sessionTitles: Record<string, string>
  blankSessions: string[]
  registeredWorkspaces: WorkspaceInfo[]

  setSessions: (sessions: Session[]) => void
  refreshSessions: () => Promise<void>
  markSessionBlank: (id: string) => void
  loadWorkspaces: () => Promise<void>
}

export const createSessionsSlice: StateCreator<AppStore, [], [], SessionsSlice> = (set, get) => ({
  sessions:             loadCached<Session[]>(SESSIONS_CACHE_KEY) ?? [],
  sessionsLoading:      false,
  sessionTitles:        loadCached<Record<string, string>>(TITLES_CACHE_KEY) ?? {},
  blankSessions:        [],
  registeredWorkspaces: [],

  setSessions: (sessions: Session[]) => set({ sessions }),

  markSessionBlank: (id: string) =>
    set((s) => ({ blankSessions: [...s.blankSessions, id] })),

  loadWorkspaces: async () => {
    try {
      const workspaces = await tauriService.listWorkspaces()
      set({ registeredWorkspaces: workspaces })
    } catch {
      // registry not available — leave empty, fall back to session-derived workspaces
    }
  },

  refreshSessions: async () => {
    set({ sessionsLoading: true })
    try {
      const sessions = await tauriService.listSessions()
      localStorage.setItem(SESSIONS_CACHE_KEY, JSON.stringify(sessions))
      set({ sessions, sessionsLoading: false })

      const knownTitles = get().sessionTitles
      // Always re-fetch active sessions (title updates as conversation progresses);
      // skip only history sessions that already have a cached title.
      const needTitle = sessions.filter((s) => !s.is_history || !knownTitles[s.id])
      if (needTitle.length === 0) return

      const entries = await Promise.all(
        needTitle.map(async (s) => {
          try {
            const title = await tauriService.getSessionTitle(s.work_dir, s.started_at)
            return title ? ([s.id, title] as [string, string]) : null
          } catch {
            return null
          }
        }),
      )

      const newTitles: Record<string, string> = { ...knownTitles }
      for (const e of entries) {
        if (e) newTitles[e[0]] = e[1]
      }
      localStorage.setItem(TITLES_CACHE_KEY, JSON.stringify(newTitles))
      set({ sessionTitles: newTitles })

      // Clear blank sessions that now have a title
      const currentBlank = get().blankSessions
      if (currentBlank.length > 0) {
        const stillBlank = currentBlank.filter((id) => !newTitles[id])
        if (stillBlank.length !== currentBlank.length) {
          set({ blankSessions: stillBlank })
        }
      }
    } catch {
      set({ sessionsLoading: false })
    }
  },
})
