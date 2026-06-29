"""Generate a release preview summary for the GitHub Actions Step Summary."""
import os
import re
from datetime import date

version = os.environ["VERSION"]
today = date.today().isoformat()

with open("Cargo.toml") as f:
    cargo = f.read()

current_ver = re.search(
    r"(?ms)^\[workspace\.package\].*?^version = \"([^\"]+)\"", cargo
).group(1)

with open("CHANGELOG.md") as f:
    cl = f.read()

match = re.search(r"(?ms)^## \[Unreleased\]\n(.*?)(?=^## \[|\Z)", cl)
notes = match.group(1).strip() if match else "_(empty)_"

summary = f"""## Release preview: v{current_ver} → v{version}

| | |
|---|---|
| Current version | `v{current_ver}` |
| **New version** | **`v{version}`** |
| Release date | `{today}` |

### Release notes

{notes}

---

### Files that will change

| File | Change |
|------|--------|
| `CHANGELOG.md` | `[Unreleased]` → `[{version}] - {today}` |
| `Cargo.toml` | `version = "{current_ver}"` → `version = "{version}"` |
| `Cargo.lock` | workspace package versions bumped |

> If this looks correct, approve the pending deployment below to proceed.
"""

with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as f:
    f.write(summary)

print(f"Summary ready — v{current_ver} → v{version} ({today})")
