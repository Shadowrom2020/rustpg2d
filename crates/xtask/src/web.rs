//! `cargo xtask-web`: the game runner for browsers. The game compiled for
//! `wasm32-unknown-unknown` talks to the browser through wasm-bindgen; this runs its second step
//! (the JavaScript that loads the module and hands it the browser's functions) and writes the
//! pair web exports copy beside a game's `index.html`:
//!
//! - `rustpg2d-game.wasm` — the module;
//! - `rustpg2d-game.js` — its loader (an ES module; `index.html` imports it).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

pub fn web(args: &[String]) -> Result<()> {
    let root = crate::root();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let input = opt("--wasm").unwrap_or_else(|| root.join("target/wasm32-unknown-unknown/release/rustpg2d-game.wasm"));
    let out = opt("--out").unwrap_or_else(|| root.join("target/web"));
    if !input.is_file() {
        bail!("{} is missing: cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game", input.display());
    }
    let tmp = out.join(".bindgen");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
    let mut b = wasm_bindgen_cli_support::Bindgen::new();
    b.input_path(&input).out_name("rustpg2d-game").typescript(false).omit_default_module_path(true);
    b.web(true).map_err(|e| anyhow::anyhow!("{e}"))?;
    b.generate(&tmp).map_err(|e| anyhow::anyhow!("wasm-bindgen: {e:#}"))?;
    // A dependency with JavaScript files of its own would need them beside the game too.
    if tmp.join("snippets").exists() {
        bail!("the web game now needs extra JavaScript files ({}): teach the web export to copy them", tmp.join("snippets").display());
    }
    std::fs::copy(tmp.join("rustpg2d-game_bg.wasm"), out.join("rustpg2d-game.wasm")).context("the module")?;
    std::fs::copy(tmp.join("rustpg2d-game.js"), out.join("rustpg2d-game.js")).context("the loader")?;
    let _ = std::fs::remove_dir_all(&tmp);
    let size = std::fs::metadata(out.join("rustpg2d-game.wasm"))?.len();
    println!("{}: the web game runner ({:.1} MB) and rustpg2d-game.js", out.join("rustpg2d-game.wasm").display(), size as f64 / 1e6);
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The bindgen step must be the version of the `wasm-bindgen` crate in Cargo.lock.
    #[test]
    fn bindgen_versions_agree() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
        let version = |name: &str| {
            let at = lock.find(&format!("name = \"{name}\"\n")).unwrap_or_else(|| panic!("{name} in Cargo.lock"));
            lock[at..].lines().nth(1).unwrap().trim_start_matches("version = ").trim_matches('"').to_string()
        };
        assert_eq!(version("wasm-bindgen"), version("wasm-bindgen-cli-support"), "bump wasm-bindgen-cli-support in crates/xtask/Cargo.toml together with wasm-bindgen");
    }
}
