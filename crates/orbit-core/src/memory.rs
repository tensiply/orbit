use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn xdg_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".local/share"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

fn memory_path() -> PathBuf {
    xdg_data_dir().join("orbit/memory/plan_runs.jsonl")
}

/// BM25 relevance score for `query` against a single `doc` string.
///
/// Parameters use standard defaults: k1 = 1.5, b = 0.75.
/// `df` maps each query term to the number of documents that contain it.
/// `total_docs` and `avg_len` are corpus-wide statistics.
fn bm25_score(
    query_terms: &[&str],
    doc_terms: &[&str],
    df: &HashMap<&str, usize>,
    total_docs: usize,
    avg_len: f64,
) -> f64 {
    const K1: f64 = 1.5;
    const B: f64 = 0.75;
    let n = total_docs as f64;
    let doc_len = doc_terms.len() as f64;

    let mut score = 0.0f64;
    for &term in query_terms {
        let tf = doc_terms.iter().filter(|&&w| w == term).count() as f64;
        if tf == 0.0 {
            continue;
        }
        let df_t = *df.get(term).unwrap_or(&0) as f64;
        let idf = ((n - df_t + 0.5) / (df_t + 0.5) + 1.0).ln();
        let numerator = tf * (K1 + 1.0);
        let denominator = tf + K1 * (1.0 - B + B * doc_len / avg_len.max(1.0));
        score += idf * numerator / denominator;
    }
    score
}

// ── PlanRunRecord ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRunRecord {
    pub plan_id: String,
    pub intent: String,
    pub outcome: String,
    pub node_count: usize,
    pub replan_count: u8,
    pub duration_secs: u64,
    pub created_at: u64,
    pub scope_key: String,
    pub tags: Vec<String>,
    /// Estimated total USD cost across all nodes (0.0 when no token data available).
    #[serde(default)]
    pub cost_usd: f64,
    /// Total tokens consumed across all nodes (prompt + completion).
    #[serde(default)]
    pub total_tokens: u64,
    /// Template name if this plan was created via `orbit plan template run`.
    #[serde(default)]
    pub template_name: Option<String>,
}

// ── API ───────────────────────────────────────────────────────────────────────

pub fn append_plan_run(record: &PlanRunRecord) -> Result<()> {
    let path = memory_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(record)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn load_recent_runs(n: usize) -> Vec<PlanRunRecord> {
    let path = memory_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    let all: Vec<PlanRunRecord> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let skip = all.len().saturating_sub(n);
    all.into_iter().skip(skip).collect()
}

pub fn find_run(plan_id: &str) -> Option<PlanRunRecord> {
    let path = memory_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return None;
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<PlanRunRecord>(line).ok())
        .find(|r| r.plan_id == plan_id)
}

/// Return up to `n` records ranked by BM25 similarity to `intent`.
/// Falls back to an empty vec when no records score above zero.
pub fn find_similar(intent: &str, n: usize) -> Vec<PlanRunRecord> {
    let path = memory_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    let records: Vec<PlanRunRecord> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    if records.is_empty() {
        return vec![];
    }

    let norm_query = intent.to_lowercase();
    let query_terms: Vec<&str> = norm_query.split_whitespace().collect();

    // Tokenise all documents once.
    let norm_intents: Vec<String> = records.iter().map(|r| r.intent.to_lowercase()).collect();
    let doc_tokens: Vec<Vec<&str>> = norm_intents
        .iter()
        .map(|s| s.split_whitespace().collect())
        .collect();

    let total_docs = doc_tokens.len();
    let avg_len = doc_tokens.iter().map(|d| d.len() as f64).sum::<f64>()
        / total_docs as f64;

    // Build document-frequency table for query terms only.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for term in &query_terms {
        let count = doc_tokens.iter().filter(|doc| doc.contains(term)).count();
        if count > 0 {
            df.insert(term, count);
        }
    }

    let mut scored: Vec<(f64, &PlanRunRecord)> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let score = bm25_score(&query_terms, &doc_tokens[i], &df, total_docs, avg_len);
            (score, r)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(n).map(|(_, r)| r.clone()).collect()
}

// ── stats ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_runs: usize,
    pub completed: usize,
    pub failed: usize,
    pub avg_duration_secs: f64,
    pub avg_node_count: f64,
    pub avg_replan_count: f64,
    /// Top 5 scope_keys by run frequency.
    pub top_scopes: Vec<(String, usize)>,
    /// Total estimated USD cost across all recorded plan runs.
    pub total_cost_usd: f64,
    /// Top 5 scope_keys by accumulated cost (scope_key, cost_usd).
    pub cost_by_scope: Vec<(String, f64)>,
    /// Top 5 template names by accumulated cost (template_name, cost_usd).
    pub cost_by_template: Vec<(String, f64)>,
}

pub fn memory_stats() -> MemoryStats {
    let path = memory_path();
    let empty = MemoryStats {
        total_runs: 0, completed: 0, failed: 0,
        avg_duration_secs: 0.0, avg_node_count: 0.0, avg_replan_count: 0.0,
        top_scopes: vec![],
        total_cost_usd: 0.0,
        cost_by_scope: vec![],
        cost_by_template: vec![],
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return empty;
    };
    let records: Vec<PlanRunRecord> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let total = records.len();
    if total == 0 {
        return empty;
    }

    let completed = records.iter().filter(|r| r.outcome == "Completed").count();
    let failed = records.iter().filter(|r| r.outcome == "Failed").count();
    let avg_dur = records.iter().map(|r| r.duration_secs as f64).sum::<f64>() / total as f64;
    let avg_nodes = records.iter().map(|r| r.node_count as f64).sum::<f64>() / total as f64;
    let avg_replan = records.iter().map(|r| r.replan_count as f64).sum::<f64>() / total as f64;

    let total_cost_usd: f64 = records.iter().map(|r| r.cost_usd).sum();

    // Top scopes by run count
    let mut scope_counts: HashMap<&str, usize> = HashMap::new();
    for r in &records {
        *scope_counts.entry(r.scope_key.as_str()).or_default() += 1;
    }
    let mut top_scopes: Vec<(String, usize)> = scope_counts
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    top_scopes.sort_by(|a, b| b.1.cmp(&a.1));
    top_scopes.truncate(5);

    // Top scopes by cost
    let mut scope_cost: HashMap<&str, f64> = HashMap::new();
    for r in &records {
        *scope_cost.entry(r.scope_key.as_str()).or_default() += r.cost_usd;
    }
    let mut cost_by_scope: Vec<(String, f64)> = scope_cost
        .into_iter()
        .filter(|(_, c)| *c > 0.0)
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    cost_by_scope.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    cost_by_scope.truncate(5);

    // Top templates by cost
    let mut tmpl_cost: HashMap<&str, f64> = HashMap::new();
    for r in &records {
        if let Some(ref name) = r.template_name {
            *tmpl_cost.entry(name.as_str()).or_default() += r.cost_usd;
        }
    }
    let mut cost_by_template: Vec<(String, f64)> = tmpl_cost
        .into_iter()
        .filter(|(_, c)| *c > 0.0)
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    cost_by_template.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    cost_by_template.truncate(5);

    MemoryStats {
        total_runs: total,
        completed,
        failed,
        avg_duration_secs: avg_dur,
        avg_node_count: avg_nodes,
        avg_replan_count: avg_replan,
        top_scopes,
        total_cost_usd,
        cost_by_scope,
        cost_by_template,
    }
}

// ── clear ─────────────────────────────────────────────────────────────────────

/// Delete all memory records. Returns the number of records removed.
pub fn clear_memory() -> Result<usize> {
    let path = memory_path();
    if !path.exists() {
        return Ok(0);
    }
    let text = fs::read_to_string(&path)?;
    let count = text.lines().filter(|l| !l.trim().is_empty()).count();
    fs::remove_file(&path)?;
    Ok(count)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_record(intent: &str) -> PlanRunRecord {
        PlanRunRecord {
            plan_id: "plan_test".into(),
            intent: intent.into(),
            outcome: "Completed".into(),
            node_count: 1,
            replan_count: 0,
            duration_secs: 10,
            created_at: 0,
            scope_key: "AI/AIDEV/AI-ECOSYSTEM/orbit".into(),
            tags: vec!["code".into()],
            cost_usd: 0.0,
            total_tokens: 0,
            template_name: None,
        }
    }

    #[test]
    fn append_and_load_recent() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data").to_str().unwrap()); }

        for i in 0..5 {
            let mut r = make_record(&format!("intent {i}"));
            r.plan_id = format!("plan_{i:08x}");
            append_plan_run(&r).unwrap();
        }

        let recent = load_recent_runs(3);
        assert_eq!(recent.len(), 3);
        assert!(recent.last().unwrap().intent.contains("4"));
    }

    #[test]
    fn find_similar_returns_close_matches() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data2").to_str().unwrap()); }

        append_plan_run(&make_record("implement feature X with tests")).unwrap();
        append_plan_run(&make_record("fix bug in authentication")).unwrap();

        let results = find_similar("implement feature Y with unit tests", 2);
        assert!(!results.is_empty());
        assert!(results[0].intent.contains("implement"));
    }

    #[test]
    fn bm25_ranks_similar_higher() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data3").to_str().unwrap()); }

        append_plan_run(&make_record("implement authentication login flow")).unwrap();
        append_plan_run(&make_record("fix database connection timeout issue")).unwrap();
        append_plan_run(&make_record("review the pull request for authentication changes")).unwrap();

        let results = find_similar("implement user authentication", 3);
        assert!(!results.is_empty(), "should find similar records");
        // The most relevant result should contain "authentication"
        assert!(results[0].intent.contains("authentication") || results[0].intent.contains("implement"));
    }

    #[test]
    fn find_similar_empty_memory() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data4").to_str().unwrap()); }
        let results = find_similar("anything", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn memory_stats_basic() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_DATA_HOME", tmp.path().join("data5").to_str().unwrap()); }

        let mut r = make_record("test intent");
        r.outcome = "Completed".into();
        r.duration_secs = 30;
        append_plan_run(&r).unwrap();
        let mut r2 = make_record("another intent");
        r2.plan_id = "plan_00000002".into();
        r2.outcome = "Failed".into();
        r2.duration_secs = 10;
        append_plan_run(&r2).unwrap();

        let stats = memory_stats();
        assert_eq!(stats.total_runs, 2);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.avg_duration_secs, 20.0);
    }
}
