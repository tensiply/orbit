"""Bump workspace version in Cargo.toml/Cargo.lock and promote CHANGELOG [Unreleased]."""
import os
import re

version = os.environ["VERSION"]
today = os.environ["TODAY"]

# ── CHANGELOG: [Unreleased] → [X.Y.Z] - date ────────────────────────────────
with open("CHANGELOG.md") as f:
    cl = f.read()

cl = cl.replace(
    "## [Unreleased]",
    f"## [Unreleased]\n\n## [{version}] - {today}",
    1,
)

with open("CHANGELOG.md", "w") as f:
    f.write(cl)

# ── Cargo.toml: bump version in [workspace.package] ─────────────────────────
with open("Cargo.toml") as f:
    lines = f.readlines()

in_ws_pkg = False
result = []
for line in lines:
    stripped = line.strip()
    if stripped == "[workspace.package]":
        in_ws_pkg = True
    elif stripped.startswith("[") and stripped != "[workspace.package]":
        in_ws_pkg = False
    if in_ws_pkg and re.match(r"^version\s*=", stripped):
        line = f'version = "{version}"\n'
    result.append(line)

with open("Cargo.toml", "w") as f:
    f.writelines(result)

# ── Cargo.lock: update workspace package versions ───────────────────────────
ws_packages = [
    "orbit", "orbit-cli", "orbit-tui", "orbit-core",
    "orbit-client", "orbit-engine", "orbit-daemon", "orbit-dev",
]

with open("Cargo.lock") as f:
    lock = f.read()

for pkg in ws_packages:
    lock = re.sub(
        rf'(name = "{re.escape(pkg)}"\nversion = ")[^"]+(")',
        rf"\g<1>{version}\g<2>",
        lock,
    )

with open("Cargo.lock", "w") as f:
    f.write(lock)

print(f"Bumped to v{version} ({today})")
