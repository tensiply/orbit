# orbit

A CLI launcher for AI coding assistants (opencode, Gemini CLI, Claude Code) with multi-tenant workspace management, session tracking, MCP server configuration, and a plugin system.

Built in Rust. Runs on Linux and macOS.

**Latest:** [v0.11.2](https://github.com/befraeloircorona/orbit/releases/tag/v0.11.2) · [Changelog](CHANGELOG.md) · [Wiki](https://github.com/befraeloircorona/orbit/wiki)

---

## What it does

orbit resolves the right context for your AI session — tenant, project, repository, instructions, MCP servers — and launches the engine ready to work. It manages sessions via tmux and provides a terminal UI for navigating active sessions, launching new ones, and managing plugins and MCP configuration.

---

## Install

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

Requires [tmux](https://github.com/tmux/tmux) and at least one AI engine: [opencode](https://opencode.ai), [Gemini CLI](https://github.com/google-gemini/gemini-cli), or [Claude Code](https://claude.ai/code).

```bash
orbit setup   # first-time config: engines, plugins, install dir
```

---

## Workspace

orbit organises context across four scope levels: **workspace → tenant → project → repository**. Each level can define its own instructions and MCP servers — orbit merges them all before launching the engine.

```
~/AI/                          ← global AI root (always loaded)
└── tenants/
    └── MYCO/                  ← tenant
        └── projects/
            └── backend/       ← project
                └── repositories/
                    └── api/   ← repository

~/MYCO/                        ← actual code lives here (separate from AI context)
    └── backend/
        └── api/
```

`~/AI` is a governance repository — it holds shared instructions, agent definitions, and MCP servers for all your workspaces. It is always loaded regardless of which workspace you launch from.

When you run `orbit launch MYCO backend api`, orbit:
1. Loads `~/AI` as the global baseline
2. Merges tenant → project → repository layers on top
3. Assembles all MCP servers from every layer
4. Launches the engine with the combined context inside a tmux session

```bash
orbit launch .                             # auto-detect scope from current directory
orbit launch WORKSPACE TENANT              # explicit scope
orbit launch WORKSPACE TENANT PROJECT REPO # full scope
```

---

## Quick reference

```bash
orbit                    # TUI dashboard
orbit launch .           # launch session from cwd
orbit doctor             # check engines, deps, plugins
orbit plugins list       # available plugins + status
orbit plugins enable playwright   # activate playwright MCP in all sessions
orbit session list       # active sessions
orbit config set engine.default claude
```

---

## Build from source

```bash
git clone https://github.com/befraeloircorona/orbit.git
cd orbit
cargo build --release
cp target/release/orbit ~/.local/bin/
```

Requires Rust 1.75+.

---

## Docs

Documentación completa en la [wiki](https://github.com/befraeloircorona/orbit/wiki).

**Uso básico**
- [Primeros pasos](https://github.com/befraeloircorona/orbit/wiki/Primeros-pasos) — instalación, `orbit setup`, primer lanzamiento
- [Lanzar y sesiones](https://github.com/befraeloircorona/orbit/wiki/Lanzar-y-sesiones) — `orbit launch`, `orbit session`, TUI, tmux
- [Comandos](https://github.com/befraeloircorona/orbit/wiki/Comandos) — referencia completa de todos los comandos

**Configuración**
- [Modelo de workspace](https://github.com/befraeloircorona/orbit/wiki/Modelo-de-workspace) — jerarquía de scopes, capas de config, governance
- [Configuración](https://github.com/befraeloircorona/orbit/wiki/Configuracion) — orbit.json, config.toml, `orbit config`
- [Secretos y variables](https://github.com/befraeloircorona/orbit/wiki/Secretos-y-variables) — `orbit secret`, `orbit env`, resolvers

**Herramientas**
- [Engines](https://github.com/befraeloircorona/orbit/wiki/Engines) — engines soportados, instalación, autenticación
- [Plugins](https://github.com/befraeloircorona/orbit/wiki/Plugins) — catálogo built-in, lifecycle, plugins custom
- [Servidores MCP](https://github.com/befraeloircorona/orbit/wiki/Servidores-MCP) — `orbit mcp`, configuración por scope

**Funciones avanzadas**
- [Sistema de planes](https://github.com/befraeloircorona/orbit/wiki/Sistema-de-planes) — `orbit plan`, ejecución autónoma, memoria
- [Compartir y descubrir](https://github.com/befraeloircorona/orbit/wiki/Compartir-y-descubrir) — `orbit serve`, `orbit discover`, LAN/mDNS

**Referencia**
- [Arquitectura](https://github.com/befraeloircorona/orbit/wiki/Arquitectura) — crates, daemon IPC, internals
- [Contribuir](https://github.com/befraeloircorona/orbit/wiki/Contribuir) — dev setup, CI gates, convenciones
- [Releases](https://github.com/befraeloircorona/orbit/wiki/Releases) — historial completo de versiones
