---
description: Create a Conventional Commit for staged changes
agent: plan
---

Create a git commit for the currently staged changes following Conventional Commits.

Steps:
1. Run `git config user.email` to verify git identity.
2. Run `git diff --staged` to understand the full scope of staged changes.
3. If nothing is staged, check `git status` and ask which files to stage.
4. If the diff is too broad or mixes unrelated changes, propose splitting before committing.
5. Choose the correct type: `feat` | `fix` | `perf` | `refactor` | `docs` | `test` | `chore` | `ci` | `build` | `style`.
6. Identify the scope: the module, crate, package, or area primarily affected.
7. Write the description in imperative present tense, no capital letter, no period.
8. Add a body only if the WHY is not obvious from the code.
9. Add footers: `BREAKING CHANGE:`, `Closes #N`, `Co-authored-by:` as applicable.
10. If there is a breaking change: add `!` after type/scope AND a `BREAKING CHANGE:` footer.
11. Execute the commit using a HEREDOC to preserve formatting.

Rules:
- One commit = one coherent change. Never mix feat + fix.
- Never use `--no-verify` unless the user explicitly requests it.
- Never amend commits already pushed to shared branches.

Format: `<type>(<scope>): <description>`
