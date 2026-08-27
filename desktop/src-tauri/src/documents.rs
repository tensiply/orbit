use base64::{Engine as _, engine::general_purpose::STANDARD};
use orbit_core::document::{load_all_entries_global, remove_entry};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DocInfo {
    pub id: String,
    pub title: String,
    pub format: String,
    pub template: Option<String>,
    pub output_path: String,
    pub source_path: String,
    pub workspace: String,
    pub tenant: String,
    pub project: String,
    pub repository: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Returns the file content as a base64 string so the frontend can build a Blob URL.
/// Using base64 avoids Tauri's default CSP blocking asset:// URLs inside iframes.
#[tauri::command]
pub fn document_read_b64(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(STANDARD.encode(&bytes))
}

/// Delete a document: removes it from the index and deletes the output file.
#[tauri::command]
pub fn document_delete(id: String, workspace: String) -> Result<(), String> {
    let entries = load_all_entries_global();
    let entry = entries.iter().find(|e| e.id == id).ok_or("document not found")?;
    let path = entry.output_path.clone();
    remove_entry(&workspace, &id).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Archive a document: moves the output file to an `archived/` subfolder and removes from index.
#[tauri::command]
pub fn document_archive(id: String, workspace: String) -> Result<(), String> {
    let entries = load_all_entries_global();
    let entry = entries.iter().find(|e| e.id == id).ok_or("document not found")?;
    let src = entry.output_path.clone();

    if src.exists() {
        let archived_dir = src.parent().map(|p| p.join("archived")).unwrap_or_default();
        std::fs::create_dir_all(&archived_dir).map_err(|e| e.to_string())?;
        let dst = archived_dir.join(src.file_name().unwrap_or_default());
        std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
    }
    remove_entry(&workspace, &id).map_err(|e| e.to_string())?;
    Ok(())
}

/// Open the folder containing the document in the system file manager.
#[tauri::command]
pub fn document_reveal(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    let dir = if p.is_dir() { p.to_owned() } else { p.parent().unwrap_or(p).to_owned() };
    std::process::Command::new("xdg-open")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn document_list() -> Vec<DocInfo> {
    let mut entries = load_all_entries_global();
    entries.sort_by_key(|e| std::cmp::Reverse(e.updated_at));
    entries
        .into_iter()
        .map(|e| DocInfo {
            id: e.id,
            title: e.title,
            format: e.format,
            template: e.template,
            output_path: e.output_path.to_string_lossy().to_string(),
            source_path: e.source_path.to_string_lossy().to_string(),
            workspace: e.workspace,
            tenant: e.tenant,
            project: e.project,
            repository: e.repository,
            created_at: e.created_at,
            updated_at: e.updated_at,
        })
        .collect()
}
