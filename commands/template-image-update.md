---
name: template-image-update
description: List all image templates in scope and update one — edit HTML, save in place
---

Show all available image templates (from all scopes) and update one by editing its HTML source.

## Steps

1. List all available templates with their source scope:
   ```bash
   orbit image template list
   ```
   This shows each template name, its source (builtin / user / workspace / tenant / project / repo), and its description.

2. Ask the user which template they want to update and what changes they want.

3. Show the current HTML source of the selected template:
   ```bash
   orbit image template show {name}
   ```

4. Determine the file path to edit:
   - If source is `builtin`: the template cannot be edited in-place. Offer to **create a user-level override** by copying the builtin content to `~/.orbit/data/templates/images/{name}.html`.
   - If source is `user:…`, `workspace:…`, `tenant:…`, `project:…`, or `repo:…`: the path after the colon is the exact file to edit.

5. Apply the changes directly to the file with the Edit tool.
   - Preserve the `<!-- --- … --- -->` YAML front matter block.
   - Keep inline CSS only — no CDN or remote assets.
   - Update the `variables:` list in the front matter if new `{{ var }}` placeholders were added.

6. Confirm the changes:
   ```bash
   orbit image template show {name}
   ```

7. Generate a test image to validate the updated template renders correctly:
   ```bash
   orbit image create --title "Test" --content "Updated template preview" --template {name}
   ```

## Notes

- To see which scope a template comes from, check the SOURCE column in `orbit image template list`.
- A template at a narrower scope (repo > project > tenant > workspace) overrides wider ones.
- To revert a user override back to the builtin, delete the file from `~/.orbit/data/templates/images/`.
