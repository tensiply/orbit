---
name: svg-update
description: Regenerate an existing orbit SVG with new content or different template variables
---

Update an existing SVG by ID or path: backs up the source, applies new content, and regenerates the file in place.

## When to use

Use this command when the user asks to:
- Revise a previously generated SVG (change text, colors, layout)
- Apply feedback to a logo, badge, or diagram created with `/svg-create`
- Regenerate an SVG with updated template variables
- Switch to a different template for an existing SVG

## Usage

```bash
orbit svg update <ID_OR_PATH> [OPTIONS]
```

| Argument / Option         | Purpose                                                          |
|---------------------------|------------------------------------------------------------------|
| `<ID_OR_PATH>`            | SVG ID (e.g., `SVG-000001`) or full path to the output file     |
| `--content "..."`         | New description or raw SVG content (inline)                      |
| `--content-file <PATH>`   | Path to a file containing the new content                        |
| `--var KEY=VALUE`         | Override a template variable (repeatable)                        |
| `--template <NAME>`       | Use a different template for this regeneration                   |
| `--open`                  | Open the output directory in the file explorer after update      |

## Workflow

1. Identify the SVG to update — ask for the ID or run `orbit svg list`.
2. Ask what changes the user wants (color, text, layout, template swap).
3. Run `orbit svg update`:

```bash
# Change description / tagline
orbit svg update SVG-000001 --content "New tagline here"

# Change a color variable
orbit svg update SVG-000001 --var "primary_color=#0ea5e9"

# Change multiple variables at once
orbit svg update SVG-000001 --var "primary_color=#0ea5e9" --var "tagline=New tagline"

# Switch to a different logo shape
orbit svg update SVG-000001 --template logo-circle

# Update badge value
orbit svg update SVG-000002 --content "v3.0.0" --var "value_color=#22c55e"
```

4. Confirm the updated output path. Share the backup location if the user needs to revert.

## Notes

- The **source `.txt` file** is backed up as `{filename}.txt.backup` before overwriting.
- The backup is kept permanently — never delete it without confirming with the user.
- The `SVG-XXXXXX` ID is preserved — no new index entry is created.
- Variables passed with `--var` **merge** with the original entry's stored variables (they don't replace all of them). To reset a variable, pass it explicitly with an empty value or the new default.
- To see existing SVGs and their IDs: `orbit svg list`
- To switch backend (template ↔ raw), the entry's stored backend is reused unless the new template or content implies a change — explicit `--template` forces template backend.

## Prefer update over create when

- The SVG already exists in `orbit svg list`
- The user says "change", "update", "fix", "adjust", "try with different color", etc.
- You want to preserve the SVG-ID and avoid accumulating files

## Prefer create --replace when

- You are iterating in the current session and haven't ended the conversation yet
- The user wants quick back-and-forth with no backup overhead
