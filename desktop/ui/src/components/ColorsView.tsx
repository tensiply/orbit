import { STATUS_COLORS, ARCH_KIND_COLORS, TERMINAL_THEME_DARK, TERMINAL_THEME_LIGHT } from '../theme'
import { useAppStore } from '../store'

function Section({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-4">
      <div>
        <p className="text-[9px] font-semibold text-foreground/30 uppercase tracking-widest">{title}</p>
        {description && <p className="text-[10px] text-foreground/40 mt-0.5">{description}</p>}
      </div>
      {children}
    </div>
  )
}

function Swatch({ label, cls, cssVar, hex }: { label: string; cls?: string; cssVar?: string; hex?: string }) {
  return (
    <div className="flex items-center gap-3 group">
      <div
        className={`w-8 h-8 rounded-md border border-foreground/10 shrink-0 ${cls ?? ''}`}
        style={hex ? { backgroundColor: hex } : undefined}
      />
      <div className="flex flex-col min-w-0">
        <span className="text-xs text-foreground/80 font-medium">{label}</span>
        {cssVar && <code className="text-[10px] text-foreground/35 font-mono">{cssVar}</code>}
        {hex    && <code className="text-[10px] text-foreground/35 font-mono">{hex}</code>}
      </div>
    </div>
  )
}

function SwatchRow({ items }: { items: { label: string; cls?: string; cssVar?: string; hex?: string }[] }) {
  return (
    <div className="grid grid-cols-2 gap-x-12 gap-y-3">
      {items.map((s) => <Swatch key={s.label} {...s} />)}
    </div>
  )
}

function PaletteRow({ colors, label }: { colors: Record<string, string>; label: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-[10px] text-foreground/40 font-mono">{label}</span>
      <div className="flex gap-1 flex-wrap">
        {Object.entries(colors).filter(([k]) => k !== '_default').map(([key, hex]) => (
          <div key={key} className="flex flex-col items-center gap-1">
            <div
              className="w-7 h-7 rounded border border-foreground/10"
              style={{ backgroundColor: hex }}
              title={`${key}: ${hex}`}
            />
            <span className="text-[8px] text-foreground/35 text-center leading-tight w-12 truncate">{key}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

function TerminalPaletteRow({ theme, label }: { theme: Record<string, string>; label: string }) {
  const ANSI_KEYS = ['black','red','green','yellow','blue','magenta','cyan','white']
  const BRIGHT_KEYS = ANSI_KEYS.map((k) => 'bright' + k.charAt(0).toUpperCase() + k.slice(1))
  return (
    <div className="flex flex-col gap-2">
      <span className="text-[10px] text-foreground/40 font-mono">{label}</span>
      <div className="rounded-lg overflow-hidden border border-foreground/10" style={{ backgroundColor: theme.background }}>
        <div className="px-3 py-2 flex gap-1.5 items-center border-b border-foreground/10">
          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: theme.red }} />
          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: theme.yellow }} />
          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: theme.green }} />
          <span className="text-[10px] ml-1 font-mono" style={{ color: theme.foreground, opacity: 0.5 }}>terminal preview</span>
        </div>
        <div className="px-3 py-3 flex flex-col gap-2">
          {/* ANSI normal */}
          <div className="flex gap-1">
            {ANSI_KEYS.map((k) => (
              <div key={k} title={k} className="w-5 h-5 rounded-sm" style={{ backgroundColor: theme[k] }} />
            ))}
          </div>
          {/* ANSI bright */}
          <div className="flex gap-1">
            {BRIGHT_KEYS.map((k) => (
              <div key={k} title={k} className="w-5 h-5 rounded-sm" style={{ backgroundColor: theme[k] }} />
            ))}
          </div>
          {/* Sample text */}
          <div className="mt-1 font-mono text-[10px] leading-relaxed" style={{ color: theme.foreground }}>
            <span>$ </span>
            <span style={{ color: theme.green }}>orbit launch</span>
            <span style={{ color: theme.cyan }}> AI AIDEV AI-ECOSYSTEM orbit</span>
            <br />
            <span style={{ color: theme.yellow }}>warning</span>
            <span>: no daemon found, starting... </span>
            <span style={{ color: theme.red }}>error</span>
            <span>: timeout</span>
          </div>
          {/* Cursor + selection demo */}
          <div className="flex items-center gap-2">
            <div className="w-2 h-3 rounded-sm" style={{ backgroundColor: theme.cursor }} title="cursor" />
            <div className="rounded px-1 text-[9px] font-mono" style={{ backgroundColor: theme.selectionBackground, color: theme.foreground }}>
              selected text
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export function ColorsView() {
  const theme = useAppStore((s) => s.theme)

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-8 py-8 flex flex-col gap-10">

        <div>
          <h1 className="text-sm font-semibold text-foreground">Colors</h1>
          <p className="text-xs text-foreground/40 mt-0.5">
            Design tokens, semantic aliases, and palette references used across orbit desktop.
            Active theme: <code className="font-mono">{theme}</code>
          </p>
        </div>

        {/* ── Layout tokens ──────────────────────────────────────────────── */}
        <Section title="Layout tokens" description="CSS custom properties — change with the active theme.">
          <SwatchRow items={[
            { label: 'background',            cls: 'bg-background',            cssVar: '--background' },
            { label: 'foreground',             cls: 'bg-foreground',            cssVar: '--foreground' },
            { label: 'card',                   cls: 'bg-card',                  cssVar: '--card' },
            { label: 'card-foreground',        cls: 'bg-card-foreground',       cssVar: '--card-foreground' },
            { label: 'popover',                cls: 'bg-popover',               cssVar: '--popover' },
            { label: 'muted',                  cls: 'bg-muted',                 cssVar: '--muted' },
            { label: 'muted-foreground',       cls: 'bg-muted-foreground',      cssVar: '--muted-foreground' },
            { label: 'accent',                 cls: 'bg-accent',                cssVar: '--accent' },
            { label: 'secondary',              cls: 'bg-secondary',             cssVar: '--secondary' },
            { label: 'primary',                cls: 'bg-primary',               cssVar: '--primary' },
            { label: 'primary-foreground',     cls: 'bg-primary-foreground',    cssVar: '--primary-foreground' },
            { label: 'destructive',            cls: 'bg-destructive',           cssVar: '--destructive' },
            { label: 'border',                 cls: 'bg-border',                cssVar: '--border' },
            { label: 'input',                  cls: 'bg-input',                 cssVar: '--input' },
            { label: 'ring',                   cls: 'bg-ring',                  cssVar: '--ring' },
          ]} />
        </Section>

        {/* ── Sidebar tokens ─────────────────────────────────────────────── */}
        <Section title="Sidebar tokens">
          <SwatchRow items={[
            { label: 'sidebar',                   cls: 'bg-sidebar',                      cssVar: '--sidebar' },
            { label: 'sidebar-foreground',        cls: 'bg-sidebar-foreground',           cssVar: '--sidebar-foreground' },
            { label: 'sidebar-primary',           cls: 'bg-sidebar-primary',              cssVar: '--sidebar-primary' },
            { label: 'sidebar-accent',            cls: 'bg-sidebar-accent',               cssVar: '--sidebar-accent' },
            { label: 'sidebar-accent-foreground', cls: 'bg-sidebar-accent-foreground',    cssVar: '--sidebar-accent-foreground' },
            { label: 'sidebar-border',            cls: 'bg-sidebar-border',               cssVar: '--sidebar-border' },
          ]} />
        </Section>

        {/* ── Semantic surface aliases ────────────────────────────────────── */}
        <Section title="Surface aliases" description="Shortcuts defined in @theme inline — single-word class names for common surface levels.">
          <SwatchRow items={[
            { label: 'dark  → var(--background)', cls: 'bg-dark',  cssVar: 'bg-dark' },
            { label: 'gray  → var(--sidebar)',    cls: 'bg-gray',  cssVar: 'bg-gray' },
            { label: 'light → var(--card)',       cls: 'bg-light', cssVar: 'bg-light' },
          ]} />
          <div className="flex flex-col gap-1.5 mt-1">
            <span className="text-[10px] text-foreground/40">Foreground opacity scale</span>
            <div className="flex gap-1 items-end">
              {[80, 60, 50, 40, 30, 20, 10, 5].map((n) => (
                <div key={n} className="flex flex-col items-center gap-1">
                  <div className={`w-7 h-7 rounded bg-foreground/${n}`} />
                  <span className="text-[8px] text-foreground/35">{n}</span>
                </div>
              ))}
            </div>
          </div>
        </Section>

        {/* ── Special tokens ─────────────────────────────────────────────── */}
        <Section title="Special tokens">
          <SwatchRow items={[
            { label: 'tab-indicator',      cls: 'bg-tab-indicator',       cssVar: '--tab-indicator' },
            { label: 'document-viewer-bg', cls: 'bg-document-viewer-bg',  cssVar: '--document-viewer-bg' },
          ]} />
        </Section>

        {/* ── Status colors ──────────────────────────────────────────────── */}
        <Section title="Status colors" description="Defined in theme.ts — used in session status indicators and workspace dots.">
          <div className="flex flex-col gap-3">
            {Object.entries(STATUS_COLORS).map(([key, hex]) => (
              <div key={key} className="flex items-center gap-3">
                <div className="relative flex items-center justify-center shrink-0">
                  <span className="absolute w-5 h-5 rounded-full animate-ping opacity-30" style={{ backgroundColor: hex }} />
                  <span className="w-3 h-3 rounded-full relative" style={{ backgroundColor: hex }} />
                </div>
                <div>
                  <span className="text-xs text-foreground/80 font-medium">STATUS_COLORS.{key}</span>
                  <code className="text-[10px] text-foreground/35 font-mono ml-2">{hex}</code>
                </div>
              </div>
            ))}
          </div>
        </Section>

        {/* ── Architecture kind colors ────────────────────────────────────── */}
        <Section title="Architecture kind colors" description="ARCH_KIND_COLORS in theme.ts — used in the minimap node colors.">
          <PaletteRow colors={ARCH_KIND_COLORS} label="ARCH_KIND_COLORS" />
          <div className="flex gap-1 flex-wrap mt-1">
            <div className="flex items-center gap-1.5 text-[10px] text-foreground/35">
              <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: ARCH_KIND_COLORS._default }} />
              <code>_default</code>
            </div>
          </div>
          {/* Kind badge preview */}
          <div className="flex flex-col gap-2 mt-2">
            <span className="text-[10px] text-foreground/40">Kind chip preview (responds to theme)</span>
            <div className="flex flex-wrap gap-1.5">
              {(['Services','Databases','Integrations','Infrastructure','APIs','Pipelines','Secrets','IAM','Teams'] as const).map((k) => {
                const chipCls: Record<string, string> = {
                  Services:       'bg-blue-100    text-blue-700    dark:bg-blue-900/50    dark:text-blue-300',
                  Databases:      'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/50 dark:text-emerald-300',
                  Integrations:   'bg-purple-100  text-purple-700  dark:bg-purple-900/50  dark:text-purple-300',
                  Infrastructure: 'bg-orange-100  text-orange-700  dark:bg-orange-900/50  dark:text-orange-300',
                  APIs:           'bg-cyan-100    text-cyan-700    dark:bg-cyan-900/50    dark:text-cyan-300',
                  Pipelines:      'bg-amber-100   text-amber-700   dark:bg-amber-900/50   dark:text-amber-300',
                  Secrets:        'bg-red-100     text-red-700     dark:bg-red-900/50     dark:text-red-300',
                  IAM:            'bg-pink-100    text-pink-700    dark:bg-pink-900/50    dark:text-pink-300',
                  Teams:          'bg-zinc-200    text-zinc-600    dark:bg-zinc-800/50    dark:text-zinc-300',
                }
                return (
                  <span key={k} className={`text-[10px] px-2 py-0.5 rounded-full font-medium ${chipCls[k]}`}>
                    {k}
                  </span>
                )
              })}
            </div>
          </div>
        </Section>

        {/* ── Terminal palettes ───────────────────────────────────────────── */}
        <Section title="Terminal palettes" description="TERMINAL_THEME_DARK / TERMINAL_THEME_LIGHT in theme.ts — passed directly to xterm.js (hex only).">
          <div className="grid grid-cols-1 gap-6">
            <TerminalPaletteRow theme={TERMINAL_THEME_DARK}  label="TERMINAL_THEME_DARK" />
            <TerminalPaletteRow theme={TERMINAL_THEME_LIGHT} label="TERMINAL_THEME_LIGHT" />
          </div>
        </Section>

      </div>
    </div>
  )
}
