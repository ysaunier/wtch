use std::fs;
use std::path::Path;

fn main() {
    tauri_build::build();

    // Generate preset registry from presets/ directory
    let presets_dir = Path::new("../presets");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("preset_registry.rs");

    let mut entries = Vec::new();
    let abs_presets = fs::canonicalize(presets_dir).ok();

    if let Some(abs_dir) = &abs_presets {
        for entry in fs::read_dir(abs_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map(|e| e == "yml").unwrap_or(false) {
                let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
                let abs_path = fs::canonicalize(&path)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/");
                entries.push((stem, abs_path));
            }
        }
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut code = String::new();
    code.push_str("/// Auto-generated preset registry. Do not edit.\n");
    code.push_str("pub fn list_all_presets() -> Vec<String> {\n");
    code.push_str("    vec![\n");
    for (name, _) in &entries {
        code.push_str(&format!("        \"{}\".to_string(),\n", name));
    }
    code.push_str("    ]\n}\n\n");

    code.push_str("pub fn load_preset_yaml(name: &str) -> Option<&'static str> {\n");
    code.push_str("    match name {\n");
    for (name, path) in &entries {
        code.push_str(&format!(
            "        \"{}\" => Some(include_str!(\"{}\")),\n",
            name, path
        ));
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n}\n");

    fs::write(&dest_path, code).unwrap();

    // Rerun if presets directory changes
    println!("cargo:rerun-if-changed=../presets");
}
