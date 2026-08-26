import { useState } from 'react'
import { Loader2, SquareTerminal, Settings, X, Plus, ChevronRight, Layers, BookOpen, Minus, Square } from 'lucide-react'
import { Drawer } from './ui/drawer'
import { cn } from '@/lib/utils'
import { STATUS_COLORS } from '../theme'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Textarea } from './ui/textarea'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select'
import { Checkbox } from './ui/checkbox'
import { Label } from './ui/label'
import { Toggle } from './ui/toggle'
import { ToggleGroup, ToggleGroupItem } from './ui/toggle-group'
import { Kbd } from './ui/kbd'
import { Separator } from './ui/separator'
import { Skeleton } from './ui/skeleton'
import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from './ui/tooltip'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from './ui/empty'

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-3">
      <p className="text-[9px] font-semibold text-foreground/30 uppercase tracking-widest">
        {title}
      </p>
      {children}
    </div>
  )
}

function Group({ label, children }: { label?: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      {label && <p className="text-[10px] text-foreground/35">{label}</p>}
      {children}
    </div>
  )
}

// ── Complex component previews ────────────────────────────────────────────────

function PreviewSessionItem({ active, working, history }: { active?: boolean; working?: boolean; history?: boolean }) {
  const rowClass = active
    ? 'bg-sidebar-accent text-sidebar-accent-foreground'
    : history
      ? 'text-sidebar-foreground/25'
      : 'text-sidebar-foreground/50'

  return (
    <div className={`relative w-full min-w-0 py-2 px-2 rounded-lg ${rowClass}`}>
      <span
        className="absolute pointer-events-none select-none text-sidebar-foreground"
        style={{ top: 6, right: 6, opacity: history ? 0.04 : 0.07 }}
      >
        <SquareTerminal size={26} />
      </span>
      <div className="flex flex-col gap-0.5 min-w-0 pr-7">
        <div className="flex items-center gap-1.5 min-w-0">
          <div className="relative flex items-center justify-center shrink-0">
            {working && (
              <span
                className="absolute w-4 h-4 rounded-full animate-ping"
                style={{ backgroundColor: STATUS_COLORS.working, opacity: 0.35 }}
              />
            )}
            <span
              className="w-2.5 h-2.5 rounded-full relative"
              style={history
                ? { border: '1.5px solid currentColor' }
                : { backgroundColor: working ? STATUS_COLORS.working : STATUS_COLORS.active }
              }
            />
          </div>
          <span className={`text-xs font-medium leading-snug truncate ${active ? 'text-sidebar-accent-foreground' : ''}`}>
            {history ? 'previous session' : working ? 'processing request…' : 'active session'}
          </span>
        </div>
        <span className={`text-[10px] leading-tight truncate mt-1 ${history ? 'text-sidebar-foreground/20' : 'text-sidebar-foreground/35'}`}>
          AIDEV › AI-ECOSYSTEM › orbit
        </span>
        <span className={`text-[10px] leading-tight ${history ? 'text-sidebar-foreground/15' : 'text-sidebar-foreground/25'}`}>
          claude · 5m ago
        </span>
      </div>
    </div>
  )
}

function PreviewTabItem({ active, title, icon }: { active?: boolean; title: string; icon: React.ReactNode }) {
  return (
    <div
      className={cn(
        'group flex items-center gap-1.5 px-3 text-xs shrink-0 border-b-2 relative h-full',
        active
          ? 'bg-sidebar text-foreground border-b-[var(--tab-indicator)]'
          : 'bg-transparent text-foreground/38 border-b-transparent',
      )}
    >
      <span className={cn('shrink-0', active ? 'text-foreground/55' : 'text-foreground/22')}>
        {icon}
      </span>
      <span className="max-w-[108px] truncate leading-none">{title}</span>
      <div className={cn('w-4 h-4 flex items-center justify-center rounded shrink-0', active ? 'text-foreground/30' : 'text-foreground/20')}>
        <X size={10} />
      </div>
      {!active && (
        <span className="absolute right-0 top-1/4 h-1/2 w-px bg-border/40 pointer-events-none" />
      )}
    </div>
  )
}

function PreviewRailButton({ icon, active }: { icon: React.ReactNode; active?: boolean }) {
  return (
    <div
      className={cn(
        'flex items-center justify-center rounded-md size-8 shrink-0',
        active
          ? 'bg-sidebar-accent text-sidebar-accent-foreground'
          : 'text-sidebar-foreground/50',
      )}
    >
      {icon}
    </div>
  )
}

function PreviewScopeFolder({ selected }: { selected?: boolean }) {
  return (
    <div
      className={cn(
        'flex items-center justify-between w-full px-2 py-1.5 rounded-md text-xs text-sidebar-foreground/55',
        selected && 'ring-1 ring-inset ring-foreground/30',
      )}
    >
      <span className="truncate">AI-ECOSYSTEM</span>
      <ChevronRight size={11} className="shrink-0 text-sidebar-foreground/20" />
    </div>
  )
}

function PreviewDocumentItem() {
  return (
    <div className="w-full py-2 px-2 rounded-md text-sidebar-foreground/40">
      <div className="flex flex-col gap-0.5 min-w-0">
        <div className="flex items-center gap-1.5 min-w-0">
          <span className="text-[10px] uppercase tracking-wider font-medium px-1 py-px rounded bg-sidebar-foreground/8 text-sidebar-foreground/40 shrink-0">
            PDF
          </span>
          <span className="text-xs font-medium leading-snug truncate text-sidebar-foreground/60">
            Weekly Report
          </span>
        </div>
        <span className="text-[10px] leading-tight truncate text-sidebar-foreground/30 pl-0.5">
          report-2026-08.pdf
        </span>
        <span className="text-[10px] leading-tight truncate text-sidebar-foreground/25 pl-0.5">
          AIDEV › orbit
        </span>
      </div>
    </div>
  )
}

// ── Drawer demo ───────────────────────────────────────────────────────────────

function DrawerDemo() {
  const [open, setOpen] = useState(false)

  return (
    <Section title="Drawer">
      <div className="flex items-start gap-2">
        {/* Trigger + host area that mimics the flex-row sibling pattern */}
        <div className="flex gap-2 flex-1 h-44 border border-foreground/8 rounded-xl overflow-hidden">
          {/* "Principal" stand-in */}
          <div className="flex-1 bg-background flex flex-col items-center justify-center gap-2">
            <span className="text-[10px] text-foreground/20">principal</span>
            <button
              onClick={() => setOpen((o) => !o)}
              className="h-6 px-2.5 rounded border border-foreground/15 text-[10px] text-foreground/50 hover:text-foreground/70 hover:border-foreground/25 transition-colors"
            >
              {open ? 'close drawer' : 'open drawer'}
            </button>
          </div>

          {/* Live Drawer — same component used in production */}
          <Drawer
            open={open}
            onClose={() => setOpen(false)}
            title="Drawer"
            width={220}
            zone="orbit.desktop.drawer.demo"
            className="bg-card"
          >
            <div className="flex-1 flex items-center justify-center">
              <span className="text-[10px] text-foreground/20">children</span>
            </div>
          </Drawer>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-0.5 text-[9px] text-foreground/40">
        <code>{'<Drawer open={bool} onClose={fn} width={288} zone="orbit.desktop.drawer.*" className="bg-card">'}</code>
        <code>{'width — number (px) or string ("25%"). Default: 288'}</code>
        <code>{'contentDelayMs — delay children render after open (pass 210 for xterm to avoid resize flicker)'}</code>
        <code>{'children: ReactNode | ((contentReady: boolean) => ReactNode)'}</code>
      </div>
    </Section>
  )
}

// ── Main view ─────────────────────────────────────────────────────────────────

export function UIKitView() {
  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-8 py-8 flex flex-col gap-8">

        <div>
          <h1 className="text-sm font-semibold text-foreground">UI Kit</h1>
          <p className="text-xs text-foreground/40 mt-0.5">
            All components, tokens, and patterns used in orbit desktop.
          </p>
        </div>

        {/* ═══ Primitives ═══════════════════════════════════════ */}

        {/* ── Button ───────────────────────────────────────────── */}
        <Section title="Button">
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-2 flex-wrap">
              <Button size="sm">Default</Button>
              <Button size="sm" variant="secondary">Secondary</Button>
              <Button size="sm" variant="outline">Outline</Button>
              <Button size="sm" variant="ghost">Ghost</Button>
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <Button size="xs">XS default</Button>
              <Button size="xs" variant="secondary">XS secondary</Button>
              <Button size="xs" variant="outline">XS outline</Button>
              <Button size="xs" variant="ghost">XS ghost</Button>
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <Button size="xs" className="bg-primary text-primary-foreground">Primary</Button>
              <Button size="xs" className="bg-green-600 text-white hover:bg-green-700">Success</Button>
              <Button size="xs" variant="destructive">Destructive</Button>
              <Button size="xs" className="bg-yellow-500 text-black hover:bg-yellow-600">Warning</Button>
            </div>
            <div className="max-w-xs">
              <Button size="sm" variant="outline" className="w-full">Block outline</Button>
            </div>
          </div>
        </Section>

        {/* ── Input ────────────────────────────────────────────── */}
        <Section title="Input">
          <div className="flex flex-col gap-3 max-w-xs">
            <Input placeholder="Default input" />
            <Input placeholder="Disabled input" disabled />
            <Input placeholder="Font mono" className="font-mono" />
          </div>
        </Section>

        {/* ── Textarea ─────────────────────────────────────────── */}
        <Section title="Textarea">
          <div className="flex flex-col gap-3 max-w-xs">
            <Textarea placeholder="Enter a description…" rows={3} />
            <Textarea placeholder="Disabled" disabled rows={2} />
          </div>
        </Section>

        {/* ── Select ───────────────────────────────────────────── */}
        <Section title="Select">
          <div className="max-w-xs">
            <Select>
              <SelectTrigger>
                <SelectValue placeholder="Select engine…" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="claude">Claude</SelectItem>
                <SelectItem value="gemini">Gemini</SelectItem>
                <SelectItem value="opencode">OpenCode</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </Section>

        {/* ── Checkbox + Label ─────────────────────────────────── */}
        <Section title="Checkbox + Label">
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-2">
              <Checkbox id="ck-checked" defaultChecked />
              <Label htmlFor="ck-checked" className="text-xs text-foreground/70 cursor-pointer">Checked</Label>
            </div>
            <div className="flex items-center gap-2">
              <Checkbox id="ck-unchecked" />
              <Label htmlFor="ck-unchecked" className="text-xs text-foreground/70 cursor-pointer">Unchecked</Label>
            </div>
            <div className="flex items-center gap-2">
              <Checkbox id="ck-disabled" disabled />
              <Label htmlFor="ck-disabled" className="text-xs text-foreground/50 cursor-not-allowed">Disabled</Label>
            </div>
          </div>
        </Section>

        {/* ── Toggle ───────────────────────────────────────────── */}
        <Section title="Toggle">
          <div className="flex items-center gap-3 flex-wrap">
            <Toggle aria-label="off">Off</Toggle>
            <Toggle aria-label="on" data-state="on">On</Toggle>
            <Toggle aria-label="outline off" variant="outline">Outline off</Toggle>
            <Toggle aria-label="outline on" variant="outline" data-state="on">Outline on</Toggle>
            <Toggle aria-label="sm" size="sm">SM</Toggle>
          </div>
        </Section>

        {/* ── ToggleGroup ──────────────────────────────────────── */}
        <Section title="ToggleGroup">
          <div className="flex flex-col gap-3">
            <Group label="Compact strip (ViewModeToggle style)">
              <div className="shrink-0 w-fit rounded border border-sidebar-border/50 overflow-hidden">
                <ToggleGroup type="single" defaultValue="all" className="gap-0">
                  <ToggleGroupItem value="all" className="text-[9px] h-[18px] px-1.5 rounded-none border-r border-sidebar-border/50">All</ToggleGroupItem>
                  <ToggleGroupItem value="scope" className="text-[9px] h-[18px] px-1.5 rounded-none">Scope</ToggleGroupItem>
                </ToggleGroup>
              </div>
            </Group>
            <Group label="Standard">
              <ToggleGroup type="single" defaultValue="a">
                <ToggleGroupItem value="a">A</ToggleGroupItem>
                <ToggleGroupItem value="b">B</ToggleGroupItem>
                <ToggleGroupItem value="c">C</ToggleGroupItem>
              </ToggleGroup>
            </Group>
          </div>
        </Section>

        {/* ── Kbd ──────────────────────────────────────────────── */}
        <Section title="Kbd">
          <div className="flex items-center gap-2 flex-wrap">
            <Kbd>Ctrl</Kbd>
            <span className="text-xs text-foreground/30">+</span>
            <Kbd>K</Kbd>
            <Separator orientation="vertical" className="h-4 mx-1" />
            <Kbd>⌘</Kbd>
            <span className="text-xs text-foreground/30">+</span>
            <Kbd>P</Kbd>
            <Separator orientation="vertical" className="h-4 mx-1" />
            <Kbd className="text-[9px] px-1 py-px border-foreground/20 text-foreground/50">Esc</Kbd>
          </div>
        </Section>

        {/* ── Separator ────────────────────────────────────────── */}
        <Section title="Separator">
          <div className="flex flex-col gap-3 max-w-xs">
            <div>
              <Separator />
              <code className="text-[9px] text-foreground/40 mt-1 block">horizontal (default) — bg-border</code>
            </div>
            <div className="flex items-center gap-3 h-6">
              <span className="text-xs text-foreground/50">A</span>
              <Separator orientation="vertical" />
              <span className="text-xs text-foreground/50">B</span>
              <code className="text-[9px] text-foreground/40">vertical</code>
            </div>
          </div>
        </Section>

        {/* ── Skeleton ─────────────────────────────────────────── */}
        <Section title="Skeleton">
          <div className="flex flex-col gap-2 max-w-xs">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-1/2" />
            <div className="flex items-center gap-2 mt-1">
              <Skeleton className="w-8 h-8 rounded-full shrink-0" />
              <div className="flex flex-col gap-1.5 flex-1">
                <Skeleton className="h-3 w-full" />
                <Skeleton className="h-3 w-2/3" />
              </div>
            </div>
          </div>
        </Section>

        {/* ── Tooltip ──────────────────────────────────────────── */}
        <Section title="Tooltip">
          <TooltipProvider>
            <div className="flex items-center gap-3">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button size="xs" variant="outline">Hover me</Button>
                </TooltipTrigger>
                <TooltipContent>Tooltip content</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button size="xs" variant="ghost">Side bottom</Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-[11px]">Tab title tooltip</TooltipContent>
              </Tooltip>
            </div>
          </TooltipProvider>
        </Section>

        {/* ═══ Tokens ═══════════════════════════════════════════ */}

        {/* ── Colors ───────────────────────────────────────────── */}
        <Section title="Colors">
          <div className="grid grid-cols-2 gap-x-8 gap-y-2">
            {(
              [
                { label: 'background',        cls: 'bg-background'        },
                { label: 'foreground',         cls: 'bg-foreground'        },
                { label: 'sidebar',            cls: 'bg-sidebar'           },
                { label: 'sidebar-accent',     cls: 'bg-sidebar-accent'    },
                { label: 'card',               cls: 'bg-card'              },
                { label: 'muted',              cls: 'bg-muted'             },
                { label: 'muted-foreground',   cls: 'bg-muted-foreground'  },
                { label: 'primary',            cls: 'bg-primary'           },
                { label: 'border',             cls: 'bg-border'            },
                { label: 'destructive',        cls: 'bg-destructive'       },
              ] as { label: string; cls: string }[]
            ).map(({ label, cls }) => (
              <div key={label} className="flex items-center gap-2.5">
                <div className={`w-4 h-4 rounded-sm shrink-0 border border-foreground/10 ${cls}`} />
                <code className="text-xs text-foreground/50">{label}</code>
              </div>
            ))}
          </div>
        </Section>

        {/* ── Opacity ──────────────────────────────────────────── */}
        <Section title="Opacity">
          <div className="flex flex-col gap-1">
            {([100, 70, 50, 35, 30, 15, 5] as const).map((n) => (
              <div key={n} className="flex items-center gap-3">
                <div className={`w-24 h-5 rounded bg-foreground/${n}`} />
                <code className="text-xs text-foreground/50">foreground/{n}</code>
              </div>
            ))}
          </div>
        </Section>

        {/* ── Typography ───────────────────────────────────────── */}
        <Section title="Typography">
          <div className="flex flex-col gap-2 text-foreground">
            {[
              { cls: 'text-sm',     label: '14px — body' },
              { cls: 'text-xs',     label: '12px — secondary' },
              { cls: 'text-[11px]', label: '11px' },
              { cls: 'text-[10px]', label: '10px — label / header' },
              { cls: 'text-[9px]',  label: '9px — micro / section title' },
            ].map(({ cls, label }) => (
              <div key={label} className="flex items-baseline gap-4">
                <span className={`${cls} w-40`}>{cls}</span>
                <code className="text-[10px] text-foreground/40">{label}</code>
              </div>
            ))}
            <div className="flex items-baseline gap-4">
              <span className="text-[9px] font-semibold uppercase tracking-widest text-foreground/30 w-40">section heading</span>
              <code className="text-[10px] text-foreground/40">text-[9px] semibold uppercase tracking-widest /30</code>
            </div>
            <div className="flex items-baseline gap-4">
              <span className="font-mono text-xs text-foreground/60 w-40">font-mono</span>
              <code className="text-[10px] text-foreground/40">paths, identifiers</code>
            </div>
          </div>
        </Section>

        {/* ── Status & Loading ─────────────────────────────────── */}
        <Section title="Status & Loading">
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-3">
              <span className="text-green-500 text-sm">●</span>
              <code className="text-xs text-foreground/50">text-green-500 ● — active / enabled</code>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-yellow-500 text-sm">○</span>
              <code className="text-xs text-foreground/50">text-yellow-500 ○ — inactive / disabled</code>
            </div>
            <div className="flex items-center gap-3">
              <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />
              <code className="text-xs text-foreground/50">Loader2 h-3 w-3 animate-spin — xs spinner</code>
            </div>
            <div className="flex items-center gap-3">
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
              <code className="text-xs text-foreground/50">Loader2 h-4 w-4 animate-spin — sm spinner</code>
            </div>
          </div>
        </Section>

        {/* ── Border radius ────────────────────────────────────── */}
        <Section title="Border radius">
          <div className="flex gap-4 items-end flex-wrap">
            {(
              [
                { label: 'rounded-sm',   cls: 'rounded-sm'   },
                { label: 'rounded',      cls: 'rounded'      },
                { label: 'rounded-lg',   cls: 'rounded-lg'   },
                { label: 'rounded-xl',   cls: 'rounded-xl'   },
                { label: 'rounded-2xl',  cls: 'rounded-2xl'  },
                { label: 'rounded-full', cls: 'rounded-full' },
              ] as { label: string; cls: string }[]
            ).map(({ label, cls }) => (
              <div key={label} className="flex flex-col items-center gap-1.5">
                <div className={`w-10 h-10 bg-foreground/15 ${cls}`} />
                <code className="text-[9px] text-foreground/40">{label}</code>
              </div>
            ))}
          </div>
        </Section>

        {/* ── Dividers ─────────────────────────────────────────── */}
        <Section title="Dividers">
          <div className="flex flex-col gap-3 max-w-xs">
            <div>
              <div className="h-px bg-foreground/5" />
              <code className="text-[9px] text-foreground/40 mt-1 block">bg-foreground/5 — panel separator</code>
            </div>
            <div>
              <div className="h-px bg-border" />
              <code className="text-[9px] text-foreground/40 mt-1 block">bg-border — standard border</code>
            </div>
          </div>
        </Section>

        {/* ═══ Complex components ═══════════════════════════════ */}

        {/* ── Empty state ──────────────────────────────────────── */}
        <Section title="Empty state">
          <div className="border border-foreground/8 rounded-xl overflow-hidden">
            <Empty>
              <EmptyHeader>
                <EmptyMedia>
                  <SquareTerminal />
                </EmptyMedia>
                <EmptyTitle className="text-sm">No sessions</EmptyTitle>
                <EmptyDescription className="text-xs">Launch a session to get started.</EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button size="sm" variant="outline" className="gap-1.5">
                  <Plus size={13} />New session
                </Button>
              </EmptyContent>
            </Empty>
          </div>
          <code className="text-[9px] text-foreground/35">Empty › EmptyHeader › EmptyMedia (icon circle) + EmptyTitle + EmptyDescription › EmptyContent</code>
        </Section>

        {/* ── SessionItem ──────────────────────────────────────── */}
        <Section title="SessionItem">
          <div className="flex flex-col gap-1 max-w-xs bg-background rounded-xl p-2">
            <PreviewSessionItem />
            <PreviewSessionItem working />
            <PreviewSessionItem active />
            <PreviewSessionItem history />
          </div>
          <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[9px] text-foreground/35">
            <code>default → text-sidebar-foreground/50</code>
            <code>working → STATUS_COLORS.working + animate-ping</code>
            <code>active (isCurrent) → bg-sidebar-accent</code>
            <code>history → text-foreground/25, hollow dot border</code>
          </div>
        </Section>

        {/* ── TabItem ──────────────────────────────────────────── */}
        <Section title="TabItem">
          <div className="flex border border-foreground/8 rounded-xl overflow-hidden h-9 bg-sidebar">
            <PreviewTabItem active title="orbit" icon={<SquareTerminal size={12} />} />
            <PreviewTabItem title="settings" icon={<Settings size={12} />} />
            <PreviewTabItem title="UI Map" icon={<Layers size={12} />} />
          </div>
          <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[9px] text-foreground/35">
            <code>active → border-b-[--tab-indicator] text-foreground</code>
            <code>inactive → border-b-transparent text-foreground/38</code>
            <code>× always visible on active, hover-only on inactive</code>
            <code>right separator span on each inactive tab</code>
          </div>
        </Section>

        {/* ── Rail button ──────────────────────────────────────── */}
        <Section title="Rail button">
          <div className="flex items-center gap-1 bg-sidebar rounded-xl p-2 w-fit">
            <PreviewRailButton icon={<SquareTerminal size={16} />} active />
            <PreviewRailButton icon={<Settings size={16} />} />
            <PreviewRailButton icon={<BookOpen size={16} />} />
            <PreviewRailButton icon={<Layers size={16} />} />
          </div>
          <code className="text-[9px] text-foreground/35">SidebarMenuButton — !size-8 !p-0 !justify-center — active: bg-sidebar-accent text-sidebar-accent-foreground</code>
        </Section>

        {/* ── ViewModeToggle ───────────────────────────────────── */}
        <Section title="ViewModeToggle">
          <div className="flex gap-3 items-center flex-wrap">
            <div className="shrink-0 rounded border border-sidebar-border/50 overflow-hidden">
              <ToggleGroup type="single" value="all" className="gap-0">
                <ToggleGroupItem
                  value="all"
                  className="text-[9px] h-[18px] px-1.5 rounded-none border-r border-sidebar-border/50 bg-sidebar-accent text-sidebar-accent-foreground"
                >All</ToggleGroupItem>
                <ToggleGroupItem
                  value="scope"
                  className="text-[9px] h-[18px] px-1.5 rounded-none bg-transparent text-sidebar-foreground/30"
                >Scope</ToggleGroupItem>
              </ToggleGroup>
            </div>
            <div className="shrink-0 rounded border border-sidebar-border/50 overflow-hidden">
              <ToggleGroup type="single" value="scope" className="gap-0">
                <ToggleGroupItem
                  value="all"
                  className="text-[9px] h-[18px] px-1.5 rounded-none border-r border-sidebar-border/50 bg-transparent text-sidebar-foreground/30"
                >All</ToggleGroupItem>
                <ToggleGroupItem
                  value="scope"
                  className="text-[9px] h-[18px] px-1.5 rounded-none bg-sidebar-accent text-sidebar-accent-foreground"
                >Scope</ToggleGroupItem>
              </ToggleGroup>
            </div>
          </div>
          <code className="text-[9px] text-foreground/35">Shown in panel header when navView ∈ {'{terminal, documents, architecture}'}</code>
        </Section>

        {/* ── ScopeFolder row ──────────────────────────────────── */}
        <Section title="ScopeFolder row">
          <div className="flex flex-col gap-0.5 max-w-xs bg-background rounded-xl p-2">
            <PreviewScopeFolder />
            <PreviewScopeFolder selected />
          </div>
          <div className="grid grid-cols-1 gap-0.5 text-[9px] text-foreground/35">
            <code>hover: bg-sidebar-accent text-sidebar-accent-foreground + ChevronRight</code>
            <code>keyboard selected: ring-1 ring-inset ring-foreground/30</code>
          </div>
        </Section>

        {/* ── DocumentItem row ─────────────────────────────────── */}
        <Section title="DocumentItem row">
          <div className="max-w-xs bg-background rounded-xl p-2">
            <PreviewDocumentItem />
          </div>
          <code className="text-[9px] text-foreground/35">Format badge: text-[10px] uppercase tracking-wider px-1 py-px rounded bg-sidebar-foreground/8</code>
        </Section>

        {/* ── SectionLabel ─────────────────────────────────────── */}
        <Section title="SectionLabel">
          <div className="flex flex-col gap-0.5 max-w-xs">
            <div className="px-2 pt-3 pb-0.5">
              <span className="text-[9px] font-medium uppercase tracking-wider text-sidebar-foreground/30">Active</span>
            </div>
            <div className="h-8 rounded-lg bg-foreground/4" />
            <div className="px-2 pt-3 pb-0.5">
              <span className="text-[9px] font-medium uppercase tracking-wider text-sidebar-foreground/30">History</span>
            </div>
            <div className="h-8 rounded-lg bg-foreground/4" />
          </div>
          <code className="text-[9px] text-foreground/35">SessionList group header — text-[9px] font-medium uppercase tracking-wider text-sidebar-foreground/30</code>
        </Section>

        {/* ── DocsPanel button ─────────────────────────────────── */}
        <Section title="DocsPanel button">
          <div className="flex flex-col gap-0.5 max-w-xs bg-background rounded-xl p-2">
            {['UI Map', 'UI Kit', 'Colors'].map((label) => (
              <button
                key={label}
                className="w-full text-left px-2 py-1.5 rounded-md text-xs text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors"
              >
                {label}
              </button>
            ))}
          </div>
          <code className="text-[9px] text-foreground/35">px-2 py-1.5 rounded-md text-xs — hover: bg-sidebar-accent text-sidebar-accent-foreground</code>
        </Section>

        {/* ── Window control (TitleBtn) ─────────────────────────── */}
        <Section title="Window control (TitleBtn)">
          <div className="flex border border-foreground/8 rounded-xl overflow-hidden h-10 w-fit">
            <button className="h-full w-11 flex items-center justify-center text-foreground/40 hover:bg-foreground/10 hover:text-foreground transition-colors">
              <Minus size={11} />
            </button>
            <button className="h-full w-11 flex items-center justify-center text-foreground/40 hover:bg-foreground/10 hover:text-foreground transition-colors">
              <Square size={10} />
            </button>
            <button className="h-full w-11 flex items-center justify-center text-foreground/40 hover:bg-destructive hover:text-destructive-foreground transition-colors">
              <X size={11} />
            </button>
          </div>
          <code className="text-[9px] text-foreground/35">h-full w-11 — Close: hover:bg-destructive hover:text-destructive-foreground</code>
        </Section>

        {/* ── WorkspacePicker buttons ──────────────────────────── */}
        <Section title="WorkspacePicker buttons">
          <div className="flex items-center gap-1.5">
            <button className="h-6 px-2 rounded text-[11px] border bg-foreground/10 text-foreground/80 border-foreground/20">
              AI
            </button>
            <button className="h-6 px-2 rounded text-[11px] border text-foreground/35 border-foreground/12">
              BeFra
            </button>
            <button className="h-6 px-2 flex items-center gap-1.5 rounded text-[11px] border border-foreground/12 text-foreground/50">
              <Layers size={10} className="shrink-0" />
              All
            </button>
          </div>
          <div className="grid grid-cols-1 gap-0.5 text-[9px] text-foreground/35">
            <code>quick-access selected: bg-foreground/10 text-foreground/80 border-foreground/20</code>
            <code>quick-access idle: text-foreground/35 border-foreground/12</code>
            <code>dropdown trigger: Layers icon + label + ChevronDown</code>
          </div>
        </Section>

        {/* ── Menus reference ──────────────────────────────────── */}
        <Section title="Menus (reference)">
          <p className="text-xs text-foreground/45 leading-relaxed">
            DropdownMenu, ContextMenu, and ContextMenuSub render in a Radix Portal — not shown inline.
            They share the same <code className="text-[10px] bg-foreground/6 px-1 py-px rounded border border-foreground/10">text-xs gap-2</code> item pattern
            and use <code className="text-[10px] bg-foreground/6 px-1 py-px rounded border border-foreground/10">DropdownMenuShortcut + Kbd</code> for keyboard hints.
          </p>
          <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[9px] text-foreground/40">
            <code>DropdownMenu — Settings rail, WorkspacePicker</code>
            <code>ContextMenu — SessionItem, ScopeFolder, ArchItem</code>
            <code>ContextMenuSub — "New session with…" engine list</code>
            <code>DropdownMenuShortcut + Kbd — Settings items</code>
          </div>
        </Section>

        {/* ═══ Layout patterns ══════════════════════════════════ */}

        {/* ── Principal card ───────────────────────────────────── */}
        <Section title="Layout: principal card">
          <div className="bg-background rounded-2xl ring-4 ring-sidebar-border/40 overflow-hidden h-36 flex flex-col">
            <div className="h-9 bg-sidebar border-b border-sidebar-border/60 flex items-center px-3 shrink-0">
              <span className="text-[10px] text-foreground/30">principal.card.tabs — h-[36px] bg-sidebar</span>
            </div>
            <div className="h-8 bg-sidebar border-b border-sidebar-border/30 flex items-center px-3 shrink-0">
              <span className="text-[10px] text-foreground/30">principal.card.session-header — h-8</span>
            </div>
            <div className="flex-1 bg-background flex items-center justify-center">
              <span className="text-[10px] text-foreground/20">principal.card.content — flex-1</span>
            </div>
          </div>
          <code className="text-[9px] text-foreground/35">rounded-2xl ring-4 ring-sidebar-border/40 — principal card container (orbit.desktop.principal.card)</code>
        </Section>

        {/* ── Sidebar two-column ───────────────────────────────── */}
        <Section title="Layout: sidebar">
          <div className="flex h-44 rounded-xl overflow-hidden border border-foreground/8">
            {/* Rail */}
            <div className="w-[52px] bg-sidebar flex flex-col items-center py-2 gap-1 shrink-0">
              <div className="w-8 h-8 rounded-md bg-sidebar-accent" />
              <div className="w-8 h-8 rounded-md bg-foreground/5" />
              <div className="w-8 h-8 rounded-md bg-foreground/5" />
              <div className="w-8 h-1 mt-auto" />
              <div className="w-8 h-8 rounded-md bg-foreground/5" />
              <div className="w-8 h-8 rounded-md bg-foreground/5" />
            </div>
            {/* Panel */}
            <div className="flex flex-col flex-1 min-h-0 pl-2">
              <div className="h-8 flex items-center px-3 shrink-0 gap-2">
                <span className="text-[9px] text-foreground/30 uppercase tracking-wider flex-1">sessions</span>
                <div className="rounded border border-sidebar-border/50 overflow-hidden flex">
                  <div className="text-[8px] h-[16px] px-1 border-r border-sidebar-border/50 bg-sidebar-accent text-sidebar-accent-foreground flex items-center">All</div>
                  <div className="text-[8px] h-[16px] px-1 text-sidebar-foreground/30 flex items-center">Scope</div>
                </div>
              </div>
              <div className="flex-1 flex flex-col gap-1 p-2 overflow-hidden">
                <div className="h-10 rounded-lg bg-sidebar-accent/50" />
                <div className="h-10 rounded-lg bg-foreground/4" />
              </div>
              <div className="h-9 border-t border-foreground/8 flex items-center px-2 shrink-0">
                <div className="w-full h-7 rounded-md border border-foreground/12 flex items-center justify-center gap-1">
                  <Plus size={11} className="text-foreground/30" />
                  <span className="text-[10px] text-foreground/30">New shell</span>
                </div>
              </div>
            </div>
          </div>
          <div className="grid grid-cols-1 gap-0.5 text-[9px] text-foreground/35">
            <code>rail: w-[52px] bg-sidebar rounded-2xl ring-1 ring-sidebar-border/80 (orbit.desktop.sidebar.rail)</code>
            <code>panel: flex-1 flex-col — header + scrollable content + footer (orbit.desktop.sidebar.panel.*)</code>
          </div>
        </Section>

        {/* ── Drawer sibling ───────────────────────────────────── */}
        <Section title="Layout: drawer sibling">
          <div className="flex h-28 gap-1 rounded-xl overflow-hidden border border-foreground/8">
            <div className="flex-1 bg-background flex items-center justify-center">
              <span className="text-[10px] text-foreground/20">principal</span>
            </div>
            <div className="w-[25%] bg-sidebar border-l border-foreground/8 flex items-center justify-center">
              <span className="text-[10px] text-foreground/30">drawer</span>
            </div>
          </div>
          <div className="grid grid-cols-1 gap-0.5 text-[9px] text-foreground/35">
            <code>open: mount → double rAF → setVisible(true) → CSS transition width+opacity 200ms</code>
            <code>close: setVisible(false) → wait 210ms → unmount (keeps flex row stable)</code>
            <code>terminal drawer: 25% width — arch-editor drawer: 288px fixed</code>
          </div>
        </Section>

        {/* ── Drawer component ─────────────────────────────────── */}
        <DrawerDemo />

      </div>
    </div>
  )
}
