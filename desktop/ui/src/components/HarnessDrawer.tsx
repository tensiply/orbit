import { useState, useEffect } from 'react'
import { X, CheckCircle2, Circle, Loader2, AlertCircle, ChevronDown, ChevronRight } from 'lucide-react'
import { useAppStore } from '../store'
import type { HarnessReport } from '../types'

// ── collapsible section ───────────────────────────────────────────────────────

function Section({ title, count, children, defaultOpen = true }: {
  title: string
  count?: number
  children: React.ReactNode
  defaultOpen?: boolean
}) {
  const [open, setOpen] = useState(defaultOpen)
  return (
    <div className="border-b border-border/30 last:border-b-0">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1.5 w-full px-3 py-2 text-left hover:bg-foreground/5 transition-colors"
      >
        {open
          ? <ChevronDown  size={11} className="text-foreground/25 shrink-0" />
          : <ChevronRight size={11} className="text-foreground/25 shrink-0" />}
        <span className="text-[10px] font-semibold uppercase tracking-wider text-foreground/40">{title}</span>
        {count !== undefined && (
          <span className="ml-auto text-[10px] text-foreground/25">{count}</span>
        )}
      </button>
      {open && <div className="pb-2">{children}</div>}
    </div>
  )
}

// ── label / value row ─────────────────────────────────────────────────────────

function Row({ label, value, dim }: { label: string; value: string; dim?: boolean }) {
  return (
    <div className="flex gap-2 px-3 py-0.5 text-[11px]">
      <span className="text-foreground/30 shrink-0 w-20 text-right">{label}</span>
      <span className={`font-mono truncate ${dim ? 'text-foreground/25' : 'text-foreground/65'}`}>{value}</span>
    </div>
  )
}

// ── layer row ─────────────────────────────────────────────────────────────────

function LayerRow({ path, exists, label }: { path: string; exists: boolean; label: string }) {
  return (
    <div className="flex items-center gap-2 px-3 py-0.5">
      {exists
        ? <CheckCircle2 size={11} className="text-green-500/70 shrink-0" />
        : <Circle       size={11} className="text-foreground/15 shrink-0" />}
      <span className={`font-mono text-[10px] truncate flex-1 ${exists ? 'text-foreground/60' : 'text-foreground/20'}`}>{path}</span>
      <span className="text-[9px] text-foreground/20 shrink-0 pl-1">{label}</span>
    </div>
  )
}

// ── harness content ───────────────────────────────────────────────────────────

function HarnessContent({ report }: { report: HarnessReport }) {
  const {
    scope, config_layers, mcp_layers, agent_overlay_dirs, instructions,
    mcp_servers, env_vars, commands, engine_hooks,
    plugin_context, activity_preview,
  } = report

  const loadedConfig   = config_layers.filter((l) => l.exists)
  const loadedMcp      = mcp_layers.filter((l) => l.exists)
  const loadedOverlays = agent_overlay_dirs.filter((l) => l.exists)
  const loadedInst     = instructions.filter((i) => i.exists)

  return (
    <div className="flex-1 overflow-y-auto text-xs">

      {/* ── Scope ── */}
      <Section title="Scope">
        <Row label="workspace"  value={scope.workspace} />
        {scope.tenant     && <Row label="tenant"     value={scope.tenant} />}
        {scope.project    && <Row label="project"    value={scope.project} />}
        {scope.repository && <Row label="repository" value={scope.repository} />}
        <Row label="engine"   value={scope.engine} />
        <Row label="auth"     value={scope.auth_status} dim={scope.auth_status === 'not configured'} />
        <Row label="work dir" value={scope.work_dir} />
        <div className="mt-1 mx-3 px-2 py-1.5 rounded bg-foreground/5 font-mono text-[10px] text-foreground/45 break-all leading-relaxed">
          {scope.exec_cmd}
        </div>
      </Section>

      {/* ── Config layers ── */}
      <Section title="Config layers" count={loadedConfig.length}>
        {loadedConfig.length === 0
          ? <p className="px-3 text-[10px] text-foreground/20 italic">none loaded</p>
          : loadedConfig.map((l, i) => <LayerRow key={i} {...l} />)}
      </Section>

      {/* ── Agent overlays ── */}
      {loadedOverlays.length > 0 && (
        <Section title="Agent overlays" count={loadedOverlays.length}>
          {loadedOverlays.map((l, i) => <LayerRow key={i} {...l} />)}
        </Section>
      )}

      {/* ── MCP layers ── */}
      <Section title="MCP layers" count={loadedMcp.length}>
        {loadedMcp.length === 0
          ? <p className="px-3 text-[10px] text-foreground/20 italic">none loaded</p>
          : loadedMcp.map((l, i) => <LayerRow key={i} {...l} />)}
      </Section>

      {/* ── Instructions ── */}
      <Section title="Instructions" count={loadedInst.length} defaultOpen={false}>
        {loadedInst.length === 0
          ? <p className="px-3 text-[10px] text-foreground/20 italic">none</p>
          : loadedInst.map((inst, i) => (
            <div key={i} className="flex items-center gap-2 px-3 py-0.5">
              <CheckCircle2 size={11} className="text-green-500/70 shrink-0" />
              <span className="font-mono text-[10px] truncate text-foreground/60">{inst.path}</span>
            </div>
          ))}
      </Section>

      {/* ── MCP servers ── */}
      <Section title="MCP Servers" count={mcp_servers.length} defaultOpen={false}>
        {mcp_servers.length === 0
          ? <p className="px-3 text-[10px] text-foreground/20 italic">none</p>
          : mcp_servers.map((s, i) => (
            <div key={i} className="flex flex-col gap-0.5 px-3 py-1 border-b border-border/20 last:border-b-0">
              <div className="flex items-center gap-2">
                <CheckCircle2 size={11} className="text-green-500/70 shrink-0" />
                <span className="text-[11px] font-medium text-foreground/65">{s.name}</span>
                <span className="ml-auto text-[9px] text-foreground/25">{s.source}</span>
              </div>
              <span className="pl-5 font-mono text-[10px] text-foreground/35 truncate">{s.command.join(' ')}</span>
            </div>
          ))}
      </Section>

      {/* ── Env vars ── */}
      {env_vars.length > 0 && (
        <Section title="Env vars" count={env_vars.length} defaultOpen={false}>
          {env_vars.map((v, i) => (
            <div key={i} className="flex items-center gap-2 px-3 py-0.5">
              <CheckCircle2 size={11} className="text-green-500/70 shrink-0" />
              <span className="font-mono text-[10px] text-foreground/55 shrink-0">{v.key}</span>
              <span className={`font-mono text-[10px] truncate ${v.redacted ? 'text-foreground/20 italic' : 'text-foreground/35'}`}>
                {v.redacted ? '<redacted>' : v.value}
              </span>
            </div>
          ))}
        </Section>
      )}

      {/* ── Commands ── */}
      <Section title="Commands" count={commands.length} defaultOpen={false}>
        {commands.length === 0
          ? <p className="px-3 text-[10px] text-foreground/20 italic">none</p>
          : commands.map((c, i) => (
            <div key={i} className="flex items-center gap-2 px-3 py-0.5">
              <span className="text-[10px] text-green-500/55">/</span>
              <span className="font-mono text-[10px] text-foreground/60 flex-1">{c.name}</span>
              <span className="text-[9px] text-foreground/20 shrink-0">{c.source}</span>
            </div>
          ))}
      </Section>

      {/* ── Engine hooks ── */}
      {scope.engine === 'claude' && (
        <Section title="Engine hooks" count={engine_hooks.length} defaultOpen={false}>
          {engine_hooks.length === 0
            ? <p className="px-3 text-[10px] text-foreground/20 italic">none enabled</p>
            : engine_hooks.map((h, i) => (
              <div key={i} className="flex items-center gap-2 px-3 py-0.5">
                <CheckCircle2 size={11} className="text-green-500/70 shrink-0" />
                <span className="text-[11px] text-foreground/60 flex-1">{h.name}</span>
                <span className="text-[9px] text-foreground/25 shrink-0">{h.events}</span>
              </div>
            ))}
        </Section>
      )}

      {/* ── Plugin context (dynamic injections) ── */}
      {plugin_context.length > 0 && (
        <Section title="Plugin context" count={plugin_context.length} defaultOpen={false}>
          <p className="px-3 pb-1 text-[9px] text-foreground/20 italic">injected at launch — not in config layers</p>
          {plugin_context.map((p, i) => (
            <div key={i} className="flex flex-col gap-0.5 px-3 py-1 border-b border-border/15 last:border-b-0">
              <div className="flex items-center gap-2">
                <CheckCircle2 size={11} className="text-blue-400/60 shrink-0" />
                <span className="text-[11px] font-medium text-foreground/60">{p.name}</span>
              </div>
              {p.prompt_preview && (
                <p className="pl-5 text-[10px] text-foreground/30 italic leading-relaxed">{p.prompt_preview}</p>
              )}
              {p.instruction_files.map((f, j) => (
                <span key={j} className="pl-5 font-mono text-[10px] text-foreground/30 truncate">{f}</span>
              ))}
            </div>
          ))}
        </Section>
      )}

      {/* ── Activity context ── */}
      {activity_preview.length > 0 && (
        <Section title="Activity context" count={activity_preview.length} defaultOpen={false}>
          <p className="px-3 pb-1 text-[9px] text-foreground/20 italic">last {activity_preview.length} sessions injected at launch</p>
          {activity_preview.map((entry, i) => (
            <div key={i} className="px-3 py-0.5">
              <span className="font-mono text-[10px] text-foreground/35">{entry}</span>
            </div>
          ))}
        </Section>
      )}
    </div>
  )
}

// ── main drawer ───────────────────────────────────────────────────────────────

export function HarnessDrawer() {
  const open        = useAppStore((s) => s.harnessDrawerOpen)
  const session     = useAppStore((s) => s.harnessSession)
  const report      = useAppStore((s) => s.harnessReport)
  const loading     = useAppStore((s) => s.harnessLoading)
  const error       = useAppStore((s) => s.harnessError)
  const closeDrawer = useAppStore((s) => s.closeHarnessDrawer)

  const [mounted, setMounted] = useState(false)
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    if (open) {
      setMounted(true)
      requestAnimationFrame(() => requestAnimationFrame(() => setVisible(true)))
    } else {
      setVisible(false)
      const t = setTimeout(() => setMounted(false), 210)
      return () => clearTimeout(t)
    }
  }, [open])

  if (!mounted) return null

  const label = session
    ? [session.tenant, session.project, session.repository].filter(Boolean).join(' › ') || session.work_dir
    : 'Harness'

  return (
    <div
      data-orbit-zone="orbit.desktop.drawer.harness"
      className="relative flex flex-col shrink-0 overflow-hidden rounded-2xl bg-card transition-all duration-200 ease-in-out"
      style={{ width: visible ? 288 : 0, opacity: visible ? 1 : 0 }}
    >
      {/* Floating close button — mirrors ArchEditDrawer */}
      <button
        onClick={closeDrawer}
        className="absolute top-2 right-2 z-10 p-1 rounded-md text-foreground/20 hover:text-foreground/60 hover:bg-foreground/5 transition-colors"
        aria-label="Close harness drawer"
      >
        <X size={12} />
      </button>

      {/* Header */}
      <div className="flex flex-col gap-0 px-3 pt-3 pb-2 border-b border-border/30 shrink-0">
        <span className="text-[9px] font-semibold uppercase tracking-widest text-foreground/25">Harness</span>
        <span className="text-xs font-medium text-foreground/60 truncate pr-6">{label}</span>
      </div>

      {/* Body */}
      {loading && (
        <div className="flex-1 flex items-center justify-center gap-2 text-foreground/30">
          <Loader2 size={14} className="animate-spin" />
          <span className="text-[11px]">Loading harness…</span>
        </div>
      )}
      {error && !loading && (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4 text-center">
          <AlertCircle size={18} className="text-destructive/60" />
          <p className="text-[11px] text-foreground/40">{error}</p>
        </div>
      )}
      {report && !loading && <HarnessContent report={report} />}
    </div>
  )
}
