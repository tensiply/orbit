use orbit_core::{
    architecture::{
        load_catalog, save_entity, delete_entity, catalog_root, CatalogEntity, EntityKind,
    },
    resolver::{self, ResolveArgs},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ArchEntityDto {
    pub id: String,
    pub kind: String,
    pub kind_folder: String,
    pub name: String,
    pub description: Option<String>,
    pub criticality: Option<String>,
    pub lifecycle: Option<String>,
    pub tenant: Option<String>,
    pub owner: Option<String>,
    pub team: Option<String>,
    pub tags: Vec<String>,
    pub connections: Vec<String>,
    pub last_updated: Option<String>,
    pub notes: Option<String>,
    pub summary: Option<String>,
    /// Environment names this entity is present in (keys of the `environments` YAML map).
    /// Falls back to `[lifecycle]` when the map is absent.
    pub environments: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ArchCatalogDto {
    pub workspace: String,
    pub tenant: String,
    pub catalog_path: String,
    pub entities: Vec<ArchEntityDto>,
    pub errors: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
pub struct SaveEntityArgs {
    pub workspace: String,
    pub tenant: String,
    pub kind_folder: String,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub criticality: Option<String>,
    pub lifecycle: Option<String>,
    pub owner: Option<String>,
    pub team: Option<String>,
    pub tags: Vec<String>,
    pub connections: Vec<String>,
    pub notes: Option<String>,
    pub last_updated: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_dto(e: CatalogEntity) -> ArchEntityDto {
    let summary = e.summary();
    let kind_folder = e.kind.folder_name().to_string();

    let env_keys: Vec<String> = e.environments
        .as_ref()
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.keys()
                .filter_map(|k| k.as_str().map(|s| s.to_lowercase()))
                .collect()
        })
        .unwrap_or_default();

    // Fall back to lifecycle when the environments map is absent or empty
    let environments = if env_keys.is_empty() {
        e.lifecycle.iter().map(|s| s.to_lowercase()).collect()
    } else {
        env_keys
    };

    ArchEntityDto {
        kind: e.kind.display_name().to_string(),
        kind_folder,
        summary,
        connections: e.connections,
        id: e.id,
        name: e.name,
        description: e.description,
        criticality: e.criticality,
        lifecycle: e.lifecycle,
        tenant: e.tenant,
        owner: e.owner,
        team: e.team,
        tags: e.tags,
        last_updated: e.last_updated,
        notes: e.notes,
        environments,
    }
}

fn positions_path(catalog_path: &str) -> PathBuf {
    PathBuf::from(catalog_path).join(".positions.json")
}

fn routes_path(catalog_path: &str) -> PathBuf {
    PathBuf::from(catalog_path).join(".routes.json")
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn architecture_load(workspace: String, tenant: String) -> Result<ArchCatalogDto, String> {
    let scope = resolver::resolve(ResolveArgs {
        workspace: Some(workspace.clone()),
        tenant: Some(tenant.clone()),
        project: None,
        repository: None,
    })
    .map_err(|e| e.to_string())?;
    let result = load_catalog(&scope.tenant_dir);
    let catalog_path = catalog_root(&scope.tenant_dir).to_string_lossy().to_string();

    let entities = result.entities.into_iter().map(to_dto).collect();
    let errors = result
        .errors
        .into_iter()
        .map(|(p, e)| {
            let filename = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            (filename, e)
        })
        .collect();

    Ok(ArchCatalogDto { workspace, tenant, catalog_path, entities, errors })
}

#[tauri::command]
pub fn architecture_save_entity(args: SaveEntityArgs) -> Result<ArchEntityDto, String> {
    let scope = resolver::resolve(ResolveArgs {
        workspace: Some(args.workspace.clone()),
        tenant: Some(args.tenant.clone()),
        project: None,
        repository: None,
    })
    .map_err(|e| e.to_string())?;

    let entity = CatalogEntity {
        kind: EntityKind::from_folder(&args.kind_folder),
        id: args.id,
        name: args.name,
        description: args.description,
        criticality: args.criticality,
        lifecycle: args.lifecycle,
        owner: args.owner,
        team: args.team,
        tenant: Some(args.tenant),
        tags: args.tags,
        connections: args.connections,
        notes: args.notes,
        last_updated: args.last_updated,
        ..Default::default()
    };

    save_entity(&scope.tenant_dir, &entity).map_err(|e| e.to_string())?;
    Ok(to_dto(entity))
}

#[tauri::command]
pub fn architecture_delete_entity(
    workspace: String,
    tenant: String,
    kind_folder: String,
    id: String,
) -> Result<(), String> {
    let scope = resolver::resolve(ResolveArgs {
        workspace: Some(workspace),
        tenant: Some(tenant),
        project: None,
        repository: None,
    })
    .map_err(|e| e.to_string())?;
    delete_entity(&scope.tenant_dir, &kind_folder, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn architecture_load_layout(
    catalog_path: String,
) -> Result<HashMap<String, [f64; 2]>, String> {
    let path = positions_path(&catalog_path);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let s = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn architecture_save_layout(
    catalog_path: String,
    positions: HashMap<String, [f64; 2]>,
) -> Result<(), String> {
    let path = positions_path(&catalog_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(&positions).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn architecture_load_routes(
    catalog_path: String,
) -> Result<HashMap<String, HashMap<String, f64>>, String> {
    let path = routes_path(&catalog_path);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let s = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn architecture_save_routes(
    catalog_path: String,
    routes: HashMap<String, HashMap<String, f64>>,
) -> Result<(), String> {
    let path = routes_path(&catalog_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(&routes).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}
