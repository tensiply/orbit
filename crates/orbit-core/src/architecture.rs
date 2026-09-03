use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Entity kind ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "PascalCase")]
pub enum EntityKind {
    Service,
    Integration,
    Database,
    Infrastructure,
    #[serde(rename = "API")]
    Api,
    Pipeline,
    SecretGroup,
    #[serde(rename = "IAM")]
    Iam,
    Team,
    #[default]
    #[serde(other)]
    Unknown,
}

impl EntityKind {
    pub fn folder_name(&self) -> &'static str {
        match self {
            EntityKind::Service => "services",
            EntityKind::Integration => "integrations",
            EntityKind::Database => "databases",
            EntityKind::Infrastructure => "infrastructure",
            EntityKind::Api => "apis",
            EntityKind::Pipeline => "pipelines",
            EntityKind::SecretGroup => "secrets",
            EntityKind::Iam => "iam",
            EntityKind::Team => "teams",
            EntityKind::Unknown => "unknown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            EntityKind::Service => "Services",
            EntityKind::Integration => "Integrations",
            EntityKind::Database => "Databases",
            EntityKind::Infrastructure => "Infrastructure",
            EntityKind::Api => "APIs",
            EntityKind::Pipeline => "Pipelines",
            EntityKind::SecretGroup => "Secrets",
            EntityKind::Iam => "IAM",
            EntityKind::Team => "Teams",
            EntityKind::Unknown => "Unknown",
        }
    }

    pub fn all_folders() -> &'static [&'static str] {
        &[
            "services",
            "integrations",
            "databases",
            "infrastructure",
            "apis",
            "pipelines",
            "secrets",
            "iam",
            "teams",
        ]
    }

    pub fn from_folder(folder: &str) -> Self {
        match folder {
            "services" => EntityKind::Service,
            "integrations" => EntityKind::Integration,
            "databases" => EntityKind::Database,
            "infrastructure" => EntityKind::Infrastructure,
            "apis" => EntityKind::Api,
            "pipelines" => EntityKind::Pipeline,
            "secrets" => EntityKind::SecretGroup,
            "iam" => EntityKind::Iam,
            "teams" => EntityKind::Team,
            _ => EntityKind::Unknown,
        }
    }
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ── Common fields present in every entity ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogEntity {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub kind: EntityKind,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub criticality: Option<String>,
    #[serde(default)]
    pub lifecycle: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub updated_by: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Target entity IDs drawn as edges on the architecture canvas.
    #[serde(default)]
    pub connections: Vec<String>,

    // Kind-specific summary fields (all optional, unknown fields captured in extra)
    #[serde(default)]
    pub tech: Option<serde_yml::Value>,
    #[serde(default)]
    pub environments: Option<serde_yml::Value>,
    #[serde(default)]
    pub depends_on: Option<serde_yml::Value>,
    #[serde(default)]
    pub exposes: Option<serde_yml::Value>,
    #[serde(default)]
    pub source: Option<serde_yml::Value>,
    #[serde(default)]
    pub target: Option<serde_yml::Value>,
    #[serde(default)]
    pub protocol: Option<serde_yml::Value>,
    #[serde(default)]
    pub engine: Option<serde_yml::Value>,
    #[serde(default)]
    pub used_by: Option<serde_yml::Value>,
    #[serde(rename = "type", default)]
    pub infra_type: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub provided_by: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub members: Option<serde_yml::Value>,
    #[serde(default)]
    pub contact: Option<serde_yml::Value>,
    #[serde(default)]
    pub monitoring: Option<serde_yml::Value>,
    #[serde(default)]
    pub security: Option<serde_yml::Value>,
    #[serde(default)]
    pub backup: Option<serde_yml::Value>,

    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_yml::Value>,
}

impl CatalogEntity {
    /// One-line summary string for list/table display, derived from kind-specific fields.
    pub fn summary(&self) -> Option<String> {
        match self.kind {
            EntityKind::Service => {
                let platform = self
                    .tech
                    .as_ref()
                    .and_then(|t| t.get("platform"))
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        self.tech
                            .as_ref()
                            .and_then(|t| t.get("runtime_env"))
                            .and_then(|v| v.as_str())
                    });
                platform.map(|p| p.to_string())
            }
            EntityKind::Database => self
                .engine
                .as_ref()
                .and_then(|e| e.get("type"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            EntityKind::Integration => {
                let src = self
                    .source
                    .as_ref()
                    .and_then(|s| s.get("service"))
                    .and_then(|v| v.as_str());
                let tgt = self
                    .target
                    .as_ref()
                    .and_then(|t| t.get("service"))
                    .and_then(|v| v.as_str());
                match (src, tgt) {
                    (Some(s), Some(t)) => Some(format!("{s} → {t}")),
                    _ => None,
                }
            }
            EntityKind::Infrastructure => self.infra_type.clone(),
            EntityKind::Api => self
                .provided_by
                .as_ref()
                .map(|s| format!("provided by {s}")),
            EntityKind::Pipeline => self.tool.clone(),
            EntityKind::SecretGroup => self.backend.clone(),
            EntityKind::Iam => self.platform.clone(),
            EntityKind::Team => self
                .members
                .as_ref()
                .and_then(|m| m.as_sequence())
                .map(|seq| format!("{} members", seq.len())),
            EntityKind::Unknown => None,
        }
    }
}

// ── Catalog load result ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogLoadResult {
    pub tenant_dir: PathBuf,
    pub entities: Vec<CatalogEntity>,
    /// Files that failed to parse: (path, error message)
    pub errors: Vec<(PathBuf, String)>,
}

impl CatalogLoadResult {
    pub fn by_kind(&self) -> std::collections::HashMap<&EntityKind, Vec<&CatalogEntity>> {
        let mut map: std::collections::HashMap<&EntityKind, Vec<&CatalogEntity>> =
            std::collections::HashMap::new();
        for e in &self.entities {
            map.entry(&e.kind).or_default().push(e);
        }
        map
    }

    pub fn count_by_kind(&self, kind: &EntityKind) -> usize {
        self.entities.iter().filter(|e| &e.kind == kind).count()
    }

    pub fn criticality_counts(&self, kind: &EntityKind) -> Vec<(String, usize)> {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for e in self.entities.iter().filter(|e| &e.kind == kind) {
            if let Some(c) = &e.criticality {
                *counts.entry(c.clone()).or_default() += 1;
            }
        }
        let order = ["critical", "high", "medium", "low"];
        let mut result: Vec<(String, usize)> = counts.into_iter().collect();
        result.sort_by_key(|(k, _)| {
            order
                .iter()
                .position(|o| *o == k.as_str())
                .unwrap_or(order.len())
        });
        result
    }
}

// ── Catalog path helpers ──────────────────────────────────────────────────────

/// `{tenant_dir}/source-of-truth/catalog/`
pub fn catalog_root(tenant_dir: &Path) -> PathBuf {
    tenant_dir.join("source-of-truth").join("catalog")
}

/// `{tenant_dir}/source-of-truth/catalog/{kind_folder}/`
pub fn catalog_kind_dir(tenant_dir: &Path, kind_folder: &str) -> PathBuf {
    catalog_root(tenant_dir).join(kind_folder)
}

// ── Loader ────────────────────────────────────────────────────────────────────

/// Load all catalog YAML files for a given tenant_dir.
/// Silently records parse errors and continues loading remaining files.
pub fn load_catalog(tenant_dir: &Path) -> CatalogLoadResult {
    let root = catalog_root(tenant_dir);
    let mut entities = Vec::new();
    let mut errors = Vec::new();

    for folder in EntityKind::all_folders() {
        let kind_dir = root.join(folder);
        if !kind_dir.is_dir() {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&kind_dir) else {
            continue;
        };
        let inferred_kind = EntityKind::from_folder(folder);

        let mut paths: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
            .collect();
        paths.sort();

        for path in paths {
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_yml::from_str::<CatalogEntity>(&s).map_err(|e| e.to_string()))
            {
                Ok(mut entity) => {
                    // Infer kind from folder when not declared in the file
                    if entity.kind == EntityKind::Unknown && entity.id.is_empty() {
                        // Completely unrecognizable file — skip
                        continue;
                    }
                    if entity.kind == EntityKind::Unknown {
                        entity.kind = inferred_kind.clone();
                    }
                    // Infer id from filename if missing
                    if entity.id.is_empty()
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        entity.id = stem.to_string();
                    }
                    entities.push(entity);
                }
                Err(e) => errors.push((path, e)),
            }
        }
    }

    entities.sort_by(|a, b| {
        let ka = a.kind.folder_name();
        let kb = b.kind.folder_name();
        ka.cmp(kb).then(a.id.cmp(&b.id))
    });

    CatalogLoadResult {
        tenant_dir: tenant_dir.to_path_buf(),
        entities,
        errors,
    }
}

/// Filter entities by project (matches against tags, case-insensitive).
pub fn filter_by_project<'a>(
    entities: &'a [CatalogEntity],
    project: &str,
) -> Vec<&'a CatalogEntity> {
    let project_lc = project.to_lowercase();
    entities
        .iter()
        .filter(|e| e.tags.iter().any(|t| t.to_lowercase() == project_lc))
        .collect()
}

// ── Write helpers ─────────────────────────────────────────────────────────────

/// Write a single entity to its YAML file, merging with any existing content
/// to preserve kind-specific fields (tech, engine, depends_on, etc.).
pub fn save_entity(tenant_dir: &Path, incoming: &CatalogEntity) -> anyhow::Result<()> {
    let kind_dir = catalog_kind_dir(tenant_dir, incoming.kind.folder_name());
    std::fs::create_dir_all(&kind_dir)?;
    let path = kind_dir.join(format!("{}.yaml", incoming.id));

    let base: CatalogEntity = if path.exists() {
        let s = std::fs::read_to_string(&path)?;
        serde_yml::from_str(&s).unwrap_or_default()
    } else {
        CatalogEntity {
            schema_version: "1".to_string(),
            ..Default::default()
        }
    };

    let merged = CatalogEntity {
        schema_version: "1".to_string(),
        kind: incoming.kind.clone(),
        id: incoming.id.clone(),
        name: incoming.name.clone(),
        description: incoming.description.clone(),
        owner: incoming.owner.clone(),
        team: incoming.team.clone(),
        tenant: incoming.tenant.clone().or(base.tenant),
        tags: incoming.tags.clone(),
        criticality: incoming.criticality.clone(),
        lifecycle: incoming.lifecycle.clone(),
        connections: incoming.connections.clone(),
        notes: incoming.notes.clone(),
        last_updated: incoming.last_updated.clone(),
        updated_by: incoming.updated_by.clone(),
        // Preserve kind-specific fields from existing file
        tech: base.tech,
        environments: base.environments,
        depends_on: base.depends_on,
        exposes: base.exposes,
        source: base.source,
        target: base.target,
        protocol: base.protocol,
        engine: base.engine,
        used_by: base.used_by,
        infra_type: base.infra_type,
        version: base.version,
        provided_by: base.provided_by,
        tool: base.tool,
        backend: base.backend,
        platform: base.platform,
        members: base.members,
        contact: base.contact,
        monitoring: base.monitoring,
        security: base.security,
        backup: base.backup,
        extra: base.extra,
    };

    let val = serde_yml::to_value(&merged)?;
    let clean = remove_nulls(val);
    let yaml = serde_yml::to_string(&clean)?;
    std::fs::write(&path, yaml)?;
    Ok(())
}

/// Delete an entity YAML file. No-op if it doesn't exist.
pub fn delete_entity(tenant_dir: &Path, kind_folder: &str, id: &str) -> anyhow::Result<()> {
    let path = catalog_kind_dir(tenant_dir, kind_folder).join(format!("{id}.yaml"));
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

fn remove_nulls(val: serde_yml::Value) -> serde_yml::Value {
    use serde_yml::Value;
    match val {
        Value::Mapping(m) => {
            let mut out = serde_yml::Mapping::new();
            for (k, v) in m {
                if !matches!(v, Value::Null) {
                    out.insert(k, remove_nulls(v));
                }
            }
            Value::Mapping(out)
        }
        Value::Sequence(s) => Value::Sequence(
            s.into_iter()
                .filter(|v| !matches!(v, Value::Null))
                .map(remove_nulls)
                .collect(),
        ),
        other => other,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, filename: &str, content: &str) {
        fs::write(dir.join(filename), content).unwrap();
    }

    #[test]
    fn load_catalog_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let result = load_catalog(tmp.path());
        assert!(result.entities.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn load_catalog_reads_services() {
        let tmp = TempDir::new().unwrap();
        let sot = tmp.path().join("source-of-truth").join("catalog");
        let svc_dir = sot.join("services");
        fs::create_dir_all(&svc_dir).unwrap();

        write_yaml(
            &svc_dir,
            "my-api.yaml",
            r#"
schema_version: "1"
kind: Service
id: my-api
name: My API
criticality: high
lifecycle: production
tags: [ecom, bwco]
"#,
        );

        let result = load_catalog(tmp.path());
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].id, "my-api");
        assert_eq!(result.entities[0].kind, EntityKind::Service);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn load_catalog_infers_kind_from_folder() {
        let tmp = TempDir::new().unwrap();
        let sot = tmp.path().join("source-of-truth").join("catalog");
        let db_dir = sot.join("databases");
        fs::create_dir_all(&db_dir).unwrap();

        write_yaml(
            &db_dir,
            "orders-db.yaml",
            r#"
id: orders-db
name: Orders DB
criticality: critical
lifecycle: production
"#,
        );

        let result = load_catalog(tmp.path());
        assert_eq!(result.entities[0].kind, EntityKind::Database);
    }

    #[test]
    fn load_catalog_records_parse_errors() {
        let tmp = TempDir::new().unwrap();
        let sot = tmp.path().join("source-of-truth").join("catalog");
        let svc_dir = sot.join("services");
        fs::create_dir_all(&svc_dir).unwrap();

        write_yaml(&svc_dir, "bad.yaml", ":\n  - invalid: yaml: here:");
        write_yaml(
            &svc_dir,
            "good.yaml",
            "id: svc-a\nname: Service A\ncriticality: low\n",
        );

        let result = load_catalog(tmp.path());
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn filter_by_project_matches_tags() {
        let e1 = CatalogEntity {
            id: "svc-a".to_string(),
            tags: vec!["ecom".to_string(), "bwco".to_string()],
            ..Default::default()
        };
        let e2 = CatalogEntity {
            id: "svc-b".to_string(),
            tags: vec!["jafra".to_string()],
            ..Default::default()
        };

        let entities = vec![e1, e2];
        let filtered = filter_by_project(&entities, "BWCO");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "svc-a");
    }

    #[test]
    fn criticality_counts_ordered() {
        let tmp = TempDir::new().unwrap();
        let result = CatalogLoadResult {
            tenant_dir: tmp.path().to_path_buf(),
            entities: vec![
                CatalogEntity {
                    kind: EntityKind::Database,
                    id: "a".into(),
                    criticality: Some("high".into()),
                    ..Default::default()
                },
                CatalogEntity {
                    kind: EntityKind::Database,
                    id: "b".into(),
                    criticality: Some("critical".into()),
                    ..Default::default()
                },
                CatalogEntity {
                    kind: EntityKind::Database,
                    id: "c".into(),
                    criticality: Some("high".into()),
                    ..Default::default()
                },
            ],
            errors: vec![],
        };

        let counts = result.criticality_counts(&EntityKind::Database);
        assert_eq!(counts[0], ("critical".to_string(), 1));
        assert_eq!(counts[1], ("high".to_string(), 2));
    }
}
