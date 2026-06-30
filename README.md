# orbit

A CLI launcher for AI coding assistants (opencode, Gemini CLI, Claude Code) with multi-tenant workspace management, session tracking, and MCP server configuration.

Built in Rust. Runs on Linux and macOS.

**Latest:** [main](https://github.com/befraeloircorona/orbit/releases/tag/main) · [Changelog](CHANGELOG.md) · [Wiki](https://github.com/befraeloircorona/orbit/wiki)

---

## What it does

`orbit` resolves the right context for your AI session — tenant, project, repository, instructions, MCP servers — and launches the engine ready to work. It manages sessions via tmux and provides a terminal UI for navigating active sessions, launching new ones, and configuring the ecosystem.

```
orbit                                              # open TUI dashboard
orbit launch WORKSPACE TENANT PROJECT REPO         # full scope
orbit launch WORKSPACE TENANT                      # workspace + tenant only
orbit session list                                 # list active sessions
orbit session attach                               # attach to a running session
```

---

## Prerequisites

- [tmux](https://github.com/tmux/tmux) — session management
- At least one AI engine installed:
  - [opencode](https://opencode.ai)
  - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
  - [Claude Code](https://claude.ai/code)
- Node.js (optional — required for npx-based MCP servers)

---

## Installation

### Download binary (recommended)

Download the latest binary for your platform from [Releases](https://github.com/befraeloircorona/orbit/releases):

```bash
# Linux (x86_64)
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-linux-x86_64 \
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# Linux (aarch64)
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-linux-aarch64 \
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# macOS (Apple Silicon)
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-macos-aarch64 \
  -o /usr/local/bin/orbit && chmod +x /usr/local/bin/orbit

# macOS (Intel)
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-macos-x86_64 \
  -o /usr/local/bin/orbit && chmod +x /usr/local/bin/orbit
```

### Build from source

```bash
git clone https://github.com/befraeloircorona/orbit.git
cd orbit
cargo build --release
cp target/release/orbit ~/.local/bin/
```

Requires Rust 1.75+.

---

## Setup

Run once after installation:

```bash
orbit setup
```

This creates `~/.config/orbit/config.toml` with your preferences:

```toml
[workspace]
ai_root = "~/AI"          # root of your AI workspace

[engine]
default = "opencode"      # default engine: opencode | gemini | claude
default_tenant = "work"   # default tenant

[install]
dir = "~/.local/bin"      # where the orbit binary lives
```

---

## Workspace structure

orbit organises context across four scope levels. Each is a positional argument to `orbit launch`:

```
~/ (home)
├── AI/                                   ← global AI root (governance repo)
│   ├── mcp.json                          # workspace-wide MCP servers
│   ├── orbit.toml                        # workspace configuration
│   └── tenants/
│       └── <TENANT>/                     ← tenant
│           ├── mcp.json                  # tenant MCP servers
│           ├── source-of-truth/          # tenant instructions & agents
│           └── projects/
│               └── <PROJECT>/            ← project
│                   ├── source-of-truth/
│                   └── repositories/
│                       └── <REPO>/       ← repository
│                           └── source-of-truth/
│
└── <WORKSPACE>/                          ← workspace root
    ├── AI/                               ← workspace AI context (optional, see below)
    │   └── tenants/
    │       └── <TENANT>/
    │           ├── source-of-truth/
    │           └── projects/
    │               └── <PROJECT>/
    └── <TENANT>/                         ← actual code lives here (not inside AI/)
        └── <PROJECT>/
            └── <REPO>/
```

### Two concepts: global AI root vs workspace root

| Concept | Path | Purpose |
|---|---|---|
| **Global AI root** | `~/AI` (configured via `ai_root`) | Governance repo. Always loaded as shared context for every workspace. |
| **Workspace root** | `~/<WORKSPACE>` | A working area. Each workspace has its own tenants, projects, and repos. |

**`~/AI` is always loaded** regardless of which workspace you launch. It provides the global baseline — shared MCP servers, global agent definitions, and the default tenant when no other is specified.

### How orbit locates AI context inside a workspace

When you run `orbit launch <WORKSPACE> <TENANT>`, orbit resolves paths in this order:

1. `workspace_root` = `~/<WORKSPACE>`
2. **AI context root**: if `~/<WORKSPACE>/AI/tenants/` exists → uses `~/<WORKSPACE>/AI/`; otherwise falls back to `~/<WORKSPACE>/` itself
3. `code_root` = `~/<WORKSPACE>/<TENANT>/` (where actual code repos live)

So if you want workspace-specific AI config, place it at `~/<WORKSPACE>/AI/tenants/...`. If you prefer a flat layout, put `tenants/` directly inside `~/<WORKSPACE>/`.

### What gets created

orbit does not auto-create workspace directories. You set them up once:

```bash
orbit init <governance-url>   # clones a governance repo as ~/AI (or configured ai_root)
orbit init --scaffold         # creates ~/AI as a local-only directory (no git)
```

For additional workspaces you create the directory structure manually or via your own governance repo.

When you run `orbit launch <WORKSPACE> <TENANT> <PROJECT> <REPO>`, orbit:
1. Resolves each scope level to a real directory (case-insensitive)
2. Loads `~/AI` as global context, then merges workspace → tenant → project → repo layers
3. Merges MCP servers from all layers
4. Assembles instructions and agent configs for the engine
5. Launches the engine inside a named tmux session

### `orbit launch` reference

All arguments are positional and optional — omit from the right to broaden scope:

```bash
orbit launch                                        # global mode (uses ai_root from config)
orbit launch WORKSPACE                              # workspace only
orbit launch WORKSPACE TENANT                       # workspace + tenant
orbit launch WORKSPACE TENANT PROJECT               # + project
orbit launch WORKSPACE TENANT PROJECT REPO          # full scope

orbit launch WORKSPACE TENANT PROJECT REPO --engine claude   # pick engine
orbit launch WORKSPACE TENANT --no-tmux                      # skip tmux
orbit launch WORKSPACE TENANT PROJECT REPO --dry-run         # print resolved config
```

---

## MCP configuration

MCP servers can be configured at any scope level. The closer the scope, the higher the priority on conflict.

Edit `mcp.json` files directly, or use the TUI System tab (`orbit` → tab `[3]` → `[a]` to add):

```json
{
  "mcpServers": {
    "context7": {
      "command": "npx",
      "args": ["-y", "@upstash/context7-mcp"]
    },
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"]
    }
  }
}
```

---

## TUI

Run `orbit` with no arguments to open the interactive dashboard:

```
[1] Sessions   [2] Launch   [3] System
```

| Tab | Keys |
|-----|------|
| Sessions | `↑↓` navigate · `a/↵` attach · `K` kill · `d` details · `c` clean · `r` refresh |
| Launch | `↑↓` fields · `←→` engine · `Space` toggle tmux · `↵` launch |
| System | `↑↓` MCP nav · `a` add MCP · `x` remove · `s` daemon toggle · `r` refresh |

Press `?` for full keybindings.

---

## Session management

```bash
orbit session list              # list all tracked sessions
orbit session attach [id]       # attach to session (defaults to most recent)
orbit session kill <id>         # send SIGTERM to session process
orbit session clean             # remove files for dead sessions
```

---

## Development mode

Switch between a stable binary, a local build, or pre-release builds:

```bash
orbit mode stable        # download and install the latest release
orbit mode dev [path]    # symlink to a local build (path saved across calls)
orbit mode beta          # download and install the latest pre-release
orbit mode status        # show active mode and binary details
```

`orbit update` respects the active mode: skips binary download in `dev` mode, targets pre-releases in `beta` mode.

---

## Daemon

The orbit daemon runs in the background and provides session state via a Unix socket:

```bash
orbit daemon start   # start daemon in background
orbit daemon stop    # graceful shutdown
orbit daemon status  # uptime, session count, PID
```

Socket: `~/.local/share/orbit/orbit.sock`

