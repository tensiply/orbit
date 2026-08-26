import { useState, useCallback } from 'react'
import {
  Settings2, Monitor, Terminal, Cpu, ShieldCheck, RotateCcw, Search,
} from 'lucide-react'
import { useAppStore } from '../store'
import type { Setting, SettingCategory } from '../types'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './ui/select'
import { Input } from './ui/input'
import { Button } from './ui/button'

// ── Category config ────────────────────────────────────────────────────────────

const CATEGORIES: { id: SettingCategory; label: string; icon: React.ReactNode }[] = [
  { id: 'general',    label: 'General',    icon: <Settings2 size={14} /> },
  { id: 'appearance', label: 'Appearance', icon: <Monitor size={14} />   },
  { id: 'terminal',   label: 'Terminal',   icon: <Terminal size={14} />  },
  { id: 'engine',     label: 'Engine',     icon: <Cpu size={14} />       },
  { id: 'privacy',    label: 'Privacy',    icon: <ShieldCheck size={14} /> },
]

// ── Controls ───────────────────────────────────────────────────────────────────

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors focus-visible:outline-none ${
        checked ? 'bg-primary' : 'bg-foreground/15'
      }`}
    >
      <span
        className={`pointer-events-none inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm transition-transform ${
          checked ? 'translate-x-4' : 'translate-x-0.5'
        }`}
      />
    </button>
  )
}

function NumberInput({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  return (
    <Input
      type="number"
      value={value}
      onChange={(e) => {
        const n = parseInt(e.target.value, 10)
        if (!isNaN(n)) onChange(n)
      }}
      className="h-7 w-28 text-xs font-mono"
    />
  )
}

function StringInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <Input
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="h-7 w-48 text-xs font-mono"
    />
  )
}

function SelectInput({
  value,
  options,
  onChange,
}: {
  value: string
  options: { label: string; value: string }[]
  onChange: (v: string) => void
}) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger className="h-7 w-44 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value} className="text-xs">
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

// ── SettingRow ─────────────────────────────────────────────────────────────────

function SettingRow({
  setting,
  onUpdate,
  onReset,
}: {
  setting: Setting
  onUpdate: (id: string, value: Setting['value']) => void
  onReset: (id: string) => void
}) {
  const isModified = setting.value !== setting.default

  const control = (() => {
    switch (setting.type) {
      case 'boolean':
        return (
          <Toggle
            checked={setting.value as boolean}
            onChange={(v) => onUpdate(setting.id, v)}
          />
        )
      case 'number':
        return (
          <NumberInput
            value={setting.value as number}
            onChange={(v) => onUpdate(setting.id, v)}
          />
        )
      case 'string':
        return (
          <StringInput
            value={setting.value as string}
            onChange={(v) => onUpdate(setting.id, v)}
          />
        )
      case 'select':
        return (
          <SelectInput
            value={setting.value as string}
            options={setting.options ?? []}
            onChange={(v) => onUpdate(setting.id, v)}
          />
        )
    }
  })()

  return (
    <div className="group flex items-start gap-4 px-6 py-4 border-b border-foreground/5 hover:bg-foreground/2 transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm text-foreground leading-snug">{setting.name}</span>
          {isModified && (
            <span className="text-[9px] px-1.5 py-px rounded-full bg-primary/15 text-primary/70 font-medium">
              modified
            </span>
          )}
        </div>
        <p className="text-[11px] text-foreground/40 leading-snug mt-0.5">{setting.description}</p>
      </div>

      <div className="flex items-center gap-2 shrink-0 pt-0.5">
        {control}
        {isModified && (
          <button
            onClick={() => onReset(setting.id)}
            title="Reset to default"
            className="text-foreground/20 hover:text-foreground/60 transition-colors opacity-0 group-hover:opacity-100"
          >
            <RotateCcw size={12} />
          </button>
        )}
      </div>
    </div>
  )
}

// ── SettingsView ───────────────────────────────────────────────────────────────

export function SettingsView() {
  const settings      = useAppStore((s) => s.settings)
  const updateSetting = useAppStore((s) => s.updateSetting)
  const resetSetting  = useAppStore((s) => s.resetSetting)
  const resetAll      = useAppStore((s) => s.resetAllSettings)

  const [activeCategory, setActiveCategory] = useState<SettingCategory>('general')
  const [search, setSearch]                 = useState('')
  const [showResetAll, setShowResetAll]     = useState(false)

  const handleUpdate = useCallback(
    (id: string, value: Setting['value']) => updateSetting(id, value),
    [updateSetting],
  )
  const handleReset = useCallback((id: string) => resetSetting(id), [resetSetting])

  const q = search.toLowerCase().trim()
  const isSearching = q.length > 0

  const filtered = isSearching
    ? settings.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.description.toLowerCase().includes(q) ||
          s.key.toLowerCase().includes(q),
      )
    : settings.filter((s) => s.category === activeCategory)

  const modifiedCount = settings.filter((s) => s.value !== s.default).length

  return (
    <div className="flex h-full overflow-hidden">
      {/* ── Left nav ───────────────────────────────────────────────────────────── */}
      <nav className="w-44 shrink-0 border-r border-foreground/8 flex flex-col py-2 overflow-y-auto">
        {CATEGORIES.map((cat) => {
          const catModified = settings.filter(
            (s) => s.category === cat.id && s.value !== s.default,
          ).length
          return (
            <button
              key={cat.id}
              onClick={() => { setActiveCategory(cat.id); setSearch('') }}
              className={`flex items-center gap-2.5 px-4 py-2 text-xs transition-colors text-left ${
                activeCategory === cat.id && !isSearching
                  ? 'bg-foreground/8 text-foreground font-medium'
                  : 'text-foreground/50 hover:text-foreground/80 hover:bg-foreground/4'
              }`}
            >
              <span className="shrink-0 text-foreground/40">{cat.icon}</span>
              <span className="flex-1">{cat.label}</span>
              {catModified > 0 && (
                <span className="text-[9px] w-4 h-4 flex items-center justify-center rounded-full bg-primary/20 text-primary/70 font-semibold shrink-0">
                  {catModified}
                </span>
              )}
            </button>
          )
        })}
      </nav>

      {/* ── Right content ──────────────────────────────────────────────────────── */}
      <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
        {/* Header */}
        <div className="flex items-center gap-3 px-6 py-3 border-b border-foreground/8 shrink-0">
          <div className="flex flex-col">
            <h1 className="text-sm font-semibold text-foreground leading-tight">Settings</h1>
            {modifiedCount > 0 && (
              <p className="text-[10px] text-foreground/35 leading-tight">
                {modifiedCount} modified
              </p>
            )}
          </div>

          <div className="flex-1" />

          {modifiedCount > 0 && (
            showResetAll ? (
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] text-foreground/50">Reset all to defaults?</span>
                <Button
                  variant="destructive"
                  size="xs"
                  onClick={() => { resetAll(); setShowResetAll(false) }}
                >
                  Reset
                </Button>
                <Button variant="ghost" size="xs" onClick={() => setShowResetAll(false)}>
                  Cancel
                </Button>
              </div>
            ) : (
              <Button
                variant="ghost"
                size="xs"
                className="gap-1.5 text-[10px] text-foreground/40 hover:text-foreground/70"
                onClick={() => setShowResetAll(true)}
              >
                <RotateCcw size={12} /> Reset all
              </Button>
            )
          )}
        </div>

        {/* Search */}
        <div className="px-6 py-2.5 border-b border-foreground/5 shrink-0">
          <label className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-foreground/10 bg-foreground/3 hover:border-foreground/20 focus-within:border-primary/50 transition-colors">
            <Search size={13} className="text-foreground/30 shrink-0" />
            <input
              type="text"
              className="flex-1 bg-transparent text-xs outline-none placeholder:text-foreground/25"
              placeholder="Search settings…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            {search && (
              <button
                className="text-[10px] text-foreground/30 hover:text-foreground/60 transition-colors"
                onClick={() => setSearch('')}
              >
                ✕
              </button>
            )}
          </label>
        </div>

        {/* Category heading when not searching */}
        {!isSearching && (
          <div className="px-6 pt-5 pb-2 shrink-0">
            <h2 className="text-[10px] font-semibold text-muted-foreground uppercase tracking-widest">
              {CATEGORIES.find((c) => c.id === activeCategory)?.label}
            </h2>
          </div>
        )}

        {/* Settings list */}
        <div className="flex-1 overflow-y-auto min-h-0">
          {filtered.length === 0 ? (
            <div className="flex items-center justify-center h-40 text-xs text-foreground/30">
              {isSearching ? `No settings match "${search}"` : 'No settings in this category'}
            </div>
          ) : (
            <>
              {isSearching
                ? filtered.map((s) => (
                    <SettingRow
                      key={s.id}
                      setting={s}
                      onUpdate={handleUpdate}
                      onReset={handleReset}
                    />
                  ))
                : filtered.map((s) => (
                    <SettingRow
                      key={s.id}
                      setting={s}
                      onUpdate={handleUpdate}
                      onReset={handleReset}
                    />
                  ))}
            </>
          )}
        </div>
      </div>
    </div>
  )
}
