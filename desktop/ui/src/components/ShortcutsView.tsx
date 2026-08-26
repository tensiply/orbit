import { useState, useEffect, useCallback, useRef } from 'react'
import { useAppStore, Shortcut, ShortcutCategory } from '../store'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Label } from './ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './ui/select'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from './ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableRow,
} from './ui/table'
import { Pencil, Trash2, Plus, RotateCcw, Search } from 'lucide-react'
import { Kbd } from './ui/kbd'

// ── Constants ─────────────────────────────────────────────────────────────────

const CATEGORY_LABELS: Record<ShortcutCategory, string> = {
  navigation: 'Navigation',
  terminal: 'Terminal',
  app: 'Application',
}

const CATEGORY_ORDER: ShortcutCategory[] = ['navigation', 'terminal', 'app']

// ── KeyBadge ──────────────────────────────────────────────────────────────────

export function KeyBadge({ keys }: { keys: string }) {
  if (!keys) return <span className="text-[10px] text-foreground/25">—</span>
  const parts = keys.split('+').filter(Boolean)
  return (
    <span className="flex items-center gap-1 flex-nowrap">
      {parts.map((k, i) => (
        <span key={i} className="flex items-center gap-1">
          {i > 0 && <span className="text-[9px] text-foreground/25 select-none">+</span>}
          <Kbd>{k}</Kbd>
        </span>
      ))}
    </span>
  )
}

// ── KeyCapture ────────────────────────────────────────────────────────────────

function KeyCapture({ value, onChange }: { value: string; onChange: (keys: string) => void }) {
  const [recording, setRecording] = useState(false)
  const [live, setLive] = useState<string[]>([])

  const start = useCallback(() => setRecording(true), [])
  const cancel = useCallback(() => { setRecording(false); setLive([]) }, [])

  useEffect(() => {
    if (!recording) return
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      if (e.key === 'Escape') { cancel(); return }
      const parts: string[] = []
      if (e.ctrlKey)  parts.push('Ctrl')
      if (e.metaKey)  parts.push('⌘')
      if (e.altKey)   parts.push('Alt')
      if (e.shiftKey) parts.push('⇧')
      const MODIFIERS = ['Control', 'Meta', 'Alt', 'Shift']
      if (MODIFIERS.includes(e.key)) { setLive(parts); return }
      const display = e.key.length === 1 ? e.key.toUpperCase() : e.key
      const all = [...parts, display]
      setLive(all)
      onChange(all.join('+'))
      setRecording(false)
      setLive([])
    }
    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [recording, cancel, onChange])

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={start}
        title={recording ? 'Press the shortcut… Esc to cancel' : 'Click to record a new shortcut'}
        className={`flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-all min-w-28 ${
          recording
            ? 'border border-primary/40 text-primary cursor-text'
            : 'border border-foreground/15 hover:border-foreground/30 text-foreground cursor-pointer'
        }`}
      >
        {recording ? (
          live.length > 0 ? (
            <span className="flex items-center gap-1 animate-pulse">
              {live.map((k, i) => (
                <span key={i} className="flex items-center gap-1">
                  {i > 0 && <span className="text-[9px] text-primary/40 select-none">+</span>}
                  <Kbd className="border-primary/40 text-primary/70">{k}</Kbd>
                </span>
              ))}
              <span className="text-[10px] text-primary/50 ml-0.5">…</span>
            </span>
          ) : (
            <span className="text-[10px] text-primary/60 animate-pulse">Press keys…</span>
          )
        ) : (
          <KeyBadge keys={value} />
        )}
      </button>
      {recording && (
        <button type="button" onClick={cancel} className="text-[10px] text-foreground/40 hover:text-foreground/70 transition-colors">
          cancel
        </button>
      )}
    </div>
  )
}

// ── CategorySelect ────────────────────────────────────────────────────────────

function CategorySelect({ value, onChange }: { value: ShortcutCategory; onChange: (v: ShortcutCategory) => void }) {
  return (
    <Select value={value} onValueChange={(v) => onChange(v as ShortcutCategory)}>
      <SelectTrigger className="h-7 text-xs border-border">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {CATEGORY_ORDER.map((c) => (
          <SelectItem key={c} value={c} className="text-xs">
            {CATEGORY_LABELS[c]}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

// ── EditModal ─────────────────────────────────────────────────────────────────

interface EditModalProps {
  shortcut: Shortcut | null
  onSave: (updates: Partial<Shortcut>) => void
  onClose: () => void
}

function EditModal({ shortcut, onSave, onClose }: EditModalProps) {
  const [keys, setKeys] = useState(shortcut?.keys ?? '')
  const [name, setName] = useState(shortcut?.name ?? '')
  const [description, setDescription] = useState(shortcut?.description ?? '')
  const [action, setAction] = useState(shortcut?.action ?? '')
  const [category, setCategory] = useState<ShortcutCategory>(shortcut?.category ?? 'app')

  const isBuiltin = shortcut?.builtin ?? false

  useEffect(() => {
    if (shortcut) {
      setKeys(shortcut.keys)
      setName(shortcut.name)
      setDescription(shortcut.description)
      setAction(shortcut.action)
      setCategory(shortcut.category)
    }
  }, [shortcut])

  const handleSave = () => {
    if (!name.trim()) return
    onSave({ keys, name: name.trim(), description: description.trim(), action: action.trim(), category })
    onClose()
  }

  return (
    <Dialog open={shortcut !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="w-[420px] max-w-[90vw] bg-card border-border">
        <DialogHeader>
          <DialogTitle className="text-sm font-semibold">
            {isBuiltin ? 'Remap Shortcut' : 'Edit Shortcut'}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Name</Label>
            {isBuiltin ? (
              <p className="text-sm text-foreground">{name}</p>
            ) : (
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Shortcut name" />
            )}
          </div>

          {!isBuiltin && (
            <div className="space-y-1.5">
              <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Description</Label>
              <Input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="What does this shortcut do?" />
            </div>
          )}

          {!isBuiltin && (
            <div className="space-y-1.5">
              <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Category</Label>
              <CategorySelect value={category} onChange={setCategory} />
            </div>
          )}

          {!isBuiltin && (
            <div className="space-y-1.5">
              <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Action</Label>
              <Input value={action} onChange={(e) => setAction(e.target.value)} placeholder="e.g. open_panel or shell command" className="font-mono" />
            </div>
          )}

          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Key Binding</Label>
            <KeyCapture value={keys} onChange={setKeys} />
            {isBuiltin && (
              <p className="text-[10px] text-foreground/35 mt-1">
                Only the key binding can be changed for built-in shortcuts.
              </p>
            )}
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button size="sm" onClick={handleSave} disabled={!name.trim()}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

// ── AddModal ──────────────────────────────────────────────────────────────────

interface AddModalProps {
  open: boolean
  onAdd: (shortcut: Omit<Shortcut, 'id' | 'builtin'>) => void
  onClose: () => void
}

function AddModal({ open, onAdd, onClose }: AddModalProps) {
  const [keys, setKeys] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [action, setAction] = useState('')
  const [category, setCategory] = useState<ShortcutCategory>('app')

  const handleAdd = () => {
    if (!name.trim()) return
    onAdd({ keys, name: name.trim(), description: description.trim(), action: action.trim(), category })
    setKeys(''); setName(''); setDescription(''); setAction('')
    onClose()
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="w-[420px] max-w-[90vw] bg-card border-border">
        <DialogHeader>
          <DialogTitle className="text-sm font-semibold">New Custom Shortcut</DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
              Name <span className="text-destructive">*</span>
            </Label>
            <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Open config file" />
          </div>
          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Description</Label>
            <Input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="What does this shortcut do?" />
          </div>
          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Category</Label>
            <CategorySelect value={category} onChange={setCategory} />
          </div>
          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Action</Label>
            <Input value={action} onChange={(e) => setAction(e.target.value)} placeholder="Identifier or command" className="font-mono" />
          </div>
          <div className="space-y-1.5">
            <Label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">Key Binding</Label>
            <KeyCapture value={keys} onChange={setKeys} />
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button size="sm" onClick={handleAdd} disabled={!name.trim()}>Add Shortcut</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

// ── ShortcutRow ───────────────────────────────────────────────────────────────

function ShortcutRow({
  shortcut,
  onEdit,
  onDelete,
}: {
  shortcut: Shortcut
  onEdit: (s: Shortcut) => void
  onDelete: (id: string) => void
}) {
  const [pendingDelete, setPendingDelete] = useState(false)

  return (
    <TableRow className="border-foreground/5 hover:bg-foreground/3 transition-colors">
      <TableCell className="py-3 pl-5 pr-3 align-top w-[35%]">
        <div className="flex flex-col gap-0.5">
          <span className="text-xs font-medium text-foreground leading-snug">{shortcut.name}</span>
          <span className="text-[10px] text-foreground/35 leading-tight">{shortcut.description}</span>
        </div>
      </TableCell>

      <TableCell className="py-3 px-3 align-middle w-[30%]">
        <KeyBadge keys={shortcut.keys} />
      </TableCell>

      <TableCell className="py-3 px-3 align-middle w-[15%]">
        {shortcut.builtin ? (
          <span className="text-[10px] text-foreground/25">built-in</span>
        ) : (
          <span className="text-[10px] text-primary/50">custom</span>
        )}
      </TableCell>

      <TableCell className="py-3 pr-5 pl-3 align-middle w-[20%]">
        <div className="flex items-center justify-end gap-1">
          {pendingDelete ? (
            <>
              <button
                className="text-[10px] text-destructive hover:text-destructive/70 transition-colors px-1"
                onClick={() => { onDelete(shortcut.id); setPendingDelete(false) }}
              >Delete</button>
              <button
                className="text-[10px] text-foreground/35 hover:text-foreground/60 transition-colors px-1"
                onClick={() => setPendingDelete(false)}
              >Cancel</button>
            </>
          ) : (
            <>
              <button
                className="w-6 h-6 flex items-center justify-center rounded text-foreground/25 hover:text-foreground/70 transition-colors"
                onClick={() => onEdit(shortcut)}
                title="Edit shortcut"
              >
                <Pencil size={12} />
              </button>
              {!shortcut.builtin && (
                <button
                  className="w-6 h-6 flex items-center justify-center rounded text-foreground/25 hover:text-destructive transition-colors"
                  onClick={() => setPendingDelete(true)}
                  title="Delete shortcut"
                >
                  <Trash2 size={12} />
                </button>
              )}
            </>
          )}
        </div>
      </TableCell>
    </TableRow>
  )
}

// ── ShortcutsView ─────────────────────────────────────────────────────────────

export function ShortcutsView() {
  const shortcuts      = useAppStore((s) => s.shortcuts)
  const addShortcut    = useAppStore((s) => s.addShortcut)
  const updateShortcut = useAppStore((s) => s.updateShortcut)
  const deleteShortcut = useAppStore((s) => s.deleteShortcut)
  const resetShortcuts = useAppStore((s) => s.resetShortcuts)

  const [search, setSearch]       = useState('')
  const [editing, setEditing]     = useState<Shortcut | null>(null)
  const [showAdd, setShowAdd]     = useState(false)
  const [showReset, setShowReset] = useState(false)

  const searchRef = useRef<HTMLInputElement>(null)
  useEffect(() => { searchRef.current?.focus() }, [])

  const handleEdit     = useCallback((s: Shortcut) => setEditing(s), [])
  const handleSaveEdit = useCallback(
    (updates: Partial<Shortcut>) => { if (editing) updateShortcut(editing.id, updates) },
    [editing, updateShortcut],
  )
  const handleAdd = useCallback((s: Omit<Shortcut, 'id' | 'builtin'>) => addShortcut(s), [addShortcut])

  const q = search.toLowerCase().trim()
  const filtered = shortcuts.filter(
    (s) => !q || s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q) ||
           s.keys.toLowerCase().includes(q) || s.action.toLowerCase().includes(q),
  )

  const grouped = CATEGORY_ORDER.reduce<Record<ShortcutCategory, Shortcut[]>>(
    (acc, cat) => { acc[cat] = filtered.filter((s) => s.category === cat); return acc },
    { navigation: [], terminal: [], app: [] },
  )

  const totalCustom = shortcuts.filter((s) => !s.builtin).length
  const hasRemapped = shortcuts.some(
    (s) => s.builtin && s.keys !== (shortcuts.find((d) => d.id === s.id)?.keys ?? s.keys),
  )

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* toolbar */}
      <div className="flex items-center gap-3 px-5 py-3 border-b border-foreground/8 shrink-0">
        <div className="flex flex-col">
          <h1 className="text-sm font-semibold text-foreground leading-tight">Keyboard Shortcuts</h1>
          <p className="text-[10px] text-foreground/35 leading-tight">
            {shortcuts.length} shortcuts · {totalCustom} custom
          </p>
        </div>

        <div className="flex-1" />

        {(totalCustom > 0 || hasRemapped) && (
          showReset ? (
            <div className="flex items-center gap-1.5">
              <span className="text-[10px] text-foreground/50">Reset all to defaults?</span>
              <Button variant="destructive" size="xs" onClick={() => { resetShortcuts(); setShowReset(false) }}>
                Reset
              </Button>
              <Button variant="ghost" size="xs" onClick={() => setShowReset(false)}>
                Cancel
              </Button>
            </div>
          ) : (
            <Button
              variant="ghost" size="xs"
              className="gap-1.5 text-[10px] text-foreground/40 hover:text-foreground/70"
              onClick={() => setShowReset(true)}
              title="Reset all shortcuts to defaults"
            >
              <RotateCcw size={12} /> Reset defaults
            </Button>
          )
        )}

        <Button size="sm" className="gap-1.5 text-xs" onClick={() => setShowAdd(true)}>
          <Plus size={13} /> Add Custom
        </Button>
      </div>

      {/* search */}
      <div className="px-5 py-2.5 border-b border-foreground/5 shrink-0">
        <label className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-foreground/10 bg-foreground/3 hover:border-foreground/20 focus-within:border-primary/50 transition-colors">
          <span className="text-foreground/30 shrink-0"><Search size={13} /></span>
          <input
            ref={searchRef}
            type="text"
            className="flex-1 bg-transparent text-xs outline-none placeholder:text-foreground/25"
            placeholder="Search shortcuts…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button
              className="text-[10px] text-foreground/30 hover:text-foreground/60 transition-colors"
              onClick={() => setSearch('')}
            >✕</button>
          )}
        </label>
      </div>

      {/* table */}
      <div className="flex-1 overflow-y-auto min-h-0">
        {filtered.length === 0 ? (
          <div className="flex items-center justify-center h-40 text-xs text-foreground/30">
            No shortcuts match "{search}"
          </div>
        ) : (
          <Table>
            {CATEGORY_ORDER.map((cat) => {
              const rows = grouped[cat]
              if (rows.length === 0) return null
              return (
                <TableBody key={cat}>
                  <TableRow className="border-0 hover:bg-transparent">
                    <TableCell colSpan={4} className="px-5 pt-5 pb-1.5">
                      <span className="text-[10px] font-semibold text-muted-foreground uppercase tracking-widest">
                        {CATEGORY_LABELS[cat]}
                      </span>
                    </TableCell>
                  </TableRow>
                  {rows.map((s) => (
                    <ShortcutRow key={s.id} shortcut={s} onEdit={handleEdit} onDelete={deleteShortcut} />
                  ))}
                </TableBody>
              )
            })}
          </Table>
        )}
      </div>

      <EditModal shortcut={editing} onSave={handleSaveEdit} onClose={() => setEditing(null)} />
      <AddModal open={showAdd} onAdd={handleAdd} onClose={() => setShowAdd(false)} />
    </div>
  )
}
