//! The starter asset library as one file: `rustpg2d-starter-library-X.Y.Z.r2dlib`, published with
//! each release (CI writes it with `xtask dist` from `crates/editor/assets/starter_library`), and
//! installed by the editor (downloaded or loaded from a file) instead of being built into it.
//!
//! The file is a zip: `starter.ron` ([`Manifest`]: the editor version that made it and the
//! [`FORMAT`] of its contents), `history.ron` (the starter history, `starter_update::StarterHistory`)
//! and `library/…` (the files of the library folder: `project.ron`, `defs/`, `sprites/`, …).
//! An editor loads a pack only when its format is the editor's own [`FORMAT`].

use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// The file extension.
pub const EXT: &str = "r2dlib";

/// The version of what a pack holds. Raise it when a pack's contents need a newer editor (a new
/// definition list, a new file kind): older editors then refuse the newer packs instead of
/// half-reading them, and newer editors refuse the older ones.
pub const FORMAT: u32 = 1;

/// The pack's file name for R(ust)PG2D `version`.
pub fn file_name(version: &str) -> String {
    format!("rustpg2d-starter-library-{version}.{EXT}")
}

/// `starter.ron`: what made the pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// The R(ust)PG2D version it was made for ("0.24.0").
    pub version: String,
    /// [`FORMAT`] of the editor that made it.
    pub format: u32,
}

/// A starter library: its files (paths relative to the library folder, with `/`) and history.
#[derive(Clone, Debug, Default)]
pub struct StarterPack {
    pub version: String,
    pub format: u32,
    pub files: Vec<(String, Vec<u8>)>,
    /// The text of the starter history (`starter_history.ron`).
    pub history: String,
}

/// Files of a library folder that are not part of it: the marks a library gets when it is used
/// (`starter.stamp`, `starter.pending`) and hidden files.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || name == "starter.stamp" || name == "starter.pending"
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if skipped(&name) {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out)?;
        } else {
            let rel = p.strip_prefix(root).unwrap_or(&p).components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push((rel, std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?));
        }
    }
    Ok(())
}

impl StarterPack {
    /// The starter library folder `dir` with the history in `history` (a file), for `version`.
    pub fn from_dir(dir: &Path, history: &Path, version: &str) -> Result<StarterPack> {
        let mut files = Vec::new();
        walk(dir, dir, &mut files)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        if !files.iter().any(|(rel, _)| rel == "project.ron") {
            bail!("{} is no library folder (no project.ron)", dir.display());
        }
        let history = std::fs::read_to_string(history).with_context(|| format!("reading {}", history.display()))?;
        Ok(StarterPack { version: version.to_string(), format: FORMAT, files, history })
    }

    /// The pack as a file.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let manifest = crate::project::ron_text(&Manifest { version: self.version.clone(), format: self.format })?;
        zw.start_file("starter.ron", opts)?;
        zw.write_all(manifest.as_bytes())?;
        zw.start_file("history.ron", opts)?;
        zw.write_all(self.history.as_bytes())?;
        for (rel, bytes) in &self.files {
            zw.start_file(format!("library/{rel}"), opts)?;
            zw.write_all(bytes)?;
        }
        Ok(zw.finish()?.into_inner())
    }

    /// Only the manifest of a pack file (cheap: the files aren't unpacked).
    pub fn manifest(bytes: &[u8]) -> Result<Manifest> {
        let mut za = zip::ZipArchive::new(Cursor::new(bytes)).context("not a starter library file")?;
        let mut text = String::new();
        za.by_name("starter.ron").context("not a starter library file (no starter.ron)")?.read_to_string(&mut text)?;
        ron::from_str(&text).context("reading starter.ron")
    }

    /// Reads a pack file, whatever its format (check [`StarterPack::format`]).
    pub fn from_bytes(bytes: &[u8]) -> Result<StarterPack> {
        let m = Self::manifest(bytes)?;
        let mut za = zip::ZipArchive::new(Cursor::new(bytes))?;
        let mut history = String::new();
        if let Ok(mut f) = za.by_name("history.ron") {
            f.read_to_string(&mut history)?;
        }
        let mut files = Vec::new();
        for i in 0..za.len() {
            let mut f = za.by_index(i)?;
            let name = f.name()?.to_string();
            let Some(rel) = name.strip_prefix("library/").map(str::to_string) else { continue };
            if f.is_dir() || rel.is_empty() {
                continue;
            }
            if !crate::project::safe_rel(&rel) {
                bail!("unsafe path in the starter library file: {rel}");
            }
            let mut data = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut data)?;
            files.push((rel, data));
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(StarterPack { version: m.version, format: m.format, files, history })
    }

    /// The files as the library code takes them ([`crate::library::LibraryFiles`]).
    pub fn library_files(&self) -> Vec<(&str, &[u8])> {
        self.files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect()
    }

    /// Whether this editor can use the pack (its format is [`FORMAT`]); else why not.
    pub fn check_format(&self) -> Result<()> {
        if self.format != FORMAT {
            bail!(
                "this starter library is for R(ust)PG2D {} (format {}), this editor reads format {FORMAT}: use the starter library of the editor's own version",
                self.version,
                self.format
            );
        }
        Ok(())
    }
}

/// The SHA-256 a `SHA256SUMS` text (`sha256sum` output) gives for `file`, as lowercase hex.
pub fn sum_for(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, name) = l.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file).then(|| hash.trim().to_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_round_trips() {
        let dir = std::env::temp_dir().join(format!("rustpg2d_starter_pack_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("lib/defs")).unwrap();
        std::fs::write(dir.join("lib/project.ron"), "(name: \"Asset library\")").unwrap();
        std::fs::write(dir.join("lib/defs/fonts.ron"), "[]").unwrap();
        std::fs::write(dir.join("lib/starter.stamp"), "x").unwrap();
        std::fs::write(dir.join("history.ron"), "(assets: {})").unwrap();
        let p = StarterPack::from_dir(&dir.join("lib"), &dir.join("history.ron"), "9.8.7").unwrap();
        assert_eq!(p.files.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>(), vec!["defs/fonts.ron", "project.ron"], "without the stamp");
        let bytes = p.to_bytes().unwrap();
        assert_eq!(StarterPack::manifest(&bytes).unwrap(), Manifest { version: "9.8.7".into(), format: FORMAT });
        let back = StarterPack::from_bytes(&bytes).unwrap();
        assert_eq!((back.version.as_str(), back.format, back.history.as_str()), ("9.8.7", FORMAT, "(assets: {})"));
        assert_eq!(back.files, p.files);
        assert!(back.check_format().is_ok());
        let old = StarterPack { format: FORMAT + 1, ..back };
        assert!(old.check_format().is_err());
        assert!(StarterPack::from_bytes(b"not a zip").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sums_are_found_by_file_name() {
        let sums = "abc123  rustpg2d-0.24.0.flatpak\nDEF456 *rustpg2d-starter-library-0.24.0.r2dlib\n";
        assert_eq!(sum_for(sums, &file_name("0.24.0")).as_deref(), Some("def456"));
        assert_eq!(sum_for(sums, "other"), None);
    }
}
