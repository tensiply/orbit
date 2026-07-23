---
description: Create a new orbit command interactively — file + manifest entry + optional scope overrides
agent: implementation
---

Create a new orbit command following the full orbit command system rules. Guides the user interactively until the command is created and registered.

## Context

Orbit commands live in `~/AI/source-of-truth/orbit/commands/<name>.md` and must be declared in `~/AI/source-of-truth/orbit/manifest.jsonc` to be materialized for Claude Code, OpenCode, and Gemini.

## Step 0 — Parse arguments

If `$ARGUMENTS` is provided, treat it as the command name and skip asking for it. Otherwise proceed to Step 1.

## Step 1 — Command name

Ask: "¿Cuál es el nombre del comando? (kebab-case, ej: `analyze-deps`)"

Rules:
- Must be kebab-case, lowercase, no spaces
- Must not already exist in `~/AI/source-of-truth/orbit/commands/`
- Check existence before accepting: `ls ~/AI/source-of-truth/orbit/commands/<name>.md 2>/dev/null`
- If it already exists, ask to confirm overwrite or choose a different name

## Step 2 — Description

Ask: "¿Qué hace este comando en una línea? (aparece en el command picker)"

Rules:
- One line, clear and specific
- No period at the end
- Example: "Analyze dependency graph and suggest upgrades"

## Step 3 — Agent

Ask: "¿Qué agente debe ejecutar este comando en OpenCode?"
Options: `plan` | `implementation` | `debug` | `review` | `build`

Guidance:
- `plan` — commands that design, analyze, or investigate before acting
- `implementation` — commands that write or modify code/files
- `debug` — commands that diagnose issues
- `review` — commands that evaluate quality or correctness
- `build` — commands that compile, run, or deploy

Note: Claude Code and Gemini ignore this field — it only affects OpenCode routing.

## Step 4 — Command body

Ask: "¿Cuáles son las instrucciones del comando? Describe los pasos que debe seguir el agente."

Guidance to give the user:
- Write in imperative form ("Run X", "Ask the user for Y", "Read file Z")
- Reference `$ARGUMENTS` if the command accepts parameters (e.g., `$ARGUMENTS` = command name passed by user)
- Reference other workflow files with their full path (`~/AI/source-of-truth/workflows/<name>.md`) for complex procedures
- Keep it executable: each step should map to a concrete action

Ask follow-up questions if the body is vague:
- "¿Qué parámetros acepta el comando?"
- "¿Qué archivos lee o escribe?"
- "¿Tiene pasos de validación o condiciones de error?"
- "¿Qué debe mostrarle al usuario al terminar?"

Keep asking until the body is complete and unambiguous.

## Step 5 — Scope overrides

Ask: "¿Necesita este comando comportamiento diferente por tenant, project o repo? (s/n)"

If yes:
- Ask which scope level: `tenant` | `project` | `repo`
- Ask which specific tenant/project/repo
- Ask what the override body should say (can be a partial override that extends the base)
- Resolve the override path:
  - tenant: `~/AI/tenants/<T>/source-of-truth/orbit/commands/<name>.md`
  - project: `~/AI/tenants/<T>/projects/<P>/source-of-truth/orbit/commands/<name>.md`
  - repo: `~/AI/tenants/<T>/projects/<P>/repositories/<R>/source-of-truth/orbit/commands/<name>.md`
- Create the override file with the same frontmatter + override body

Repeat for additional scopes if needed.

## Step 6 — Confirm before writing

Show a summary of everything that will be created:

```
Command:     <name>
Description: <description>
Agent:       <agent>
File:        ~/AI/source-of-truth/orbit/commands/<name>.md
Manifest:    ~/AI/source-of-truth/orbit/manifest.jsonc
Overrides:   <list or "none">
```

Ask: "¿Todo correcto? (s para crear / n para corregir)"

If no, go back to the step the user wants to fix.

## Step 7 — Create the command file

Write `~/AI/source-of-truth/orbit/commands/<name>.md`:

```markdown
---
description: <description>
agent: <agent>
---

<body>
```

## Step 8 — Update manifest.jsonc

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

Preserve all existing entries and formatting. Do not remove or reorder other keys.

## Step 9 — Create override files

If overrides were defined in Step 5, create each file now with its corresponding body.

Ensure the parent directory exists before writing:
```bash
mkdir -p <override-dir>
```

## Step 10 — Verify

Confirm everything was created correctly:
- `ls ~/AI/source-of-truth/orbit/commands/<name>.md` — file exists
- Grep the manifest: `grep -A1 '"<name>"' ~/AI/source-of-truth/orbit/manifest.jsonc` — entry present
- If overrides: `ls <override-path>` for each

Report: which files were created, where they live, and how to use the command (`/<name>` in Claude Code / OpenCode / Gemini).
