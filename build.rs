use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=scss");

    let out_root = Path::new("static/css");
    fs::create_dir_all(out_root).expect("create static/css");

    for bundle in ["public", "admin"] {
        let entry: PathBuf = ["scss", bundle, "main.scss"].iter().collect();
        if !entry.exists() {
            continue;
        }
        let css = grass::from_path(&entry, &grass::Options::default())
            .unwrap_or_else(|e| panic!("scss compile failed for {}: {e}", entry.display()));
        let out_path = out_root.join(format!("{bundle}.css"));
        fs::write(&out_path, css).expect("write css output");
        println!(
            "cargo:warning=compiled {} -> {}",
            entry.display(),
            out_path.display()
        );
    }

    for entry in walkdir::WalkDir::new("scss")
        .into_iter()
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("scss") {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}
