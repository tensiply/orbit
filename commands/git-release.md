---
description: Prepare and publish a release with notes generated from commits
agent: plan
---

Prepare and publish a release for the current repository. $ARGUMENTS can be a version (e.g. `v0.12.0`) or left empty to auto-calculate.

Steps:
1. Run `git tag --sort=-v:refname | head -5` to identify the previous tag.
2. Run `git log <prev-tag>..HEAD --oneline` to list all commits since the last release.
3. Calculate the next version using SemVer (unless a version was passed as argument):
   - Any `BREAKING CHANGE` → MAJOR bump
   - Any `feat` → MINOR bump
   - `fix`, `perf`, or others only → PATCH bump
4. Group commits by type for the release notes:
   - Include: `feat`, `fix`, `perf`, `docs` (if significant), any `BREAKING CHANGE`
   - Exclude: `chore`, `style`, `test`, `ci`, `build` (unless a notable deps bump), merge commits, bot commits
5. Draft the release notes with sections: `## Breaking Changes`, `## Features`, `## Bug Fixes`, `## Performance`. Omit empty sections.
6. If `CHANGELOG.md` exists, prepend the new release section to it.
7. Create and push the annotated tag:
   ```
   git tag -a v<VERSION> -m "chore: release v<VERSION>"
   git push origin v<VERSION>
   ```
8. Publish the release with `gh release create v<VERSION> --title "v<VERSION>" --notes "..."`.

Rules:
- Tag always on `main` or a release branch, never on feature branches.
- Never reuse tags — use `v<VERSION>-rc.N` if a release fails.
- The bump commit (if any) message: `chore: release v<VERSION>`.
