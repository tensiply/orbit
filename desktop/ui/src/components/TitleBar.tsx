import { useMemo, useState, useEffect } from 'react'
import { STATUS_COLORS } from '../theme'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Minus, Square, X, ChevronDown, Check, Layers } from 'lucide-react'
import { useAppStore } from '../store'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from './ui/dropdown-menu'
import { Button } from './ui/button'
import { cn } from '@/lib/utils'

const win = getCurrentWindow()

function TitleBtn({
  onClick,
  title,
  danger,
  children,
}: {
  onClick: () => void
  title: string
  danger?: boolean
  children: React.ReactNode
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`h-full w-11 flex items-center justify-center text-foreground/40 transition-colors ${
        danger
          ? 'hover:bg-destructive hover:text-destructive-foreground'
          : 'hover:bg-foreground/10 hover:text-foreground'
      }`}
    >
      {children}
    </button>
  )
}

function workspaceFromWorkDir(workDir: string): string | null {
  const parts = workDir.split('/').filter(Boolean)
  return parts.length >= 3 && parts[0] === 'home' ? parts[2] : null
}

function WorkspacePicker() {
  const sessions             = useAppStore((s) => s.sessions)
  const registeredWorkspaces = useAppStore((s) => s.registeredWorkspaces)
  const selectedWorkspace    = useAppStore((s) => s.selectedWorkspace)
  const setSelectedWorkspace = useAppStore((s) => s.setSelectedWorkspace)
  const blurSidebar          = useAppStore((s) => s.blurSidebar)
  const [menuOpen, setMenuOpen] = useState(false)

  const handleMenuOpenChange = (open: boolean) => {
    if (open) blurSidebar()
    setMenuOpen(open)
  }

  // Workspaces with at least one active (non-history) session
  const openWorkspaceNames = useMemo(() => {
    const set = new Set<string>()
    for (const s of sessions) {
      if (s.is_history) continue
      const ws = workspaceFromWorkDir(s.work_dir)
      if (ws) set.add(ws)
    }
    return set
  }, [sessions])

  // Alt+W opens this dropdown
  useEffect(() => {
    const handler = () => setMenuOpen(true)
    window.addEventListener('orbit:open-workspace-menu', handler)
    return () => window.removeEventListener('orbit:open-workspace-menu', handler)
  }, [])

  if (registeredWorkspaces.length === 0) return null

  return (
    <div data-orbit-zone="orbit.desktop.principal.titlebar.workspace" className="flex items-center h-full gap-1.5 pr-2">
      {/* Quick-access button for each open workspace */}
      {registeredWorkspaces
        .filter((ws) => openWorkspaceNames.has(ws.name))
        .map((ws) => {
          const isSelected = selectedWorkspace === ws.name
          return (
            <Button
              key={ws.slug}
              variant="ghost"
              onClick={() => setSelectedWorkspace(ws.name)}
              title={ws.name}
              className={cn(
                'h-6 px-2 text-[11px] truncate max-w-[80px] border',
                isSelected
                  ? 'bg-foreground/10 text-foreground/80 border-foreground/20'
                  : 'text-foreground/35 border-foreground/12 hover:text-foreground/70 hover:bg-foreground/8 hover:border-foreground/20',
              )}
            >
              {ws.name}
            </Button>
          )
        })}

      {/* Workspace selector dropdown — controlled so Alt+W can open it */}
      <DropdownMenu open={menuOpen} onOpenChange={handleMenuOpenChange}>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" className="h-6 px-2 flex items-center gap-1.5 text-[11px] border border-foreground/12 text-foreground/50 hover:text-foreground/80 hover:bg-foreground/8 hover:border-foreground/20 data-[state=open]:bg-foreground/8 data-[state=open]:text-foreground/80 data-[state=open]:border-foreground/20">
            <Layers size={10} className="shrink-0" />
            <span className="max-w-[100px] truncate">
              {selectedWorkspace ?? 'All'}
            </span>
            <ChevronDown size={9} className="shrink-0 opacity-60" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="min-w-[160px]">
          <DropdownMenuItem onClick={() => setSelectedWorkspace(null)}>
            <span className="flex-1">All workspaces</span>
            {selectedWorkspace === null && <Check size={12} className="ml-2 shrink-0" />}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          {registeredWorkspaces.map((ws) => (
            <DropdownMenuItem key={ws.slug} onClick={() => setSelectedWorkspace(ws.name)}>
              <span className="flex-1">{ws.name}</span>
              {openWorkspaceNames.has(ws.name) && (
                <span className="w-1.5 h-1.5 rounded-full ml-1 shrink-0" style={{ backgroundColor: STATUS_COLORS.active }} />
              )}
              {selectedWorkspace === ws.name && <Check size={12} className="ml-2 shrink-0" />}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      data-orbit-zone="orbit.desktop.principal.titlebar"
      className="h-10 flex items-center justify-end select-none shrink-0 bg-sidebar"
    >
      <WorkspacePicker />

      {/* Separator before window controls */}
      <div className="h-4 w-px bg-foreground/10 mx-1 shrink-0" />

      <div data-orbit-zone="orbit.desktop.principal.titlebar.controls" className="flex items-center h-full">
        <TitleBtn onClick={() => win.minimize()} title="Minimize">
          <Minus size={11} />
        </TitleBtn>

        <TitleBtn onClick={() => win.toggleMaximize()} title="Maximize">
          <Square size={10} />
        </TitleBtn>

        <TitleBtn onClick={() => win.close()} title="Close" danger>
          <X size={11} />
        </TitleBtn>
      </div>
    </div>
  )
}
