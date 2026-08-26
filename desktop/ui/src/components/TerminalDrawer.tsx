import { useAppStore } from '../store'
import { TerminalPane } from './Terminal'
import { Drawer } from './ui/drawer'

export function TerminalDrawer() {
  const drawerOpen   = useAppStore((s) => s.drawerOpen)
  const drawerTabId  = useAppStore((s) => s.drawerTabId)
  const closeDrawer  = useAppStore((s) => s.closeDrawer)
  const focusedPanel = useAppStore((s) => s.focusedPanel)

  return (
    <Drawer
      open={drawerOpen}
      onClose={closeDrawer}
      title="Terminal"
      width="25%"
      zone="orbit.desktop.drawer.terminal"
      className="bg-sidebar ring-4 ring-sidebar"
      contentDelayMs={210}
    >
      {(contentReady) => (
        <div className="flex-1 min-h-0 overflow-hidden">
          {contentReady && drawerTabId && (
            <TerminalPane
              tabId={drawerTabId}
              active={drawerOpen}
              panelFocused={focusedPanel === 'drawer'}
            />
          )}
        </div>
      )}
    </Drawer>
  )
}
