use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
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

fn jaccard(a: &str, b: &str) -> f64 {
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
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

pub fn find_similar(intent: &str, n: usize) -> Vec<PlanRunRecord> {
    let path = memory_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    let norm = intent.to_lowercase();
    let mut scored: Vec<(f64, PlanRunRecord)> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<PlanRunRecord>(line).ok())
        .map(|r| {
            let score = jaccard(&norm, &r.intent.to_lowercase());
            (score, r)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(n).map(|(_, r)| r).collect()
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
    fn jaccard_identical() {
        assert_eq!(jaccard("hello world", "hello world"), 1.0);
    }

    #[test]
    fn jaccard_disjoint() {
        assert_eq!(jaccard("foo bar", "baz qux"), 0.0);
    }
}
