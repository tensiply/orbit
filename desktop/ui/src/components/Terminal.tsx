import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { useAppStore } from '../store'
import { tauriService } from '../services/tauri'
import type { TerminalCmd } from '../lib/terminalBus'
import { TERMINAL_THEME_DARK, TERMINAL_THEME_LIGHT, cssVarToHex } from '../theme'

interface Props {
  tabId:        string
  active:       boolean
  panelFocused?: boolean
}

export function TerminalPane({ tabId, active, panelFocused }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef      = useRef<Terminal | null>(null)
  const fitRef       = useRef<FitAddon | null>(null)
  const theme        = useAppStore((s) => s.theme)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", "Courier New", monospace',
      fontSize: 13,
      lineHeight: 1.25,
      theme: {
        ...(theme === 'light' ? TERMINAL_THEME_LIGHT : TERMINAL_THEME_DARK),
        background: cssVarToHex('--card'),
      },
      allowTransparency: false,
      scrollback: 0,
    })

    const fit = new FitAddon()
    term.loadAddon(fit)
    term.loadAddon(new WebLinksAddon())
    term.open(el)

    termRef.current = term
    fitRef.current  = fit

    let fitTimer: ReturnType<typeof setTimeout> | null = null
    const ro = new ResizeObserver(() => {
      if (el.offsetWidth > 0 && el.offsetHeight > 0) {
        if (fitTimer !== null) clearTimeout(fitTimer)
        fitTimer = setTimeout(() => { fit.fit(); fitTimer = null }, 30)
      }
    })
    ro.observe(el)

    term.onData((data) => {
      tauriService.ptyWrite(tabId, data).catch(console.error)
    })

    term.onResize(({ cols, rows }) => {
      tauriService.ptyResize(tabId, cols, rows).catch(console.error)
    })

    let unlisten: (() => void) | null = null
    listen<{ tab_id: string; data: string }>('pty-data', (event) => {
      if (event.payload.tab_id === tabId) {
        term.write(event.payload.data)
      }
    }).then((fn) => {
      unlisten = fn
      // Double RAF: first frame settles flex layout, second gives the WebView
      // time to report final dimensions. fit() and SIGWINCH run together so
      // the PTY always receives the correct post-fit size.
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          fit.fit()
          term.focus()
          const { cols, rows } = term
          tauriService.ptyResize(tabId, cols + 1, rows)
            .then(() => tauriService.ptyResize(tabId, cols, rows))
            .catch(console.error)
        })
      })
    })

    return () => {
      ro.disconnect()
      if (fitTimer !== null) clearTimeout(fitTimer)
      unlisten?.()
      term.dispose()
      termRef.current = null
      fitRef.current  = null
    }
  }, [tabId])

  // Apply theme to live terminal instance when it changes
  useEffect(() => {
    if (termRef.current) {
      const baseTheme = theme === 'light' ? TERMINAL_THEME_LIGHT : TERMINAL_THEME_DARK
      termRef.current.options.theme = { ...baseTheme, background: cssVarToHex('--card') }
    }
  }, [theme])

  useEffect(() => {
    if (active) {
      requestAnimationFrame(() => {
        fitRef.current?.fit()
        termRef.current?.focus()
      })
    }
  }, [active])

  useEffect(() => {
    if (panelFocused) {
      requestAnimationFrame(() => termRef.current?.focus())
    }
  }, [panelFocused])

  // Listen for terminal commands dispatched by the global shortcut handler.
  useEffect(() => {
    if (!active) return
    const handler = (e: Event) => {
      const { tabId: t, cmd } = (e as CustomEvent<{ tabId: string; cmd: TerminalCmd }>).detail
      if (t !== tabId || !termRef.current) return
      if (cmd === 'clear')             termRef.current.clear()
      else if (cmd === 'scroll_up')   termRef.current.scrollLines(-termRef.current.rows)
      else if (cmd === 'scroll_down') termRef.current.scrollLines(termRef.current.rows)
      else if (cmd === 'focus')       termRef.current.focus()
    }
    document.addEventListener('orbit:terminal-cmd', handler)
    return () => document.removeEventListener('orbit:terminal-cmd', handler)
  }, [active, tabId])

  const handleClick = () => termRef.current?.focus()

  return (
    <div className="w-full h-full p-2.5 bg-card">
      <div
        ref={containerRef}
        className="w-full h-full bg-"
        onClick={handleClick}
      />
    </div>
  )
}
