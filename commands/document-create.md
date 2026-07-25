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

The global default template is `befra-clean`: clean minimal PDF, no footer, pagination only, with title, optional subtitle, author, and timestamp. To override, pass `--template <name>`.

Run `orbit document template list` to see all available templates.

## When to use

Use this command whenever you need to produce a formatted document. Route all document generation through `orbit document create` rather than writing files directly — this ensures consistent formatting, template support, and scope-aware metadata.
