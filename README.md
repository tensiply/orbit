# orbit

A CLI launcher for AI coding assistants (opencode, Gemini CLI, Claude Code) with multi-tenant workspace management, session tracking, and MCP server configuration.

Built in Rust. Runs on Linux and macOS.

**Latest:** [main](https://github.com/befraeloircorona/orbit/releases/tag/main) · [Changelog](CHANGELOG.md) · [Wiki](https://github.com/befraeloircorona/orbit/wiki)

---

## What it does

`orbit` resolves the right context for your AI session — tenant, project, repository, instructions, MCP servers — and launches the engine ready to work. It manages sessions via tmux and provides a terminal UI for navigating active sessions, launching new ones, and configuring the ecosystem.

```
orbit                                              # open TUI dashboard
orbit launch .                                     # auto-detect scope from current directory
orbit launch WORKSPACE TENANT PROJECT REPO         # full scope
orbit launch WORKSPACE TENANT                      # workspace + tenant only
orbit session list                                 # list active sessions
orbit session attach                               # attach to a running session
orbit config list                                  # show all config values
orbit config set engine.default claude             # change default engine
orbit doctor                                       # check engines, dependencies, config
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

### Windows (WSL)

orbit runs natively inside Windows Subsystem for Linux. Use the Linux instructions above inside your WSL terminal.

Prerequisites inside WSL:

```bash
# Install tmux if missing
sudo apt-get install -y tmux

# Install Node.js (for npx-based MCP servers and engine installers)
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs
```

Then run `orbit setup` — it will detect which engines are installed and offer to install missing ones.

---

### Windows (WSL)

orbit runs natively inside Windows Subsystem for Linux. Use the Linux instructions above inside your WSL terminal.

Prerequisites inside WSL:

```bash
# Install tmux
sudo apt-get install -y tmux

# Install Node.js (required for engine installers and npx-based MCP servers)
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs
```

After installing the binary, run `orbit setup` — it will detect which AI engines are installed and offer to install any that are missing.

---

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

This creates `~/.config/orbit/config.toml` with your preferences, and walks you through installing any AI engines that are missing:

```
  Default engine (opencode / gemini / claude) [opencode]:
  ...

  Checking engines...

  ✓  opencode
  ○  gemini  — not installed
    Install gemini? [y/N]: y
    Installing gemini... done
  ✓  claude

  auth: Run `gemini auth` or set GOOGLE_API_KEY / GEMINI_API_KEY
```

Pass `--no-install` to skip the engine installation prompts:

```bash
orbit setup --no-install
```

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
orbit launch .                                      # auto-detect scope from cwd
orbit launch WORKSPACE                              # workspace only
orbit launch WORKSPACE TENANT                       # workspace + tenant
orbit launch WORKSPACE TENANT PROJECT               # + project
orbit launch WORKSPACE TENANT PROJECT REPO          # full scope

orbit launch WORKSPACE TENANT PROJECT REPO --engine claude   # pick engine (default: config)
orbit launch WORKSPACE TENANT --no-tmux                      # skip tmux
orbit launch WORKSPACE TENANT PROJECT REPO --dry-run         # print resolved config
```

---

## Config

View and change preferences without re-running `orbit setup`:

```bash
orbit config list                          # show all values
orbit config get engine.default            # print one value
orbit config set engine.default claude     # change default engine
orbit config set engine.default_tenant work
orbit config set workspace.ai_root ~/AI
orbit config set install.dir ~/.local/bin
orbit config edit                          # open config in $EDITOR
```

Config file lives at `~/.config/orbit/config.toml`. Valid engines: `opencode`, `gemini`, `claude`.

---

## Launching from the current directory

Use `.` as the workspace argument and orbit will auto-detect scope from your working directory:

```bash
cd ~/MYCO/backend/api
orbit launch .                             # resolves workspace, tenant, project, repo automatically
orbit launch . --engine gemini            # same, with explicit engine
```

This works whenever your working directory is inside a workspace managed by orbit.

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

## Snapshot

`orbit snapshot` syncs the context file generated by an engine's `/init` command into the governance repo at the correct scope layer.

**Typical workflow:**

```bash
# 1. Launch a session for a repo
orbit launch MYCO backend api

# 2. Inside Claude (or any engine), run /init to generate CLAUDE.md
#    (Claude does this automatically)

# 3. Back in the shell (or with ! inside the engine), sync to governance
! orbit snapshot
```

orbit auto-detects `CLAUDE.md` (claude), `AGENTS.md` (opencode), or `GEMINI.md` (gemini) in the current directory and copies it to the right `source-of-truth/context.md` in the governance repo:

| Scope | Governance destination |
|---|---|
| Repository | `~/AI/tenants/<T>/projects/<P>/repositories/<R>/source-of-truth/context.md` |
| Project | `~/AI/tenants/<T>/projects/<P>/source-of-truth/context.md` |
| Tenant | `~/AI/tenants/<T>/source-of-truth/context.md` |

```bash
orbit snapshot                          # auto-detect file, auto-detect scope from cwd
orbit snapshot --file path/to/file.md   # explicit source file
orbit snapshot --stdin                  # read from stdin
orbit snapshot --dry-run                # show source and dest without writing
orbit snapshot --output ~/AI/tenants/X/source-of-truth/context.md  # override dest
```

---

## Doctor

Run `orbit doctor` to check the environment at a glance:

```
orbit doctor

engines
  ✓  opencode
  ✗  gemini  — not found in PATH
      install: npm install -g @google/gemini-cli
  ✓  claude

dependencies
  ✓  tmux
  ✓  node

workspace
  ✓  AI root (git)   /home/user/AI

config
  file                    ~/.config/orbit/config.toml
  engine.default          opencode
  engine.default_tenant   work
  workspace.ai_root       ~/AI
  install.dir             ~/.local/bin

daemon
  ✗  daemon  — not running — start with `orbit daemon start`

binary
  ✓  install dir  /home/user/.local/bin
  ✓  orbit binary /home/user/.local/bin/orbit
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

