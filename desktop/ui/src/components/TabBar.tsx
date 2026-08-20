import { useAppStore, Tab } from '../store'

export function TabBar() {
  const tabs = useAppStore((s) => s.tabs)
  const activeTabId = useAppStore((s) => s.activeTabId)
  const setActiveTab = useAppStore((s) => s.setActiveTab)
  const closeTab = useAppStore((s) => s.closeTab)

  return (
    <div
      role="tablist"
      className="tabs tabs-bordered bg-[#141415] shrink-0 select-none flex-nowrap overflow-x-auto px-1"
    >
      {tabs.length === 0 && (
        <span className="tab text-base-content/30 pointer-events-none text-xs">
          No terminals open
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
  return (
    <div
      role="tab"
      className={`tab gap-1.5 group ${active ? 'tab-active' : ''}`}
      onClick={onActivate}
    >
      <span className="max-w-28 truncate text-xs">{tab.title}</span>
      <button
        className="btn btn-ghost btn-xs btn-circle w-4 h-4 min-h-0 text-xs opacity-0 group-hover:opacity-50 hover:!opacity-100 hover:text-error"
        onClick={(e) => {
          e.stopPropagation()
          onClose()
        }}
        title="Close"
      >
        ✕
      </button>
    </div>
  )
}
