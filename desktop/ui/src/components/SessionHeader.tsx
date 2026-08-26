import { useAppStore } from '../store'
import type { Session } from '../types'
import {
  ClaudeEngineIcon,
  GeminiEngineIcon,
  OpenCodeEngineIcon,
  DefaultEngineIcon,
} from '../icons'

function workspaceFromWorkDir(workDir: string): string | null {
  const parts = workDir.split('/').filter(Boolean)
  return parts.length >= 3 && parts[0] === 'home' ? parts[2] : null
}

function fullScopeParts(session: Session): string[] {
  const parts: string[] = []
  const ws = workspaceFromWorkDir(session.work_dir)
  if (ws)                   parts.push(ws)
  if (session.tenant)       parts.push(session.tenant)
  if (session.project)      parts.push(session.project)
  if (session.repository)   parts.push(session.repository)
  return parts
}

function EngineIcon({ engine, size = 11 }: { engine: string; size?: number }) {
  switch (engine) {
    case 'claude':   return <ClaudeEngineIcon size={size} />
    case 'gemini':   return <GeminiEngineIcon size={size} />
    case 'opencode': return <OpenCodeEngineIcon size={size} />
    default:         return <DefaultEngineIcon size={size} />
  }
}

export function SessionHeader() {
  const tabs        = useAppStore((s) => s.tabs)
  const activeTabId = useAppStore((s) => s.activeTabId)
  const sessions    = useAppStore((s) => s.sessions)

  const activeTab = tabs.find((t) => t.id === activeTabId)
  if (!activeTab || activeTab.type !== 'terminal' || !activeTab.sessionId) return null

  const session = sessions.find((s) => s.id === activeTab.sessionId)
  if (!session) return null

  const engine = session.engine.toLowerCase()
  const parts  = fullScopeParts(session)

  return (
    <div data-orbit-zone="orbit.desktop.principal.card.session-header" className="flex items-center gap-3 px-4 h-8 shrink-0 border-b border-sidebar-border/40 bg-card">
      {/* Engine icon */}
      <span className="text-foreground/25 shrink-0">
        <EngineIcon engine={engine} size={11} />
      </span>

      {/* Scope breadcrumb */}
      <div className="flex items-center gap-0.5 flex-1 min-w-0 overflow-hidden">
        {session.global_mode ? (
          <span className="text-[10px] font-medium text-foreground/40">Global</span>
        ) : parts.length > 0 ? (
          parts.map((part, i) => (
            <span key={i} className="flex items-center gap-0.5 shrink-0">
              {i > 0 && (
                <span className="text-[10px] text-foreground/18 mx-0.5">›</span>
              )}
              <span
                className={`text-[10px] font-medium ${
                  i === parts.length - 1 ? 'text-foreground/55' : 'text-foreground/28'
                }`}
              >
                {part}
              </span>
            </span>
          ))
        ) : (
          <span className="text-[10px] text-foreground/25">—</span>
        )}
      </div>

    </div>
  )
}
