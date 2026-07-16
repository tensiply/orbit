# Releases

> **Versión actual:** v0.11.0 — 14 de julio, 2026

## Saltar a un hito

- [Sistema de planes — v0.11.0](#sistema-de-planes--v0110)
- [Refactor de configuración — v0.10.x](#refactor-de-configuraci%C3%B3n--v010x)
- [Dry-run y ASCII art — v0.8.0–v0.9.0](#dry-run-y-ascii-art--v080v090)
- [Governance y engines — v0.6.0–v0.7.0](#governance-y-engines--v060v070)
- [Modelo de workspace — v0.3.0–v0.5.0](#modelo-de-workspace--v030v050)
- [Fundación — v0.1.0–v0.2.0](#fundaci%C3%B3n--v010v020)

---

## Sistema de planes — v0.11.0

### v0.11.0 (2026-07-14)

**Nuevos comandos:** `orbit plan`, `orbit memory`, `orbit serve`, `orbit discover`

**Motor de planes (P4–P8)**
- Motor de ejecución de planes con templates: PR, código, verificación, test, review
- Programación de planes (cron) y disparo vía webhook
- Notificaciones de escritorio al completar ejecuciones

**Presupuesto y memoria (P9–P10)**
- Control de presupuesto por ejecución (tokens o costo)
- Sistema de memoria y auditoría con historial consultable
- Seguimiento de costo real por ejecución

**Multi-scope y workspace (P11–P13)**
- Vista de planes multi-scope (`orbit plan list --all-scopes`)
- Registro y gestión de múltiples workspaces (`orbit workspace`)
- Scoping multi-workspace en context loading

**Refactor del resolver (P14)**
- Resolución de scope case-insensitive
- Auto-creación de entradas SOT cuando el directorio existe pero no tiene governance

**Integration tests (P15)**
- Harness completo de integration tests con 5000+ líneas de cobertura

**Executor plugins (P16)**
- Sistema de plugins para el executor de planes
- Permite conectar backends alternativos al engine por defecto

**Planner con IA (P17)**
- Integración del planner con el engine de IA para descomponer intenciones

**Sharing LAN (P18–P19)**
- `orbit serve`: servidor TCP + mDNS, RBAC, JWT local, descubrimiento cero-config
- `orbit serve` no-bloqueante por defecto
- `orbit serve stop` y `orbit serve status`
- Tab Peers en el TUI (tecla `9`)

---

## Refactor de configuración — v0.10.x

### v0.10.5 (2026-07-06)

**Nuevas features**
- **Formato `orbit.json`** — `orbit.json` / `orbit.jsonc` son ahora los nombres de config preferidos. `opencode.json` sigue siendo compatible.

**Correcciones**
- Resolución de scope case-insensitive en todos los niveles
- Auto-creación de entradas SOT para proyectos y repos sin governance
- Config de tenant/project/repo ahora solo carga desde el workspace AI root correspondiente; `~/AI` solo se consulta a nivel de raíz
- Labels correctos en dry-run: `(global)` para `~/AI`, `(workspace)` para `~/WORKSPACE/AI`
- La capa `opencode global` se oculta para engines no-opencode (`-e claude`, `-e gemini`)
- Dry-run muestra solo las entradas realmente cargadas
- `merge_layer` con semántica first-match: carga solo el archivo de mayor prioridad por scope

### v0.10.4 (2026-07-05)

- Actualización de dependencias: ratatui 0.29→0.30, crossterm 0.28→0.29, sha2 0.10→0.11, toml 0.8→1

### v0.10.3 (2026-07-05)

- `orbit status` ahora muestra el modo binario activo (`stable`, `dev`, o `beta`)

### v0.10.2 (2026-07-05)

- Plugin `rust-analyzer` — servidor de lenguaje Rust. Instala via `rustup component add` (recomendado), `cargo install`, `apt-get` o Homebrew
- Contexto Rust inyectado al lanzar con rust-analyzer habilitado
- `best_install_method()` reconoce `rustup` como método de instalación válido

---

## Dry-run y ASCII art — v0.8.0–v0.9.0

### v0.9.0 (2026-07-01)

- **ASCII art banner** — banner ORBIT en cyan en todos los comandos de usuario (`launch --dry-run`, `setup`, `doctor`, `status`). Se suprime automáticamente cuando stdout no es TTY
- **`--dry-run` legible** — reporte estructurado: scope resuelto, capas de config con `✓`/`·`, directorios de overlays de agentes, capas de MCP, archivos de instrucciones, servidores MCP activos
- **`config::inspect()` API** — nueva función pública en `orbit-engine` que devuelve `(MergedConfig, ScopeReport)` con visibilidad completa de capas
- **Carga dual de governance** — config e instrucciones ahora siguen el mismo patrón dual-root que ya tenía MCP: carga desde `~/AI` primero, luego `~/WORKSPACE/AI` en cada nivel de scope

### v0.8.0 (2026-07-01)

- **Título de terminal** — `orbit launch` establece el título del terminal vía xterm OSC: `orbit · <engine> · <tenant>/<project>/<repo>`. También establece el nombre de ventana tmux
- Fix: `orbit setup` ahora usa atomic rename para evitar `ETXTBSY` al reemplazar el binario en ejecución
- CI: matriz de tests Linux + macOS, job de gate, Dependabot

---

## Governance y engines — v0.6.0–v0.7.0

### v0.7.0 (2026-06-30)

**Catálogo y engines**
- **Catálogo de engines y MCPs** — compilado en el binario (`config/catalog/`). Actualizado en cada release
- **GitHub Copilot** — añadido al catálogo. Instala via `gh extension install github/gh-copilot`
- **`orbit engines`** — instalación, actualización e info de engines
- **`orbit auth`** — detección y gestión de autenticación (sin llamadas de red). `--check` para CI

**Nuevos comandos**
- **`orbit mcp`** — gestiona MCPs por scope: `list`, `enable`, `disable`, `info`
- **`orbit status`** — snapshot en ≤ 8 líneas: workspace, engine, daemon, versión. `--json` para scripts
- **`orbit config list`** y `orbit config set engine.default_workspace`
- **Auto-update** — actualiza governance repo y binario en background (24h TTL). Controla con `update.*` en config

**Sistema de plugins (base)**
- **`orbit plugins`** — `list`, `install`, `enable`, `disable`, `info`, `wrap`, `unwrap`
- Plugin `headroom` — compresión de contexto (60–95% menos tokens), puede envolver el engine
- Plugin `playwright` — automatización de browser vía `@playwright/mcp`

**Fix**
- Detección de auth de Gemini corregida: `~/.gemini/` en lugar de `.config/gemini`

### v0.6.0 (2026-06-30)

- `orbit plugins enable/disable` — activa/desactiva MCPs de plugins en todas las sesiones
- Plugin `playwright` inicial
- `orbit config get/set/list/edit` — lee y escribe config con notación de puntos
- `orbit snapshot` — sincroniza context file del engine al repo de governance
- `orbit setup` detecta engines faltantes y ofrece instalarlos
- `orbit doctor` reorganizado en secciones: engines, dependencies, workspace, config, daemon, binary
- Fix: `orbit launch` ahora respeta `engine.default` de config (antes siempre usaba opencode)

---

## Modelo de workspace — v0.3.0–v0.5.0

### v0.5.0 (2026-06-30)

- **`orbit launch .`** — auto-detección de scope desde el directorio actual
- **`orbit ls`** — navega la jerarquía workspace/tenant/project/repo
- **`orbit completions`** — autocompletado para bash, zsh, fish
- **`orbit doctor`** — diagnóstico inicial: tmux, engines, AI root, socket del daemon, directorio de instalación
- Daemon session launch vía IPC: `orbit launch` enruta por el daemon
- Daemon auto-start: si el socket no existe, `orbit launch` lo arranca
- TUI Launch: workspace selector con `[←→]`, dropdowns filtrables para tenant/project/repo
- TUI Sessions: árbol workspace cuando no hay sesiones activas

### v0.4.1 (2026-06-29)

- **`orbit mode`** — tres modos explícitos: `stable`, `dev [path]`, `beta`
  - `stable`: descarga e instala la última release de GitHub
  - `dev [path]`: symlink a un build local (ruta guardada en `~/.local/share/orbit/dev_path`)
  - `beta`: descarga e instala la última pre-release
  - `status`: muestra el modo activo
- Fix: `orbit mode stable/beta` ya no sobreescribe el build de dev

### v0.3.0 (2026-06-29)

- Multi-workspace en TUI: barra con workspace activo, `[w]` para ciclar. Detecta workspaces escaneando `~/` por `orbit.toml` o `tenants/`
- `orbit session attach` — attach directo si hay una sola sesión; selector si hay varias. Usa `switch-client` dentro de tmux
- `orbit update` — descarga, verifica SHA-256 y reinstala el binario atómicamente

---

## Fundación — v0.1.0–v0.2.0

### v0.2.0 (2026-06-28)

- Notificación de nueva versión al iniciar (cachéada 24h). Desactivar con `ORBIT_NO_UPDATE_CHECK=1` o `update.check_on_startup = false`

### v0.1.0 (2026-06-27)

**TUI inicial**
- Tab Sessions: lista, attach via tmux, kill, inspeccionar, limpiar entradas muertas
- Tab Launch: formulario para lanzar sesiones con opencode, gemini, claude
- Tab System: gestor de MCP servers por scope (global / tenant / project / repo)
- Daemon-aware: consulta socket del daemon (timeout 500ms), fallback a carga por archivo

**CLI base**
- `orbit init` — clona un repo de governance como AI root
- `orbit init --scaffold` — crea AI root local sin repo remoto
- `orbit launch` — lanza sesiones con contexto tenant/project/repo
- `orbit update` — self-update desde GitHub releases
- `orbit session list|kill|attach|clean`
- `orbit daemon serve|start|stop|status`

**Infraestructura**
- MCP en cuatro scopes: global, tenant, project, repository
- CI: format check, clippy `-D warnings`, tests en cada push/PR a `main`
- Release workflow: binarios estáticos para linux-x86_64, linux-aarch64 + SHA-256 checksums
