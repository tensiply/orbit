import type { StateCreator } from 'zustand'
import type { Setting, SettingCategory } from '../../types'
import type { AppStore } from '../types'

const DEFAULT_SETTINGS: Setting[] = [
  // ── General ──────────────────────────────────────────────────────────────────
  {
    id: 'general.startup_view',
    category: 'general',
    key: 'general.startup_view',
    name: 'Startup View',
    description: 'Which panel to open when the app launches.',
    type: 'select',
    value: 'terminal',
    default: 'terminal',
    options: [
      { label: 'Sessions', value: 'terminal' },
      { label: 'Tasks',    value: 'tasks'    },
      { label: 'Plans',    value: 'plans'    },
      { label: 'Activity', value: 'activity' },
    ],
  },
  {
    id: 'general.confirm_close',
    category: 'general',
    key: 'general.confirm_close',
    name: 'Confirm Before Close',
    description: 'Ask for confirmation before closing an active session tab.',
    type: 'boolean',
    value: true,
    default: true,
  },
  {
    id: 'general.session_poll_interval',
    category: 'general',
    key: 'general.session_poll_interval',
    name: 'Session Poll Interval',
    description: 'How often (in seconds) to refresh the sessions list from the daemon.',
    type: 'number',
    value: 5,
    default: 5,
  },
  // ── Appearance ────────────────────────────────────────────────────────────────
  {
    id: 'appearance.theme',
    category: 'appearance',
    key: 'appearance.theme',
    name: 'Color Theme',
    description: 'Application color theme.',
    type: 'select',
    value: 'dark',
    default: 'dark',
    options: [
      { label: 'Dark',  value: 'dark'  },
      { label: 'Light', value: 'light' },
    ],
  },
  {
    id: 'appearance.sidebar_default_width',
    category: 'appearance',
    key: 'appearance.sidebar_default_width',
    name: 'Sidebar Default Width',
    description: 'Default sidebar width in pixels when the app starts.',
    type: 'number',
    value: 268,
    default: 268,
  },
  {
    id: 'appearance.font_family',
    category: 'appearance',
    key: 'appearance.font_family',
    name: 'Terminal Font Family',
    description: 'Font family used in terminal panes. Must be installed on your system.',
    type: 'string',
    value: 'monospace',
    default: 'monospace',
  },
  {
    id: 'appearance.font_size',
    category: 'appearance',
    key: 'appearance.font_size',
    name: 'Terminal Font Size',
    description: 'Font size (px) used in terminal panes.',
    type: 'number',
    value: 13,
    default: 13,
  },
  // ── Terminal ──────────────────────────────────────────────────────────────────
  {
    id: 'terminal.scrollback',
    category: 'terminal',
    key: 'terminal.scrollback',
    name: 'Scrollback Buffer',
    description: 'Maximum number of lines kept in the scrollback buffer.',
    type: 'number',
    value: 5000,
    default: 5000,
  },
  {
    id: 'terminal.cursor_blink',
    category: 'terminal',
    key: 'terminal.cursor_blink',
    name: 'Cursor Blink',
    description: 'Animate the terminal cursor.',
    type: 'boolean',
    value: true,
    default: true,
  },
  {
    id: 'terminal.copy_on_select',
    category: 'terminal',
    key: 'terminal.copy_on_select',
    name: 'Copy on Select',
    description: 'Automatically copy selected text to the clipboard.',
    type: 'boolean',
    value: false,
    default: false,
  },
  {
    id: 'terminal.bell',
    category: 'terminal',
    key: 'terminal.bell',
    name: 'Audible Bell',
    description: 'Play a sound when the terminal bell character is received.',
    type: 'boolean',
    value: false,
    default: false,
  },
  // ── Engine ────────────────────────────────────────────────────────────────────
  {
    id: 'engine.default',
    category: 'engine',
    key: 'engine.default',
    name: 'Default Engine',
    description: 'Engine to use when no engine is specified in the scope config.',
    type: 'select',
    value: 'claude',
    default: 'claude',
    options: [
      { label: 'Claude Code', value: 'claude'    },
      { label: 'OpenCode',    value: 'opencode'  },
      { label: 'Gemini',      value: 'gemini'    },
    ],
  },
  {
    id: 'engine.timeout',
    category: 'engine',
    key: 'engine.timeout',
    name: 'Engine Timeout',
    description: 'Seconds to wait for the engine to start before reporting an error.',
    type: 'number',
    value: 30,
    default: 30,
  },
  {
    id: 'engine.auto_reconnect',
    category: 'engine',
    key: 'engine.auto_reconnect',
    name: 'Auto Reconnect',
    description: 'Automatically attempt to reconnect if the engine connection drops.',
    type: 'boolean',
    value: true,
    default: true,
  },
  // ── Privacy ───────────────────────────────────────────────────────────────────
  {
    id: 'privacy.telemetry',
    category: 'privacy',
    key: 'privacy.telemetry',
    name: 'Usage Telemetry',
    description: 'Send anonymous usage data to help improve orbit.',
    type: 'boolean',
    value: false,
    default: false,
  },
  {
    id: 'privacy.crash_reports',
    category: 'privacy',
    key: 'privacy.crash_reports',
    name: 'Crash Reports',
    description: 'Automatically send crash reports when the app encounters an unexpected error.',
    type: 'boolean',
    value: false,
    default: false,
  },
]

const STORAGE_KEY = 'orbit-settings'

function loadSettings(): Setting[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_SETTINGS
    const saved: Record<string, Setting['value']> = JSON.parse(raw)
    return DEFAULT_SETTINGS.map((d) =>
      d.key in saved ? { ...d, value: saved[d.key] } : d,
    )
  } catch {
    return DEFAULT_SETTINGS
  }
}

function persistSettings(settings: Setting[]) {
  const map: Record<string, Setting['value']> = {}
  for (const s of settings) {
    if (s.value !== s.default) map[s.key] = s.value
  }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
}

export interface SettingsSlice {
  settings: Setting[]

  getSetting: (id: string) => Setting | undefined
  getSettingValue: (key: string) => Setting['value'] | undefined
  updateSetting: (id: string, value: Setting['value']) => void
  resetSetting: (id: string) => void
  resetAllSettings: () => void
}

export const createSettingsSlice: StateCreator<AppStore, [], [], SettingsSlice> = (set, get) => ({
  settings: loadSettings(),

  getSetting: (id) => get().settings.find((s) => s.id === id),

  getSettingValue: (key) => {
    const s = get().settings.find((s) => s.key === key)
    return s?.value
  },

  updateSetting: (id, value) => {
    const settings = get().settings.map((s) => (s.id === id ? { ...s, value } : s))
    persistSettings(settings)
    set({ settings })
  },

  resetSetting: (id) => {
    const settings = get().settings.map((s) => (s.id === id ? { ...s, value: s.default } : s))
    persistSettings(settings)
    set({ settings })
  },

  resetAllSettings: () => {
    const settings = DEFAULT_SETTINGS
    localStorage.removeItem(STORAGE_KEY)
    set({ settings })
  },
})

export type { SettingCategory }
