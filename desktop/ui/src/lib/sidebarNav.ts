import type { Session, DocEntry, ScopeTreeWorkspace } from '../types'

// ── Item types ────────────────────────────────────────────────────────────────

export type SidebarNavItem =
  | { type: 'scope-back' }
  | { type: 'scope-folder'; name: string }
  | { type: 'session'; session: Session }
  | { type: 'document'; doc: DocEntry }
  | { type: 'scope-architecture'; workspace: string; tenant: string }

// ── Scope helpers ─────────────────────────────────────────────────────────────

export function workspaceFromWorkDir(workDir: string): string | null {
  const parts = workDir.split('/').filter(Boolean)
  return parts.length >= 3 && parts[0] === 'home' ? parts[2] : null
}

// When selectedWorkspace is set, path is relative (starts at tenant).
// Compute the full [workspace, tenant, project, repo] path for lookups.
function effectivePath(path: string[], selectedWorkspace: string | null | undefined): string[] {
  return selectedWorkspace ? [selectedWorkspace, ...path] : path
}

export function getScopeChildren(
  tree: ScopeTreeWorkspace[],
  path: string[],
  selectedWorkspace?: string | null,
): string[] {
  const ep = effectivePath(path, selectedWorkspace)
  if (ep.length === 0) return tree.map((w) => w.name)
  const ws = tree.find((w) => w.name === ep[0])
  if (!ws) return []
  if (ep.length === 1) return ws.tenants.map((t) => t.name)
  const tenant = ws.tenants.find((t) => t.name === ep[1])
  if (!tenant) return []
  if (ep.length === 2) return tenant.projects.map((p) => p.name)
  const project = tenant.projects.find((p) => p.name === ep[2])
  if (!project) return []
  if (ep.length === 3) return project.repositories.map((r) => r.name)
  return []
}

export function filterSessionsByScope(
  sessions: Session[],
  path: string[],
  selectedWorkspace?: string | null,
): Session[] {
  const ep = effectivePath(path, selectedWorkspace)
  if (ep.length === 0) return sessions
  return sessions.filter((s) => {
    if (ep[0] && workspaceFromWorkDir(s.work_dir) !== ep[0]) return false
    if (ep[1] && s.tenant     !== ep[1]) return false
    if (ep[2] && s.project    !== ep[2]) return false
    if (ep[3] && s.repository !== ep[3]) return false
    return true
  })
}

export function filterDocsByScope(
  docs: DocEntry[],
  path: string[],
  selectedWorkspace?: string | null,
): DocEntry[] {
  const ep = effectivePath(path, selectedWorkspace)
  if (ep.length === 0) return docs
  return docs.filter((d) => {
    if (ep[0] && d.workspace  !== ep[0]) return false
    if (ep[1] && d.tenant     !== ep[1]) return false
    if (ep[2] && d.project    !== ep[2]) return false
    if (ep[3] && d.repository !== ep[3]) return false
    return true
  })
}

const MAX_SIDEBAR_SESSIONS = 10

export function visibleSessionsFromList(sessions: Session[]): Session[] {
  const nonHistory   = sessions.filter((s) => !s.is_history)
  const historySlots = Math.max(0, MAX_SIDEBAR_SESSIONS - nonHistory.length)
  const history      = sessions.filter((s) => !!s.is_history).slice(0, historySlots)
  return [...nonHistory, ...history]
}

// ── Unified item list ─────────────────────────────────────────────────────────

export function computeSidebarItems(params: {
  navView:           string
  scopeViewMode:     'all' | 'scope'
  scopePath:         string[]
  scopeTree:         ScopeTreeWorkspace[]
  sessions:          Session[]
  documents:         DocEntry[]
  selectedWorkspace: string | null
  archHistory:       Array<{ workspace: string; tenant: string }>
}): SidebarNavItem[] {
  const { navView, scopeViewMode, scopePath, scopeTree, sessions, documents, selectedWorkspace, archHistory } = params

  const items: SidebarNavItem[] = []

  // ── Architecture ───────────────────────────────────────────────────────────
  if (navView === 'architecture') {
    if (scopeViewMode === 'scope') {
      const ep = effectivePath(scopePath, selectedWorkspace)
      if (ep.length >= 2) {
        // Drilled to tenant level: show back + architecture action for current tenant
        items.push({ type: 'scope-back' })
        items.push({ type: 'scope-architecture', workspace: ep[0], tenant: ep[1] })
      } else {
        // Navigation level: show back + children to drill into
        if (scopePath.length > 0) items.push({ type: 'scope-back' })
        for (const name of getScopeChildren(scopeTree, scopePath, selectedWorkspace)) {
          items.push({ type: 'scope-folder', name })
        }
      }
    } else {
      // All mode: show recently opened architectures (history), filtered by selected workspace
      const filtered = selectedWorkspace
        ? archHistory.filter((h) => h.workspace === selectedWorkspace)
        : archHistory
      for (const h of filtered) {
        items.push({ type: 'scope-architecture', workspace: h.workspace, tenant: h.tenant })
      }
    }
    return items
  }

  if (navView !== 'terminal' && navView !== 'documents') return []

  // ── Sessions / Documents ───────────────────────────────────────────────────

  // Scope navigator
  if (scopeViewMode === 'scope') {
    if (scopePath.length > 0) {
      items.push({ type: 'scope-back' })
    }
    for (const name of getScopeChildren(scopeTree, scopePath, selectedWorkspace)) {
      items.push({ type: 'scope-folder', name })
    }
  }

  // Resource items
  const scopedPath = scopeViewMode === 'scope' ? scopePath : []
  if (navView === 'terminal') {
    for (const s of visibleSessionsFromList(filterSessionsByScope(sessions, scopedPath, selectedWorkspace))) {
      items.push({ type: 'session', session: s })
    }
  } else {
    for (const doc of filterDocsByScope(documents, scopedPath, selectedWorkspace)) {
      items.push({ type: 'document', doc })
    }
  }

  return items
}
