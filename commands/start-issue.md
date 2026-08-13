---
description: Prepare and start work on a Linear issue — read scope, close gaps, create branch, mark In Progress
agent: plan
---

Prepare and start work on issue $ARGUMENTS.

## Steps

### 1. Load the issue

Use Linear MCP (`get_issue`) to retrieve the full issue: title, description, owner, acceptance criteria, verify steps, dependencies, blockedBy, milestone, current state.

If $ARGUMENTS is not a valid Linear ID, ask the user for it before continuing.

### 2. Verify it can be started

Check that all blockers are resolved:
- List the `blockedBy` relations of the issue
- For each, verify status via Linear MCP
- If any blocker is not Done: **stop and list the active blockers** — do not continue

### 3. Load relevant context in parallel

Based on the issue's **Owner** field, load:

| Owner | Files to load |
|---|---|
| CORE | shared product docs, v1-scope.md |
| IAM | shared product docs, auth/identity docs if they exist |
| INFRA | dev-environment docs, CI/Docker docs |

Always load:
- `strategy/milestone-plan.md` — to understand the milestone and current phase
- `knowledge-index.md` of the tenant — to detect existing relevant documentation

### 4. Detect gaps in the issue

Evaluate whether the issue has everything needed to implement without open questions:

**Blocking gaps** (prevent starting):
- Ambiguous or non-testable acceptance criteria
- Verify does not describe concrete observable behavior
- Implicit technical dependency not documented
- Stack or technology not defined when necessary

**Non-blocking gaps** (can be resolved during implementation):
- Non-critical implementation details
- Optional optimizations
- Internal service documentation

If **blocking gaps** exist: propose exact text to close them and ask for confirmation before updating the issue in Linear.

### 5. Prepare implementation context

Generate an executive summary to start:

```
## Implementation context: [ID] — [title]

### What this issue builds
[2-3 sentences — what it constructs, not what the issue says]

### Relevant stack
[language, framework, services involved]

### Starting files / repos
[where to create or modify code]

### Acceptance criteria reference
[the ACs that define when it's done]

### Verify steps
[exact steps to verify it works]

### Pending decisions before writing code
[if any — if none, say "none"]
```

### 6. Create git branch (if applicable)

If the issue has a `gitBranchName` in Linear, propose:
```bash
git checkout -b <gitBranchName>
```
Ask for confirmation before executing. If the current directory has no valid git repo, note it and skip this step.

### 7. Mark In Progress in Linear

Once confirmed the user wants to start, update the issue:
- State → In Progress
- Ask for confirmation before updating

### 8. Final output

Show a summary ready to copy at the start of work:
- Issue ID and title
- Branch created (if applicable)
- The 3 concrete first steps to start implementation
- Link to the issue in Linear
