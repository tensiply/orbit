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
   If the user wants to change the dimensions, present the 4 standard ratios:

   | # | Ratio | Dimensions | Nombre     | Uso típico                                 |
   |---|-------|------------|------------|--------------------------------------------|
   | 1 | 16:9  | 1200×630   | Panorámica  | OG images, presentaciones, YouTube thumb   |
   | 2 | 1:1   | 1080×1080  | Cuadrado    | Instagram feed, avatar, post cuadrado      |
   | 3 | 4:5   | 1080×1350  | Retrato     | Instagram portrait, pin vertical           |
   | 4 | 9:16  | 1080×1920  | Story       | Instagram/TikTok Stories, Reels            |
   | 5 | Custom | —         | Personalizado | pedir width y height exactos             |

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
   - If dimensions changed: update `width` and `height` in the front matter **and** in the CSS. Always use `{{ width }}px`/`{{ height }}px` variables in `html, body` — never hardcode pixel values.

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
