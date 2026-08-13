---
description: Create a new orbit command interactively — file + scope targeting
agent: implementation
---

Create a new orbit command. Guides the user interactively through scope, body, and file creation.

## How orbit commands work

| Location | Effect |
|---|---|
| `orbit/commands/<name>.md` | Compiled into binary — requires rebuild. Built-in commands in the orbit repo. |
| `~/.orbit/commands/<name>.md` | **Global user commands** — visible in ALL sessions across all workspaces. No rebuild needed. |
| `~/AI/source-of-truth/orbit/commands/<name>.md` | **Workspace override** — overrides a built-in for all sessions in this workspace. |
| `~/AI/tenants/<T>/source-of-truth/orbit/commands/<name>.md` | **Tenant-only** — visible only in sessions for that tenant. |
| `~/AI/tenants/<T>/projects/<P>/source-of-truth/orbit/commands/<name>.md` | **Project-only** — visible only in sessions for that project. |
| `~/AI/tenants/<T>/projects/<P>/repositories/<R>/source-of-truth/orbit/commands/<name>.md` | **Repo-only** — visible only in sessions for that repo. |

Overlays cascade: repo merges on top of project, on top of tenant, on top of the base.

## Step 0 — Parse arguments

If `$ARGUMENTS` is provided, treat it as the command name and skip Step 1.

## Step 1 — Command name

Ask: "¿Cuál es el nombre del comando? (kebab-case, ej: `analyze-deps`)"

Rules:
- Must be kebab-case, lowercase, no spaces

## Step 2 — Scope

Ask: "¿En qué scope debe vivir este comando?"

| # | Scope | Descripción |
|---|---|---|
| 1 | `global` | Visible en todas las sesiones de orbit (`~/.orbit/commands/`) |
| 2 | `workspace` | Override para todas las sesiones de este workspace (`~/AI/source-of-truth/orbit/commands/`) |
| 3 | `tenant` | Solo visible en sesiones de un tenant específico |
| 4 | `project` | Solo visible en sesiones de un project específico |
| 5 | `repo` | Solo visible en sesiones de un repo específico |

If `tenant`: ask which tenant (under `~/AI/tenants/<T>/`).
If `project`: ask which tenant and project.
If `repo`: ask which tenant, project, and repo.

Resolve the file path:
- global: `~/.orbit/commands/<name>.md`
- workspace: `~/AI/source-of-truth/orbit/commands/<name>.md`
- tenant: `~/AI/tenants/<T>/source-of-truth/orbit/commands/<name>.md`
- project: `~/AI/tenants/<T>/projects/<P>/source-of-truth/orbit/commands/<name>.md`
- repo: `~/AI/tenants/<T>/projects/<P>/repositories/<R>/source-of-truth/orbit/commands/<name>.md`

## Step 3 — Description

Ask: "¿Qué hace este comando en una línea? (aparece en el command picker)"

## Step 4 — Agent

Ask: "¿Qué agente debe ejecutar este comando en OpenCode?"
Options: `plan` | `implementation` | `debug` | `review` | `build`

Note: Claude Code and Gemini ignore this field.

## Step 5 — Command body

Ask: "¿Cuáles son las instrucciones del comando?"

Guidance:
- Write in imperative form ("Run X", "Ask the user for Y")
- Reference `$ARGUMENTS` if the command accepts parameters
- Reference workflow files with full path: `~/AI/source-of-truth/workflows/<name>.md`
- Keep it executable: each step maps to a concrete action

Ask follow-ups until the body is complete:
- "¿Qué parámetros acepta?"
- "¿Qué archivos lee o escribe?"
- "¿Pasos de validación o condiciones de error?"
- "¿Qué muestra al terminar?"

## Step 6 — Confirm

Show summary before writing:

```
Command:     <name>
Scope:       <scope>
File:        <resolved path>
Description: <description>
Agent:       <agent>
```

Ask: "¿Todo correcto? (s para crear / n para corregir)"

## Step 7 — Create the file

Create parent directories if needed:
```bash
mkdir -p <parent_dir>
```

Write the command file:
```markdown
---
description: <description>
agent: <agent>
---

<body>
```

## Step 8 — Update manifest.jsonc (only for `workspace` or `global` scope)

Read `~/AI/source-of-truth/orbit/manifest.jsonc` and add under `"commands"`:

```json
"<name>": {
  "source": "commands/<name>.md",
  "overrides": [
    "tenants/*/source-of-truth/orbit/commands/<name>.md",
    "tenants/*/projects/*/source-of-truth/orbit/commands/<name>.md",
    "tenants/*/projects/*/repositories/*/source-of-truth/orbit/commands/<name>.md"
  ]
}
```

For scope-only commands (tenant/project/repo): skip this step — orbit discovers them automatically from scope directories.

## Step 9 — Verify

Confirm file exists:
```bash
ls <resolved_path>
```

Report:
- File path created
- How to use: `/<name>` in Claude Code / OpenCode / Gemini
- Scope: "visible en todas las sesiones" or "solo en sesiones de <scope>"
- Note: "reinicia la sesión de orbit para que el comando aparezca en sesiones existentes"
