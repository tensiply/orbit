import type { StateCreator } from 'zustand'
import type { ArchEntityDto } from '../../types'
import type { AppStore } from '../types'

export type ArchDrawerMode = 'edit' | 'detail'

export interface ArchDrawerSlice {
  archDrawerOpen:        boolean
  archDrawerMode:        ArchDrawerMode
  archDrawerEntity:      ArchEntityDto | null | 'new'
  archDrawerWorkspace:   string
  archDrawerTenant:      string
  archDrawerAllEntities: ArchEntityDto[]

  openArchDrawer:          (entity: ArchEntityDto | 'new', workspace: string, tenant: string, allEntities: ArchEntityDto[]) => void
  openArchDetail:          (entity: ArchEntityDto, workspace: string, tenant: string, allEntities: ArchEntityDto[]) => void
  closeArchDrawer:         () => void
  setArchDrawerEntity:     (entity: ArchEntityDto | null | 'new') => void
  setArchDrawerAllEntities:(entities: ArchEntityDto[]) => void
}

export const createArchDrawerSlice: StateCreator<AppStore, [], [], ArchDrawerSlice> = (set) => ({
  archDrawerOpen:        false,
  archDrawerMode:        'edit',
  archDrawerEntity:      null,
  archDrawerWorkspace:   '',
  archDrawerTenant:      '',
  archDrawerAllEntities: [],

  openArchDrawer: (entity, workspace, tenant, allEntities) =>
    set({ archDrawerOpen: true, archDrawerMode: 'edit', archDrawerEntity: entity, archDrawerWorkspace: workspace, archDrawerTenant: tenant, archDrawerAllEntities: allEntities }),

  openArchDetail: (entity, workspace, tenant, allEntities) =>
    set({ archDrawerOpen: true, archDrawerMode: 'detail', archDrawerEntity: entity, archDrawerWorkspace: workspace, archDrawerTenant: tenant, archDrawerAllEntities: allEntities }),

  closeArchDrawer: () => set({ archDrawerOpen: false, archDrawerEntity: null }),

  setArchDrawerEntity: (entity) => set({ archDrawerEntity: entity }),

  setArchDrawerAllEntities: (entities) => set({ archDrawerAllEntities: entities }),
})
