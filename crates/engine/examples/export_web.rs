//! Exports a project's web version without the editor: `cargo run -p rustpg2d-engine --example
//! export_web -- PROJECT_DIR OUT_DIR [NAME] [--wasm FILE]`. The runner is `target/web/
//! rustpg2d-game.wasm` unless given (build it with `cargo build --release --target
//! wasm32-unknown-unknown -p rustpg2d-game && cargo xtask-web`).

use std::path::{Path, PathBuf};

use rustpg2d_engine::project::Project;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(project), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: export_web PROJECT_DIR OUT_DIR [NAME] [--wasm FILE]");
        std::process::exit(2);
    };
    let wasm = args.iter().position(|a| a == "--wasm").and_then(|i| args.get(i + 1)).map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/web/rustpg2d-game.wasm"));
    let run = || -> anyhow::Result<()> {
        let p = Project::load(Path::new(project))?;
        let name = args.get(2).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| p.config.name.clone());
        let r = rustpg2d_engine::release::export_web(&p, &name, &wasm, Path::new(out), false)?;
        println!("{}: {} files, {:.1} MB", r.folder.display(), r.files, r.bytes as f64 / 1e6);
        for w in r.warnings {
            println!("warning: {w}");
        }
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("export_web: {e:#}");
        std::process::exit(1);
    }
}
