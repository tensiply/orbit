import { useEffect } from 'react'
import { useAppStore } from '../store'
import { EditPanel } from './arch/EditPanel'
import { EntityDetailPanel } from './arch/EntityDetailPanel'
import { Drawer } from './ui/drawer'
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

  // Close when the user navigates away from an architecture tab
  useEffect(() => {
    if (!open) return
    const activeTab = tabs.find((t) => t.id === activeTabId)
    if (activeTab?.type !== 'architecture') {
      closeArchDrawer()
    }
  }, [activeTabId])

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
    <Drawer
      open={open}
      onClose={closeArchDrawer}
      title={mode === 'detail' ? 'Details' : 'Edit'}
      zone="orbit.desktop.drawer.arch-editor"
      className="bg-card"
    >
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
    </Drawer>
  )
}
