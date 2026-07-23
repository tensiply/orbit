---
description: Initialize governance structure for a new scope (workspace / tenant / project / repo)
---

Initialize the governance structure for $ARGUMENTS following the init-scope workflow.

Follow the instructions in `~/AI/source-of-truth/workflows/init-scope.md` exactly.

Steps:
1. Parse $ARGUMENTS to identify WORKSPACE, TENANT, and optionally PROJECT and REPO.
   If arguments are missing, ask the user before proceeding.
2. Run `orbit launch <WORKSPACE> <TENANT> [PROJECT] [REPO] --dry-run` to confirm real paths.
3. Execute each step from the workflow (1–4) for the applicable levels only.
   Do not overwrite files that already exist — check first.
4. After creating files, run the dry-run again to confirm orbit recognizes the new layers.
5. Report which files were created and which already existed (skipped).
