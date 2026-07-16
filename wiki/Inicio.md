# orbit

**orbit** es un launcher de CLI para asistentes de IA (opencode, Gemini CLI, Claude Code, GitHub Copilot) con gestión de workspace multi-tenant, seguimiento de sesiones, configuración de servidores MCP y un sistema de plugins.

Construido en Rust. Corre en Linux y macOS.

**Versión actual:** [v0.11.0](https://github.com/befraeloircorona/orbit/releases/tag/v0.11.0)

---

## ¿Qué hace?

orbit resuelve el contexto correcto para tu sesión de IA — tenant, proyecto, repositorio, instrucciones, servidores MCP — y lanza el engine listo para trabajar. Gestiona las sesiones vía tmux y provee una interfaz TUI para navegar sesiones activas, lanzar nuevas y administrar plugins y MCP.

---

## Highlights

| Feature | Descripción |
|---|---|
| **Contexto por capas** | Instrucciones y MCP se acumulan desde global → workspace → tenant → proyecto → repo |
| **Multi-engine** | opencode, Gemini CLI, Claude Code, GitHub Copilot |
| **Sesiones tmux** | Lanza y gestiona sesiones persistentes con attach/kill/clean |
| **TUI** | Dashboard interactivo: sesiones, lanzador, MCP, plugins |
| **Plugins** | Herramientas opcionales con lifecycle propio (headroom, playwright, rust-analyzer…) |
| **MCP por scope** | Servidores MCP configurados a nivel global, tenant, proyecto o repositorio |
| **Sistema de planes** | Ejecución autónoma de tareas con IA, templates y auditoría |
| **Sharing LAN** | Comparte tu instancia orbit en red local vía mDNS |

---

## Navegación

### Uso básico
- [Primeros pasos](Primeros-pasos) — instalación y primer lanzamiento
- [Lanzar y sesiones](Lanzar-y-sesiones) — `orbit launch`, `orbit session`
- [Comandos](Comandos) — referencia completa de todos los comandos

### Configuración
- [Modelo de workspace](Modelo-de-workspace) — jerarquía de scopes, capas de config
- [Configuración](Configuracion) — orbit.json, config.toml, `orbit config`
- [Secretos y variables](Secretos-y-variables) — `orbit secret`, `orbit env`

### Herramientas
- [Engines](Engines) — engines soportados, instalación, autenticación
- [Plugins](Plugins) — catálogo built-in, lifecycle, plugins custom
- [Servidores MCP](Servidores-MCP) — `orbit mcp`, configuración por scope

### Funciones avanzadas
- [Sistema de planes](Sistema-de-planes) — `orbit plan`, ejecución autónoma, memoria
- [Compartir y descubrir](Compartir-y-descubrir) — `orbit serve`, `orbit discover`, LAN

### Referencia técnica
- [Arquitectura](Arquitectura) — crates, daemon IPC, internals
- [Contribuir](Contribuir) — dev setup, CI gates, convenciones
- [Releases](Releases) — historial completo de versiones

---

## Inicio rápido

```bash
# Instalar (Linux x86_64)
curl -fsSL https://github.com/befraeloircorona/orbit/releases/latest/download/orbit-linux-x86_64 \
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# Primera configuración
orbit setup

# Lanzar desde el directorio actual
orbit launch .

# TUI interactivo
orbit
```
