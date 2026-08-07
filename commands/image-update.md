---
name: image-update
description: Regenerate an existing orbit image with new or revised content
---

Regenerate an existing image by running `orbit image create` with the same title and updated content.

`orbit image create` is idempotent: same title → same output file → backup existing to `.bk` → regenerate → update index entry in-place (same `IMG-XXXXXX` ID).

## Steps

1. Identify the image to update (ask the user for the title or run `orbit image list` to find it).
2. Ask what content changes the user wants.
3. Write the revised text content.
4. Run `orbit image create` with the same title and the new content:

```bash
orbit image create --title "<same title>" --content "<new content>"
```

5. Confirm the output path and show the backup location to the user.

## Options available

```
orbit image create --title "<title>" --content "<text>"
                   [--backend template|ai]
                   [--type png|jpeg|webp]
                   [--template <name>]
                   [--var KEY=VALUE ...]
                   [--width N] [--height N]
                   [--force]   # skip backup, overwrite directly
```

## Notes

- The existing `.png`/`.jpg`/`.webp` is backed up as `{filename}.ext.bk` before overwriting.
- The existing `.txt` source is overwritten with the new content.
- The `IMG-XXXXXX` ID is preserved — no new index entry is created.
- To see existing images: `orbit image list`
- To change backend or template, pass the new flags — they override the original settings.
