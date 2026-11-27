//! Packs: the one file format what creators make travels in. A pack holds assets (definitions
//! with their files and the script functions their events call), plugins and preview pictures
//! under a manifest that says what it is, whose it is, under which licence, which version, and
//! which other packs it needs. A source (a web address, or a folder) offers packs through an
//! index file with each pack's manifest, size and SHA-256.
//!
//! A pack file (`.r2dpack`) is a zip:
//!
//! ```text
//! pack.ron                 the manifest (PackManifest)
//! assets/manifest.ron      the definitions and script functions (as 0.35's asset packages)
//! assets/files/…           their files, by their place in a project
//! plugins/<id>/…           plugin folders
//! previews/…               pictures for the library's cards
//! history.ron              optional: the versions of its assets it shipped before (updates)
//! ```
//!
//! The three file kinds of 0.35 are still read ([`Pack::from_bytes`] tells them apart): an
//! asset package (`.r2dpack` with `manifest.ron` at its top), a plugin file (`.r2dplugin`) and
//! the starter library file (`.r2dlib`). Each comes back as a pack with a manifest made up
//! from what the file says about itself, marked [`PackManifest::legacy`].

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::library::Bundle;

/// The file extension of packs.
pub const EXT: &str = "r2dpack";

/// The version of the pack format itself (what the zip holds and how). A reader refuses packs
/// of a newer format instead of half-reading them.
pub const FORMAT: u32 = 1;

/// The id of the starter library as a pack.
pub const STARTER_ID: &str = "rustpg2d.starter";

/// The name of a source's index file.
pub const INDEX_FILE: &str = "index.ron";

/// Another pack this one needs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Need {
    pub id: String,
    /// The lowest version that will do ("" = any).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
}

/// `pack.ron`: what a pack is.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    /// [`FORMAT`] of what wrote it.
    pub format: u32,
    /// What the pack is filed under, everywhere: lower-case letters, digits, `_`, `-` and dots
    /// (`anna.forest-props`). Two packs with one id are versions of the same pack.
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Its version, compared part by part as numbers (`1.2.10` is newer than `1.2.9`).
    pub version: String,
    /// The licence its contents are under, by name ("CC BY 4.0", "CC0", "MIT").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub licence: String,
    /// The line a game that uses it must show (the credits offer it): who made what.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribution: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The oldest R(ust)PG2D that can use it ("0.36"; "" = any).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub engine: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<Need>,
    /// Its preview pictures, by their names under `previews/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previews: Vec<String>,
    /// Read from a file of 0.35 (an asset package, a plugin file, the starter library file):
    /// the manifest was made up from what that file says.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub legacy: bool,
}

/// Whether `id` can be a pack's id.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.')) && !id.starts_with('.') && !id.ends_with('.') && !id.contains("..")
}

/// A pack id out of any name ("Anna's Forest Props" -> "annas-forest-props").
pub fn id_from(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        match c {
            c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            '.' | '_' => out.push(c),
            ' ' | '-' | '/' if !out.ends_with('-') && !out.is_empty() => out.push('-'),
            _ => {}
        }
    }
    let out = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if out.is_empty() {
        "pack".into()
    } else {
        out
    }
}

/// Compares two versions part by part, numbers as numbers ("1.10" is newer than "1.9").
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    crate::plugins::compare_versions(a, b)
}

impl PackManifest {
    /// A manifest to start from: an id out of the title, version 1.0.
    pub fn new(title: &str) -> PackManifest {
        PackManifest { format: FORMAT, id: id_from(title), title: title.trim().to_string(), version: "1.0".into(), ..Default::default() }
    }

    /// What is wrong with it for a pack to be written (`None`: nothing).
    pub fn problem(&self) -> Option<String> {
        if !valid_id(&self.id) {
            return Some(format!("\"{}\" can't be a pack's id: lower-case letters, digits, - _ and dots", self.id));
        }
        if self.title.trim().is_empty() {
            return Some("the pack needs a title".into());
        }
        if self.version.trim().is_empty() || !self.version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-')) {
            return Some(format!("\"{}\" is no version: numbers with dots, as 1.0 or 2.3.1", self.version));
        }
        if let Some(n) = self.needs.iter().find(|n| !valid_id(&n.id)) {
            return Some(format!("it needs \"{}\", which is no pack id", n.id));
        }
        None
    }

    /// Whether this editor's engine can use it; else why not.
    pub fn fits(&self) -> Result<()> {
        if self.format > FORMAT {
            bail!("{} is a pack of a newer R(ust)PG2D (pack format {}, this one reads {FORMAT})", self.title, self.format);
        }
        let ours = env!("CARGO_PKG_VERSION").split('-').next().unwrap_or_default();
        if !self.engine.is_empty() && compare_versions(ours, &self.engine) == std::cmp::Ordering::Less {
            bail!("{} needs R(ust)PG2D {} or newer (this is {ours})", self.title, self.engine);
        }
        Ok(())
    }

    /// `<id>-<version>.r2dpack`.
    pub fn file_name(&self) -> String {
        format!("{}-{}.{EXT}", self.id, self.version)
    }
}

/// A plugin in a pack: its id and its folder's files (paths relative to the folder).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackedPlugin {
    pub id: String,
    pub files: Vec<(String, Vec<u8>)>,
}

/// A pack, read or to be written.
#[derive(Clone, Debug, Default)]
pub struct Pack {
    pub manifest: PackManifest,
    /// Its assets: definitions, their files, the script functions they need.
    pub assets: Bundle,
    pub plugins: Vec<PackedPlugin>,
    /// Preview pictures: name and PNG bytes ([`PackManifest::previews`] lists the names).
    pub previews: Vec<(String, Vec<u8>)>,
    /// The text of its history (`starter_update::StarterHistory`): what earlier versions of it
    /// shipped, so an update can tell an asset its creator changed from one the pack changed.
    pub history: Option<String>,
}

/// What kind of file some bytes are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Pack,
    /// 0.35: an asset package, a plugin file, the starter library file.
    OldPackage,
    OldPlugin,
    OldStarter,
}

/// Tells a pack from the file kinds of 0.35 by what is inside (not by its name).
pub fn kind_of(bytes: &[u8]) -> Result<FileKind> {
    if let Ok(mut za) = zip::ZipArchive::new(Cursor::new(bytes)) {
        if za.by_name("pack.ron").is_ok() {
            return Ok(FileKind::Pack);
        }
        if za.by_name("starter.ron").is_ok() {
            return Ok(FileKind::OldStarter);
        }
        if za.by_name("manifest.ron").is_ok() {
            return Ok(FileKind::OldPackage);
        }
        bail!("this zip is no pack (it has no pack.ron)");
    }
    if crate::plugins::unpack(bytes).is_ok() {
        return Ok(FileKind::OldPlugin);
    }
    bail!("this is no pack, asset package, plugin or starter library file")
}

fn read_text(za: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Option<String>> {
    match za.by_name(name) {
        Ok(mut f) => {
            let mut text = String::new();
            f.read_to_string(&mut text).with_context(|| format!("reading {name}"))?;
            Ok(Some(text))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {name}")),
    }
}

impl Pack {
    /// Whether it holds nothing to install.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty() && self.plugins.is_empty()
    }

    /// What it holds that runs: the script functions of its assets and its plugins (each with
    /// its script files). A pack with any of these says so before it is installed.
    pub fn code(&self) -> Vec<String> {
        let mut out: Vec<String> = self.assets.scripts.keys().map(|f| format!("script function {f}")).collect();
        for p in &self.plugins {
            let scripts: Vec<&str> = p.files.iter().filter(|(f, _)| f.ends_with(".rhai")).map(|(f, _)| f.as_str()).collect();
            out.push(if scripts.is_empty() { format!("plugin {}", p.id) } else { format!("plugin {} ({})", p.id, scripts.join(", ")) });
        }
        out
    }

    /// The pack as a file's bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        if let Some(p) = self.manifest.problem() {
            bail!(p);
        }
        let mut manifest = self.manifest.clone();
        manifest.format = FORMAT;
        manifest.legacy = false;
        manifest.previews = self.previews.iter().map(|(n, _)| n.clone()).collect();
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let put = |zw: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: String, bytes: &[u8]| -> Result<()> {
            zw.start_file(name, opts)?;
            zw.write_all(bytes)?;
            Ok(())
        };
        put(&mut zw, "pack.ron".into(), crate::project::ron_text(&manifest)?.as_bytes())?;
        put(&mut zw, "assets/manifest.ron".into(), self.assets.manifest_text()?.as_bytes())?;
        for (rel, bytes) in &self.assets.files {
            put(&mut zw, format!("assets/files/{rel}"), bytes)?;
        }
        for p in &self.plugins {
            if !crate::plugins::valid_id(&p.id) {
                bail!("\"{}\" is no plugin id", p.id);
            }
            for (rel, bytes) in &p.files {
                put(&mut zw, format!("plugins/{}/{rel}", p.id), bytes)?;
            }
        }
        for (name, bytes) in &self.previews {
            if !crate::project::safe_rel(name) || name.contains('/') {
                bail!("\"{name}\" is no name for a preview picture");
            }
            put(&mut zw, format!("previews/{name}"), bytes)?;
        }
        if let Some(h) = &self.history {
            put(&mut zw, "history.ron".into(), h.as_bytes())?;
        }
        Ok(zw.finish()?.into_inner())
    }

    /// Only the manifest of a pack file or a file of 0.35 (for a pack: without unpacking it).
    pub fn manifest_of(bytes: &[u8]) -> Result<PackManifest> {
        match kind_of(bytes)? {
            FileKind::Pack => {
                let mut za = zip::ZipArchive::new(Cursor::new(bytes))?;
                let text = read_text(&mut za, "pack.ron")?.context("the pack has no pack.ron")?;
                ron::from_str(&text).context("reading pack.ron")
            }
            _ => Ok(Pack::from_bytes(bytes)?.manifest),
        }
    }

    /// Reads a pack, or one of the three file kinds of 0.35 as a pack.
    pub fn from_bytes(bytes: &[u8]) -> Result<Pack> {
        match kind_of(bytes)? {
            FileKind::Pack => Pack::read(bytes),
            FileKind::OldPackage => {
                let assets = Bundle::from_pack(bytes)?;
                let title = if assets.name.trim().is_empty() { "Asset package".to_string() } else { assets.name.clone() };
                Ok(Pack { manifest: PackManifest { version: "0".into(), legacy: true, ..PackManifest::new(&title) }, assets, ..Default::default() })
            }
            FileKind::OldPlugin => {
                let (id, def, files) = crate::plugins::unpack(bytes)?;
                let manifest = PackManifest { id: format!("plugin.{}", id_from(&id)), title: if def.name.is_empty() { id.clone() } else { def.name.clone() }, author: def.author.clone(), version: if def.version.is_empty() { "0".into() } else { def.version.clone() }, description: def.description.clone(), legacy: true, format: FORMAT, ..Default::default() };
                Ok(Pack { manifest, plugins: vec![PackedPlugin { id, files }], ..Default::default() })
            }
            FileKind::OldStarter => {
                let starter = crate::starter_pack::StarterPack::from_bytes(bytes)?;
                starter.check_format()?;
                let assets = Bundle::from_files(&starter.library_files())?;
                // (Its plugins lie in the library's `plugins/` folder.)
                let mut plugins: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
                for (rel, bytes) in &starter.files {
                    if let Some((id, file)) = rel.strip_prefix("plugins/").and_then(|r| r.split_once('/')) {
                        plugins.entry(id.to_string()).or_default().push((file.to_string(), bytes.clone()));
                    }
                }
                let manifest = PackManifest { format: FORMAT, id: STARTER_ID.into(), title: "Starter asset library".into(), author: "R(ust)PG2D".into(), version: starter.version.clone(), legacy: true, ..Default::default() };
                Ok(Pack { manifest, assets, plugins: plugins.into_iter().map(|(id, files)| PackedPlugin { id, files }).collect(), previews: Vec::new(), history: Some(starter.history).filter(|h| !h.trim().is_empty()) })
            }
        }
    }

    fn read(bytes: &[u8]) -> Result<Pack> {
        let mut za = zip::ZipArchive::new(Cursor::new(bytes)).context("not a pack")?;
        let manifest: PackManifest = ron::from_str(&read_text(&mut za, "pack.ron")?.context("the pack has no pack.ron")?).context("reading pack.ron")?;
        manifest.fits()?;
        if let Some(p) = manifest.problem() {
            bail!("the pack's manifest: {p}");
        }
        let assets_text = read_text(&mut za, "assets/manifest.ron")?;
        let history = read_text(&mut za, "history.ron")?;
        let mut files = BTreeMap::new();
        let mut plugins: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
        let mut previews = Vec::new();
        for i in 0..za.len() {
            let mut f = za.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let name = f.name()?.to_string();
            let mut data = Vec::with_capacity(f.size() as usize);
            let mut take = |f: &mut dyn Read| -> Result<Vec<u8>> {
                f.read_to_end(&mut data)?;
                Ok(std::mem::take(&mut data))
            };
            if let Some(rel) = name.strip_prefix("assets/files/") {
                if !crate::project::safe_rel(rel) {
                    bail!("unsafe path in the pack: {rel}");
                }
                files.insert(rel.to_string(), take(&mut f)?);
            } else if let Some((id, rel)) = name.strip_prefix("plugins/").and_then(|r| r.split_once('/')) {
                if !crate::plugins::valid_id(id) || !crate::project::safe_rel(rel) {
                    bail!("unsafe path in the pack: {name}");
                }
                plugins.entry(id.to_string()).or_default().push((rel.to_string(), take(&mut f)?));
            } else if let Some(pic) = name.strip_prefix("previews/") {
                if !crate::project::safe_rel(pic) || pic.contains('/') {
                    bail!("unsafe path in the pack: {name}");
                }
                previews.push((pic.to_string(), take(&mut f)?));
            }
        }
        let assets = match assets_text {
            Some(text) => Bundle::from_manifest(&text, files).context("the pack's assets")?,
            None => Bundle::default(),
        };
        if let Some((id, _)) = plugins.iter().find(|(_, files)| !files.iter().any(|(f, _)| f == crate::plugins::PLUGIN_FILE)) {
            bail!("the pack's plugin {id} has no {}", crate::plugins::PLUGIN_FILE);
        }
        // (In the manifest's order; pictures it doesn't list go last.)
        previews.sort_by_key(|(n, _)| manifest.previews.iter().position(|p| p == n).unwrap_or(usize::MAX));
        Ok(Pack { manifest, assets, plugins: plugins.into_iter().map(|(id, files)| PackedPlugin { id, files }).collect(), previews, history })
    }
}

/// A pack as a source offers it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub manifest: PackManifest,
    /// Where its file is: an address of its own (`https://…`), else a path beside the index.
    pub file: String,
    /// The file's size in bytes.
    pub size: u64,
    /// The file's SHA-256, in hex: a download with another is refused.
    pub sha256: String,
    /// What it holds, by kind, for the list ("12 props", "3 characters").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<String>,
}

/// `index.ron`: what a source offers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceIndex {
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub packs: Vec<IndexEntry>,
}

/// The SHA-256 of a file's bytes, in hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    crate::hash::hex(&crate::hash::sha256(bytes))
}

/// What a pack holds, by kind: "12 props", "3 characters", "1 plugin".
pub fn holds(pack: &Pack) -> Vec<String> {
    let mut count: Vec<(crate::refactor::RefKind, usize)> = Vec::new();
    for (kind, _) in crate::library::all_assets(&pack.assets.db) {
        match count.iter_mut().find(|c| c.0 == kind) {
            Some(c) => c.1 += 1,
            None => count.push((kind, 1)),
        }
    }
    let plural = |n: usize, one: &str| if n == 1 { format!("1 {one}") } else if one.ends_with('s') { format!("{n} {one}es") } else { format!("{n} {one}s") };
    let mut out: Vec<String> = count.into_iter().map(|(k, n)| plural(n, &k.label().to_lowercase())).collect();
    if !pack.plugins.is_empty() {
        out.push(plural(pack.plugins.len(), "plugin"));
    }
    out
}

impl IndexEntry {
    /// The entry of a pack written as `bytes`, its file beside the index under its usual name.
    pub fn of(pack: &Pack, bytes: &[u8]) -> IndexEntry {
        let mut manifest = pack.manifest.clone();
        manifest.format = FORMAT;
        manifest.previews = pack.previews.iter().map(|(n, _)| n.clone()).collect();
        IndexEntry { file: manifest.file_name(), manifest, size: bytes.len() as u64, sha256: sha256_hex(bytes), holds: holds(pack) }
    }

    /// The downloaded file is the one the index names: its size and SHA-256. Else why not.
    pub fn check(&self, bytes: &[u8]) -> Result<()> {
        if bytes.len() as u64 != self.size {
            bail!("{} is {} bytes, the source's index says {}: the download is broken or was changed", self.file, bytes.len(), self.size);
        }
        let got = sha256_hex(bytes);
        if !got.eq_ignore_ascii_case(self.sha256.trim()) {
            bail!("{} doesn't have the SHA-256 the source's index names (it has {got}): refused", self.file);
        }
        Ok(())
    }

    /// The address of its file for an index read from `index_url`.
    pub fn url(&self, index_url: &str) -> String {
        if self.file.contains("://") {
            return self.file.clone();
        }
        match index_url.rsplit_once('/') {
            Some((base, _)) => format!("{base}/{}", self.file),
            None => self.file.clone(),
        }
    }
}

impl SourceIndex {
    pub fn from_text(text: &str) -> Result<SourceIndex> {
        ron::from_str(text).context("this is no index of a pack source")
    }

    pub fn to_text(&self) -> Result<String> {
        crate::project::ron_text(self)
    }

    /// Adds a pack's entry, in place of the entry of the same pack and version; the newest
    /// version of each pack first.
    pub fn put(&mut self, entry: IndexEntry) {
        self.packs.retain(|e| !(e.manifest.id == entry.manifest.id && e.manifest.version == entry.manifest.version));
        self.packs.push(entry);
        self.packs.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id).then(compare_versions(&b.manifest.version, &a.manifest.version)));
    }

    /// The newest version of each pack that this editor can use.
    pub fn newest(&self) -> Vec<&IndexEntry> {
        let mut out: Vec<&IndexEntry> = Vec::new();
        for e in self.packs.iter().filter(|e| e.manifest.fits().is_ok()) {
            match out.iter_mut().find(|o| o.manifest.id == e.manifest.id) {
                Some(o) if compare_versions(&e.manifest.version, &o.manifest.version) == std::cmp::Ordering::Greater => *o = e,
                Some(_) => {}
                None => out.push(e),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{ItemDef, SheetDef, SpriteRef};

    /// A pack with an item, its icon's sheet and file, a script function, a plugin and a preview.
    fn sample() -> Pack {
        let mut assets = Bundle { name: "Sample".into(), tile_size: 32, ..Default::default() };
        assets.db.sheets.push(SheetDef::new("charms", "sprites/charms.png", 32, 32));
        assets.db.items.push(ItemDef { id: "moon_charm".into(), name: "Moon charm".into(), icon: SpriteRef::new("charms", 0), ..Default::default() });
        assets.db.reindex();
        assets.files.insert("sprites/charms.png".into(), crate::imageio::Rgba::new(32, 32).encode().unwrap());
        assets.scripts.insert("moon_glow".into(), "fn moon_glow() { toast(\"It glows.\"); }".into());
        let plugin = PackedPlugin { id: "night_sky".into(), files: vec![("plugin.ron".into(), b"(name: \"Night sky\", version: \"1.2\", author: \"Anna\")".to_vec()), ("scripts/sky.rhai".into(), b"fn sky() {}".to_vec())] };
        let manifest = PackManifest { author: "Anna".into(), licence: "CC BY 4.0".into(), attribution: "Charms by Anna".into(), tags: vec!["jewellery".into()], needs: vec![Need { id: STARTER_ID.into(), version: "0.36".into() }], ..PackManifest::new("Anna's Moon Charms") };
        Pack { manifest, assets, plugins: vec![plugin], previews: vec![("card.png".into(), crate::imageio::Rgba::new(8, 8).encode().unwrap())], history: None }
    }

    #[test]
    fn a_pack_is_written_and_read_again_as_it_was() {
        let pack = sample();
        assert_eq!((pack.manifest.id.as_str(), pack.manifest.version.as_str(), pack.manifest.file_name().as_str()), ("annas-moon-charms", "1.0", "annas-moon-charms-1.0.r2dpack"));
        let bytes = pack.to_bytes().unwrap();
        assert_eq!(kind_of(&bytes).unwrap(), FileKind::Pack);
        // Its manifest alone, then everything.
        let m = Pack::manifest_of(&bytes).unwrap();
        assert_eq!((m.title.as_str(), m.licence.as_str(), m.previews.as_slice(), m.format), ("Anna's Moon Charms", "CC BY 4.0", &["card.png".to_string()][..], FORMAT));
        let back = Pack::from_bytes(&bytes).unwrap();
        assert_eq!(back.manifest, m);
        assert!(back.assets.db.item("moon_charm").is_some() && back.assets.db.sheet("charms").is_some());
        assert_eq!(back.assets.files, pack.assets.files);
        assert_eq!(back.assets.scripts, pack.assets.scripts);
        assert_eq!((back.plugins.clone(), back.previews.clone()), (pack.plugins.clone(), pack.previews.clone()));
        // What it holds, and that some of it runs: said before it is installed.
        assert_eq!(holds(&back), ["1 sprite sheet", "1 item", "1 plugin"]);
        assert_eq!(back.code(), ["script function moon_glow", "plugin night_sky (scripts/sky.rhai)"]);
        assert!(!back.is_empty() && Pack::default().is_empty());
    }

    #[test]
    fn what_is_no_good_pack_is_refused() {
        // Ids, versions.
        assert!(valid_id("anna.forest-props") && valid_id(STARTER_ID) && !valid_id("Anna") && !valid_id("a..b") && !valid_id(".a") && !valid_id(""));
        assert_eq!((id_from("  Forest / Props 2 "), id_from("???")), ("forest-props-2".to_string(), "pack".to_string()));
        let mut pack = sample();
        pack.manifest.id = "Not An Id".into();
        assert!(pack.to_bytes().is_err());
        pack.manifest = PackManifest { version: "one point oh!".into(), ..sample().manifest };
        assert!(pack.manifest.problem().is_some_and(|p| p.contains("no version")));
        // A pack of a newer format, and one for a newer engine, say so instead of being half-read.
        let newer = PackManifest { format: FORMAT + 1, ..sample().manifest };
        assert!(newer.fits().unwrap_err().to_string().contains("newer"));
        let later = PackManifest { engine: "99.0".into(), ..sample().manifest };
        assert!(later.fits().unwrap_err().to_string().contains("needs R(ust)PG2D 99.0"));
        assert!(PackManifest { engine: "0.1".into(), ..sample().manifest }.fits().is_ok());
        // A file an asset names that isn't in the pack; bytes that are nothing at all.
        let mut broken = sample();
        broken.assets.files.clear();
        assert!(Pack::from_bytes(&broken.to_bytes().unwrap()).unwrap_err().chain().any(|e| e.to_string().contains("missing sprites/charms.png")));
        assert!(kind_of(b"hello").is_err() && Pack::from_bytes(&[]).is_err());
    }

    #[test]
    fn the_file_kinds_of_before_are_read_as_packs() {
        // An asset package of 0.35: its assets, under a manifest made up from its name.
        let old = sample().assets.to_pack().unwrap();
        assert_eq!(kind_of(&old).unwrap(), FileKind::OldPackage);
        let pack = Pack::from_bytes(&old).unwrap();
        assert!(pack.manifest.legacy && pack.manifest.id == "sample" && pack.manifest.version == "0");
        assert!(pack.assets.db.item("moon_charm").is_some() && pack.assets.scripts.contains_key("moon_glow") && pack.plugins.is_empty());
        // A plugin file: the plugin, with what its own definition says.
        let plugin = crate::pack::write(&sample().plugins[0].files.iter().map(|(f, b)| (format!("night_sky/{f}"), b.clone())).collect::<Vec<_>>()).unwrap();
        assert_eq!(kind_of(&plugin).unwrap(), FileKind::OldPlugin);
        let pack = Pack::from_bytes(&plugin).unwrap();
        assert_eq!((pack.manifest.id.as_str(), pack.manifest.title.as_str(), pack.manifest.version.as_str(), pack.manifest.author.as_str()), ("plugin.night_sky", "Night sky", "1.2", "Anna"));
        assert_eq!(pack.plugins, sample().plugins);
        // Written again, both are packs of now.
        let again = Pack::from_bytes(&pack.to_bytes().unwrap()).unwrap();
        assert!(!again.manifest.legacy && again.plugins == pack.plugins);
    }

    #[test]
    fn the_starter_library_file_is_the_starter_pack() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/assets");
        let starter = crate::starter_pack::StarterPack::from_dir(&root.join("starter_library"), &root.join("starter_history.ron"), "0.36.0").unwrap();
        let bytes = starter.to_bytes().unwrap();
        assert_eq!(kind_of(&bytes).unwrap(), FileKind::OldStarter);
        let pack = Pack::from_bytes(&bytes).unwrap();
        assert_eq!((pack.manifest.id.as_str(), pack.manifest.version.as_str()), (STARTER_ID, "0.36.0"));
        assert!(pack.assets.db.character("hero").is_some() && pack.assets.db.props.len() > 100 && pack.history.is_some());
        assert!(pack.plugins.iter().all(|p| p.files.iter().any(|(f, _)| f == crate::plugins::PLUGIN_FILE)));
    }

    #[test]
    fn a_sources_index_offers_the_newest_and_checks_what_was_downloaded() {
        let (v1, mut v2) = (sample(), sample());
        v2.manifest.version = "1.10".into();
        let (b1, b2) = (v1.to_bytes().unwrap(), v2.to_bytes().unwrap());
        let mut index = SourceIndex { title: "Anna's packs".into(), ..Default::default() };
        index.put(IndexEntry::of(&v1, &b1));
        index.put(IndexEntry::of(&v2, &b2));
        index.put(IndexEntry::of(&v1, &b1));
        assert_eq!(index.packs.iter().map(|e| e.manifest.version.as_str()).collect::<Vec<_>>(), ["1.10", "1.0"], "one entry a version, the newest first (1.10 is newer than 1.0 and than 1.9)");
        // As a file and back.
        let back = SourceIndex::from_text(&index.to_text().unwrap()).unwrap();
        assert_eq!(back, index);
        let newest = back.newest();
        assert_eq!((newest.len(), newest[0].manifest.version.as_str(), newest[0].holds.as_slice()), (1, "1.10", &["1 sprite sheet".to_string(), "1 item".to_string(), "1 plugin".to_string()][..]));
        // Where its file is: beside the index, or where the entry says.
        assert_eq!(newest[0].url("https://example.org/packs/index.ron"), "https://example.org/packs/annas-moon-charms-1.10.r2dpack");
        assert_eq!(IndexEntry { file: "https://cdn.example.org/a.r2dpack".into(), ..newest[0].clone() }.url("https://example.org/packs/index.ron"), "https://cdn.example.org/a.r2dpack");
        // The download is the file the index names, or it is refused: another file, a changed byte.
        assert!(newest[0].check(&b2).is_ok());
        assert!(newest[0].check(&b1).is_err());
        let mut changed = b2.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(newest[0].check(&changed).unwrap_err().to_string().contains("SHA-256"));
        // A pack this editor can't use isn't offered.
        let mut future = sample();
        future.manifest.id = "from-the-future".into();
        future.manifest.engine = "99.0".into();
        let fb = future.to_bytes().unwrap();
        index.put(IndexEntry::of(&future, &fb));
        assert!(index.newest().iter().all(|e| e.manifest.id != "from-the-future"));
        assert!(SourceIndex::from_text("not an index").is_err());
    }
}
