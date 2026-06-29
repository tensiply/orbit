"""Update CHANGELOG footer links, README latest line, and write release_notes.md."""
import os
import re

version = os.environ["VERSION"]
tag = os.environ["TAG"]
repo = os.environ["REPO"]

# ── CHANGELOG: update footer links ──────────────────────────────────────────
with open("CHANGELOG.md") as f:
    changelog = f.read()

changelog = re.sub(
    r"\[Unreleased\]: .*",
    f"[Unreleased]: https://github.com/{repo}/compare/{tag}...HEAD",
    changelog,
)

version_link = f"[{version}]: https://github.com/{repo}/releases/tag/{tag}"
if version_link not in changelog:
    changelog = re.sub(
        r"(\[Unreleased\]: [^\n]+\n)",
        rf"\g<1>{version_link}\n",
        changelog,
    )

with open("CHANGELOG.md", "w") as f:
    f.write(changelog)

# ── README: update "Latest" line ────────────────────────────────────────────
with open("README.md") as f:
    readme = f.read()

new_latest = (
    f"**Latest:** [{tag}](https://github.com/{repo}/releases/tag/{tag})"
    f" · [Changelog](CHANGELOG.md)"
    f" · [Wiki](https://github.com/{repo}/wiki)\n"
)

if "**Latest:**" in readme:
    readme = re.sub(r"\*\*Latest:\*\*[^\n]+\n", new_latest, readme)
else:
    readme = re.sub(r"(\n---\n)", f"\n{new_latest}\\1", readme, count=1)

with open("README.md", "w") as f:
    f.write(readme)

# ── release_notes.md: extract section for wiki ──────────────────────────────
pattern = rf"(?m)^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|\Z)"
match = re.search(pattern, changelog, re.DOTALL)
body = match.group(1).strip() if match else "_No notes found._"

with open("release_notes.md", "w") as f:
    f.write(f"# Release {tag}\n\n")
    f.write(body)
    f.write("\n")

print(f"Docs updated for {tag}")
