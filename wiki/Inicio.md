# orbit

**orbit** es un launcher de CLI para asistentes de IA (opencode, Gemini CLI, Claude Code, GitHub Copilot) con gestión de workspace multi-tenant, seguimiento de sesiones, configuración de servidores MCP y un sistema de plugins.

Construido en Rust. Corre en Linux y macOS.

**Versión actual:** [v0.19.0](https://github.com/eloircorona/orbit/releases/tag/v0.19.0) · en desarrollo activo

---

## ¿Qué hace?

orbit resuelve el contexto correcto para tu sesión de IA — tenant, proyecto, repositorio, instrucciones, servidores MCP — y lanza el engine listo para trabajar. Gestiona las sesiones vía tmux y provee una interfaz TUI con chat integrado, navegación de sesiones, planner de IA y administración de plugins y MCP. Todos los datos de orbit viven en `~/.orbit/`.

---

## Highlights

| Feature | Descripción |
|---|---|
| **Contexto por capas** | Instrucciones y MCP se acumulan desde global → workspace → tenant → proyecto → repo |
| **Multi-engine** | opencode, Gemini CLI, Claude Code, GitHub Copilot |
| **Sesiones tmux** | Lanza y gestiona sesiones persistentes con attach/kill/clean |
| **Chat TUI** | Tab 0 del TUI: chat directo con el planner de IA integrado |
| **Planner híbrido** | Ejecución autónoma de tareas con clasificación de intenciones, gap resolver y validación |
| **Plugins** | Herramientas opcionales con lifecycle propio (jenkins, sonarcloud, linear, playwright…) |
| **MCP por scope** | Servidores MCP configurados a nivel global, workspace, tenant, proyecto o repositorio |
| **Activity log** | Historial de sesiones por scope; inyectado automáticamente al lanzar |
| **Scope catalog** | `orbit scope` — escanea y verifica la salud de toda tu estructura de workspaces |
| **Documentos** | Genera PDF, HTML, DOCX, XLSX, CSV desde templates y reglas YAML |
| **Secrets por workspace** | `keychain://token` resuelve primero `{workspace}/token` antes del key global |
| **`~/.orbit/` centralizado** | Toda la data en un solo directorio: `data/`, `cache/`, `state/`, `run/` |
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
- [Actividad](Actividad) — `orbit activity`, historial de sesiones por scope

### Referencia técnica
- [Arquitectura](Arquitectura) — crates, daemon IPC, internals
- [Contribuir](Contribuir) — dev setup, CI gates, convenciones
- [Releases](Releases) — historial completo de versiones

---

## Inicio rápido

```bash
# Instalar (Linux x86_64)
curl -fsSL https://github.com/eloircorona/orbit/releases/latest/download/orbit-linux-x86_64 \
  -o ~/.local/bin/orbit && chmod +x ~/.local/bin/orbit

# Primera configuración
orbit setup
eval "$(orbit shell-init)"   # integrar con tu shell
orbit completions install    # autocompletado

# Lanzar desde el directorio actual
orbit launch .

# TUI interactivo
orbit
```
