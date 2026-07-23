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
            let content = fs::read_to_string(entry.path()).unwrap();
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
}
