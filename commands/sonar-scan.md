---
description: SonarCloud analysis report — quality gate, issues, hotspots, coverage, and security
agent: plan
---

Run a SonarCloud analysis report for the current project using the SonarCloud MCP.

If the SonarCloud MCP (`sonarqube`) is not available in this session, say so and stop — run `orbit plugins enable sonarcloud` and re-launch.

## Step 1 — Identify the project key

Check in this order:
1. `SONARQUBE_PROJECT_KEY` environment variable
2. `sonar-project.properties` in the current directory (`sonar.projectKey=...`)
3. `.sonarcloud.properties` in the current directory
4. If not found: ask the user for the project key before continuing

## Step 2 — Fetch data in parallel

Run all of the following at the same time:

a. **Quality gate** — `get_project_quality_gate_status` for the project key
b. **Open issues** — `search_sonar_issues_in_projects` for the project key, current branch, ordered by severity descending (BLOCKER → CRITICAL → MAJOR → MINOR). Limit 50.
c. **Security hotspots** — `search_security_hotspots` for the project key, status=TO_REVIEW
d. **Key metrics** — `get_component_measures` with metrics: `coverage`, `duplicated_lines_density`, `ncloc`, `reliability_rating`, `security_rating`, `sqale_rating`, `bugs`, `vulnerabilities`, `code_smells`

If `$ARGUMENTS` contains `coverage`: also call `search_files_by_coverage` and collect files with < 50% line coverage.

If `$ARGUMENTS` contains `rule:<id>`: call `show_rule` for that rule id and include the explanation in the report.

## Step 3 — Report

Structure the output in this order:

### Quality Gate
- PASSED ✓ or FAILED ✗
- If failed: list which conditions failed and by how much

### Metrics Summary
| Metric | Value |
|---|---|
| Lines of code | ncloc |
| Coverage | coverage % |
| Duplication | duplicated_lines_density % |
| Reliability | reliability_rating (A–E) |
| Security | security_rating (A–E) |
| Maintainability | sqale_rating (A–E) |
| Bugs | bugs |
| Vulnerabilities | vulnerabilities |
| Code smells | code_smells |

### Security Hotspots
List each hotspot: category, file, line, message.
If none: "No open security hotspots ✓"

### Issues by Severity
Group by: BLOCKER → CRITICAL → MAJOR → MINOR → INFO.
For BLOCKER and CRITICAL: show file, line, message, rule key.
For MAJOR and below: show count only unless `$ARGUMENTS` contains `verbose`.

### Coverage by File (only if requested)
Files with < 50% line coverage, sorted ascending. Show filename and coverage %.

## Step 4 — Action list

End with a prioritized list of what to address first:
1. Any BLOCKER issues or failed quality gate conditions — these block release
2. Security hotspots marked TO_REVIEW — require manual triage
3. Vulnerabilities rated CRITICAL
4. Coverage gaps in core modules (if coverage data was fetched)

Keep the action list to 5 items max. Be specific: file + line when available.
