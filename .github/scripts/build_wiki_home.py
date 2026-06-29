"""Rebuild wiki/Home.md index from CHANGELOG release sections."""
import os
import re

repo = os.environ["REPO"]

with open("CHANGELOG.md") as f:
    changelog = f.read()

releases = re.findall(
    r"^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})",
    changelog,
    re.MULTILINE,
)

rows = "\n".join(
    f"| [v{ver}](Release-v{ver}) | {date} |"
    for ver, date in releases
)

home = f"""# orbit

A CLI launcher for AI coding assistants (opencode, Gemini CLI, Claude Code) with
multi-tenant workspace management, session tracking, and MCP server configuration.

→ [Repository](https://github.com/{repo}) \xb7 [Releases](https://github.com/{repo}/releases) \xb7 [Changelog](https://github.com/{repo}/blob/main/CHANGELOG.md)

## Releases

| Version | Date |
|---------|------|
{rows}
"""

with open("wiki/Home.md", "w") as f:
    f.write(home)

print("wiki/Home.md rebuilt")
