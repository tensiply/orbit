import { useAppStore, Session, NavView } from '../store'

const NAV_ITEMS: { label: string; view: NavView }[] = [
  { label: 'Tasks', view: 'tasks' },
  { label: 'Plans', view: 'plans' },
  { label: 'Plugins', view: 'plugins' },
  { label: 'MCPs', view: 'mcps' },
  { label: 'Activity', view: 'activity' },
]

export function Sidebar({ width }: { width: number }) {
  const sessions = useAppStore((s) => s.sessions)
  const sessionsLoading = useAppStore((s) => s.sessionsLoading)
  const navView = useAppStore((s) => s.navView)
  const openSession = useAppStore((s) => s.openSession)
  const openShell = useAppStore((s) => s.openShell)
  const setNavView = useAppStore((s) => s.setNavView)

  return (
    <aside
      style={{ width, minWidth: width }}
      className="flex flex-col shrink-0 select-none bg-[#141415] rounded-md overflow-hidden"
    >
      <div className="flex items-center py-2 px-3 shrink-0">
        <span className="text-xs font-medium text-[#6b6b75] uppercase tracking-wider flex-1">
          Sessions
        </span>
        {sessionsLoading && (
          <span className="loading loading-spinner loading-xs text-[#6b6b75]" />
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-1 min-h-0">
        <ul className="menu menu-xs p-0 w-full">
          {sessions.length === 0 && !sessionsLoading && (
            <li className="menu-title text-[#6b6b75] italic font-normal normal-case">
              No active sessions
            </li>
          )}
          {sessions.map((s) => (
            <SessionItem key={s.id} session={s} onOpen={() => openSession(s)} />
          ))}
        </ul>
      </div>

      <div className="flex flex-col items-stretch gap-1 p-2 shrink-0">
        <button className="btn btn-soft btn-block btn-sm" onClick={openShell}>
          + New shell
        </button>
        <ul className="menu menu-xs p-0">
          {NAV_ITEMS.map(({ label, view }) => (
            <li key={view}>
              <a
                onClick={() => setNavView(view)}
                className={navView === view ? 'menu-active' : ''}
              >
                {label}
              </a>
            </li>
          ))}
        </ul>
      </div>
    </aside>
  )
}


function SessionItem({ session, onOpen }: { session: Session; onOpen: () => void }) {
  const scope = session.repository || session.project || session.tenant
  const engineShort = session.engine.toLowerCase()
  const subtitle = `[${engineShort}] ${scope}`

  return (
    <li>
      <a onClick={onOpen} className="block py-2 px-2 rounded-lg">
        <div className="flex flex-col gap-0.5 w-full overflow-hidden">
          <span className="text-xs font-medium leading-snug truncate">{scope}</span>
          <span className="text-[10px] text-base-content/50 leading-tight truncate">{subtitle}</span>
          <span className="text-[10px] text-base-content/35 font-mono leading-tight truncate">{session.work_dir}</span>
        </div>
      </a>
    </li>
  )
}
