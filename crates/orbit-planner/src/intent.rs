// ── IntentClassification ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Complexity {
    Single,
    Multi,
    Complex,
}

#[derive(Debug, Clone)]
pub struct IntentClassification {
    pub normalized: String,
    pub complexity: Complexity,
    pub tags: Vec<String>,
}

const KNOWN_TAGS: &[&str] = &[
    "test",
    "fix",
    "review",
    "pr",
    "deploy",
    "build",
    "refactor",
    "implement",
    "add",
    "remove",
];

pub fn classify(intent: &str) -> IntentClassification {
    let normalized = intent.trim().to_lowercase();
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let complexity = match words.len() {
        0..=9 => Complexity::Single,
        10..=25 => Complexity::Multi,
        _ => Complexity::Complex,
    };
    let tags = KNOWN_TAGS
        .iter()
        .filter(|&&kw| normalized.contains(kw))
        .map(|&kw| kw.to_string())
        .collect();
    IntentClassification {
        normalized,
        complexity,
        tags,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_simple() {
        let c = classify("fix the bug");
        assert!(matches!(c.complexity, Complexity::Single));
        assert!(c.tags.contains(&"fix".to_string()));
    }

    #[test]
    fn classify_tags_pr() {
        let c = classify("open a pr with review changes");
        assert!(c.tags.contains(&"pr".to_string()));
        assert!(c.tags.contains(&"review".to_string()));
    }

    #[test]
    fn classify_complex() {
        let intent = "implement the full authentication flow with unit tests integration tests code review \
                      security audit fix any issues found and then deploy to staging environment with smoke tests";
        let c = classify(intent);
        assert!(matches!(c.complexity, Complexity::Complex));
    }
}
