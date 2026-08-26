import { X } from 'lucide-react'
import { useState, useEffect } from 'react'
import { useAppStore } from '../store'
import { TerminalPane } from './Terminal'

export function TerminalDrawer() {
  const drawerOpen    = useAppStore((s) => s.drawerOpen)
  const drawerTabId   = useAppStore((s) => s.drawerTabId)
  const closeDrawer   = useAppStore((s) => s.closeDrawer)
  const focusedPanel  = useAppStore((s) => s.focusedPanel)
  const [mounted,     setMounted]     = useState(false)
  const [visible,     setVisible]     = useState(false)
  const [termMounted, setTermMounted] = useState(false)

  useEffect(() => {
    if (drawerOpen) {
      setMounted(true)
      requestAnimationFrame(() => requestAnimationFrame(() => setVisible(true)))
      // Mount the terminal only after the CSS transition (200ms) so xterm's
      // ResizeObserver never sees intermediate widths and fit() runs once at
      // the final size — eliminating the "lines jumping" jank on open.
      const t = setTimeout(() => setTermMounted(true), 210)
      return () => clearTimeout(t)
    } else {
      setVisible(false)
      setTermMounted(false)
      const t = setTimeout(() => setMounted(false), 210)
      return () => clearTimeout(t)
    }
  }, [drawerOpen])

  // Not in DOM when fully closed → no gap-3 slot consumed in the flex row
  if (!mounted) return null

  return (
    <div
      data-orbit-zone="orbit.desktop.drawer.terminal"
      className="relative flex flex-col shrink-0 overflow-hidden rounded-2xl bg-sidebar ring-4 ring-sidebar transition-all duration-200 ease-in-out"
      style={{ width: visible ? '25%' : 0, opacity: visible ? 1 : 0 }}
    >
      {/* Floating close button */}
      <button
        onClick={closeDrawer}
        className="absolute top-2 right-2 z-10 p-1 rounded-md text-foreground/20 hover:text-foreground/60 hover:bg-foreground/5 transition-colors"
        aria-label="Close drawer"
      >
        <X size={12} />
      </button>

      {/* Terminal fills the full card — same height as the terminal panel */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {termMounted && drawerTabId && (
          <TerminalPane tabId={drawerTabId} active={drawerOpen} panelFocused={focusedPanel === 'drawer'} />
        )}
      </div>
    </div>
  )
}
