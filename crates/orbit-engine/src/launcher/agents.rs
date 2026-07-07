use anyhow::Result;
use orbit_core::{context::OrbitScope, engine::Engine};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

// ── public entry point ────────────────────────────────────────────────────────

/// Materialise agent, command, and skill files for the engine.
///
/// - OpenCode: writes `.opencode/agents|commands|skills` in the runtime dir,
///   then copies them into `work_dir/.opencode/`.
/// - Claude: writes `.claude/CLAUDE.md` + `.claude/agents/`, then symlinks
///   `work_dir/.claude` → runtime `.claude`.
/// - Gemini: no agent container needed (uses `includeDirectories`).
pub fn build(
    scope: &OrbitScope,
    engine: Engine,
    runtime_dir: &Path,
    instructions: &[PathBuf],
) -> Result<()> {
    match engine {
        Engine::Opencode => build_opencode(scope, runtime_dir),
        Engine::Claude => build_claude(scope, runtime_dir, instructions),
        Engine::Gemini => Ok(()),
    }
}

// ── path helpers ──────────────────────────────────────────────────────────────

fn shared_opencode_dir(scope: &OrbitScope) -> PathBuf {
    scope.global_ai_root.join("source-of-truth/orbit")
}

fn local_opencode_dir(scope: &OrbitScope) -> PathBuf {
    scope.ai_context_root.join("source-of-truth/orbit")
}

/// Ordered scope overlay paths for a given kind/name (tenant → project → repo).
fn overlay_paths(scope: &OrbitScope, kind: &str, name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if scope.global_mode || scope.tenant.is_empty() {
        return paths;
    }
    let base = scope
        .ai_context_root
        .join("tenants")
        .join(&scope.tenant)
        .join("source-of-truth/orbit")
        .join(kind)
        .join(format!("{name}.md"));
    paths.push(base);

    if !scope.project.is_empty() {
        paths.push(
            scope
                .ai_context_root
                .join("tenants")
                .join(&scope.tenant)
                .join("projects")
                .join(&scope.project)
                .join("source-of-truth/orbit")
                .join(kind)
                .join(format!("{name}.md")),
        );
        if !scope.repository.is_empty() {
            paths.push(
                scope
                    .ai_context_root
                    .join("tenants")
                    .join(&scope.tenant)
                    .join("projects")
                    .join(&scope.project)
                    .join("repositories")
                    .join(&scope.repository)
                    .join("source-of-truth/orbit")
                    .join(kind)
                    .join(format!("{name}.md")),
            );
        }
    }
    paths
}

// ── OpenCode ──────────────────────────────────────────────────────────────────

fn build_opencode(scope: &OrbitScope, runtime_dir: &Path) -> Result<()> {
    let runtime_opencode = runtime_dir.join(".opencode");
    let agents_dir = runtime_opencode.join("agents");
    let commands_dir = runtime_opencode.join("commands");
    let skills_dir = runtime_opencode.join("skills");

    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&commands_dir)?;
    fs::create_dir_all(&skills_dir)?;
    clear_dir(&agents_dir)?;

    let shared = shared_opencode_dir(scope);
    let local = local_opencode_dir(scope);
    let manifest = load_manifest(&shared, &local);

    // ── agents ────────────────────────────────────────────────────────────────
    for (name, meta) in manifest_section(&manifest, "agents") {
        let base_file = shared.join("agents").join(format!("{name}.md"));
        let merged = merge_layered_markdown(scope, "agents", &name, &shared, &local);

        match merged {
            Some(text) => {
                let base_text = read_opt(&base_file);
                let dest = agents_dir.join(format!("{name}.md"));
                if base_text.as_deref() == Some(&text) {
                    ensure_symlink(&dest, &base_file)?;
                } else {
                    fs::write(&dest, &text)?;
                }
            }
            None => {
                let description = meta_description(meta, &name);
                let body = format!("You are the {name} agent.");
                write_agent_file(&agents_dir, &name, &description, &body, None)?;
            }
        }
    }

    // ── commands ──────────────────────────────────────────────────────────────
    for (name, _) in manifest_section(&manifest, "commands") {
        if let Some(text) = merge_layered_markdown(scope, "commands", &name, &shared, &local) {
            fs::write(commands_dir.join(format!("{name}.md")), text)?;
        }
    }

    // ── skills symlink ────────────────────────────────────────────────────────
    let shared_skills = shared.join("skills");
    if shared_skills.is_dir() {
        ensure_symlink(&skills_dir, &shared_skills)?;
    }

    // ── materialise into work_dir/.opencode ───────────────────────────────────
    materialize_workdir(&runtime_opencode, &scope.work_dir.join(".opencode"))
}

// ── Claude ────────────────────────────────────────────────────────────────────

fn build_claude(scope: &OrbitScope, runtime_dir: &Path, instructions: &[PathBuf]) -> Result<()> {
    let runtime_claude = runtime_dir.join(".claude");
    let agents_dir = runtime_claude.join("agents");
    let skills_dir = runtime_claude.join("skills");

    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(runtime_claude.join("rules"))?;

    let shared = shared_opencode_dir(scope);
    let local = local_opencode_dir(scope);
    let manifest = load_manifest(&shared, &local);

    // ── CLAUDE.md ─────────────────────────────────────────────────────────────
    let mut parts = vec![render_catalog(&manifest).trim_end().to_string()];
    if !instructions.is_empty() {
        parts.push("\n# Context\n".to_string());
        for path in instructions {
            parts.push(format!("@{}", path.display()));
        }
    }
    fs::write(runtime_claude.join("CLAUDE.md"), parts.join("\n") + "\n")?;

    // ── agents ────────────────────────────────────────────────────────────────
    for (name, meta) in manifest_section(&manifest, "agents") {
        let source = shared.join("agents").join(format!("{name}.md"));
        let description = meta_description(meta, &name);
        let body = read_opt(&source).unwrap_or_else(|| format!("You are the {name} agent."));
        let extra = if name == "reviewer" {
            Some(vec![("tools", vec!["Read", "Grep", "Glob"])])
        } else {
            None
        };
        write_agent_file_with_list(&agents_dir, &name, &description, &body, extra)?;
    }

    // ── skills + README symlinks ──────────────────────────────────────────────
    let shared_skills = shared.join("skills");
    if shared_skills.is_dir() {
        ensure_symlink(&skills_dir, &shared_skills)?;
    }
    let shared_readme = shared.join("README.md");
    if shared_readme.is_file() {
        ensure_symlink(&runtime_claude.join("README.md"), &shared_readme)?;
    }

    // ── work_dir/.claude → runtime/.claude ───────────────────────────────────
    let work_claude = scope.work_dir.join(".claude");
    remove_any(&work_claude)?;
    symlink(&runtime_claude, &work_claude)?;

    Ok(())
}

// ── manifest helpers ──────────────────────────────────────────────────────────

fn load_manifest(shared: &Path, local: &Path) -> serde_json::Value {
    use crate::config::jsonc::load_file;
    let mut manifest = load_file(&shared.join("manifest.jsonc"));
    let local_manifest = load_file(&local.join("manifest.jsonc"));
    // Local entries fill in only what shared doesn't already define
    for section in ["agents", "commands"] {
        let shared_section = manifest.as_object_mut().and_then(|o| {
            o.entry(section)
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
        });
        if let (Some(shared_sec), Some(local_sec)) = (
            shared_section,
            local_manifest.get(section).and_then(|v| v.as_object()),
        ) {
            for (name, meta) in local_sec {
                shared_sec.entry(name).or_insert_with(|| meta.clone());
            }
        }
    }
    manifest
}

fn manifest_section<'a>(
    manifest: &'a serde_json::Value,
    section: &str,
) -> Vec<(String, &'a serde_json::Value)> {
    manifest
        .get(section)
        .and_then(|v| v.as_object())
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v)).collect())
        .unwrap_or_default()
}

fn meta_description(meta: &serde_json::Value, fallback: &str) -> String {
    meta.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback)
        .to_string()
}

// ── agent catalog (for CLAUDE.md) ────────────────────────────────────────────

fn render_catalog(manifest: &serde_json::Value) -> String {
    let mut lines = vec!["# AI Agent Catalog".to_string(), String::new()];
    let agents = manifest_section(manifest, "agents");
    if agents.is_empty() {
        lines.push("- No agents defined".to_string());
    } else {
        lines.push("## Shared agents".to_string());
        for (name, meta) in &agents {
            let source = meta.get("source").and_then(|v| v.as_str()).unwrap_or("");
            lines.push(format!("- `{name}`: `{source}`"));
            if let Some(overrides) = meta.get("overrides").and_then(|v| v.as_array()) {
                let list: Vec<String> = overrides
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| format!("`{s}`"))
                    .collect();
                if !list.is_empty() {
                    lines.push(format!("  - overrides: {}", list.join(", ")));
                }
            }
        }
    }
    let commands = manifest_section(manifest, "commands");
    if !commands.is_empty() {
        lines.push(String::new());
        lines.push("## Shared commands".to_string());
        for (name, meta) in &commands {
            let source = meta.get("source").and_then(|v| v.as_str()).unwrap_or("");
            lines.push(format!("- `{name}`: `{source}`"));
        }
    }
    lines.join("\n") + "\n"
}

// ── markdown merge ────────────────────────────────────────────────────────────

/// Try base files then apply scope overlays in order.
/// Returns `None` if no base file exists.
fn merge_layered_markdown(
    scope: &OrbitScope,
    kind: &str,
    name: &str,
    shared: &Path,
    local: &Path,
) -> Option<String> {
    // Find base text (shared takes priority over local)
    let base_candidates = [
        shared.join(kind).join(format!("{name}.md")),
        local.join(kind).join(format!("{name}.md")),
    ];
    let mut text = base_candidates.iter().find_map(|p| read_opt(p))?;

    // Apply scope overlays
    for overlay_path in overlay_paths(scope, kind, name) {
        if let Some(overlay) = read_opt(&overlay_path) {
            text = merge_preserve_base(&text, &overlay);
        }
    }
    Some(text)
}

/// Merge overlay into base: base frontmatter wins on conflicts, bodies concatenate.
fn merge_preserve_base(base: &str, overlay: &str) -> String {
    let (mut base_fm, base_body) = parse_frontmatter(base);
    let (overlay_fm, overlay_body) = parse_frontmatter(overlay);

    // Base keys take priority — only add overlay keys absent from base
    for (key, value) in overlay_fm {
        if !base_fm.iter().any(|(k, _)| k == &key) {
            base_fm.push((key, value));
        }
    }

    let body = {
        let base_trimmed = base_body.trim_end();
        let overlay_trimmed = overlay_body.trim();
        if overlay_trimmed.is_empty() {
            base_trimmed.to_string()
        } else if base_trimmed.is_empty() {
            overlay_trimmed.to_string()
        } else {
            format!("{base_trimmed}\n\n{overlay_trimmed}")
        }
    };

    if base_fm.is_empty() {
        format!("{body}\n")
    } else {
        format!("{}\n\n{body}\n", serialize_frontmatter(&base_fm))
    }
}

// ── frontmatter parser ────────────────────────────────────────────────────────

type Frontmatter = Vec<(String, String)>;

/// Parse the YAML frontmatter block from a markdown file.
/// Returns (key-value pairs in order, remaining body).
fn parse_frontmatter(text: &str) -> (Frontmatter, String) {
    if !text.starts_with("---\n") && !text.starts_with("---\r\n") {
        return (vec![], text.to_string());
    }
    let after = &text[4..];
    let end = after.find("\n---").or_else(|| after.find("\r\n---"));
    let Some(end_idx) = end else {
        return (vec![], text.to_string());
    };

    let fm_str = &after[..end_idx];
    let body_start = end_idx + "\n---".len();
    let body = after[body_start..]
        .trim_start_matches('\n')
        .trim_start_matches("\r\n");

    let pairs: Frontmatter = fm_str
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let colon = line.find(':')?;
            let key = line[..colon].trim().to_string();
            let raw_val = line[colon + 1..].trim();
            // Keep the raw value as-is so we can round-trip it
            Some((key, raw_val.to_string()))
        })
        .collect();

    (pairs, body.to_string())
}

/// Serialise frontmatter back into a `---\n...\n---` block.
fn serialize_frontmatter(fm: &Frontmatter) -> String {
    let mut lines = vec!["---".to_string()];
    for (key, value) in fm {
        lines.push(format!("{key}: {value}"));
    }
    lines.push("---".to_string());
    lines.join("\n")
}

// ── file writing ──────────────────────────────────────────────────────────────

fn write_agent_file(
    dir: &Path,
    name: &str,
    description: &str,
    body: &str,
    _extra: Option<()>,
) -> Result<()> {
    let content = format!(
        "---\nname: {name:?}\ndescription: {description:?}\n---\n\n{}\n",
        body.trim_end()
    );
    fs::write(dir.join(format!("{name}.md")), content)?;
    Ok(())
}

fn write_agent_file_with_list(
    dir: &Path,
    name: &str,
    description: &str,
    body: &str,
    extra_tools: Option<Vec<(&str, Vec<&str>)>>,
) -> Result<()> {
    let mut fm_lines = vec![
        "---".to_string(),
        format!("name: {name:?}"),
        format!("description: {description:?}"),
    ];
    if let Some(extras) = extra_tools {
        for (key, values) in extras {
            let list: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
            fm_lines.push(format!("{key}: [{}]", list.join(", ")));
        }
    }
    fm_lines.push("---".to_string());
    let content = format!("{}\n\n{}\n", fm_lines.join("\n"), body.trim_end());
    fs::write(dir.join(format!("{name}.md")), content)?;
    Ok(())
}

// ── filesystem utilities ──────────────────────────────────────────────────────

/// Create or update a symlink. No-op if it already points to the right target.
fn ensure_symlink(link: &Path, target: &Path) -> Result<()> {
    if let Ok(existing) = link.read_link() {
        if existing == target {
            return Ok(());
        }
        fs::remove_file(link)?;
    } else if link.exists() {
        // Not a symlink but something else — leave it alone
        return Ok(());
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    symlink(target, link)?;
    Ok(())
}

/// Remove a file, symlink, or directory at `path` (no-op if missing).
fn remove_any(path: &Path) -> Result<()> {
    if path.is_symlink() || path.is_file() {
        fs::remove_file(path)?;
    } else if path.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

/// Remove all direct children of a directory (leaves the directory itself).
fn clear_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        remove_any(&path)?;
    }
    Ok(())
}

/// Copy `src/.opencode/{agents,commands,skills}` into `dst/.opencode/`.
fn materialize_workdir(src: &Path, dst: &Path) -> Result<()> {
    remove_any(dst)?;
    fs::create_dir_all(dst)?;
    for name in ["agents", "commands", "skills"] {
        let source = src.join(name);
        let target = dst.join(name);
        if source.is_dir() {
            copy_dir_all(&source, &target)?;
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn read_opt(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_frontmatter() {
        let text = "---\nname: plan\ndescription: A planner agent\n---\n\nBody text here.";
        let (fm, body) = parse_frontmatter(text);
        assert_eq!(fm.len(), 2);
        assert_eq!(fm[0], ("name".into(), "plan".into()));
        assert_eq!(body, "Body text here.");
    }

    #[test]
    fn parses_no_frontmatter() {
        let text = "Just a body, no frontmatter.";
        let (fm, body) = parse_frontmatter(text);
        assert!(fm.is_empty());
        assert_eq!(body, text);
    }

    #[test]
    fn merge_preserve_base_base_wins_on_conflict() {
        let base = "---\nname: plan\ndescription: base desc\n---\n\nBase body.";
        let overlay =
            "---\nname: plan\ndescription: overlay desc\nextra: new key\n---\n\nOverlay body.";
        let merged = merge_preserve_base(base, overlay);
        // base description must win
        assert!(merged.contains("base desc"));
        assert!(!merged.contains("overlay desc"));
        // extra key from overlay should be added
        assert!(merged.contains("extra: new key"));
        // both bodies should be present
        assert!(merged.contains("Base body."));
        assert!(merged.contains("Overlay body."));
    }

    #[test]
    fn merge_preserve_base_no_overlay_body() {
        let base = "---\nmode: primary\n---\n\nBase body only.";
        let overlay = "---\nextra: value\n---\n";
        let merged = merge_preserve_base(base, overlay);
        assert!(merged.contains("Base body only."));
        assert!(!merged.contains("\n\n\n")); // no double blank line at end
    }

    #[test]
    fn render_catalog_lists_agents_and_commands() {
        let manifest = serde_json::json!({
            "agents": {
                "plan": { "source": "agents/plan.md", "overrides": ["tenants/*/..."] }
            },
            "commands": {
                "session-start": { "source": "commands/session-start.md" }
            }
        });
        let catalog = render_catalog(&manifest);
        assert!(catalog.contains("## Shared agents"));
        assert!(catalog.contains("`plan`"));
        assert!(catalog.contains("## Shared commands"));
        assert!(catalog.contains("`session-start`"));
    }

    #[test]
    fn build_claude_creates_claude_md_and_agents() {
        use orbit_core::context::OrbitScope;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        // Create minimal shared opencode dir with manifest and one agent
        let shared = tmp.path().join("AI/source-of-truth/orbit");
        let agents_dir = shared.join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(
            shared.join("manifest.jsonc"),
            r#"{ "agents": { "plan": { "source": "agents/plan.md" } }, "commands": {} }"#,
        )
        .unwrap();
        fs::write(
            agents_dir.join("plan.md"),
            "---\ndescription: The plan agent\n---\n\nYou plan things.",
        )
        .unwrap();

        let runtime_dir = tmp.path().join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();

        let work_dir = tmp.path().join("work");
        fs::create_dir_all(&work_dir).unwrap();

        let scope = OrbitScope {
            global_ai_root: tmp.path().join("AI"),
            ai_context_root: tmp.path().join("AI"),
            work_dir: work_dir.clone(),
            global_mode: true,
            ..Default::default()
        };

        build_claude(&scope, &runtime_dir, &[PathBuf::from("/some/README.md")]).unwrap();

        let claude_md = runtime_dir.join(".claude/CLAUDE.md");
        assert!(claude_md.exists(), "CLAUDE.md should be created");
        let content = fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("# AI Agent Catalog"));
        assert!(content.contains("@/some/README.md"));

        let plan_agent = runtime_dir.join(".claude/agents/plan.md");
        assert!(plan_agent.exists(), "plan.md agent should be created");

        // work_dir/.claude should be a symlink to runtime/.claude
        let symlink_path = work_dir.join(".claude");
        assert!(symlink_path.is_symlink(), ".claude should be a symlink");
    }
}
