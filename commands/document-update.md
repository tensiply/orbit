# /document-update

Update an existing orbit document: back up the source, apply new content, and regenerate.

## When to use

Use this command when the user asks to:
- Edit or revise a previously generated document
- Apply feedback or changes to a report, proposal, spec, or any document created with `/document-create`
- Regenerate a document with updated content

## Usage

```bash
orbit document update <ID_OR_PATH> [OPTIONS]
```

| Argument / Option         | Purpose                                                           |
|---------------------------|-------------------------------------------------------------------|
| `<ID_OR_PATH>`            | Document ID (e.g., `DOC-000001`) or full path to the output file |
| `--content "..."`         | New content inline (markdown / JSON / CSV)                        |
| `--content-file <PATH>`   | Path to a file containing the new content                         |
| `--var KEY=VALUE`         | Override a template variable (repeatable)                         |
| `--template <NAME>`       | Use a different template for this regeneration                    |
| `--open`                  | Open the output directory in the file explorer after update       |

## Workflow

1. Ask the user what changes they want in the document.
2. Write the revised content (full document, not a diff).
3. Save to a temporary file if the content is long, then use `--content-file`.
4. Run `orbit document update`:

```bash
# Inline content
orbit document update DOC-000001 --content "# Updated Title\n\n..."

# From file
orbit document update DOC-000001 --content-file /tmp/updated-content.md --open

# With template variables
orbit document update DOC-000001 --content-file /tmp/new.md --var author="Jane Doe"
```

5. Confirm the output path and show any backup location to the user.

## Notes

- The **source file** (e.g., `.md`) is backed up as `{filename}.backup` before overwriting.
- The backup is kept permanently — never delete it without confirming with the user.
- To see available document IDs: `orbit document list`
- The document is regenerated using the same format and template as the original, unless overridden.
- If `--open` is passed, the **directory** (not the file) is opened in the file explorer.
