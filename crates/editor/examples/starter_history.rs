//! Writes `crates/editor/assets/starter_history.ron`: the fingerprints of every version of every
//! starter asset and script function the editor ever shipped (every commit that changed the
//! starter set, read from git, plus the working tree). *Update asset library* uses it to tell
//! assets the creator never touched (replaced) from those they changed (asked), and to find
//! the assets the starter set dropped. Run it after changing the starter library (the test
//! `starter_history_covers_the_starter` fails until then):
//!
//! ```sh
//! cargo run -p rustpg2d-editor --example starter_history
//! ```

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use rustpg2d_engine::library::Bundle;
use rustpg2d_engine::project::TILE_SIZE;
use rustpg2d_engine::starter_update::{starter_bundle, StarterHistory};

const FOLDER: &str = "crates/editor/assets/starter_library";
/// Where the starter set lived before 0.11.0.
const PACK: &str = "crates/editor/assets/starter.r2dpack";
const OUT: &str = "crates/editor/assets/starter_history.ron";

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").current_dir(repo).args(args).output().context("running git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// `git cat-file --batch`: blobs by `rev:path`.
struct Blobs {
    input: std::process::ChildStdin,
    output: BufReader<std::process::ChildStdout>,
}

impl Blobs {
    fn new(repo: &Path) -> Result<Blobs> {
        let mut child = Command::new("git").current_dir(repo).args(["cat-file", "--batch"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
        Ok(Blobs { input: child.stdin.take().unwrap(), output: BufReader::new(child.stdout.take().unwrap()) })
    }

    fn get(&mut self, spec: &str) -> Result<Option<Vec<u8>>> {
        writeln!(self.input, "{spec}")?;
        self.input.flush()?;
        let mut header = String::new();
        self.output.read_line(&mut header)?;
        if header.trim_end().ends_with("missing") {
            return Ok(None);
        }
        let size: usize = header.split_whitespace().nth(2).context("cat-file header")?.parse()?;
        let mut bytes = vec![0; size + 1];
        self.output.read_exact(&mut bytes)?;
        bytes.pop();
        Ok(Some(bytes))
    }
}

/// The starter set as it was at commit `rev`.
fn starter_at(repo: &Path, blobs: &mut Blobs, rev: &str) -> Result<Option<Bundle>> {
    let paths = git(repo, &["ls-tree", "-r", "--name-only", rev, "--", FOLDER])?;
    if !paths.trim().is_empty() {
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for p in paths.lines() {
            if let Some(b) = blobs.get(&format!("{rev}:{p}"))? {
                files.push((p.trim_start_matches(FOLDER).trim_start_matches('/').to_string(), b));
            }
        }
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect();
        return starter_bundle(&refs).map(Some);
    }
    match blobs.get(&format!("{rev}:{PACK}"))? {
        Some(bytes) => {
            let mut b = Bundle::from_pack(&bytes)?;
            b.rescale(TILE_SIZE)?;
            Ok(Some(b))
        }
        None => Ok(None),
    }
}

/// The working tree's starter folder.
fn starter_now(repo: &Path) -> Result<Bundle> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
        for e in std::fs::read_dir(dir)?.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out)?;
            } else {
                let rel = p.strip_prefix(base)?.to_string_lossy().replace('\\', "/");
                out.push((rel, std::fs::read(&p)?));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(&repo.join(FOLDER), &repo.join(FOLDER), &mut files)?;
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect();
    starter_bundle(&refs)
}

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let mut revs: Vec<String> = git(&repo, &["log", "--branches", "--tags", "--remotes", "--full-history", "--format=%H", "--", FOLDER, PACK])?.lines().map(str::to_string).collect();
    revs.extend(git(&repo, &["tag", "--list", "v*"])?.lines().map(str::to_string));
    revs.sort();
    revs.dedup();
    let mut blobs = Blobs::new(&repo)?;
    let mut history = StarterHistory::default();
    let (mut read, mut skipped) = (0, 0);
    for rev in &revs {
        match starter_at(&repo, &mut blobs, rev) {
            Ok(Some(b)) => {
                history.record(&b);
                read += 1;
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("skipped {rev}: {e:#}");
                skipped += 1;
            }
        }
    }
    history.record(&starter_now(&repo)?);
    let text = history.to_ron()?;
    std::fs::write(repo.join(OUT), format!("// Written by `cargo run -p rustpg2d-editor --example starter_history`: every version of every\n// starter asset ever shipped (Update asset library). Don't edit by hand.\n{text}"))?;
    let versions: usize = history.assets.values().map(|h| h.defs.len()).sum();
    println!("{OUT}: {} assets ({versions} definition versions), {} script functions, from {read} starter sets + the working tree ({skipped} skipped)", history.assets.len(), history.functions.len());
    Ok(())
}
