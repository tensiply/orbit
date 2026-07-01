# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-06-30

### Added

- **Engine catalog** — curated list of engines and MCPs embedded in the binary at compile time (`config/catalog/engines.toml`, `config/catalog/mcps.toml`). Updated only on release; no dynamic fetching. Powers `orbit setup`, `orbit engines`, `orbit mcp`, `orbit auth`, and `orbit doctor`.
- **GitHub Copilot CLI support** — `copilot` added to the engine catalog. Installed via `gh extension install github/gh-copilot`; updated via `gh extension upgrade gh-copilot`. Auth detected from `GITHUB_TOKEN`/`GH_TOKEN` env vars or `~/.config/gh`. Supports `orbit engines install/update/info copilot` and `orbit auth copilot`.
- **`orbit mcp`** — manage MCP servers per scope. Subcommands: `list` (catalog MCPs with enabled/disabled status), `enable <name>` (prompts for required vars, writes to scope's `mcp.json`), `disable <name>`, `info <name>` (description, vars, per-layer status). Scope auto-detected from cwd; use `--scope global|tenant|project|repo` to target a specific layer. Secret vars flagged with hint to use env vars.
- **`orbit auth`** — engine auth detection and management. `orbit auth` shows configured/not-configured status for all engines (checks env vars and config dirs — no network calls). `orbit auth <engine>` proxies to the engine's native auth flow. `orbit auth --check` exits 1 if any engine is not configured (CI-friendly).
- **`orbit status`** — quick operational snapshot in ≤ 8 lines: workspace, engine (install + auth), tenant, scope from cwd, daemon status with session count, version. `orbit status --json` outputs the same as structured JSON for scripts and shell prompts. Exit code 1 on critical issues. Runs in < 200ms (no network, 150ms daemon timeout).
- **`orbit engines`** — engine lifecycle management. Subcommands: `list` (all catalog engines with installed version and cached update availability), `install <name>` (npm or custom install command), `update [name]` (one or all installed engines), `info <name>` (description, installed vs. latest npm version, auth status). Installed version detected via `<bin> --version` with semver token extraction. Npm latest cached 24h in `~/.local/share/orbit/engine-versions/`.
- **`orbit config list`** — shows all active config values. `orbit config set engine.default_workspace <name>` configures a default workspace applied like the default tenant.
- **Auto-update** — `orbit` now pulls the governance repo and updates its own binary in the background on every invocation (24h TTL, skipped in dev mode or during git operations). Controlled via `update.auto_update_governance` and `update.auto_update_binary` in config. Use `--no-update` to skip for a single invocation. A notification is printed on the next run when a new binary is installed.
- **`orbit doctor`** engines section now shows install + auth status per engine from the catalog, with `orbit auth <engine>` hint for unconfigured engines.
- **`orbit setup`** engine loop now shows detected auth status live (env var or config dir found), with `orbit auth <engine>` suggestion when not configured. MCP configuration step added: select from catalog MCPs, collect required vars, write to `~/.config/orbit/mcps.json`.
- Engine catalog supports non-npm engines via `install_cmd` and `update_cmd` fields (used by `copilot`).
- **Plugin system** (`orbit plugins list/install/enable/disable/info/wrap/unwrap`) — optional external tools with their own install lifecycle. Plugins defined as TOML files in `plugins/`; users can also drop `.toml` files into `~/.config/orbit/plugins/` without rebuilding.
- `orbit plugins enable/disable` — registers or removes a plugin's MCP servers in `~/.config/orbit/plugins.mcp.json`, loaded as the baseline MCP layer in every orbit session. State persisted in `~/.config/orbit/plugin-state.toml`.
- Built-in plugin: `headroom` — context compression layer (60–95% fewer tokens). Supports `orbit plugins wrap headroom` to proxy the active engine.
- Built-in plugin: `playwright` — browser automation via `@playwright/mcp`. MCP server runs `npx -y @playwright/mcp@latest` when enabled.
- `orbit doctor` and `orbit setup` now include a plugins section.

### Fixed

- Gemini auth detection: `auth_config_dirs` corrected from `.config/gemini` to `.gemini` (the actual location of `oauth_creds.json` used by `@google/gemini-cli`). `orbit auth` and `orbit doctor` now correctly detect gemini as configured when `~/.gemini/` exists.

### Changed

- `orbit doctor` engines section driven by the catalog instead of hardcoded engine list.
- `orbit setup` auth hints replaced by live `detect_auth` detection.
- README: simplified to core concepts (what it does, install, workspace model, quick reference). Full documentation moved to the wiki.
- Wiki: comprehensive pages generated at release — Commands, Plugins, Workspace.

## [0.6.0] - 2026-06-30

### Added

- `orbit plugins enable/disable` — activate or deactivate a plugin's MCP servers for all orbit sessions. State persisted in `~/.config/orbit/plugin-state.toml`; MCP entries written to `~/.config/orbit/plugins.mcp.json` which is loaded as the baseline MCP layer by the engine launcher. Any scope-level `mcp.json` can override plugin MCPs.
- `playwright` plugin — browser automation via `@playwright/mcp`. Includes an MCP server (`npx -y @playwright/mcp@latest`). Enable with `orbit plugins enable playwright`.
- `orbit plugins` — plugin system for optional tools. Plugins are defined as individual TOML files in `plugins/`. Commands: `orbit plugins list` (status of all plugins), `orbit plugins install <name>` (install with method selection), `orbit plugins info <name>` (full details), `orbit plugins wrap/unwrap <name>` (wrap an AI engine if the plugin supports it). `orbit doctor` now includes a `plugins` section. `orbit setup` offers to install plugins interactively. Users can add custom plugins by dropping `.toml` files into `~/.config/orbit/plugins/`. Ships with `headroom` (context compression layer).
- `orbit config get/set/list/edit` — read and write individual config values without re-running `orbit setup`. Supports dot-notation keys: `engine.default`, `engine.default_tenant`, `workspace.ai_root`, `install.dir`. `orbit config edit` opens the file in `$EDITOR`.
- `orbit snapshot` — syncs the context file generated by an engine's `/init` command (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`) to the correct `source-of-truth/context.md` layer in the governance repo. Scope is auto-detected from the current directory. Supports `--file`, `--stdin`, `--output`, and `--dry-run`.
- `orbit setup` now detects which AI engines are missing and offers to install them via `npm install -g` interactively. Shows authentication hints for each engine. New `--no-install` flag skips this step.
- `orbit doctor` now shows a `config` section with all active config values, groups output into named sections (`engines`, `dependencies`, `workspace`, `config`, `daemon`, `binary`), and prints install commands for missing engines.
- README: WSL installation guide with tmux and Node.js setup steps.
- README: dedicated sections for `orbit config`, `orbit snapshot`, `orbit doctor`, and `orbit launch .`.

### Fixed

- `orbit launch` ignored `engine.default` from config and always defaulted to `opencode`. The `--engine` flag is now optional; when omitted, the engine is read from `UserConfig`.

### Changed

- `orbit launch --engine` is now optional (was required with a hardcoded default). Omitting it reads `engine.default` from `~/.config/orbit/config.toml`.

## [0.5.0] - 2026-06-30

### Added

- `orbit ls [WORKSPACE] [TENANT] [PROJECT]` — browse workspace / tenant / project / repository hierarchy.
- `orbit completions <shell>` — print shell completion scripts for bash, zsh, and fish.
- `orbit doctor` — environment diagnostics: checks tmux, AI engines, AI root, daemon socket, and install directory.
- `orbit launch .` (and `orbit .`) — auto-resolves scope from the current working directory by walking ancestors to find the workspace root and mapping path segments to tenant / project / repository.
- Daemon session launch via IPC: `orbit launch` now tries to route through the daemon using the new `LaunchSession` request; daemon spawns a detached tmux session without replacing its own process.
- Daemon auto-start: `orbit launch` starts the daemon automatically if the socket is not present, then routes through it and attaches to the resulting tmux session.
- TUI Launch tab — **workspace selector**: cycles between detected workspaces with `[←→]`; switching resets tenant / project / repository fields.
- TUI Launch tab — **dropdown selectors**: pressing `[↓]` on the Tenant, Project, or Repository field opens a filterable popup populated from the workspace filesystem.
- TUI Sessions tab — **workspace tree**: when no sessions are running, the tab shows the tenant → project → repository tree for the current workspace.

## [0.4.1] - 2026-06-29

### Fixed

- `orbit mode stable` and `orbit mode beta` no longer overwrite the dev build when running in dev mode.
  `update_binary` now installs to the configured `install_dir` instead of resolving `current_exe()`,
  which on Linux follows symlinks and returned the local build path instead of the install target.

### Changed

- `orbit dev` replaced by `orbit mode` with three explicit modes: `stable`, `dev`, and `beta`.
  - `orbit mode stable` — downloads and atomically installs the latest GitHub release (no symlink).
  - `orbit mode dev [path]` — creates a symlink to a local build; path is saved in
    `~/.local/share/orbit/dev_path` so subsequent calls require no argument.
  - `orbit mode beta` — downloads and installs the latest GitHub pre-release.
  - `orbit mode status` — shows the active mode and binary details.
  - `orbit update` now respects the active mode: skips binary download in `dev` mode,
    targets pre-releases in `beta` mode.

## [0.3.0] - 2026-06-29

### Added

- Multi-workspace support in TUI ([#6](https://github.com/befraeloircorona/orbit/issues/6)).
  Tab bar shows the active workspace name. Press `[w]` to cycle through detected workspaces.
  Workspace detection scans `~/` for directories containing `orbit.toml` or `tenants/`.
  Switching reloads MCP entries, launch defaults, and sessions from the new workspace root.
- `orbit session attach` auto-attaches when only one tmux session is active; shows a selector when there are multiple ([#4](https://github.com/befraeloircorona/orbit/issues/4)).
  Verifies the tmux window still exists before attaching; clear error message if it's gone.
  TUI `[a]`/`↵` in the Sessions tab also checks window existence before handing off the terminal.
  Uses `switch-client` when already inside tmux, `attach-session` otherwise.
- `orbit update` now downloads, checksum-verifies, and atomically installs the new binary ([#3](https://github.com/befraeloircorona/orbit/issues/3)).
  Shows progress, validates SHA-256 against `checksums.txt`, skips if already on latest.
  New flag `--force` to reinstall even when current. Respects `ORBIT_NO_UPDATE_CHECK=1`.

## [0.2.0] - 2026-06-28

### Added

- Check latest release on startup and print a one-line notice when a newer version is available ([#1](https://github.com/befraeloircorona/orbit/issues/1)).
  Opt-out: `ORBIT_NO_UPDATE_CHECK=1` or `update.check_on_startup = false` in `orbit.toml`. Check is cached 24 h.

## [0.1.0] - 2026-06-27

### Added

- TUI with three tabs: Sessions, Launch, System
  - Sessions: list active sessions, attach via tmux, kill, inspect details, clean dead entries
  - Launch: interactive form to launch AI sessions (opencode, gemini, claude) with optional tmux
  - System: MCP server manager — add/remove servers across global / tenant / project / repo scopes
- Daemon-aware session refresh: TUI queries the daemon socket (500 ms timeout) and falls back to file-based loading
- Daemon controls in TUI: start/stop orbit daemon from the System tab (`s`)
- `orbit init` — clone a governance repository as the AI root
- `orbit init --scaffold` — create a local-only AI root without a governance repo
- `orbit launch` — launch AI sessions with tenant/project/repo context and optional tmux
- `orbit update` — self-update binary; defaults to `befraeloircorona/orbit` GitHub releases
- `orbit session list|kill|attach|clean` — CLI session management
- `orbit daemon serve|start|stop|status` — daemon lifecycle management
- `orbit dev enable|disable|status|generate-token` — dev-mode toggle via token-based auth
- MCP configuration at four scope levels: global, tenant, project, repository
- CI workflow: format check, clippy `-D warnings`, tests on every push/PR to `main`
- Release workflow: cross-compiled static binaries for linux-x86_64 and linux-aarch64 + SHA-256 checksums

[Unreleased]: https://github.com/befraeloircorona/orbit/compare/main...HEAD
[main]: https://github.com/befraeloircorona/orbit/releases/tag/main
[0.4.1]: https://github.com/befraeloircorona/orbit/releases/tag/v0.4.1
[0.4.0]: https://github.com/befraeloircorona/orbit/releases/tag/v0.4.0
[0.3.0]: https://github.com/befraeloircorona/orbit/releases/tag/v0.3.0
[0.2.0]: https://github.com/befraeloircorona/orbit/releases/tag/v0.2.0
[0.1.0]: https://github.com/befraeloircorona/orbit/releases/tag/v0.1.0
