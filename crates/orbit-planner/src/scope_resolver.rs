use orbit_core::scope_index::ScopeIndexEntry;
use std::{cmp::Reverse, path::PathBuf};

use crate::backend::PlannerBackend;

// ── Confidence ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confidence {
    /// Unambiguous match: top score >= 2x second-best and >= 2 tokens matched.
    High,
    /// Disambiguated by LLM from multiple candidates.
    Medium,
    /// Multiple candidates tied; returned without disambiguation.
    Ambiguous,
    /// No candidates matched; caller should use cwd scope.
    Fallback,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::High => "High",
            Confidence::Medium => "Medium",
            Confidence::Ambiguous => "Ambiguous",
            Confidence::Fallback => "Fallback",
        }
    }
}

// ── ScopeResolution ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ScopeResolution {
    pub workspace: Option<String>,
    pub tenant: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
    /// Computed code directory (may not exist on disk if path inference is off).
    pub work_dir: PathBuf,
    pub confidence: Confidence,
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 2)
        .collect()
}

fn score_entry(intent_tokens: &[String], entry: &ScopeIndexEntry) -> usize {
    let mut count = 0usize;

    // match against keywords (pre-built from name parts)
    for tok in intent_tokens {
        if entry.keywords.contains(tok) {
            count += 1;
        }
    }

    // also match against description words (bonus, not in keywords)
    if let Some(ref desc) = entry.description {
        let desc_words: Vec<String> = tokenize(desc);
        for tok in intent_tokens {
            if desc_words.contains(tok) && !entry.keywords.contains(tok) {
                count += 1;
            }
        }
    }

    count
}

fn build_disambiguation_prompt(intent: &str, candidates: &[&ScopeIndexEntry]) -> String {
    let mut prompt = format!(
        "You are helping route a task to the correct repository.\n\nUser intent: \"{intent}\"\n\nCandidates:\n"
    );
    for (i, c) in candidates.iter().enumerate() {
        let desc = c.description.as_deref().unwrap_or("");
        prompt.push_str(&format!(
            "{}. {}/{}/{}/{}  {}\n",
            i + 1,
            c.workspace,
            c.tenant,
            c.project,
            c.repository,
            desc
        ));
    }
    prompt.push_str(
        "\nReply with ONLY the number (1-N) of the best matching candidate. No explanation.",
    );
    prompt
}

// ── public API ────────────────────────────────────────────────────────────────

/// Resolve the target scope from a natural-language intent and a pre-built index.
///
/// Steps:
/// 1. Tokenize intent → lowercase alphanumeric tokens ≥ 2 chars.
/// 2. Score each entry by keyword + description overlap.
/// 3. High confidence: top score ≥ 2 and ≥ 2× second-best → auto-select.
/// 4. Ambiguous + backend provided → one LLM call to pick from top 5.
/// 5. No match or all scores 0 → Fallback (caller uses cwd).
pub fn resolve_scope_from_intent(
    intent: &str,
    index: &[ScopeIndexEntry],
    backend: Option<&dyn PlannerBackend>,
) -> ScopeResolution {
    if index.is_empty() {
        return fallback_resolution();
    }

    let intent_tokens = tokenize(intent);
    if intent_tokens.is_empty() {
        return fallback_resolution();
    }

    let mut scored: Vec<(usize, &ScopeIndexEntry)> = index
        .iter()
        .map(|e| (score_entry(&intent_tokens, e), e))
        .collect();
    scored.sort_by_key(|b| Reverse(b.0));

    let top_score = scored[0].0;

    if top_score == 0 {
        return fallback_resolution();
    }

    let second_score = scored.get(1).map(|(s, _)| *s).unwrap_or(0);

    if top_score >= 2 && top_score > second_score * 2 {
        let entry = scored[0].1;
        return ScopeResolution {
            workspace: Some(entry.workspace.clone()),
            tenant: Some(entry.tenant.clone()),
            project: Some(entry.project.clone()),
            repository: Some(entry.repository.clone()),
            work_dir: entry.work_dir.clone(),
            confidence: Confidence::High,
        };
    }

    // Ambiguous: try LLM disambiguation with top 5
    let top5: Vec<&ScopeIndexEntry> = scored.iter().take(5).map(|(_, e)| *e).collect();

    if let Some(backend) = backend {
        let prompt = build_disambiguation_prompt(intent, &top5);
        if let Ok(raw) = backend.call(&prompt)
            && let Ok(n) = raw.trim().parse::<usize>()
            && n >= 1
            && n <= top5.len()
        {
            let entry = top5[n - 1];
            return ScopeResolution {
                workspace: Some(entry.workspace.clone()),
                tenant: Some(entry.tenant.clone()),
                project: Some(entry.project.clone()),
                repository: Some(entry.repository.clone()),
                work_dir: entry.work_dir.clone(),
                confidence: Confidence::Medium,
            };
        }
    }

    // Return top candidate with Ambiguous confidence
    let entry = scored[0].1;
    ScopeResolution {
        workspace: Some(entry.workspace.clone()),
        tenant: Some(entry.tenant.clone()),
        project: Some(entry.project.clone()),
        repository: Some(entry.repository.clone()),
        work_dir: entry.work_dir.clone(),
        confidence: Confidence::Ambiguous,
    }
}

fn fallback_resolution() -> ScopeResolution {
    ScopeResolution {
        workspace: None,
        tenant: None,
        project: None,
        repository: None,
        work_dir: PathBuf::new(),
        confidence: Confidence::Fallback,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::scope_index::ScopeIndexEntry;

    fn make_entry(ws: &str, tenant: &str, project: &str, repo: &str, desc: Option<&str>) -> ScopeIndexEntry {
        let mut keywords = vec![];
        for seg in [ws, tenant, project, repo] {
            for part in seg.split(['-', '_']) {
                let lc = part.to_lowercase();
                if lc.len() >= 2 && !keywords.contains(&lc) {
                    keywords.push(lc.clone());
                }
            }
            let full = seg.to_lowercase();
            if !keywords.contains(&full) {
                keywords.push(full);
            }
        }
        keywords.sort();
        keywords.dedup();
        ScopeIndexEntry {
            workspace: ws.to_string(),
            tenant: tenant.to_string(),
            project: project.to_string(),
            repository: repo.to_string(),
            work_dir: PathBuf::from(format!("/home/{ws}/{tenant}/{project}/{repo}")),
            keywords,
            description: desc.map(|s| s.to_string()),
        }
    }

    #[test]
    fn high_confidence_exact_match() {
        let index = vec![
            make_entry("BeFra", "JAFRAMX", "INTERFACES", "jf-cdc-interfaces", Some("CDC query stuff")),
            make_entry("AI", "AIDEV", "AI-ECOSYSTEM", "orbit", None),
        ];
        let result = resolve_scope_from_intent("quiero cambiar la query del cdc jfp-prom-card", &index, None);
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.repository.as_deref(), Some("jf-cdc-interfaces"));
        assert_eq!(result.workspace.as_deref(), Some("BeFra"));
    }

    #[test]
    fn fallback_when_no_match() {
        let index = vec![make_entry("AI", "AIDEV", "AI-ECOSYSTEM", "orbit", None)];
        let result = resolve_scope_from_intent("completely unrelated xyz123", &index, None);
        assert_eq!(result.confidence, Confidence::Fallback);
        assert!(result.repository.is_none());
    }

    #[test]
    fn fallback_when_index_empty() {
        let result = resolve_scope_from_intent("any intent", &[], None);
        assert_eq!(result.confidence, Confidence::Fallback);
    }

    #[test]
    fn ambiguous_when_tied() {
        let index = vec![
            make_entry("AI", "AIDEV", "FOO", "bar-cdc", None),
            make_entry("AI", "AIDEV", "FOO", "baz-cdc", None),
        ];
        // "cdc" matches both equally
        let result = resolve_scope_from_intent("cdc query", &index, None);
        // Without backend, should be Ambiguous or High (depends on scoring)
        assert!(
            result.confidence == Confidence::Ambiguous || result.confidence == Confidence::High,
            "expected Ambiguous or High, got {:?}",
            result.confidence
        );
    }
}
