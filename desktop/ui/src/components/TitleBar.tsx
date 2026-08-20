import { getCurrentWindow } from '@tauri-apps/api/window'

const win = getCurrentWindow()

function TitleBtn({
  onClick,
  title,
  danger,
  children,
}: {
  onClick: () => void
  title: string
  danger?: boolean
  children: React.ReactNode
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`h-full w-11 flex items-center justify-center text-base-content/40 transition-colors ${
        danger
          ? 'hover:bg-error hover:text-error-content'
          : 'hover:bg-white/10 hover:text-base-content'
      }`}
    >
      {children}
    </button>
  )
}

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      className="h-7 bg-transparent flex items-center select-none shrink-0"
    >
      <span className="text-xs text-base-content/25 flex-1 pl-3 pointer-events-none select-none">
        orbit
      </span>

      <TitleBtn onClick={() => win.minimize()} title="Minimize">
        <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round">
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
      </TitleBtn>

      <TitleBtn onClick={() => win.toggleMaximize()} title="Maximize">
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="1" />
        </svg>
      </TitleBtn>

      <TitleBtn onClick={() => win.close()} title="Close" danger>
        <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </TitleBtn>
    </div>
  )
}
