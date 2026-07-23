---
description: Open a pull request with a conventional title and structured description
agent: plan
---

Open a pull request for the current branch following the git-pr workflow.

Steps:
1. Run `git status` to verify there are no uncommitted changes.
2. Run `git log <base>..HEAD --oneline` to review all commits in scope (use `main` as default base if not specified).
3. Run `git diff <base>...HEAD` to understand the full set of changes.
4. Check if the branch is up to date: `git fetch && git log HEAD..origin/<base> --oneline`.
5. Choose the PR title: `<type>(<scope>): <short description>` — max 72 chars, same rules as a commit.
   - If the PR contains multiple types, use the highest impact: `feat` > `fix` > `refactor`.
   - Add `!` if there is a breaking change.
6. Draft the PR body with:
   - `## Summary` — 1–3 bullet points of what changed and why
   - `## Changes` — key implementation details if non-obvious
   - `## Test Plan` — checklist of what to verify
7. Verify the pre-open checklist:
   - Branch is up to date with base
   - Tests pass locally
   - No debug files or secrets in the diff
   - PR is focused (no mixed breaking + non-urgent changes)
8. Run `gh pr create` with the title and body using a HEREDOC.

Rules:
- Never push directly to `main`/`master` without a PR unless the user explicitly authorizes it.
- If the PR cannot be described in one sentence, consider splitting it.
