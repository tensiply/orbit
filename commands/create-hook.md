---
description: Create a new orbit hook interactively — engine hook (Claude Code) or planner hook
agent: implementation
---

Create a new orbit hook interactively. Orbit has two distinct hook systems — guide the user through the right one and create all necessary files.

## Step 0 — Hook type

Ask: "¿Qué tipo de hook quieres crear?"

Options:
1. **Engine hook** — se dispara en eventos del ciclo de vida de Claude Code (`Stop`, `Notification`, `PreToolUse`, etc.). Se define en un archivo TOML y orbit lo inyecta en cada sesión claude via `--settings`.
2. **Planner hook** — se dispara en eventos del planner (`pre_plan`, `post_plan`, `pre_node`, etc.). Se define en `~/.config/orbit/hooks.toml`.

If `$ARGUMENTS` is provided, treat it as the hook name and skip asking for it in the applicable step.

---

## ENGINE HOOK FLOW

Follow this flow if the user chose engine hook.

### E1 — Name

Ask: "¿Cuál es el nombre del hook? (kebab-case, ej: `post-plan-notify`)"

Rules:
- kebab-case, lowercase, no spaces
- Must not already exist: check `ls ~/AI/AIDEV/AI-ECOSYSTEM/orbit/hooks/<name>.toml 2>/dev/null`
- If it exists, ask to confirm overwrite or choose a different name

### E2 — Description

Ask: "¿Qué hace este hook en una línea?"

One line, clear and specific. Example: "Send a desktop notification when Claude finishes a task"

### E3 — Category

Ask: "¿Categoría?"
Options: `governance` | `productivity` | `security` | `dev`

Guidance:
- `governance` — logging, auditing, compliance
- `productivity` — notifications, integrations, shortcuts
- `security` — secret scanning, access alerts
- `dev` — build triggers, auto-format, test runners

### E4 — Events

Ask: "¿En qué eventos de Claude Code debe dispararse?"

Available events:

| Event | When |
|---|---|
| `Stop` | La sesión termina |
| `Notification` | Claude envía un mensaje/notificación |
| `PreToolUse` | Antes de cualquier tool call |
| `PostToolUse` | Después de cualquier tool call |
| `PreBash` | Antes de cada invocación de Bash |

Multiple events are allowed — ask if the user wants more than one.

For `PreToolUse` / `PostToolUse`: ask "¿Quieres restringirlo a una herramienta específica? (ej: `Bash`, `Edit`, `Write`) — deja vacío para todos"
If yes, record the `matcher` value.

### E5 — Script or command

Ask: "¿El hook ejecuta un script existente o necesitas crear uno nuevo?"
Options:
1. **Script existente** — solo introduce el path
2. **Script nuevo** — orbit lo crea como `[[scripts]]` dentro del TOML

If existing script: ask for the full path (supports `$HOME`).
If new script:
- Ask for the script path (suggestion: `$HOME/.claude/hooks/<name>.sh`)
- Ask: "¿El hook debe ser asíncrono? (async = no bloquea la sesión)" — default `false` for `Stop`, `true` for `Notification`
- Ask: "¿El script necesita algún binario externo? (ej: `notify-send`, `jq`) — deja vacío si no"
- Ask for the script body:
  - "Describe qué debe hacer el script. Puedo escribirlo por ti si me das los detalles."
  - The script receives JSON on stdin — tell the user what fields are available per event:
    - `Stop`: `{ "session_id": "...", "stop_hook_active": bool }`
    - `Notification`: `{ "message": "...", "session_id": "..." }`
    - `PreToolUse` / `PostToolUse`: `{ "tool_name": "...", "tool_input": {...} }`
    - `PreBash`: `{ "command": "..." }`
  - Write a complete, working bash script. Always include `#!/bin/bash` and a comment explaining the event/input.
  - Ask follow-up questions if needed: "¿Qué debe pasar si falla el binario?", "¿Necesitas parsear el JSON?", "¿Dónde loguea?"

### E6 — Location

Ask: "¿Dónde va este hook?"
Options:
1. **Built-in** — `hooks/<name>.toml` en el repo de orbit (se compila en el binario, disponible para todos)
2. **Usuario** — `~/.config/orbit/engine-hooks/<name>.toml` (solo tu máquina, no requiere recompilar)

Recommendation: user hooks para scripts personales o de entorno; built-in solo si debe distribuirse con orbit.

### E7 — Enable now

Ask: "¿Habilitar el hook inmediatamente con `orbit hooks enable <name>`? (s/n)"

### E8 — Confirm before writing

Show summary:

```
Tipo:        Engine hook
Nombre:      <name>
Descripción: <description>
Categoría:   <category>
Eventos:     <event list>
Script:      <path> (<nuevo/existente>)
Async:       <true/false>
Ubicación:   <built-in path | user path>
Habilitar:   <sí/no>
```

Ask: "¿Todo correcto? (s para crear / n para corregir)"

### E9 — Write the TOML file

Write the hook TOML to the chosen location.

Built-in format (with new script):
```toml
name = "<name>"
description = "<description>"
category = "<category>"
requires_binary = "<binary>"   # omit if none

[[events]]
event = "<Event>"
command = "<script-path>"
is_async = <true|false>
matcher = "<ToolName>"         # omit if no matcher

[[scripts]]
path = "<script-path>"
executable = true
content = """
<script body>
"""
```

If multiple events, add multiple `[[events]]` blocks.
Omit `requires_binary` if the user left it empty.
Omit `matcher` if the user left it empty.
Omit `[[scripts]]` if using an existing script.

### E10 — Enable if requested

If the user chose to enable:
```bash
orbit hooks enable <name>
```

### E11 — Verify

```bash
orbit hooks list
orbit hooks info <name>
```

Report: where the file lives, whether it's enabled, and how to test it.

---

## PLANNER HOOK FLOW

Follow this flow if the user chose planner hook.

### P1 — Event

Ask: "¿En qué evento del planner debe dispararse?"

| Event | When |
|---|---|
| `pre_plan` | Antes de que empiece la ejecución del plan |
| `post_plan` | Después de que el plan termina (éxito o falla) |
| `pre_node` | Antes de ejecutar cada nodo |
| `post_node` | Después de que cada nodo termina |
| `on_plan_created` | Cuando un plan es guardado y encolado |
| `on_schedule_fired` | Cuando el scheduler dispara un plan programado |

### P2 — Command

Ask: "¿Qué comando debe ejecutarse? Introduce el binario y sus argumentos."

Examples:
- `["notify-send", "Orbit", "Plan terminado"]`
- `["/home/user/scripts/post-plan.sh"]`
- `["curl", "-X", "POST", "-d", "{\"text\":\"done\"}", "https://..."]`

Tip: for complex logic, suggest creating a shell script and pointing to it.

Available env var at runtime: `ORBIT_HOOK_EVENT` (the event name).

If the user wants a new script: ask for the path and body, then create the file with `chmod +x`.

### P3 — Confirm before writing

Show summary:

```
Tipo:    Planner hook
Evento:  <event>
Comando: <command array>
Archivo: ~/.config/orbit/hooks.toml (o $ORBIT_CONFIG_HOME/orbit/hooks.toml)
```

Ask: "¿Todo correcto? (s para crear / n para corregir)"

### P4 — Append to hooks.toml

Resolve the config path:
```bash
echo "${ORBIT_CONFIG_HOME:-$HOME/.config}/orbit/hooks.toml"
```

If the file doesn't exist, create it. If it exists, append to it.

Add:
```toml
[[hooks]]
event = "<event>"
command = [<command array>]
```

### P5 — Verify

```bash
cat "${ORBIT_CONFIG_HOME:-$HOME/.config}/orbit/hooks.toml"
```

Report: where the entry was added, which event it listens to, and how to test it (run a plan and check for the side effect).
