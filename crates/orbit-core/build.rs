use std::{fs, path::Path};

fn embed_dir(dir: &Path, out_path: &Path, ext: &str) {
    let mut entries: Vec<(String, String)> = Vec::new();

    if let Ok(read) = fs::read_dir(dir) {
        let mut paths: Vec<_> = read
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ex| ex == ext))
            .collect();
        paths.sort_by_key(|e| e.path());

        for entry in paths {
            println!("cargo:rerun-if-changed={}", entry.path().display());
            let name = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let raw = fs::read_to_string(entry.path()).unwrap();
            // For HTML templates: inline any local <link> CSS files so the builtin is self-contained.
            let content = if ext == "html" {
                inline_css_links(&raw, dir)
            } else {
                raw
            };
            entries.push((name, content));
        }
    }

    let mut code = String::from("&[\n");
    for (name, content) in &entries {
        code.push_str(&format!("    ({:?}, {:?}),\n", name, content));
    }
    code.push(']');

    fs::write(out_path, code).unwrap();
}

/// Replace `<link rel="stylesheet" href="local/path.css">` with `<style>…</style>` at build time.
fn inline_css_links(html: &str, base_dir: &Path) -> String {
    let mut out = String::with_capacity(html.len() + 4096);
    let mut rest = html;

    while let Some(tag_start) = rest.find("<link") {
        out.push_str(&rest[..tag_start]);
        let after = &rest[tag_start..];
        let tag_end = after.find('>').unwrap_or(after.len().saturating_sub(1));
        let tag = &after[..tag_end + 1];

        let inlined = if tag.contains("stylesheet") {
            extract_href_build(tag).and_then(|href| {
                if href.starts_with("http") || href.starts_with("//") {
                    return None;
                }
                let css_path = base_dir.join(&href);
                println!("cargo:rerun-if-changed={}", css_path.display());
                let css = fs::read_to_string(&css_path).ok()?;
                Some(format!("<style>{css}</style>"))
            })
        } else {
            None
        };

        match inlined {
            Some(block) => out.push_str(&block),
            None => out.push_str(tag),
        }
        rest = &rest[tag_start + tag_end + 1..];
    }
    out.push_str(rest);
    out
}

fn extract_href_build(tag: &str) -> Option<String> {
    let start = tag.find("href")?;
    let after_eq = tag[start + 4..].trim_start_matches(|c: char| c.is_whitespace() || c == '=');
    let quote = after_eq.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &after_eq[1..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    println!("cargo:rerun-if-changed=../../plugins");
    println!("cargo:rerun-if-changed=../../hooks");
    println!("cargo:rerun-if-changed=../../commands");
    println!("cargo:rerun-if-changed=../../config/catalog");

    let plugins_dir = Path::new(&manifest_dir).join("../../plugins");
    embed_dir(&plugins_dir, &out_dir.join("builtin_plugins.rs"), "toml");

    let hooks_dir = Path::new(&manifest_dir).join("../../hooks");
    embed_dir(&hooks_dir, &out_dir.join("builtin_engine_hooks.rs"), "toml");

    let commands_dir = Path::new(&manifest_dir).join("../../commands");
    embed_dir(&commands_dir, &out_dir.join("builtin_commands.rs"), "md");

    println!("cargo:rerun-if-changed=../../document-rules");
    println!("cargo:rerun-if-changed=../../templates/documents");

    let rules_dir = Path::new(&manifest_dir).join("../../document-rules");
    embed_dir(
        &rules_dir,
        &out_dir.join("builtin_document_rules.rs"),
        "yaml",
    );

    let doc_tpl_dir = Path::new(&manifest_dir).join("../../templates/documents");
    embed_dir(
        &doc_tpl_dir,
        &out_dir.join("builtin_document_templates.rs"),
        "html",
    );

    println!("cargo:rerun-if-changed=../../image-rules");
    println!("cargo:rerun-if-changed=../../templates/images");

    let img_rules_dir = Path::new(&manifest_dir).join("../../image-rules");
    embed_dir(
        &img_rules_dir,
        &out_dir.join("builtin_image_rules.rs"),
        "yaml",
    );

    let img_tpl_dir = Path::new(&manifest_dir).join("../../templates/images");
    embed_dir(
        &img_tpl_dir,
        &out_dir.join("builtin_image_templates.rs"),
        "html",
    );
}
