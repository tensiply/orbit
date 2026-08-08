---
name: template-image-create
description: Create a new image template scoped to global, workspace, tenant, project, or repo
---

Create an HTML image template and place it in the correct scope directory so orbit picks it up automatically.

## Scope → directory mapping

| Scope     | Directory                                                                                                       |
|-----------|-----------------------------------------------------------------------------------------------------------------|
| global    | `~/.orbit/data/templates/images/{name}.html`                                                                    |
| workspace | `$AI_CONTEXT_ROOT/templates/image/{name}.html`                                                                  |
| tenant    | `$AI_CONTEXT_ROOT/tenants/{TENANT}/templates/image/{name}.html`                                                 |
| project   | `$AI_CONTEXT_ROOT/tenants/{TENANT}/projects/{PROJECT}/templates/image/{name}.html`                              |
| repo      | `$AI_CONTEXT_ROOT/tenants/{TENANT}/projects/{PROJECT}/repositories/{REPO}/templates/image/{name}.html`          |

Templates at more specific scopes override less specific ones (repo > project > tenant > workspace > global > builtin).

## Steps

1. Ask the user for the basics:
   - **Template name** (kebab-case, no extension, e.g. `comunicado-interno`)
   - **Purpose / use case** (e.g. "internal notice", "deployment announcement", "weekly report cover")
   - **Scope**: global / workspace / tenant / project / repo — if tenant/project/repo, ask for the names

2. Ask about **style** — collect all answers before designing anything:
   - **Color scheme**: dark / light / brand colors? Ask for hex codes if specific.
   - **Typography**: large bold headline / editorial / minimal / technical?
   - **Layout**: centered card / full bleed / split (text + side) / header + body?
   - **Accent / highlight**: badge, border, icon, gradient — what visual emphasis?
   - **Dimensions**: default 1200×630 (OG image) — or custom width/height?
   - **Variables**: what text fields should the template expose? (default: `title`, `description`)
   - **Tone**: corporate / friendly / urgent / informational?

3. Summarize the style choices back to the user and confirm before generating.

4. Resolve the target directory from the scope table above.
   - Read `$AI_CONTEXT_ROOT` with `echo $AI_CONTEXT_ROOT` if needed.
   - For tenant/project/repo: construct the path from `AI_CONTEXT_ROOT` + scope names.

5. Create the directory if it doesn't exist:
   ```bash
   mkdir -p <target_dir>
   ```

6. Write the HTML template to `<target_dir>/{name}.html`.
   - Include the YAML front matter block at the very top:
     ```html
     <!-- ---
     name: {name}
     description: {one-line description}
     variables:
       - title
       - description
       - {other vars}
     width: 1200
     height: 630
     --- -->
     ```
   - Use **inline CSS only** — no CDN links (Chrome headless may lack network access).
   - Use minijinja syntax for variables: `{{ title }}`, `{{ description }}`, etc.
   - `orbit_scope` (tenant/project/repo) and `orbit_workspace` are always auto-injected.

7. Confirm the template is visible:
   ```bash
   orbit image template list
   orbit image template show {name}
   ```

8. Generate a test image to validate the template renders correctly:
   ```bash
   orbit image create --title "Test" --content "Template preview" --template {name}
   ```

## Notes

- Built-in templates cannot be modified. To override one, create a template with the same name at a wider scope.
- The template with the narrowest matching scope always wins at render time.
- Avoid external fonts or remote assets — embed base64 or use system fonts.
