import { useCallback, useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './store'
import { TerminalPane } from './components/Terminal'
import { TabBar } from './components/TabBar'
import { Sidebar } from './components/Sidebar'
import { TitleBar } from './components/TitleBar'

const SIDEBAR_MIN = 160
const SIDEBAR_MAX = 480
const SIDEBAR_DEFAULT = 208
const EDGE = 5 // px — resize handle hit area

const win = getCurrentWindow()

type ResizeDir =
  | 'East' | 'West' | 'North' | 'South'
  | 'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest'

const EDGES: { dir: ResizeDir; style: React.CSSProperties }[] = [
  { dir: 'North',     style: { top: 0, left: EDGE, right: EDGE, height: EDGE, cursor: 'n-resize' } },
  { dir: 'South',     style: { bottom: 0, left: EDGE, right: EDGE, height: EDGE, cursor: 's-resize' } },
  { dir: 'West',      style: { left: 0, top: EDGE, bottom: EDGE, width: EDGE, cursor: 'w-resize' } },
  { dir: 'East',      style: { right: 0, top: EDGE, bottom: EDGE, width: EDGE, cursor: 'e-resize' } },
  { dir: 'NorthWest', style: { top: 0, left: 0, width: EDGE, height: EDGE, cursor: 'nw-resize' } },
  { dir: 'NorthEast', style: { top: 0, right: 0, width: EDGE, height: EDGE, cursor: 'ne-resize' } },
  { dir: 'SouthWest', style: { bottom: 0, left: 0, width: EDGE, height: EDGE, cursor: 'sw-resize' } },
  { dir: 'SouthEast', style: { bottom: 0, right: 0, width: EDGE, height: EDGE, cursor: 'se-resize' } },
]

function ResizeHandles() {
  return (
    <>
      {EDGES.map(({ dir, style }) => (
        <div
          key={dir}
          style={{ position: 'fixed', zIndex: 9999, ...style }}
          onMouseDown={(e) => { e.preventDefault(); void win.startResizeDragging(dir) }}
        />
      ))}
    </>
  )
}

export default function App() {
  const tabs = useAppStore((s) => s.tabs)
  const activeTabId = useAppStore((s) => s.activeTabId)
  const refreshSessions = useAppStore((s) => s.refreshSessions)

  const everActiveRef = useRef<Set<string>>(new Set())
  if (activeTabId) everActiveRef.current.add(activeTabId)

  const [sidebarWidth, setSidebarWidth] = useState(SIDEBAR_DEFAULT)

  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    const startX = e.clientX
    const startWidth = sidebarWidth
    const onMove = (ev: MouseEvent) => {
      setSidebarWidth(Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, startWidth + ev.clientX - startX)))
    }
    const onUp = () => {
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }, [sidebarWidth])

  useEffect(() => {
    void refreshSessions()
    const id = setInterval(() => void refreshSessions(), 5000)
    return () => clearInterval(id)
  }, [])

  return (
    <div className="flex flex-col h-screen w-screen text-base-content overflow-hidden bg-[#0a0a0b]">
      <ResizeHandles />
      <TitleBar />

      <div className="flex flex-1 min-h-0 overflow-hidden px-1 pb-1 pt-1 gap-0.5">
        <Sidebar width={sidebarWidth} />

        <div
          className="w-px shrink-0 cursor-col-resize"
          onMouseDown={handleResizeStart}
        />

        <main className="flex-1 flex flex-col min-w-0 overflow-hidden rounded-md bg-[#141415]">
          <TabBar />

          <div className="flex-1 relative overflow-hidden border-l-2 border-r-2 border-b-2 border-[#141415] rounded-b-md">
            {tabs.length === 0 && (
              <div className="absolute inset-0 flex items-center justify-center">
                <div className="text-center space-y-2">
                  <p className="text-zinc-500 text-sm">Select a session or open a shell</p>
                  <p className="text-zinc-700 text-xs">
                    Click a session in the sidebar or{' '}
                    <span className="text-zinc-400 font-medium">+ New shell</span>
                  </p>
                </div>
              </div>
            )}

            {tabs.map((tab) => {
              const isActive = tab.id === activeTabId
              const everSeen = everActiveRef.current.has(tab.id)
              return (
                <div key={tab.id} className={`absolute inset-0 ${isActive ? '' : 'hidden'}`}>
                  {everSeen && <TerminalPane tabId={tab.id} active={isActive} />}
                </div>
              )
            })}
          </div>
        </main>
      </div>
    </div>
  )
}
