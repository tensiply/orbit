---
name: template-image-create
description: Create a new image template scoped to global, workspace, tenant, project, or repo
---

Create an HTML image template and place it in the correct scope directory so orbit picks it up automatically.

## Scope → directory mapping

| Scope     | Directory                                                                                    |
|-----------|----------------------------------------------------------------------------------------------|
| global    | `~/.orbit/data/templates/images/{name}.html`                                                 |
| workspace | `$AI_CONTEXT_ROOT/templates/image/{name}.html`                                               |
| tenant    | `$AI_CONTEXT_ROOT/tenants/{TENANT}/templates/image/{name}.html`                              |
| project   | `$AI_CONTEXT_ROOT/tenants/{TENANT}/projects/{PROJECT}/templates/image/{name}.html`           |
| repo      | `$AI_CONTEXT_ROOT/tenants/{TENANT}/projects/{PROJECT}/repositories/{REPO}/templates/image/{name}.html` |

Templates at more specific scopes override less specific ones (repo > project > tenant > workspace > global > builtin).

## Steps

1. Ask the user:
   - **Template name** (kebab-case, no extension, e.g. `comunicado-interno`)
   - **Visual description**: layout, colors, typography, dimensions (default 1200×630)
   - **Scope**: global / workspace / tenant / project / repo — if tenant/project/repo, ask for the names
   - **Variables**: what minijinja `{{ var }}` placeholders the template should expose

2. Resolve the target directory from the scope using the table above.
   - `$AI_CONTEXT_ROOT` is available as the env var `AI_CONTEXT_ROOT`; read it with `echo $AI_CONTEXT_ROOT` if needed.
   - For tenant/project/repo: construct the path manually from `AI_CONTEXT_ROOT` + scope names.

3. Create the directory if it doesn't exist:
   ```bash
   mkdir -p <target_dir>
   ```

4. Write the HTML template to `<target_dir>/{name}.html`.
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

5. Confirm the template is visible:
   ```bash
   orbit image template list
   orbit image template show {name}
   ```

6. Generate a test image to validate the template renders correctly:
   ```bash
   orbit image create --title "Test" --content "Template preview" --template {name}
   ```

## Notes

- Built-in templates cannot be modified. To override one, create a template with the same name at a wider scope.
- The template with the narrowest matching scope always wins at render time.
- Avoid external fonts or remote assets — embed base64 or use system fonts.
