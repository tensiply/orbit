export type TerminalCmd = 'clear' | 'scroll_up' | 'scroll_down' | 'focus'

export function sendTerminalCmd(tabId: string, cmd: TerminalCmd) {
  document.dispatchEvent(
    new CustomEvent('orbit:terminal-cmd', { detail: { tabId, cmd } }),
  )
}
