---
name: svg-create
description: Generate an SVG file using orbit's svg pipeline — minijinja templates or raw SVG content
---

Generate an SVG file from a template or raw SVG markup. Always saves a `.txt` source alongside the output.

## Usage

```
orbit svg create --title "<title>" [--content "<text>"] [--backend template|raw] [--template <name>] [--var KEY=VALUE...] [--force] [--replace [SVG-ID]]
```

## Backends

| Backend    | Flag                  | Description                                          |
|------------|-----------------------|------------------------------------------------------|
| `template` | `--backend template`  | Render a minijinja `.svg` template (default)         |
| `raw`      | `--backend raw`       | Write `--content` verbatim as the `.svg` file        |

Use `template` for design-driven SVGs (logos, badges, diagrams). Use `raw` when you already have the SVG markup.

## Builtin templates

| Template          | Dimensions | Use case                                          |
|-------------------|------------|---------------------------------------------------|
| `blank`           | 800×480    | Empty canvas with title and optional description  |
| `badge`           | 160×28     | Horizontal label/value badge (shields.io style)   |
| `logo-rectangle`  | 480×120    | Horizontal logo — headers, nav bars, emails       |
| `logo-square`     | 240×240    | Stacked logo — app icons, social thumbnails       |
| `logo-circle`     | 240×240    | Circular logo — avatars, profile pictures         |

Run `orbit svg template list` to see all available templates including workspace overrides.

## Auto-injected variables (always available in templates)

| Variable          | Value                                            |
|-------------------|--------------------------------------------------|
| `title`           | from `--title`                                   |
| `description`     | from `--content`                                 |
| `orbit_workspace` | workspace name from `AI_WORKSPACE_ROOT`          |
| `orbit_scope`     | `tenant/project/repo` from env                   |

## Logo template variables (optional, all have defaults)

| Variable        | Default        | Description                                    |
|-----------------|----------------|------------------------------------------------|
| `primary_color` | `#4F46E5`      | Icon mark fill color                           |
| `text_color`    | `#111827`      | Name text color                                |
| `bg_color`      | `transparent`  | SVG background                                 |
| `icon_letter`   | first char of title (uppercase) | Override the letter in the icon mark |
| `tagline`       | `description`  | Subtitle below the name                        |

## Badge template variables

| Variable        | Default        | Description             |
|-----------------|----------------|-------------------------|
| `label`         | `title`        | Left label text         |
| `value`         | `description`  | Right value text        |
| `label_color`   | `#555`         | Left section background |
| `value_color`   | `#4c9be8`      | Right section background|

## Examples

Generate with the default blank template:
```
orbit svg create --title "My Diagram" --content "Work in progress"
```

Generate a horizontal logo:
```
orbit svg create --title "Acme Corp" --content "Software solutions" --template logo-rectangle --var "primary_color=#0ea5e9"
```

Generate a square logo with custom icon letter:
```
orbit svg create --title "Acme Corp" --template logo-square --var "primary_color=#16a34a" --var "icon_letter=A"
```

Generate a circular logo for a profile picture:
```
orbit svg create --title "Acme Corp" --content "acmecorp.io" --template logo-circle --var "primary_color=#dc2626"
```

Generate a badge:
```
orbit svg create --title "version" --content "v2.1.0" --template badge --var "value_color=#22c55e"
```

Generate from raw SVG markup:
```
orbit svg create --title "Custom Icon" --content "<svg xmlns='http://www.w3.org/2000/svg'><circle cx='50' cy='50' r='40' fill='blue'/></svg>" --backend raw
```

Replace the last generated SVG in a session:
```
orbit svg create --title "Acme Corp" --content "Updated tagline" --template logo-rectangle --replace
```

Replace a specific SVG by ID:
```
orbit svg create --title "Acme Corp" --content "Final version" --template logo-rectangle --replace SVG-000003
```

## Iterating on an SVG

Use `--replace` to overwrite an existing SVG without creating a new file or ID:

- `--replace` (no value) → replaces the **last** SVG created in the workspace
- `--replace SVG-000001` → replaces a **specific** SVG by ID

`--replace` skips the `.bk` backup and reuses the same output path and SVG-ID regardless of title changes. Use this when refining an SVG across multiple iterations in the same session.

## Rules

- ALWAYS use `orbit svg create` — never write SVG files manually.
- NEVER specify `--output` — let orbit choose the scope-based path under `~/.orbit/files/svgs/`.
- The description is ALWAYS saved as a `.txt` file alongside the SVG for traceability.
- Run `orbit svg template list` to see all available templates before picking one.
- When iterating on an SVG in a session, use `--replace` to avoid accumulating multiple files.
- For logo templates, always pass `primary_color` — the default indigo rarely matches the user's brand.
- For the `badge` template, pass `--title` as the label and `--content` as the value.
