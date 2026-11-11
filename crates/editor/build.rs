//! Embeds the starter asset library (`assets/starter_library`, an ordinary library folder) in
//! the editor: `OUT_DIR/starter_files.rs` lists every file with `include_bytes!`, sorted by path.
use std::path::{Path, PathBuf};

const DIR: &str = "assets/starter_library";

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        // starter.stamp / starter.pending appear when the folder itself is opened as the asset library.
        if name.starts_with('.') || name == "starter.stamp" || name == "starter.pending" {
            continue;
        }
        if p.is_dir() {
            walk(&p, files);
        } else {
            files.push(p);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed={DIR}");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR);
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut entries: Vec<(String, PathBuf)> = files.into_iter().map(|f| (f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"), f)).collect();
    entries.sort();
    assert!(entries.iter().any(|(rel, _)| rel == "project.ron"), "{DIR} has no project.ron");
    let mut out = String::from("/// The files of the starter asset library (`assets/starter_library`), sorted by path.\npub static STARTER_FILES: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &entries {
        out += &format!("    ({rel:?}, include_bytes!({:?})),\n", path.to_string_lossy());
    }
    out += "];\n";
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("starter_files.rs");
    std::fs::write(dest, out).unwrap();
}
