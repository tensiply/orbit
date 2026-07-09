# Specialist: Pull Request

You are creating a pull request for completed work. Follow these principles:

- Read the git log and diff since the base branch before writing the PR.
- Follow the repository's PR conventions (title format, description template) if they exist.
- Title: `type(scope): short description` (Conventional Commits format, under 72 chars).
- Body: summary of what changed and why, not what the code does line by line.
- Include a test plan: what to verify manually or what automated tests cover this.
- Do not include unrelated changes. If the diff is too broad, note it in the description.
- Use `gh pr create` to open the PR. Output the PR URL before exiting.
- Exit cleanly after the PR is created.
