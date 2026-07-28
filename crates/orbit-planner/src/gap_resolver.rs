use anyhow::Result;
use orbit_core::{memory::PlanRunRecord, plan::PlanScope};
use serde::{Deserialize, Serialize};

use crate::backend::PlannerBackend;

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GapKind {
    /// Intent references a repo/scope that had multiple candidates — now resolved by scope_resolver.
    AmbiguousRepo,
    /// Intent is too vague to generate a concrete plan (e.g. "add a feature").
    MissingParameter,
    /// Intent references an entity the system can't map (e.g. "the config" with no context).
    UnknownReference,
    /// Intent implies cross-scope work but the target is unclear (e.g. "sync this with X").
    CrossScopeUnclear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub kind: GapKind,
    pub description: String,
    /// Whether the gap was resolved automatically from available context.
    pub auto_resolved: bool,
    /// The value that was resolved (if auto_resolved is true).
    pub resolved_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapResolution {
    /// Original intent enriched with any auto-resolved context.
    pub enriched_intent: String,
    pub gaps: Vec<Gap>,
    /// True if there are gaps that could not be auto-resolved and need user clarification.
    pub needs_user_input: bool,
    /// Concrete questions for the user (one per unresolved gap).
    pub user_questions: Vec<String>,
}

// ── LLM response parsing ──────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct GapResponseDraft {
    #[serde(default)]
    gaps: Vec<GapDraft>,
    #[serde(default)]
    enriched_intent: String,
}

#[derive(Deserialize)]
struct GapDraft {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    auto_resolved: bool,
    #[serde(default)]
    resolved_value: Option<String>,
    #[serde(default)]
    user_question: Option<String>,
}

fn parse_gap_kind(s: &str) -> GapKind {
    match s {
        "AmbiguousRepo" => GapKind::AmbiguousRepo,
        "MissingParameter" => GapKind::MissingParameter,
        "CrossScopeUnclear" => GapKind::CrossScopeUnclear,
        _ => GapKind::UnknownReference,
    }
}

fn extract_json(raw: &str) -> &str {
    if let Some(start) = raw.find("```json") {
        let after = &raw[start + 7..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim();
    }
    if let Some(start) = raw.find("```") {
        let after = &raw[start + 3..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim();
    }
    // find first '{' to handle leading prose
    if let Some(pos) = raw.find('{') {
        return &raw[pos..];
    }
    raw.trim()
}

fn build_gap_prompt(intent: &str, scope: &PlanScope, recent_runs: &[PlanRunRecord]) -> String {
    let mut prompt = format!(
        r#"You are an orbit planning assistant. Analyze the following user intent and identify any ambiguities or missing information BEFORE generating a plan.

User intent: "{intent}"

Current scope: workspace={ws}, tenant={tenant}, project={project}, repository={repo}

"#,
        ws = scope.workspace.as_deref().unwrap_or("(unknown)"),
        tenant = scope.tenant.as_deref().unwrap_or("(unknown)"),
        project = scope.project.as_deref().unwrap_or("(unknown)"),
        repo = scope.repository.as_deref().unwrap_or("(unknown)"),
    );

    if !recent_runs.is_empty() {
        prompt.push_str("Recent similar plan runs (for context):\n");
        for run in recent_runs.iter().rev().take(3) {
            prompt.push_str(&format!("  - \"{}\": {}\n", run.intent, run.outcome));
        }
        prompt.push('\n');
    }

    prompt.push_str(
        r#"Identify gaps in the intent. For each gap:
- auto_resolved=true if you can fill it from the scope/context above
- auto_resolved=false if you need the user to clarify

Gap kinds:
- MissingParameter: intent is too vague (e.g. "add a feature" without saying what)
- UnknownReference: references something unresolvable from context
- CrossScopeUnclear: implies work in another repo/workspace but target is ambiguous

IMPORTANT: Only list gaps that would prevent generating a useful plan.
If the intent is specific enough, return an empty gaps array.

Return ONLY a JSON object:
{
  "gaps": [
    {
      "kind": "MissingParameter",
      "description": "...",
      "auto_resolved": false,
      "resolved_value": null,
      "user_question": "Which specific feature do you want to add?"
    }
  ],
  "enriched_intent": "original intent + any auto-resolved context inline"
}
"#,
    );

    prompt
}

// ── public API ────────────────────────────────────────────────────────────────

/// Analyze the intent for ambiguities before generating a plan.
/// Returns an enriched intent and any gaps that need resolution.
///
/// On LLM failure (network, timeout, parse error), returns a no-gap resolution
/// so the planner can still proceed — gap resolution is best-effort.
pub fn resolve_gaps(
    intent: &str,
    scope: &PlanScope,
    recent_runs: &[PlanRunRecord],
    backend: &dyn PlannerBackend,
) -> Result<GapResolution> {
    let prompt = build_gap_prompt(intent, scope, recent_runs);

    let raw = match backend.call(&prompt) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("gap_resolver LLM call failed ({e}), skipping gap analysis");
            return Ok(no_gaps(intent));
        }
    };

    let json_str = extract_json(&raw);
    let draft: GapResponseDraft = match serde_json::from_str(json_str) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("gap_resolver response unparseable ({e}), skipping gap analysis");
            return Ok(no_gaps(intent));
        }
    };

    let mut gaps = Vec::new();
    let mut user_questions = Vec::new();
    let mut needs_user_input = false;

    for gd in draft.gaps {
        if !gd.auto_resolved {
            needs_user_input = true;
            if let Some(ref q) = gd.user_question {
                user_questions.push(q.clone());
            } else {
                user_questions.push(gd.description.clone());
            }
        }
        gaps.push(Gap {
            kind: parse_gap_kind(&gd.kind),
            description: gd.description,
            auto_resolved: gd.auto_resolved,
            resolved_value: gd.resolved_value,
        });
    }

    let enriched_intent = if draft.enriched_intent.is_empty() {
        intent.to_string()
    } else {
        draft.enriched_intent
    };

    Ok(GapResolution {
        enriched_intent,
        gaps,
        needs_user_input,
        user_questions,
    })
}

fn no_gaps(intent: &str) -> GapResolution {
    GapResolution {
        enriched_intent: intent.to_string(),
        gaps: vec![],
        needs_user_input: false,
        user_questions: vec![],
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    fn scope() -> PlanScope {
        PlanScope {
            workspace: Some("AI".into()),
            tenant: Some("AIDEV".into()),
            project: Some("AI-ECOSYSTEM".into()),
            repository: Some("orbit".into()),
        }
    }

    #[test]
    fn no_gaps_on_specific_intent() {
        let backend = MockBackend::new(
            r#"{"gaps": [], "enriched_intent": "fix the auth bug in orbit"}"#,
        );
        let result = resolve_gaps("fix the auth bug in orbit", &scope(), &[], &backend).unwrap();
        assert!(!result.needs_user_input);
        assert!(result.gaps.is_empty());
        assert_eq!(result.enriched_intent, "fix the auth bug in orbit");
    }

    #[test]
    fn unresolved_gap_needs_user_input() {
        let backend = MockBackend::new(r#"{
            "gaps": [{"kind":"MissingParameter","description":"Feature not specified","auto_resolved":false,"resolved_value":null,"user_question":"Which feature?"}],
            "enriched_intent": "add a feature"
        }"#);
        let result = resolve_gaps("add a feature", &scope(), &[], &backend).unwrap();
        assert!(result.needs_user_input);
        assert_eq!(result.user_questions, vec!["Which feature?"]);
    }

    #[test]
    fn auto_resolved_gap_no_user_input() {
        let backend = MockBackend::new(r#"{
            "gaps": [{"kind":"UnknownReference","description":"'the config' resolved to orbit/config.toml","auto_resolved":true,"resolved_value":"~/.config/orbit/config.toml","user_question":null}],
            "enriched_intent": "update ~/.config/orbit/config.toml planner section"
        }"#);
        let result = resolve_gaps("update the config", &scope(), &[], &backend).unwrap();
        assert!(!result.needs_user_input);
        assert_eq!(result.gaps.len(), 1);
        assert!(result.gaps[0].auto_resolved);
        assert!(result.enriched_intent.contains("config.toml"));
    }

    #[test]
    fn graceful_on_llm_failure() {
        use crate::backend::FailingBackend;
        let backend = FailingBackend::new("network error");
        let result = resolve_gaps("add a feature", &scope(), &[], &backend).unwrap();
        // Should succeed with no-gap fallback
        assert!(!result.needs_user_input);
        assert!(result.gaps.is_empty());
    }
}
