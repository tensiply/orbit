import { useAppStore } from '../store'
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from './ui/dialog'
import {
  ClaudeEngineIcon,
  GeminiEngineIcon,
  OpenCodeEngineIcon,
} from '../icons'
import { Button } from './ui/button'

const ENGINES = [
  { id: 'claude',   label: 'Claude',   Icon: ClaudeEngineIcon },
  { id: 'gemini',   label: 'Gemini',   Icon: GeminiEngineIcon },
  { id: 'opencode', label: 'OpenCode', Icon: OpenCodeEngineIcon },
]

export function LaunchPickerModal() {
  const open               = useAppStore((s) => s.launchPickerOpen)
  const scopePath          = useAppStore((s) => s.launchPickerScopePath)
  const closePicker        = useAppStore((s) => s.closeLaunchPicker)
  const launchScopeSession = useAppStore((s) => s.launchScopeSession)
  const blurSidebar        = useAppStore((s) => s.blurSidebar)

  const scopeLabel = scopePath ? (scopePath[scopePath.length - 1] ?? null) : null

  const launch = (engine: string) => {
    if (!scopePath) return
    closePicker()
    blurSidebar()
    void launchScopeSession(scopePath, engine)
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) closePicker() }}>
      <DialogContent data-orbit-zone="orbit.desktop.modal.launch-picker" className="max-w-[260px] p-0 overflow-hidden gap-0 rounded-xl">
        <div className="px-4 pt-3.5 pb-3 border-b border-border/50">
          <DialogTitle className="text-sm font-medium leading-none">
            Launch session
          </DialogTitle>
          {scopeLabel && (
            <DialogDescription className="text-xs mt-1 truncate">
              in {scopeLabel}
            </DialogDescription>
          )}
        </div>
        <div className="py-1.5">
          {ENGINES.map(({ id, label, Icon }) => (
            <Button
              key={id}
              variant="ghost"
              onClick={() => launch(id)}
              className="flex items-center gap-3 w-full justify-start h-auto px-4 py-2 text-sm text-foreground/75 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
            >
              <span className="text-foreground/50">
                <Icon size={14} />
              </span>
              {label}
            </Button>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  )
}
