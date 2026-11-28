//! Imports the SRD 5.1's races, subraces, backgrounds and monsters from a checkout of 5e-bits/5e-database
//! into a library or project folder (default: the editor's starter library); see
//! `crates/editor-core/src/srd_import.rs`. The data is pinned to a commit:
//!
//! ```sh
//! git clone https://github.com/5e-bits/5e-database /tmp/5e-database
//! git -C /tmp/5e-database checkout a6212beb4b278917c2cff41c2b04c4ef4f3e0d6f
//! cargo run -p rustpg2d-editor-core --example srd_import -- /tmp/5e-database
//! ```

#[path = "../src/srd_import.rs"]
mod srd_import;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(checkout) = args.first().map(PathBuf::from) else {
        anyhow::bail!("usage: srd_import <5e-database checkout> [library or project folder]");
    };
    let root = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("crates/editor/assets/starter_library"));
    let head = std::process::Command::new("git").arg("-C").arg(&checkout).args(["rev-parse", "HEAD"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if head != srd_import::SRD_COMMIT {
        eprintln!("[srd_import] warning: the checkout is at {head:?}, the data is pinned to {} (git checkout it, or update SRD_COMMIT on purpose)", srd_import::SRD_COMMIT);
    }
    println!("[srd_import] {}", srd_import::import_into(&checkout, &root)?);
    // The NPC stat block templates the engine builds in (run from the repository's root).
    let npcs = PathBuf::from("crates/engine/src/srd/npcs.ron");
    if npcs.exists() {
        println!("[srd_import] {}: {} NPC templates", npcs.display(), srd_import::write_npcs(&checkout, &npcs)?);
        let rules = PathBuf::from("crates/engine/src/srd/rules.ron");
        srd_import::write_rules(&checkout, &rules)?;
        println!("[srd_import] {}: conditions, damage types, languages, alignments, abilities, skills", rules.display());
        // The example's characters of SRD monsters (its wolf, bat, troll) get the same data as
        // the library's copies.
        let example = PathBuf::from("example_project");
        if example.join("project.ron").exists() {
            println!("[srd_import] {}: {} characters updated", example.display(), srd_import::update_monsters_in(&checkout, &example)?);
        }
    }
    Ok(())
}
