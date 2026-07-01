use std::{fs, path::Path};

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let plugins_dir = Path::new(&manifest_dir).join("../../plugins");

    println!("cargo:rerun-if-changed=../../plugins");
    println!("cargo:rerun-if-changed=../../config/catalog");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("builtin_plugins.rs");

    let mut entries: Vec<(String, String)> = Vec::new();

    if let Ok(dir) = fs::read_dir(&plugins_dir) {
        let mut paths: Vec<_> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
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
            let content = fs::read_to_string(entry.path()).unwrap();
            entries.push((name, content));
        }
    }

    let mut code = String::from("&[\n");
    for (name, content) in &entries {
        code.push_str(&format!("    ({:?}, {:?}),\n", name, content));
    }
    code.push(']');

    fs::write(&out_path, code).unwrap();
}
