//! Fingerprints the engine sources so the editor can tell whether the game binary
//! it launches was built from the same engine (and thus reads the same map data).
use std::path::Path;

fn walk(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, files);
        } else if p.extension().is_some_and(|x| x == "rs") {
            files.push(p);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    let mut files = Vec::new();
    walk(Path::new("src"), &mut files);
    files.sort();
    // FNV-1a over paths and contents.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for f in &files {
        let bytes = f.to_string_lossy().into_owned().into_bytes();
        for b in bytes.iter().chain(std::fs::read(f).unwrap_or_default().iter()) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    println!("cargo:rustc-env=RUSTPG2D_ENGINE_BUILD={h:016x}");
}
