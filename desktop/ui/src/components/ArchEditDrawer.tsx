import { X } from 'lucide-react'
import { useState, useEffect } from 'react'
import { useAppStore } from '../store'
import { EditPanel } from './arch/EditPanel'
import { EntityDetailPanel } from './arch/EntityDetailPanel'
import type { ArchEntityDto } from '../types'

export function ArchEditDrawer() {
  const open            = useAppStore((s) => s.archDrawerOpen)
  const mode            = useAppStore((s) => s.archDrawerMode)
  const entity          = useAppStore((s) => s.archDrawerEntity)
  const workspace       = useAppStore((s) => s.archDrawerWorkspace)
  const tenant          = useAppStore((s) => s.archDrawerTenant)
  const allEntities     = useAppStore((s) => s.archDrawerAllEntities)
  const closeArchDrawer = useAppStore((s) => s.closeArchDrawer)
  const openArchDrawer  = useAppStore((s) => s.openArchDrawer)
  const setArchDrawerEntity = useAppStore((s) => s.setArchDrawerEntity)
  const activeTabId     = useAppStore((s) => s.activeTabId)
  const tabs            = useAppStore((s) => s.tabs)

  // Mounted controls whether the element exists in the flex row at all.
  // Visible controls the width/opacity transition.
  // Pattern mirrors TerminalDrawer's termMounted delay.
  const [mounted,  setMounted]  = useState(false)
  const [visible,  setVisible]  = useState(false)

  // Close when the user navigates away from an architecture tab
  useEffect(() => {
    if (!open) return
    const activeTab = tabs.find((t) => t.id === activeTabId)
    if (activeTab?.type !== 'architecture') {
      closeArchDrawer()
    }
  }, [activeTabId])

  useEffect(() => {
    if (open) {
      setMounted(true)
      // Double RAF: paint at width:0 first so the CSS transition runs from 0 → full
      requestAnimationFrame(() => requestAnimationFrame(() => setVisible(true)))
    } else {
      setVisible(false)
      const t = setTimeout(() => setMounted(false), 210)
      return () => clearTimeout(t)
    }
  }, [open])

  // Not in DOM when fully closed → no gap-2 slot consumed in the flex row
  if (!mounted) return null

  const panelData = entity === 'new' ? null : entity

  const handleSave = (saved: ArchEntityDto, prev: ArchEntityDto | null) => {
    setArchDrawerEntity(saved)
    document.dispatchEvent(
      new CustomEvent('orbit:arch-save', { detail: { entity: saved, isNew: prev === null } }),
    )
  }

  const handleDelete = (deleted: ArchEntityDto) => {
    closeArchDrawer()
    document.dispatchEvent(
      new CustomEvent('orbit:arch-delete', { detail: { entity: deleted } }),
    )
  }

  return (
    <div
      data-orbit-zone="orbit.desktop.drawer.arch-editor"
      className="relative flex flex-col shrink-0 overflow-hidden rounded-2xl bg-card transition-all duration-200 ease-in-out"
      style={{ width: visible ? 288 : 0, opacity: visible ? 1 : 0 }}
    >
      {/* Floating close button */}
      <button
        onClick={closeArchDrawer}
        className="absolute top-2 right-2 z-10 p-1 rounded-md text-foreground/20 hover:text-foreground/60 hover:bg-foreground/5 transition-colors"
        aria-label="Close panel"
      >
        <X size={12} />
      </button>

      {mode === 'detail' && entity && entity !== 'new' ? (
        <EntityDetailPanel
          entity={entity}
          allEntities={allEntities}
          onClose={closeArchDrawer}
          onEdit={() => openArchDrawer(entity, workspace, tenant, allEntities)}
        />
      ) : (
        <EditPanel
          workspace={workspace}
          tenant={tenant}
          entity={panelData}
          allEntities={allEntities}
          onSave={handleSave}
          onDelete={handleDelete}
          onClose={closeArchDrawer}
        />
      )}
    </div>
  )
}
