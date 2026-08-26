import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { AppStore } from '../types'
import type { DocEntry } from '../../types'

export interface DocumentsSlice {
  documents: DocEntry[]
  documentsLoading: boolean
  fetchDocuments: () => Promise<void>
  openDocument: (doc: DocEntry) => void
  deleteDocument: (id: string, workspace: string) => Promise<void>
  archiveDocument: (id: string, workspace: string) => Promise<void>
}

export const createDocumentsSlice: StateCreator<AppStore, [], [], DocumentsSlice> = (set, get) => ({
  documents: [],
  documentsLoading: false,

  fetchDocuments: async () => {
    set({ documentsLoading: true })
    try {
      const docs = await invoke<DocEntry[]>('document_list')
      set({ documents: docs, documentsLoading: false })
    } catch {
      set({ documentsLoading: false })
    }
  },

  openDocument: (doc: DocEntry) => {
    const tabId = `doc-${doc.id}`
    const existing = get().tabs.find((t) => t.id === tabId)
    if (existing) {
      set({ activeTabId: tabId })
      return
    }
    const tab = { id: tabId, title: doc.title, type: 'document' as const, docId: doc.id }
    set((s) => ({ tabs: [...s.tabs, tab], activeTabId: tabId }))
  },

  deleteDocument: async (id: string, workspace: string) => {
    await invoke('document_delete', { id, workspace })
    set((s) => ({ documents: s.documents.filter((d) => d.id !== id) }))
    // Close the tab if open
    const tabId = `doc-${id}`
    set((s) => ({
      tabs: s.tabs.filter((t) => t.id !== tabId),
      activeTabId: s.activeTabId === tabId
        ? (s.tabs.find((t) => t.id !== tabId)?.id ?? null)
        : s.activeTabId,
    }))
  },

  archiveDocument: async (id: string, workspace: string) => {
    await invoke('document_archive', { id, workspace })
    set((s) => ({ documents: s.documents.filter((d) => d.id !== id) }))
    const tabId = `doc-${id}`
    set((s) => ({
      tabs: s.tabs.filter((t) => t.id !== tabId),
      activeTabId: s.activeTabId === tabId
        ? (s.tabs.find((t) => t.id !== tabId)?.id ?? null)
        : s.activeTabId,
    }))
  },
})
