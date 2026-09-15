use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single level of the scope hierarchy, ordered from least to most specific.
///
/// Pure value type (no I/O): consumers map it to concrete paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeLevel {
    Global,
    Workspace,
    Tenant,
    Project,
    Repository,
}

impl ScopeLevel {
    /// Depth in the hierarchy — higher means more specific.
    pub fn depth(self) -> u8 {
        match self {
            ScopeLevel::Global => 0,
            ScopeLevel::Workspace => 1,
            ScopeLevel::Tenant => 2,
            ScopeLevel::Project => 3,
            ScopeLevel::Repository => 4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ScopeLevel::Global => "global",
            ScopeLevel::Workspace => "workspace",
            ScopeLevel::Tenant => "tenant",
            ScopeLevel::Project => "project",
            ScopeLevel::Repository => "repository",
        }
    }
}

impl std::fmt::Display for ScopeLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fully resolved scope for a given invocation.
///
/// Every field is derived from the CLI arguments by the engine resolver.
/// Once built, this struct is the single source of truth for all paths
/// used downstream (config loader, agent materializer, engine launcher).
#[derive(Debug, Clone, Default)]
pub struct OrbitScope {
    // ── roots ────────────────────────────────────────────────────────────────
    /// The workspace root directory (e.g. ~/AI).
    pub workspace_root: PathBuf,
    /// Where AI context layers live. Either `workspace_root/AI` (if that
    /// subdirectory contains a `tenants/` dir) or `workspace_root` itself.
    pub ai_context_root: PathBuf,
    /// Always `~/AI` — the global shared context.
    pub global_ai_root: PathBuf,

    // ── resolved names ───────────────────────────────────────────────────────
    /// Tenant name as it exists on disk (original casing, case-insensitively matched).
    pub tenant: String,
    /// Project name (empty if not specified).
    pub project: String,
    /// Repository name (empty if not specified).
    pub repository: String,

    // ── derived paths ────────────────────────────────────────────────────────
    /// `ai_context_root/tenants/<tenant>`
    pub tenant_dir: PathBuf,
    /// `workspace_root/<tenant>` — where actual code lives.
    pub code_root: PathBuf,
    /// The most-specific working directory: repository > project > code_root.
    pub work_dir: PathBuf,

    /// True when invoked with no arguments (global mode → opens ~/AI).
    pub global_mode: bool,
}
