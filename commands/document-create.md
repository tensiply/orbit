---
name: document-create
description: Generate a document (PDF, HTML, DOCX, XLSX, CSV) using orbit's document pipeline
---

Generate a document from the current content or an explicit request.

## Usage

```
orbit document create --title "<title>" [--type <format>] [--content-file <path>] [--template <name>] [--var KEY=VALUE...] [--open]
```

## Examples

Generate a PDF report from a markdown file:
```
orbit document create --title "Architecture Report" --type pdf --content-file docs/architecture.md --template technical-report --var project=orbit --var version=1.0
```

Generate an HTML document inline:
```
orbit document create --title "Release Notes" --type html --content "# v1.2.0\n\n- Added document generation\n- Fixed config merge"
```

Generate an XLSX spreadsheet from JSON data:
```
orbit document create --title "Metrics" --type xlsx --content '[{"name":"orbit","version":"1.0","status":"active"}]'
```

## Formats

| Format | Flag         | Renderer      |
|--------|--------------|---------------|
| PDF    | `--type pdf` | weasyprint    |
| HTML   | `--type html`| builtin       |
| DOCX   | `--type docx`| pandoc        |
| XLSX   | `--type xlsx`| xlsxwriter    |
| CSV    | `--type csv` | builtin       |

## Default template

The active template is resolved from the workspace PDF rule (`$AI_CONTEXT_ROOT/document-rules/pdf.yaml`). Do NOT guess or hardcode a template name — omit `--template` to use the workspace default. Pass `--template <name>` only when the user explicitly requests a specific template.

Run `orbit document template list` to see available templates.

## Output location

NEVER specify `--output`. Let orbit choose the path — it saves the file under `~/.orbit/documents/{workspace}/{tenant}/...` and registers it in the index so `orbit document list` and `orbit document update` work.

## When to use

Use this command whenever you need to produce a formatted document. NEVER use weasyprint, pandoc, or direct file writes — always route through `orbit document create`. This ensures the file is indexed, traceable, and goes to the correct scope directory.
