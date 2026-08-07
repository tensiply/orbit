---
name: image-create
description: Generate an image (PNG, JPEG, WEBP) using orbit's image pipeline — HTML templates via Chrome or AI (DALL-E 3)
---

Generate an image from text content. Always saves a source .txt file alongside the image.

## Usage

```
orbit image create --title "<title>" [--content "<text>"] [--backend template|ai] [--type png|jpeg|webp] [--template <name>] [--var KEY=VALUE...] [--width N] [--height N] [--open]
```

## Examples

Generate a notice image (default template, PNG):
```
orbit image create --title "System Maintenance" --content "Scheduled maintenance Sunday 2-4am UTC. All services unavailable."
```

Generate with explicit template:
```
orbit image create --title "Deployment Complete" --content "v2.1.0 deployed to production." --template notice
```

Generate with AI backend (requires OPENAI_API_KEY):
```
orbit image create --title "Product Launch" --content "Our new feature is live. Try it now." --backend ai
```

Generate a custom-sized image:
```
orbit image create --title "Header Banner" --content "Welcome" --width 1920 --height 600
```

Inject custom template variables:
```
orbit image create --title "Alert" --content "Critical issue detected." --var accent=#ef4444
```

## Backends

| Backend  | Flag                 | Requirements                |
|----------|----------------------|-----------------------------|
| template | `--backend template` | Google Chrome or Chromium   |
| ai       | `--backend ai`       | OPENAI_API_KEY              |

## Formats

| Format | Flag           |
|--------|----------------|
| PNG    | `--type png`   |
| JPEG   | `--type jpeg`  |
| WEBP   | `--type webp`  |

## Template variables (builtin `notice` template)

| Variable      | Description                         |
|---------------|-------------------------------------|
| `title`       | Auto-injected from --title          |
| `description` | Auto-injected from --content        |
| `orbit_scope` | Auto-injected (tenant/project/repo) |

## Rules

- ALWAYS use `orbit image create` — never generate images manually.
- NEVER specify `--output` — let orbit choose the scope-based path.
- The text content is ALWAYS saved as a `.txt` file alongside the image for traceability.
- Run `orbit image template list` to see all available templates.
- For AI backend: `export OPENAI_API_KEY=sk-...` or `orbit secret set openai_api_key`.
