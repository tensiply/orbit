"""Generate wiki pages: Home.md, Commands.md, Plugins.md, Workspace.md."""
import os
import re

repo = os.environ["REPO"]
tag = os.environ.get("TAG", "main")
version = os.environ.get("VERSION", "main")

# ── helpers ───────────────────────────────────────────────────────────────────

def read(path):
    try:
        with open(path) as f:
            return f.read()
    except FileNotFoundError:
        return ""

def write_wiki(name, content):
    with open(f"wiki/{name}", "w") as f:
        f.write(content)
    print(f"  wrote wiki/{name}")

def release_table():
    changelog = read("CHANGELOG.md")
    releases = re.findall(
        r"^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})",
        changelog, re.MULTILINE,
    )
    if not releases:
        return "_No releases yet._"
    rows = "\n".join(
        f"| [v{ver}](Release-v{ver}) | {date} |"
        for ver, date in releases
    )
    return f"| Version | Date |\n|---------|------|\n{rows}"

def plugin_rows():
    """Read plugins/*.toml and extract name + description."""
    import glob, re
    rows = []
    for path in sorted(glob.glob("plugins/*.toml")):
        content = read(path)
        name = re.search(r'^name\s*=\s*"([^"]+)"', content, re.MULTILINE)
        desc = re.search(r'^description\s*=\s*"([^"]+)"', content, re.MULTILINE)
        cat  = re.search(r'^category\s*=\s*"([^"]+)"', content, re.MULTILINE)
        url  = re.search(r'^url\s*=\s*"([^"]+)"', content, re.MULTILINE)
        has_mcp = "[[mcp]]" in content
        has_wrap = "[wrap]" in content
        if name and desc:
            n = name.group(1)
            d = desc.group(1)
            c = cat.group(1) if cat else "-"
            u = url.group(1) if url else ""
            extras = []
            if has_mcp:
                extras.append("MCP")
            if has_wrap:
                extras.append("wrap")
            caps = " · ".join(extras) if extras else "CLI"
            name_cell = f"[{n}]({u})" if u else n
            rows.append(f"| {name_cell} | {c} | {caps} | {d} |")
    return "\n".join(rows) if rows else "_No plugins defined._"

# ── Home.md ───────────────────────────────────────────────────────────────────

HOME = f"""# orbit

A CLI launcher for AI coding assistants with multi-tenant workspace management, session tracking, MCP server configuration, and a plugin system.

**Engines supported:** opencode · Gemini CLI · Claude Code — [Releases](https://github.com/{repo}/releases) · [Changelog](https://github.com/{repo}/blob/main/CHANGELOG.md)

---

## Install

```bash
# Linux (x86_64)
curl -fsSL https://github.com/{repo}/releases/latest/download/orbit-linux-x86_64 \\
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# macOS (Apple Silicon)
curl -fsSL https://github.com/{repo}/releases/latest/download/orbit-macos-aarch64 \\
  -o /usr/local/bin/orbit && chmod +x /usr/local/bin/orbit
```

Then run once:

```bash
orbit setup
```

---

## Quick start

```bash
orbit                          # open TUI dashboard
orbit launch .                 # auto-detect scope from current directory
orbit launch WORKSPACE TENANT  # launch with explicit scope
orbit doctor                   # check engines, dependencies, plugins
orbit plugins list             # see available plugins
```

---

## Pages

| Page | What's inside |
|------|---------------|
| [Commands](Commands) | All CLI commands with flags and examples |
| [Plugins](Plugins) | Plugin system: install, enable, disable, create |
| [Workspace](Workspace) | Workspace structure, scopes, and MCP layers |

---

## Releases

{release_table()}
"""

# ── Commands.md ───────────────────────────────────────────────────────────────

COMMANDS = f"""# Commands

Full reference for all `orbit` commands. Run `orbit <command> --help` for details.

---

## orbit (no args)

Opens the TUI dashboard.

```
[1] Sessions   [2] Launch   [3] System
```

| Tab | Keys |
|-----|------|
| Sessions | `↑↓` navigate · `a/↵` attach · `K` kill · `d` details · `c` clean |
| Launch | `↑↓` fields · `←→` engine · `Space` toggle tmux · `↵` launch |
| System | `↑↓` MCP nav · `a` add · `x` remove · `s` daemon toggle |

Press `?` for the full keybindings list.

---

## orbit setup

First-time configuration. Creates `~/.config/orbit/config.toml`, installs the binary, and walks you through installing AI engines and plugins.

```bash
orbit setup                   # interactive
orbit setup -y                # accept all defaults
orbit setup --no-install      # skip engine install step
orbit setup --no-plugins      # skip plugin install step
```

---

## orbit launch

Launch an AI session. Arguments are positional and optional — omit from the right to broaden scope.

```bash
orbit launch                                        # global mode
orbit launch .                                      # auto-detect scope from cwd
orbit launch WORKSPACE                              # workspace only
orbit launch WORKSPACE TENANT                       # + tenant
orbit launch WORKSPACE TENANT PROJECT               # + project
orbit launch WORKSPACE TENANT PROJECT REPO          # full scope

orbit launch WORKSPACE TENANT --engine claude       # pick engine
orbit launch WORKSPACE TENANT --no-tmux             # skip tmux
orbit launch WORKSPACE TENANT PROJECT REPO --dry-run  # preview without launching
```

orbit resolves context in this order: global AI root → workspace → tenant → project → repository. Each layer adds MCP servers and instructions.

---

## orbit plugins

Manage optional tools that extend orbit sessions.

```bash
orbit plugins list                        # all plugins + install/MCP status
orbit plugins install <name>              # install a plugin
orbit plugins install <name> --method npm # pick install method
orbit plugins enable <name>               # activate MCP servers in all sessions
orbit plugins disable <name>              # deactivate MCP servers
orbit plugins info <name>                 # full details
orbit plugins wrap <name>                 # wrap active engine (if supported)
orbit plugins unwrap <name>               # undo wrap
```

See [[Plugins]] for the full guide.

---

## orbit config

Read and write config values without re-running `orbit setup`.

```bash
orbit config list                          # show all values
orbit config get engine.default            # print one value
orbit config set engine.default claude     # change a value
orbit config set engine.default_tenant work
orbit config set workspace.ai_root ~/AI
orbit config set install.dir ~/.local/bin
orbit config edit                          # open in $EDITOR
```

Valid keys: `engine.default`, `engine.default_tenant`, `workspace.ai_root`, `install.dir`.
Valid engines: `opencode`, `gemini`, `claude`.

---

## orbit doctor

Checks the environment and prints a status report.

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

plugins
  ✓  headroom  [mcp: inactive — orbit plugins enable headroom]
  ○  playwright  — not installed

workspace
  ✓  AI root (git)   /home/user/AI

config
  file                    ~/.config/orbit/config.toml
  engine.default          opencode
  ...

daemon
  ✗  daemon — not running

binary
  ✓  orbit binary /home/user/.local/bin/orbit
```

---

## orbit snapshot

Syncs the context file an engine generates (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`) into the governance repo at the correct scope layer.

```bash
orbit snapshot                          # auto-detect file + scope from cwd
orbit snapshot --file path/to/file.md   # explicit source
orbit snapshot --stdin                  # read from stdin
orbit snapshot --dry-run                # show source and destination
orbit snapshot --output /path/to/dest   # override destination
```

| Scope | Destination |
|-------|-------------|
| Repository | `~/AI/tenants/<T>/projects/<P>/repositories/<R>/source-of-truth/context.md` |
| Project | `~/AI/tenants/<T>/projects/<P>/source-of-truth/context.md` |
| Tenant | `~/AI/tenants/<T>/source-of-truth/context.md` |

---

## orbit session

Manage active tmux sessions.

```bash
orbit session list              # list all tracked sessions
orbit session attach [id]       # attach (defaults to most recent)
orbit session kill <id>         # send SIGTERM
orbit session kill <id> --force # send SIGKILL
orbit session clean             # remove files for dead sessions
```

---

## orbit daemon

Background process that manages session state via a Unix socket (`~/.local/share/orbit/orbit.sock`).

```bash
orbit daemon start   # start in background
orbit daemon stop    # graceful shutdown
orbit daemon status  # uptime, session count, PID
```

`orbit launch` starts the daemon automatically if needed.

---

## orbit init

Set up the AI root (governance repository).

```bash
orbit init <git-url>    # clone a governance repo as ~/AI
orbit init --scaffold   # create ~/AI as a local-only directory (no git)
```

---

## orbit ls

Browse the workspace hierarchy.

```bash
orbit ls                          # list workspaces
orbit ls WORKSPACE                # list tenants
orbit ls WORKSPACE TENANT         # list projects
orbit ls WORKSPACE TENANT PROJECT # list repositories
```

---

## orbit mode

Switch between binary modes.

```bash
orbit mode stable        # install latest release
orbit mode dev [path]    # symlink to a local build
orbit mode beta          # install latest pre-release
orbit mode status        # show active mode and binary details
```

---

## orbit update

Self-update to the latest release. Respects the active mode (skips in `dev`, targets pre-releases in `beta`).

```bash
orbit update
orbit update --force   # reinstall even if already on latest
```

---

## orbit completions

Print shell completion script.

```bash
orbit completions bash   >> ~/.bashrc
orbit completions zsh    >> ~/.zshrc
orbit completions fish   > ~/.config/fish/completions/orbit.fish
```
"""

# ── Plugins.md ────────────────────────────────────────────────────────────────

PLUGINS = f"""# Plugins

orbit's plugin system manages optional external tools — CLIs, local services, and AI utilities — that enrich the workflow. Plugins can optionally contribute MCP servers to orbit sessions.

---

## What is a plugin?

A plugin is any external tool with its own install lifecycle. It may:

- Be a standalone CLI (`headroom`, `playwright`)
- Expose MCP servers that orbit injects into sessions when enabled
- Wrap the active AI engine (e.g. `headroom wrap claude`)

**Key distinction from scope-level MCPs:** plugins are global and require no per-project configuration. Tools that need connection strings, API keys, or project-specific paths belong in `mcp.json` at the appropriate scope level (tenant / project / repo), not in plugins.

---

## Available plugins

| Plugin | Category | Capabilities | Description |
|--------|----------|--------------|-------------|
{plugin_rows()}

---

## Commands

### List

```bash
orbit plugins list
```

Shows every plugin with its status:

- `●` installed + MCP active
- `✓` installed (no MCP, or MCP inactive)
- `○` not installed

```
plugins

  ○  headroom   (compression)  Context compression layer — 60–95% fewer tokens  [mcp: inactive]
  ●  playwright (browser)      Browser automation for AI agents                   [mcp: active]

  1/2 installed  ·  1/1 MCP active  ·  orbit plugins install/enable <name>
```

### Install

```bash
orbit plugins install <name>              # interactive method picker if multiple
orbit plugins install <name> --method pip # pick a specific method
orbit plugins install <name> -y           # accept first available method
```

### Enable / Disable

Enable registers the plugin's MCP servers so they are available in every new orbit session:

```bash
orbit plugins enable playwright
# ● playwright enabled
#   MCP registered: playwright
#   Config: ~/.config/orbit/plugins.mcp.json
#   Active in new orbit sessions.

orbit plugins disable playwright
# ✓ playwright disabled — MCP removed: playwright
```

MCP state is persisted in `~/.config/orbit/plugin-state.toml`. The MCP entries live in `~/.config/orbit/plugins.mcp.json` and are loaded as the baseline layer before any scope-level `mcp.json` files.

### Info

```bash
orbit plugins info headroom
```

Shows description, install methods, auth hints, MCP servers, and wrap capabilities.

### Wrap / Unwrap

Some plugins can proxy the active AI engine:

```bash
orbit plugins wrap headroom              # wraps the default engine
orbit plugins wrap headroom --engine claude
orbit plugins unwrap headroom
```

---

## Adding a custom plugin

Drop a `.toml` file into `~/.config/orbit/plugins/` — no rebuild required, available immediately.

To add a built-in plugin to the orbit project, add a `.toml` file to the `plugins/` directory and rebuild.

### Plugin TOML format

```toml
name = "my-tool"
description = "What this tool does"
category = "category-name"
url = "https://example.com"           # optional

[check]
binary = "my-tool"                    # binary to check with `which`

[[install]]
method = "npm"                        # pip | npm | cargo | brew | apt
cmd = ["npm", "install", "-g", "my-tool"]
label = "npm (Node.js)"

[[install]]                           # multiple install methods supported
method = "pip"
cmd = ["pip", "install", "my-tool"]
label = "PyPI (Python)"

[auth]                                # optional
hint = "Run `my-tool auth` or set MY_TOOL_API_KEY"

[[mcp]]                               # optional — repeat for multiple MCP servers
name = "my-tool-mcp"                  # name used in mcpServers config
command = "npx"
args = ["-y", "my-tool-mcp@latest"]
label = "My Tool MCP server"

[wrap]                                # optional — if the tool can proxy an engine
cmd_template = "my-tool wrap {{engine}}"
unwrap_cmd_template = "my-tool unwrap {{engine}}"
engines = ["claude", "opencode", "gemini"]
```

Fields:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Unique identifier, used in all commands |
| `description` | yes | One-line description shown in `list` and `info` |
| `category` | yes | Grouping label (e.g. compression, browser, ai) |
| `url` | no | Link to project page |
| `check.binary` | no | Binary name to detect if installed |
| `install[].method` | yes | Installer type: `pip`, `npm`, `cargo`, `brew`, `apt` |
| `install[].cmd` | yes | Full install command as an array |
| `install[].label` | yes | Human-readable label for the picker |
| `auth.hint` | no | Shown after install |
| `mcp[].name` | no | Key in `mcpServers` |
| `mcp[].command` | no | Binary to run the MCP server |
| `mcp[].args` | no | Arguments |
| `wrap.cmd_template` | no | Template with `{{engine}}` placeholder |

---

## How plugin MCPs work

When you run `orbit plugins enable <name>`, orbit writes the plugin's MCP entries to `~/.config/orbit/plugins.mcp.json`:

```json
{{
  "mcpServers": {{
    "playwright": {{
      "command": "npx",
      "args": ["-y", "@playwright/mcp@latest"]
    }}
  }}
}}
```

The orbit engine launcher loads this file as the **first** MCP layer, before any workspace or scope-level `mcp.json` files. Any scope-level file can override a plugin MCP by defining a server with the same name.

```
plugins.mcp.json          ← plugin MCPs (baseline)
~/AI/mcp.json             ← global AI root
~/AI/tenants/<T>/mcp.json ← tenant level
...                       ← project / repo (highest priority)
```
"""

# ── Workspace.md ──────────────────────────────────────────────────────────────

WORKSPACE = f"""# Workspace Structure

orbit organises AI context across four scope levels. Each level can have its own instructions, agent definitions, and MCP servers — all merged together at launch time.

---

## Directory layout

```
~/ (home)
├── AI/                                     ← global AI root (always loaded)
│   ├── mcp.json                            # workspace-wide MCP servers
│   ├── orbit.toml                          # workspace configuration
│   └── tenants/
│       └── <TENANT>/                       ← tenant scope
│           ├── mcp.json
│           ├── source-of-truth/            # tenant instructions & agents
│           └── projects/
│               └── <PROJECT>/              ← project scope
│                   ├── source-of-truth/
│                   └── repositories/
│                       └── <REPO>/         ← repository scope
│                           └── source-of-truth/
│
└── <WORKSPACE>/                            ← workspace root (code lives here)
    ├── AI/                                 # workspace AI context (optional)
    │   └── tenants/
    │       └── <TENANT>/
    └── <TENANT>/
        └── <PROJECT>/
            └── <REPO>/                     ← actual code repository
```

---

## The two roots

| Root | Path | Purpose |
|------|------|---------|
| **Global AI root** | `~/AI` | Always loaded. Governance repo with shared context, global MCP servers, agent definitions. |
| **Workspace root** | `~/<WORKSPACE>` | A working area. Each workspace has its own tenants, projects, and repos. |

`~/AI` loads for every session regardless of workspace. It provides the global baseline.

---

## MCP merge order

MCP servers are merged from lowest to highest priority. Higher layers override servers with the same name:

```
~/.config/orbit/plugins.mcp.json   ← enabled plugins (baseline)
~/AI/mcp.json                       ← global AI root
~/AI/tenants/<T>/mcp.json           ← tenant (shared)
~/AI/tenants/<T>/projects/<P>/mcp.json
~/AI/tenants/<T>/projects/<P>/repositories/<R>/mcp.json
<WORKSPACE>/AI/tenants/<T>/mcp.json ← workspace-local overrides
... (same hierarchy, local wins over shared)
```

---

## Context load order

When you run `orbit launch WORKSPACE TENANT PROJECT REPO`, orbit:

1. Loads `~/AI` as global context
2. Merges workspace → tenant → project → repository layers
3. Assembles all MCP servers in priority order
4. Writes a single engine config file
5. Launches the engine inside a named tmux session

---

## orbit launch scope levels

All arguments are positional and optional:

```bash
orbit launch                                    # global mode
orbit launch .                                  # auto-detect from cwd
orbit launch WORKSPACE                          # workspace only
orbit launch WORKSPACE TENANT                   # + tenant
orbit launch WORKSPACE TENANT PROJECT           # + project
orbit launch WORKSPACE TENANT PROJECT REPO      # full scope
```

Auto-detection (`orbit launch .`) walks ancestor directories to find the workspace root, then maps path segments to tenant / project / repository.

---

## Setup commands

```bash
orbit init <git-url>    # clone a governance repo as ~/AI
orbit init --scaffold   # create ~/AI without git (local only)
```

orbit does not auto-create workspace directories. You set them up once using your governance repo or manually.

---

## Config file

`~/.config/orbit/config.toml`:

```toml
[workspace]
ai_root = "~/AI"

[engine]
default = "opencode"
default_tenant = "work"

[install]
dir = "~/.local/bin"
```

Edit values with `orbit config set <key> <value>` or `orbit config edit`.
"""

# ── write all pages ───────────────────────────────────────────────────────────

write_wiki("Home.md", HOME)
write_wiki("Commands.md", COMMANDS)
write_wiki("Plugins.md", PLUGINS)
write_wiki("Workspace.md", WORKSPACE)

print("Wiki pages generated.")
