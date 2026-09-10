---
description: "Query CI/CD pipeline status for the current scope and summarize results, errors, and next steps"
agent: implementation
---

Query and summarize the CI/CD pipeline status for the current scope.

Steps:

1. Run `orbit pipelines status` to fetch the current pipeline status.
   - If the command fails (not configured), run `orbit pipelines list` to show what is configured.
   - If no pipelines are configured, inform the user and suggest adding one to `orbit.json`.

2. Parse the output and summarize in a concise table:
   | Pipeline | Status | Branch | Triggered by | Last run |
   |---|---|---|---|---|

3. For any pipeline with `failure` or `error` status:
   - List the failing steps.
   - Show the error messages.
   - Suggest a diagnosis if the error message is clear (e.g., "test failure in step X", "lint error", etc.).

4. If all pipelines are green: confirm that the scope is in a healthy state.

5. If pipelines are running: note which ones are in-progress and suggest checking again in a moment.

Output format: concise markdown. No preamble. Table first, then details for failed pipelines.
