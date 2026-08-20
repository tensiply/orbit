import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'

interface Props {
  tabId: string
  active: boolean
}

const THEME = {
  background: '#0a0a0b',
  foreground: '#f4f4f5',
  cursor: '#a1a1aa',
  cursorAccent: '#09090b',
  selectionBackground: '#3f3f4660',
  black: '#18181b',
  red: '#ef4444',
  green: '#22c55e',
  yellow: '#eab308',
  blue: '#3b82f6',
  magenta: '#a855f7',
  cyan: '#06b6d4',
  white: '#d4d4d8',
  brightBlack: '#52525b',
  brightRed: '#f87171',
  brightGreen: '#4ade80',
  brightYellow: '#fde047',
  brightBlue: '#60a5fa',
  brightMagenta: '#c084fc',
  brightCyan: '#22d3ee',
  brightWhite: '#f4f4f5',
}

export function TerminalPane({ tabId, active }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitRef = useRef<FitAddon | null>(null)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", "Courier New", monospace',
      fontSize: 13,
      lineHeight: 1.25,
      theme: THEME,
      // Allow the terminal to receive all keyboard events
      allowTransparency: false,
    })

    const fit = new FitAddon()
    term.loadAddon(fit)
    term.loadAddon(new WebLinksAddon())
    term.open(el)

    termRef.current = term
    fitRef.current = fit

    // ResizeObserver fires whenever the container gets real dimensions —
    // more reliable than setTimeout because it reacts to actual layout changes
    const ro = new ResizeObserver(() => {
      if (el.offsetWidth > 0 && el.offsetHeight > 0) {
        fit.fit()
      }
    })
    ro.observe(el)

    // Initial fit after the next paint
    requestAnimationFrame(() => {
      fit.fit()
      term.focus()
    })

    term.onData((data) => {
      invoke('pty_write', { tabId, data }).catch(console.error)
    })

    term.onResize(({ cols, rows }) => {
      invoke('pty_resize', { tabId, cols, rows }).catch(console.error)
    })

    let unlisten: (() => void) | null = null
    listen<{ tab_id: string; data: string }>('pty-data', (event) => {
      if (event.payload.tab_id === tabId) {
        term.write(event.payload.data)
      }
    }).then((fn) => {
      unlisten = fn
    })

    return () => {
      ro.disconnect()
      unlisten?.()
      term.dispose()
      termRef.current = null
      fitRef.current = null
    }
  }, [tabId])

  // Focus when this tab becomes active
  useEffect(() => {
    if (active) {
      requestAnimationFrame(() => {
        fitRef.current?.fit()
        termRef.current?.focus()
      })
    }
  }, [active])

  // Click anywhere in the terminal area → ensure xterm has focus
  const handleClick = () => termRef.current?.focus()

  return (
    <div className="w-full h-full p-1 bg-[#0a0a0b]">
      <div
        ref={containerRef}
        className="w-full h-full"
        onClick={handleClick}
      />
    </div>
  )
}
