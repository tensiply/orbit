use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

// ── paths ─────────────────────────────────────────────────────────────────────

fn xdg_config_dir() -> PathBuf {
    if let Ok(orbit_home) = std::env::var("ORBIT_CONFIG_HOME") {
        return PathBuf::from(orbit_home);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        directories::BaseDirs::new()
            .map(|b| b.home_dir().join(".config"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

pub fn templates_dir() -> PathBuf {
    xdg_config_dir().join("orbit/plans")
}

pub fn template_path(name: &str) -> PathBuf {
    templates_dir().join(format!("{name}.toml"))
}

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTemplate {
    /// Derived from filename; not stored in TOML.
    #[serde(skip)]
    pub name: String,
    pub description: String,
    /// Intent string; supports {{key}} variable substitution.
    pub intent: String,
    /// Default repo paths passed as --repo when running the template.
    #[serde(default)]
    pub repos: Vec<String>,
}

impl PlanTemplate {
    /// Returns all unique {{key}} variable names used in the intent.
    pub fn variables(&self) -> Vec<String> {
        let mut vars: Vec<String> = Vec::new();
        let mut s = self.intent.as_str();
        while let Some(start) = s.find("{{") {
            s = &s[start + 2..];
            if let Some(end) = s.find("}}") {
                let key = s[..end].trim().to_string();
                if !key.is_empty() && !vars.contains(&key) {
                    vars.push(key);
                }
                s = &s[end + 2..];
            } else {
                break;
            }
        }
        vars
    }

    /// Substitute all {{key}} placeholders. Fails if any required variable is missing.
    pub fn render(&self, vars: &HashMap<String, String>) -> Result<String> {
        for key in self.variables() {
            if !vars.contains_key(&key) {
                bail!(
                    "Template variable '{{{{{}}}}}' is required. Pass it as: {}=<value>",
                    key,
                    key
                );
            }
        }
        let mut result = self.intent.clone();
        for (key, val) in vars {
            result = result.replace(&format!("{{{{{key}}}}}"), val);
        }
        Ok(result)
    }
}

// ── load / list / save ────────────────────────────────────────────────────────

pub fn load_template(name: &str) -> Result<PlanTemplate> {
    let path = template_path(name);
    if !path.exists() {
        bail!("Template '{}' not found. Expected: {}", name, path.display());
    }
    let text = fs::read_to_string(&path)?;
    let mut t: PlanTemplate = toml::from_str(&text)?;
    t.name = name.to_string();
    Ok(t)
}

pub fn list_templates() -> Vec<PlanTemplate> {
    let dir = templates_dir();
    if !dir.exists() {
        return vec![];
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };
    let mut templates: Vec<PlanTemplate> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x == "toml")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_stem()?.to_str()?.to_string();
            if name.is_empty() {
                return None;
            }
            let text = fs::read_to_string(&path).ok()?;
            let mut t: PlanTemplate = toml::from_str(&text).ok()?;
            t.name = name;
            Some(t)
        })
        .collect();
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    templates
}

pub fn save_template(name: &str, template: &PlanTemplate) -> Result<()> {
    let dir = templates_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{name}.toml"));

    // Only serialize TOML-visible fields (name is derived from filename).
    #[derive(Serialize)]
    struct Storable<'a> {
        description: &'a str,
        intent: &'a str,
        #[serde(skip_serializing_if = "<[_]>::is_empty")]
        repos: &'a [String],
    }
    let storable = Storable {
        description: &template.description,
        intent: &template.intent,
        repos: &template.repos,
    };
    fs::write(path, toml::to_string_pretty(&storable)?)?;
    Ok(())
}

/// Returns a minimal TOML starter for `template create`.
pub fn starter_toml(name: &str) -> String {
    format!(
        r#"description = "Describe what this template does"
intent = "Your intent here — use {{{{variable}}}} for substitution"
# repos = ["./frontend", "./backend"]
# Template name: {name}
"#
    )
}

// ── parse key=value pairs ─────────────────────────────────────────────────────

/// Parse `["key=value", ...]` CLI args into a variable map.
pub fn parse_vars(raw: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for item in raw {
        let Some((k, v)) = item.split_once('=') else {
            bail!("Expected key=value, got: {item}");
        };
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tpl(intent: &str) -> PlanTemplate {
        PlanTemplate {
            name: "test".into(),
            description: "test".into(),
            intent: intent.into(),
            repos: vec![],
        }
    }

    #[test]
    fn variables_extracts_keys() {
        let t = tpl("Fix {{module}} and {{file}} issues");
        let vars = t.variables();
        assert_eq!(vars, vec!["module", "file"]);
    }

    #[test]
    fn render_substitutes() {
        let t = tpl("Fix {{module}} issues");
        let mut vars = HashMap::new();
        vars.insert("module".into(), "auth".into());
        assert_eq!(t.render(&vars).unwrap(), "Fix auth issues");
    }

    #[test]
    fn render_fails_on_missing_var() {
        let t = tpl("Fix {{module}} issues");
        let vars = HashMap::new();
        assert!(t.render(&vars).is_err());
    }

    #[test]
    fn render_no_vars() {
        let t = tpl("Review the pull request");
        let vars = HashMap::new();
        assert_eq!(t.render(&vars).unwrap(), "Review the pull request");
    }

    #[test]
    fn parse_vars_works() {
        let raw = vec!["module=auth".to_string(), "file=main.rs".to_string()];
        let map = parse_vars(&raw).unwrap();
        assert_eq!(map["module"], "auth");
        assert_eq!(map["file"], "main.rs");
    }

    #[test]
    fn parse_vars_rejects_no_eq() {
        let raw = vec!["invalid".to_string()];
        assert!(parse_vars(&raw).is_err());
    }
}
