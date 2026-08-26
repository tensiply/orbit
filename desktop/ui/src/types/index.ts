export interface Session {
  id: string
  pid: number
  engine: string
  tenant: string
  project: string
  repository: string
  work_dir: string
  started_at: number
  global_mode: boolean
  is_history?: boolean
  tmux_session?: string
  status?: 'working' | 'ready'
}

export interface LaunchScope {
  workspace: string | null
  tenant: string | null
  project: string | null
  repository: string | null
  engine: string
  new_session: boolean
}

export interface LaunchedInfo {
  session_id: string
  tmux_name: string
}

export interface Tab {
  id: string
  title: string
  type?: 'terminal' | 'shortcuts' | 'uikit' | 'colors' | 'settings' | 'document' | 'architecture' | 'ui-map'
  sessionId?: string
  tmuxSession?: string
  docId?: string
  archWorkspace?: string
  archTenant?: string
}

export interface WorkspaceInfo {
  name: string
  slug: string
  is_default: boolean
}

export interface DocEntry {
  id: string
  title: string
  format: string
  template: string | null
  output_path: string
  source_path: string
  workspace: string
  tenant: string
  project: string
  repository: string
  created_at: number
  updated_at: number
}

export type SettingCategory = 'general' | 'appearance' | 'terminal' | 'engine' | 'privacy'
export type SettingType = 'boolean' | 'string' | 'number' | 'select'

export interface SettingOption { label: string; value: string }

export interface Setting {
  id: string
  category: SettingCategory
  key: string
  name: string
  description: string
  type: SettingType
  value: boolean | string | number
  default: boolean | string | number
  options?: SettingOption[]
}

export type NavView =
  | 'terminal'
  | 'tasks'
  | 'plans'
  | 'plugins'
  | 'mcps'
  | 'activity'
  | 'documents'
  | 'architecture'
  | 'settings'
  | 'profile'
  | 'docs'

export interface ArchEntityDto {
  id: string
  kind: string
  kind_folder: string
  name: string
  description: string | null
  criticality: string | null
  lifecycle: string | null
  tenant: string | null
  owner: string | null
  team: string | null
  tags: string[]
  connections: string[]
  last_updated: string | null
  notes: string | null
  summary: string | null
  /** Environment names this entity is deployed in (keys of the YAML environments map). */
  environments: string[]
}

export interface ArchCatalogDto {
  workspace: string
  tenant: string
  catalog_path: string
  entities: ArchEntityDto[]
  errors: [string, string][]
}

export interface SaveEntityArgs {
  workspace: string
  tenant: string
  kind_folder: string
  id: string
  name: string
  description: string | null
  criticality: string | null
  lifecycle: string | null
  owner: string | null
  team: string | null
  tags: string[]
  connections: string[]
  notes: string | null
  last_updated: string | null
}

export type ArchLayout = Record<string, [number, number]>
export type ArchRoutes = Record<string, Record<string, number>>

export interface ScopeTreeRepo {
  name: string
  work_dir: string
  description: string | null
}

export interface ScopeTreeProject {
  name: string
  repositories: ScopeTreeRepo[]
}

export interface ScopeTreeTenant {
  name: string
  projects: ScopeTreeProject[]
}

export interface ScopeTreeWorkspace {
  name: string
  tenants: ScopeTreeTenant[]
}

// ── Harness inspection ────────────────────────────────────────────────────────

export interface HarnessLayer {
  path: string
  exists: boolean
  label: string
}

export interface HarnessMcpServer {
  name: string
  command: string[]
  source: string
}

export interface HarnessInstruction {
  path: string
  exists: boolean
}

export interface HarnessEnvVar {
  key: string
  value: string
  redacted: boolean
}

export interface HarnessCommand {
  name: string
  source: string
}

export interface HarnessHook {
  name: string
  events: string
}

export interface HarnessPluginContext {
  name: string
  prompt_preview: string | null
  instruction_files: string[]
}

export interface HarnessScopeInfo {
  workspace: string
  workspace_root: string
  tenant: string
  project: string
  repository: string
  engine: string
  work_dir: string
  exec_cmd: string
  auth_status: string
}

export interface HarnessReport {
  scope: HarnessScopeInfo
  config_layers: HarnessLayer[]
  mcp_layers: HarnessLayer[]
  agent_overlay_dirs: HarnessLayer[]
  instructions: HarnessInstruction[]
  mcp_servers: HarnessMcpServer[]
  env_vars: HarnessEnvVar[]
  commands: HarnessCommand[]
  engine_hooks: HarnessHook[]
  plugin_context: HarnessPluginContext[]
  activity_preview: string[]
}

export type Theme = 'dark' | 'light'
export type ShortcutCategory = 'navigation' | 'terminal' | 'app'

export interface Shortcut {
  id: string
  name: string
  description: string
  keys: string
  action: string
  category: ShortcutCategory
  builtin: boolean
}
