import { SquareTerminal, Settings, Keyboard, Layers, FileText, Network, Map, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAppStore } from '../store'
import type { Tab } from '../store'
import { Tooltip, TooltipContent, TooltipTrigger } from './ui/tooltip'
import { Button } from './ui/button'

function tabIcon(type: Tab['type']) {
  switch (type) {
    case 'settings':     return Settings
    case 'shortcuts':    return Keyboard
    case 'uikit':        return Layers
    case 'document':     return FileText
    case 'architecture': return Network
    case 'ui-map':       return Map
    default:             return SquareTerminal
  }
}

export function TabBar() {
  const tabs        = useAppStore((s) => s.tabs)
  const activeTabId = useAppStore((s) => s.activeTabId)
  const setActiveTab = useAppStore((s) => s.setActiveTab)
  const closeTab    = useAppStore((s) => s.closeTab)

  return (
    <div data-orbit-zone="orbit.desktop.principal.card.tabs" className="flex shrink-0 select-none bg-card h-[36px] border-b border-sidebar-border/60">
      {/* Scrollable tab list */}
      <div
        role="tablist"
        className="flex items-stretch flex-1 min-w-0 overflow-x-auto no-scrollbar"
      >
        {tabs.length === 0 && (
          <span className="flex items-end pb-2 px-3 text-xs text-foreground/20 pointer-events-none">
            No tabs open
          </span>
        )}
        {tabs.map((tab) => (
          <TabItem
            key={tab.id}
            tab={tab}
            active={tab.id === activeTabId}
            onActivate={() => setActiveTab(tab.id)}
            onClose={() => closeTab(tab.id)}
          />
        ))}
      </div>

    </div>
  )
}

function TabItem({
  tab,
  active,
  onActivate,
  onClose,
}: {
  tab: Tab
  active: boolean
  onActivate: () => void
  onClose: () => void
}) {
  const Icon = tabIcon(tab.type)

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          role="tab"
          aria-selected={active}
          onClick={onActivate}
          className={cn(
            'group flex items-center gap-1.5 px-3 text-xs cursor-pointer shrink-0 transition-colors border-b-2 relative',
            active
              ? 'bg-card text-foreground border-b-tab-indicator'
              : 'text-foreground/38 border-b-transparent hover:bg-sidebar-accent/40 hover:text-foreground/58',
          )}
        >
          <Icon
            size={12}
            className={cn(
              'shrink-0 transition-colors',
              active ? 'text-foreground/55' : 'text-foreground/22 group-hover:text-foreground/40',
            )}
          />
          <span className="max-w-[108px] truncate leading-none">{tab.title}</span>
          <Button
            variant="ghost"
            className={cn(
              'size-4 p-0 rounded shrink-0 transition-all',
              active
                ? 'text-foreground/30 hover:text-destructive hover:bg-destructive/10'
                : 'text-transparent group-hover:text-foreground/28 hover:!text-destructive hover:!bg-destructive/10',
            )}
            onClick={(e) => {
              e.stopPropagation()
              onClose()
            }}
            title="Close"
          >
            <X size={10} />
          </Button>

          {/* Separator — right edge, hidden for active and its right neighbour */}
          {!active && (
            <span className="absolute right-0 top-1/4 h-1/2 w-px bg-border/40 pointer-events-none" />
          )}
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="text-[11px]">
        {tab.title}
      </TooltipContent>
    </Tooltip>
  )
}
