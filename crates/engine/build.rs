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

/// The versions of the crates that define the on-disk formats (from the workspace's Cargo.lock):
/// an upgrade of one of them changes the fingerprint like an engine change would.
fn format_crate_versions() -> String {
    let Ok(lock) = std::fs::read_to_string("../../Cargo.lock") else { return String::new() };
    let mut out = String::new();
    let mut name = "";
    for line in lock.lines() {
        if let Some(n) = line.strip_prefix("name = ") {
            name = n.trim_matches('"');
        } else if let Some(v) = line.strip_prefix("version = ") {
            if matches!(name, "ron" | "postcard" | "lz4_flex" | "serde") {
                out += &format!("{name} {v}\n");
            }
        }
    }
    out
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    let mut files = Vec::new();
    walk(Path::new("src"), &mut files);
    files.sort();
    // FNV-1a over paths and contents, then the format crates' versions.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format_crate_versions().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    for f in &files {
        // `/` on every platform, so a Windows build fingerprints like a Linux one.
        let bytes = f.to_string_lossy().replace('\\', "/").into_bytes();
        for b in bytes.iter().chain(std::fs::read(f).unwrap_or_default().iter()) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    println!("cargo:rustc-env=RUSTPG2D_ENGINE_BUILD={h:016x}");
}
