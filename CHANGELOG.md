# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

- Multi-workspace support in TUI ([#6](https://github.com/tensiply/orbit/issues/6)).
  Tab bar shows the active workspace name. Press `[w]` to cycle through detected workspaces.
  Workspace detection scans `~/` for directories containing `orbit.toml` or `tenants/`.
  Switching reloads MCP entries, launch defaults, and sessions from the new workspace root.
- `orbit session attach` auto-attaches when only one tmux session is active; shows a selector when there are multiple ([#4](https://github.com/tensiply/orbit/issues/4)).
  Verifies the tmux window still exists before attaching; clear error message if it's gone.
  TUI `[a]`/`↵` in the Sessions tab also checks window existence before handing off the terminal.
  Uses `switch-client` when already inside tmux, `attach-session` otherwise.
- `orbit update` now downloads, checksum-verifies, and atomically installs the new binary ([#3](https://github.com/tensiply/orbit/issues/3)).
  Shows progress, validates SHA-256 against `checksums.txt`, skips if already on latest.
  New flag `--force` to reinstall even when current. Respects `ORBIT_NO_UPDATE_CHECK=1`.

## [0.2.0] - 2026-06-28

### Added

- Check latest release on startup and print a one-line notice when a newer version is available ([#1](https://github.com/tensiply/orbit/issues/1)).
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
- `orbit update` — self-update binary; defaults to `tensiply/orbit` GitHub releases
- `orbit session list|kill|attach|clean` — CLI session management
- `orbit daemon serve|start|stop|status` — daemon lifecycle management
- `orbit dev enable|disable|status|generate-token` — dev-mode toggle via token-based auth
- MCP configuration at four scope levels: global, tenant, project, repository
- CI workflow: format check, clippy `-D warnings`, tests on every push/PR to `main`
- Release workflow: cross-compiled static binaries for linux-x86_64 and linux-aarch64 + SHA-256 checksums
- MIT license — Copyright (c) 2026 Eloir Corona

[Unreleased]: https://github.com/tensiply/orbit/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/tensiply/orbit/releases/tag/v0.3.0
[0.2.0]: https://github.com/tensiply/orbit/releases/tag/v0.2.0
[0.1.0]: https://github.com/tensiply/orbit/releases/tag/v0.1.0
