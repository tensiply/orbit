---
description: Analyze an idea or feature and decide whether to integrate it into the current plan or defer to future backlog
agent: plan
---

Analyze the following idea or feature and decide if it belongs in the current plan or should be deferred: $ARGUMENTS

## Steps

### 1. Understand the idea

If $ARGUMENTS is vague or incomplete, ask up to 3 clarifying questions before continuing:
- What problem does it solve? For whom?
- Is it a new capability, an improvement to something existing, or a scope correction?
- Does it have known technical dependencies?

Continue only when the idea is sufficiently clear to analyze.

### 2. Load context in parallel

a. Current scope's `strategy/v1-scope.md` — what is in-scope, what is deferred
b. Current scope's `strategy/milestone-plan.md` — active milestones and their objectives
c. Current scope's shared product principles
d. Linear MCP: list issues of the active milestone to understand what is already covered

### 3. Evaluate across these dimensions

**Value:**
- Does it solve a real problem for the target customer profile?
- Is it a differentiator or commodity?
- How many existing issues does it enable or improve?

**Fit for current version:**
- Is it in scope according to `v1-scope.md`? Must / should / defer?
- Does it fit into the active milestone phase without delaying exit criteria?
- Does it add unnecessary complexity to something already defined?

**Implementation cost:**
- How many new dependencies does it introduce?
- Does it require changes to already-defined issues?
- Does it block anything already in progress?

**Risk of omission:**
- What breaks or stays incomplete without it in v1?
- Can it be done in v1.1 without impact?

### 4. Verdict

**INTEGRATE INTO CURRENT PLAN** — if it covers a real gap, fits a phase without displacing exit criteria, and doesn't break the dependency graph.

**DEFER TO FUTURE BACKLOG** — if valuable but not blocking, or introduces complexity that compromises the timeline.

**REJECT / DO NOT DOCUMENT** — if outside the target customer profile, already covered, or contradicts product principles.

### 5. Action

**If INTEGRATE:** propose the complete issue and ask for confirmation before creating in Linear.

**If DEFER:** create an issue in M3 or a future milestone with context on why it was deferred. Confirm before creating.

**If REJECT:** explain the reason and do not create any issue.

### Output

```
## Idea: [proposed title]

### Evaluation
- Value: [high / medium / low] — [reason]
- Fit v1: [yes / partial / no] — [reason]
- Cost: [low / medium / high] — [reason]
- Risk of omission: [high / medium / low] — [reason]

### Verdict: INTEGRATE / DEFER / REJECT
[Justification in 2-3 sentences]

### Proposed action
[Issue draft or reason for not creating]
```
