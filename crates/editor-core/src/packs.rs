//! Packs in the editor: what a pack file would bring, installing it into the asset library,
//! the packs that are installed, a selection of the project shared as a pack with its entry in
//! a source's index, and sources read (a web address or a folder).
//!
//! The format is the engine's (`rustpg2d_engine::packs`). The library keeps what is installed
//! in `packs.ron` beside its definitions, and each pack's preview pictures under
//! `packs/<id>/`.

use std::path::{Path, PathBuf};

use rustpg2d_engine::library::{self, AssetRef, Bundle};
use rustpg2d_engine::packs::{self, IndexEntry, Need, Pack, PackManifest, SourceIndex};
use rustpg2d_engine::refactor::RefKind;
use serde::{Deserialize, Serialize};

use crate::workspace::Workspace;

/// The library's record of installed packs.
pub const RECORD: &str = "packs.ron";

/// A pack that is installed in the library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Installed {
    pub manifest: PackManifest,
    /// The assets it brought: kind and the id each has in the library.
    #[serde(default)]
    pub assets: Vec<(String, String)>,
    #[serde(default)]
    pub plugins: Vec<String>,
    /// The index it came from ("" = a file).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

/// What happens to an asset of a pack that the library has under the same id with other content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SameId {
    /// The library's stays; the pack's other assets use it.
    Keep,
    /// The pack's takes its place.
    Replace,
    /// The pack's comes in beside it under a new id (renamed inside what the pack brings).
    Copy,
}

/// What a pack file is and what installing it would do, before anything is done.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub manifest: PackManifest,
    /// "12 props", "3 characters", "1 plugin".
    pub holds: Vec<String>,
    /// What in it runs: script functions and plugins (empty = pictures and data only).
    pub code: Vec<String>,
    /// Assets the library doesn't have, has as they are, and has under the same id but different.
    pub new: Vec<AssetRef>,
    pub same: Vec<AssetRef>,
    pub differing: Vec<AssetRef>,
    /// Packs it needs that aren't installed (or only older).
    pub missing: Vec<Need>,
    /// The version of it that is installed.
    pub installed: Option<String>,
}

impl Preview {
    /// In a few words, for a list and for scripts.
    pub fn line(&self) -> String {
        let m = &self.manifest;
        let mut out = format!("{} {} ({}){}: {}", m.title, m.version, m.id, if m.author.is_empty() { String::new() } else { format!(" by {}", m.author) }, if self.holds.is_empty() { "nothing".to_string() } else { self.holds.join(", ") });
        out += &format!("; new {} same {} differing {}", self.new.len(), self.same.len(), self.differing.len());
        if !self.code.is_empty() {
            out += &format!("; runs: {}", self.code.join(", "));
        }
        if !self.missing.is_empty() {
            out += &format!("; needs {}", self.missing.iter().map(|n| if n.version.is_empty() { n.id.clone() } else { format!("{} {}", n.id, n.version) }).collect::<Vec<_>>().join(", "));
        }
        if let Some(v) = &self.installed {
            out += &format!("; installed: {v}");
        }
        out
    }
}

fn def_json(db: &rustpg2d_engine::defs::Database, kind: RefKind, id: &str) -> Option<serde_json::Value> {
    use rustpg2d_engine::defs::Def;
    rustpg2d_engine::with_def_kind!(kind, |T| T::list(db).iter().find(|d| d.id() == id).and_then(|d| serde_json::to_value(d).ok()), else => None)
}

fn read_record(root: &Path) -> Vec<Installed> {
    std::fs::read_to_string(root.join(RECORD)).ok().and_then(|t| ron::from_str(&t).ok()).unwrap_or_default()
}

fn write_record(root: &Path, list: &[Installed]) -> Result<(), String> {
    let text = rustpg2d_engine::project::ron_text(&list.to_vec()).map_err(|e| format!("{e:#}"))?;
    std::fs::write(root.join(RECORD), text).map_err(|e| format!("writing {RECORD}: {e}"))
}

/// Reads a source's index: a web address (`https://…/index.ron`), a folder with an `index.ron`,
/// or that file.
pub fn read_index(address: &str) -> Result<SourceIndex, String> {
    let address = address.trim();
    let text = if address.starts_with("http://") || address.starts_with("https://") {
        let mut resp = agent().get(address).call().map_err(|e| format!("can't reach {address}: {e}"))?;
        if resp.status().as_u16() != 200 {
            return Err(format!("{address} answered {}", resp.status().as_u16()));
        }
        resp.body_mut().with_config().limit(8 << 20).read_to_string().map_err(|e| format!("reading {address}: {e}"))?
    } else {
        let path = index_path(address);
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?
    };
    SourceIndex::from_text(&text).map_err(|e| format!("{address}: {e:#}"))
}

/// The index file of a folder source (the address is the folder, or the file itself).
fn index_path(address: &str) -> PathBuf {
    let p = PathBuf::from(address);
    if p.is_dir() {
        p.join(packs::INDEX_FILE)
    } else {
        p
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_connect(Some(std::time::Duration::from_secs(20))).http_status_as_error(false).build().into()
}

/// Fetches the pack of `entry` of the index at `address` and checks its size and SHA-256
/// against the index: anything else is refused.
pub fn download(entry: &IndexEntry, address: &str) -> Result<Vec<u8>, String> {
    let address = address.trim();
    let bytes = if address.starts_with("http://") || address.starts_with("https://") || entry.file.contains("://") {
        let url = entry.url(address);
        let mut resp = agent().get(&url).call().map_err(|e| format!("can't reach {url}: {e}"))?;
        if resp.status().as_u16() != 200 {
            return Err(format!("{url} answered {}", resp.status().as_u16()));
        }
        resp.body_mut().with_config().limit(1 << 30).read_to_vec().map_err(|e| format!("downloading {url}: {e}"))?
    } else {
        let path = index_path(address).parent().map(PathBuf::from).unwrap_or_default().join(&entry.file);
        std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?
    };
    entry.check(&bytes).map_err(|e| format!("{e:#}"))?;
    Ok(bytes)
}

impl Workspace {
    fn library_root(&mut self) -> Result<PathBuf, String> {
        self.lib.ensure_loaded();
        match &self.lib.lib {
            Some(lib) => Ok(lib.root.clone()),
            None => Err(self.lib.error.clone().unwrap_or_else(|| "the asset library isn't available".into())),
        }
    }

    /// The packs installed in the library, by title.
    pub fn installed_packs(&mut self) -> Vec<Installed> {
        let Ok(root) = self.library_root() else { return Vec::new() };
        let mut list = read_record(&root);
        list.sort_by_key(|a| a.manifest.title.to_lowercase());
        list
    }

    /// Where an installed pack's preview pictures are.
    pub fn pack_previews(&mut self, id: &str) -> Vec<PathBuf> {
        let Ok(root) = self.library_root() else { return Vec::new() };
        let dir = root.join("packs").join(id);
        read_record(&root).iter().find(|p| p.manifest.id == id).map(|p| p.manifest.previews.iter().map(|n| dir.join(n)).filter(|f| f.is_file()).collect()).unwrap_or_default()
    }

    /// What `bytes` (a pack, or a file of 0.35) is and what installing it would do.
    pub fn pack_preview(&mut self, bytes: &[u8]) -> Result<Preview, String> {
        let pack = Pack::from_bytes(bytes).map_err(|e| format!("{e:#}"))?;
        let root = self.library_root()?;
        let lib = self.lib.lib.as_ref().ok_or("the asset library isn't available")?;
        let (mut new, mut same, mut differing) = (Vec::new(), Vec::new(), Vec::new());
        // (A file the pack would write over a different one makes the assets that show it differ.)
        let changed_files: Vec<&String> = pack.assets.files.iter().filter(|(rel, bytes)| std::fs::read(root.join(rel)).is_ok_and(|have| have != **bytes)).map(|(rel, _)| rel).collect();
        for (kind, id) in library::all_assets(&pack.assets.db) {
            let (ours, theirs) = (def_json(&lib.db, kind, &id), def_json(&pack.assets.db, kind, &id));
            let files_differ = || theirs.as_ref().is_some_and(|t| changed_files.iter().any(|f| t.to_string().contains(&format!("\"{f}\""))));
            match ours {
                None => new.push((kind, id)),
                Some(o) if Some(&o) == theirs.as_ref() && !files_differ() => same.push((kind, id)),
                Some(_) => differing.push((kind, id)),
            }
        }
        let installed = read_record(&root);
        let missing = pack.manifest.needs.iter().filter(|n| !installed.iter().any(|p| p.manifest.id == n.id && (n.version.is_empty() || packs::compare_versions(&p.manifest.version, &n.version) != std::cmp::Ordering::Less))).cloned().collect();
        Ok(Preview { holds: packs::holds(&pack), code: pack.code(), new, same, differing, missing, installed: installed.iter().find(|p| p.manifest.id == pack.manifest.id).map(|p| p.manifest.version.clone()), manifest: pack.manifest })
    }

    /// Installs a pack into the asset library: its assets with their files and script
    /// functions, its plugins into the library's `plugins/`, its previews; `same` says what
    /// happens to assets the library has under the same id with other content. `source`: the
    /// index it came from ("" = a file). Returns what was done, in words.
    pub fn install_pack(&mut self, bytes: &[u8], same: SameId, source: &str) -> Result<String, String> {
        let pack = Pack::from_bytes(bytes).map_err(|e| format!("{e:#}"))?;
        pack.manifest.fits().map_err(|e| format!("{e:#}"))?;
        let preview = self.pack_preview(bytes)?;
        if !preview.missing.is_empty() {
            return Err(format!("{} needs {}: install that first", pack.manifest.title, preview.missing.iter().map(|n| if n.version.is_empty() { n.id.clone() } else { format!("{} {} or newer", n.id, n.version) }).collect::<Vec<_>>().join(" and ")));
        }
        let root = self.library_root()?;
        let lib = self.lib.lib.as_mut().ok_or("the asset library isn't available")?;
        let mut bundle = pack.assets.clone();
        bundle.rescale(rustpg2d_engine::project::TILE_SIZE).map_err(|e| format!("{e:#}"))?;
        let all = library::all_assets(&bundle.db);
        let report = match same {
            SameId::Keep => bundle.add_missing(&mut lib.db, &root),
            SameId::Replace => bundle.replace_into(&mut lib.db, &root),
            SameId::Copy => bundle.merge_into(&mut lib.db, &root),
        }
        .map_err(|e| format!("{e:#}"))?;
        lib.db.reindex();
        lib.save_defs().map_err(|e| format!("{e:#}"))?;
        // Its plugins lie in the library's folder, as the starter library's do (a project adds one from there).
        for p in &pack.plugins {
            let dir = rustpg2d_engine::plugins::plugin_dir(&root, &p.id);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("removing {}: {e}", dir.display()))?;
            }
            for (rel, b) in &p.files {
                let file = dir.join(rel);
                std::fs::create_dir_all(file.parent().unwrap_or(&dir)).map_err(|e| e.to_string())?;
                std::fs::write(&file, b).map_err(|e| format!("writing {}: {e}", file.display()))?;
            }
        }
        let pics = root.join("packs").join(&pack.manifest.id);
        let _ = std::fs::remove_dir_all(&pics);
        if !pack.previews.is_empty() {
            std::fs::create_dir_all(&pics).map_err(|e| e.to_string())?;
            for (name, b) in &pack.previews {
                std::fs::write(pics.join(name), b).map_err(|e| format!("writing the preview {name}: {e}"))?;
            }
        }
        // What it brought, under the ids it has in the library now (a copy has a new one).
        let now = |kind: RefKind, id: &str| report.renamed.iter().find(|(k, old, _)| *k == kind && old == id).map_or(id.to_string(), |(_, _, new)| new.clone());
        let assets: Vec<(String, String)> = all.iter().map(|(k, id)| (k.name().to_string(), now(*k, id))).collect();
        let mut manifest = pack.manifest.clone();
        manifest.previews = pack.previews.iter().map(|(n, _)| n.clone()).collect();
        let mut list = read_record(&root);
        let was = list.iter().find(|p| p.manifest.id == manifest.id).map(|p| p.manifest.version.clone());
        list.retain(|p| p.manifest.id != manifest.id);
        list.push(Installed { manifest: manifest.clone(), assets, plugins: pack.plugins.iter().map(|p| p.id.clone()).collect(), source: source.to_string() });
        write_record(&root, &list)?;
        self.lib.tex_dirty = true;
        let what = match was {
            Some(v) if v == manifest.version => format!("Installed {} {} again", manifest.title, manifest.version),
            Some(v) => format!("Updated {} from {v} to {}", manifest.title, manifest.version),
            None => format!("Installed {} {}", manifest.title, manifest.version),
        };
        let plugins = if pack.plugins.is_empty() { String::new() } else { format!("; plugins: {}", pack.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(", ")) };
        let said = format!("{what}: {}{plugins}", report.summary());
        self.set_status(said.clone());
        Ok(said)
    }

    /// Takes a pack off the list of installed ones (what it brought stays in the library: a
    /// project may use it).
    pub fn forget_pack(&mut self, id: &str) -> Result<(), String> {
        let root = self.library_root()?;
        let mut list = read_record(&root);
        if !list.iter().any(|p| p.manifest.id == id) {
            return Err(format!("no pack \"{id}\" is installed"));
        }
        list.retain(|p| p.manifest.id != id);
        let _ = std::fs::remove_dir_all(root.join("packs").join(id));
        write_record(&root, &list)
    }

    /// Puts assets of the project together as a pack: each with what it needs (pictures,
    /// sounds, models, the script functions its events call), `plugins` of the project by id,
    /// under `manifest`, with `previews` (name and PNG). Nothing is written.
    pub fn make_pack(&self, manifest: &PackManifest, assets: &[AssetRef], plugins: &[String], previews: Vec<(String, Vec<u8>)>) -> Result<Pack, String> {
        if let Some(p) = manifest.problem() {
            return Err(p);
        }
        if assets.is_empty() && plugins.is_empty() {
            return Err("nothing is picked to share".into());
        }
        let bundle = Bundle::collect_for_library(&self.project.db, &self.project.root, rustpg2d_engine::project::TILE_SIZE, &manifest.title, assets, false).map_err(|e| format!("{e:#}"))?;
        let mut packed = Vec::new();
        for id in plugins {
            let files = rustpg2d_engine::pack::read_dir_files(&rustpg2d_engine::plugins::plugin_dir(&self.project.root, id)).map_err(|e| format!("the plugin {id}: {e:#}"))?;
            if !files.iter().any(|(f, _)| f == rustpg2d_engine::plugins::PLUGIN_FILE) {
                return Err(format!("the project has no plugin \"{id}\""));
            }
            packed.push(packs::PackedPlugin { id: id.clone(), files });
        }
        let mut manifest = manifest.clone();
        manifest.format = packs::FORMAT;
        if manifest.engine.is_empty() {
            // (What it was made with: an older editor may not know everything in it.)
            let v = env!("CARGO_PKG_VERSION").split('-').next().unwrap_or_default();
            manifest.engine = v.rsplitn(2, '.').last().unwrap_or(v).to_string();
        }
        Ok(Pack { manifest, assets: bundle, plugins: packed, previews, history: None })
    }

    /// Pictures for a pack's card: of the first few of `assets` that have one (an item's
    /// icon, a prop's picture, a character's head), as PNGs. None without loaded art.
    pub fn pack_pictures(&self, assets: &[AssetRef]) -> Vec<(String, Vec<u8>)> {
        use rustpg2d_engine::imageio::Rgba;
        let db = &self.project.db;
        let of_image = |img: &gfx::texture::Image, r: gfx::math::Rect| -> Option<Vec<u8>> {
            let (x0, y0, w, h) = (r.x.max(0.0) as u32, r.y.max(0.0) as u32, r.w as u32, r.h as u32);
            if w == 0 || h == 0 || x0 + w > img.width as u32 || y0 + h > img.height as u32 {
                return None;
            }
            let mut out = Rgba::new(w, h);
            for y in 0..h {
                let from = ((y0 + y) * img.width as u32 + x0) as usize * 4;
                out.px[(y * w) as usize * 4..((y + 1) * w) as usize * 4].copy_from_slice(&img.bytes[from..from + w as usize * 4]);
            }
            out.encode().ok()
        };
        let sprite = |sp: &rustpg2d_engine::defs::SpriteRef| -> Option<Vec<u8>> {
            let sheet = self.assets.sheets.get(&sp.sheet)?;
            let (w, h) = sp.cells();
            of_image(&sheet.image, sheet.region(sp.frame, w, h))
        };
        let mut out = Vec::new();
        for (kind, id) in assets {
            let png = match kind {
                RefKind::Item => db.item(id).and_then(|d| sprite(&d.icon)),
                RefKind::Prop => db.prop(id).and_then(|d| sprite(&d.sprite)),
                RefKind::Sheet => sprite(&rustpg2d_engine::defs::SpriteRef::new(id, 0)),
                RefKind::Character if self.gpu != crate::Gpu::No => db.character(id).and_then(|c| self.assets.models.bust_image(db, &self.assets.sheets, &c.appearance)).and_then(|img| of_image(&img, gfx::math::Rect::new(0.0, 0.0, img.width as f32, img.height as f32))),
                _ => None,
            };
            if let Some(png) = png {
                out.push((format!("{}-{id}.png", kind.name()), png));
                if out.len() == 4 {
                    break;
                }
            }
        }
        out
    }

    /// *Share…*: writes the pack as `<id>-<version>.r2dpack` into the folder `out` and puts
    /// its entry into the `index.ron` there (made when there is none: the folder is then a
    /// source others can add, as it is or uploaded). Returns the pack's file.
    pub fn share_pack(&mut self, manifest: &PackManifest, assets: &[AssetRef], plugins: &[String], previews: Vec<(String, Vec<u8>)>, out: &Path) -> Result<PathBuf, String> {
        let pack = self.make_pack(manifest, assets, plugins, previews)?;
        let bytes = pack.to_bytes().map_err(|e| format!("{e:#}"))?;
        // (Read again before it leaves: what was written is a pack that installs.)
        Pack::from_bytes(&bytes).map_err(|e| format!("the pack that was written can't be read again: {e:#}"))?;
        std::fs::create_dir_all(out).map_err(|e| format!("making {}: {e}", out.display()))?;
        let file = out.join(pack.manifest.file_name());
        std::fs::write(&file, &bytes).map_err(|e| format!("writing {}: {e}", file.display()))?;
        let index_file = out.join(packs::INDEX_FILE);
        let mut index = match std::fs::read_to_string(&index_file) {
            Ok(text) => SourceIndex::from_text(&text).map_err(|e| format!("{}: {e:#}", index_file.display()))?,
            Err(_) => SourceIndex { title: if manifest.author.is_empty() { "Packs".into() } else { format!("{}'s packs", manifest.author) }, ..Default::default() },
        };
        index.put(IndexEntry::of(&pack, &bytes));
        std::fs::write(&index_file, index.to_text().map_err(|e| format!("{e:#}"))?).map_err(|e| format!("writing {}: {e}", index_file.display()))?;
        self.set_status(format!("Shared {} {} as {} ({} bytes), listed in {}", pack.manifest.title, pack.manifest.version, file.display(), bytes.len(), index_file.display()));
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    /// The example as the project, and an empty library of its own in a scratch folder.
    fn scratch(tag: &str) -> (Workspace, PathBuf) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-packs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir.join("game"));
        let lib = dir.join("library");
        std::fs::create_dir_all(&lib).unwrap();
        crate::files::create_project(&lib).unwrap();
        let mut ws = Workspace::new(dir.join("game"), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&lib).unwrap());
        (ws, dir)
    }

    fn manifest(title: &str, version: &str) -> PackManifest {
        PackManifest { author: "Anna".into(), licence: "CC BY 4.0".into(), attribution: "Market things by Anna".into(), version: version.into(), ..PackManifest::new(title) }
    }

    #[test]
    fn a_selection_is_shared_as_a_pack_and_installed_into_a_library() {
        let (mut ws, dir) = scratch("share");
        let out = dir.join("shared");
        // The example's chest and its guard, shared: each comes with what it needs.
        let file = ws.share_pack(&manifest("Market things", "1.0"), &[(RefKind::Prop, "chest".into()), (RefKind::Character, "guard".into())], &[], vec![("card.png".into(), vec![1, 2, 3])], &out).unwrap();
        assert_eq!(file, out.join("market-things-1.0.r2dpack"));
        let bytes = std::fs::read(&file).unwrap();
        let pack = Pack::from_bytes(&bytes).unwrap();
        assert!(pack.assets.db.prop("chest").is_some() && pack.assets.db.character("guard").is_some());
        let model = pack.assets.db.character("guard").unwrap().appearance.model.clone();
        assert!(pack.assets.db.model(&model).is_some_and(|m| pack.assets.files.contains_key(&m.file)), "the guard's model and its file came along");
        assert!(!pack.manifest.engine.is_empty() && pack.manifest.fits().is_ok());
        // The folder is a source: its index names the pack with its size and SHA-256.
        let index = read_index(&out.display().to_string()).unwrap();
        assert_eq!((index.title.as_str(), index.packs.len(), index.packs[0].size), ("Anna's packs", 1, bytes.len() as u64));
        assert_eq!(download(&index.packs[0], &out.display().to_string()).unwrap(), bytes);
        // Before installing: what it is, what it holds, that everything in it is new here.
        let p = ws.pack_preview(&bytes).unwrap();
        assert_eq!((p.manifest.title.as_str(), p.installed.clone(), p.same.len(), p.differing.len(), p.missing.len()), ("Market things", None, 0, 0, 0));
        assert!(p.new.contains(&(RefKind::Prop, "chest".to_string())) && p.holds.iter().any(|h| h.ends_with("props") || h == "1 prop"), "{:?}", p.holds);
        assert!(p.line().starts_with("Market things 1.0 (market-things) by Anna: "), "{}", p.line());
        // Installed: the library has the assets and their files, and knows the pack.
        let said = ws.install_pack(&bytes, SameId::Keep, &out.display().to_string()).unwrap();
        assert!(said.starts_with("Installed Market things 1.0"), "{said}");
        let lib = ws.lib.lib.as_ref().unwrap();
        assert!(lib.db.prop("chest").is_some() && lib.db.character("guard").is_some() && lib.db.model(&model).is_some_and(|m| lib.path(&m.file).is_file()));
        let packs = ws.installed_packs();
        assert_eq!((packs.len(), packs[0].manifest.id.as_str(), packs[0].source.as_str()), (1, "market-things", out.to_str().unwrap()));
        assert!(packs[0].assets.contains(&("prop".to_string(), "chest".to_string())));
        assert_eq!(ws.pack_previews("market-things").len(), 1);
        // Read from its files again, the library is what was installed.
        let again = rustpg2d_engine::project::Project::load(&dir.join("library")).unwrap();
        assert!(again.db.prop("chest").is_some() && read_record(&again.root).len() == 1);
        // The same pack again: everything in it is there as it is.
        let p = ws.pack_preview(&bytes).unwrap();
        assert_eq!((p.new.len(), p.differing.len(), p.installed.as_deref()), (0, 0, Some("1.0")));
        assert!(!p.same.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_newer_version_updates_and_the_same_id_is_kept_replaced_or_copied() {
        let (mut ws, dir) = scratch("update");
        let out = dir.join("shared");
        let chest = [(RefKind::Prop, "chest".to_string())];
        let v1 = std::fs::read(ws.share_pack(&manifest("Chests", "1.0"), &chest, &[], Vec::new(), &out).unwrap()).unwrap();
        ws.install_pack(&v1, SameId::Keep, "").unwrap();
        // The project's chest changes and is shared as version 1.1: the index offers that one.
        let mut value = ws.def_value(RefKind::Prop, "chest").unwrap();
        value["name"] = "Sea chest".into();
        ws.set_def_value(RefKind::Prop, "chest", value).unwrap();
        let v2 = std::fs::read(ws.share_pack(&manifest("Chests", "1.1"), &chest, &[], Vec::new(), &out).unwrap()).unwrap();
        let index = read_index(&out.display().to_string()).unwrap();
        assert_eq!(index.newest().iter().map(|e| e.manifest.version.as_str()).collect::<Vec<_>>(), ["1.1"]);
        let p = ws.pack_preview(&v2).unwrap();
        assert_eq!((p.installed.as_deref(), p.differing.as_slice()), (Some("1.0"), &chest[..]), "the chest differs from the library's");
        // Kept: the library's chest stays as it was. Replaced: it is the pack's.
        ws.install_pack(&v2, SameId::Keep, "").unwrap();
        assert_eq!(ws.lib.lib.as_ref().unwrap().db.prop("chest").unwrap().name, "Chest");
        let said = ws.install_pack(&v2, SameId::Replace, "").unwrap();
        assert!(said.starts_with("Installed Chests 1.1 again"), "{said}");
        assert_eq!(ws.lib.lib.as_ref().unwrap().db.prop("chest").unwrap().name, "Sea chest");
        // The first version again, as a copy: beside it under a new id, and the pack knows which.
        let said = ws.install_pack(&v1, SameId::Copy, "").unwrap();
        assert!(said.starts_with("Updated Chests from 1.1 to 1.0"), "{said}");
        let lib = ws.lib.lib.as_ref().unwrap();
        let copy = lib.db.props.iter().find(|p| p.id != "chest" && p.name == "Chest").expect("the copy").id.clone();
        assert_eq!(lib.db.prop("chest").unwrap().name, "Sea chest");
        assert!(ws.installed_packs()[0].assets.contains(&("prop".to_string(), copy)));
        // A pack that needs another says so and installs nothing; a changed download is refused.
        let needy = PackManifest { needs: vec![Need { id: "not-there".into(), version: "2.0".into() }], ..manifest("Needy", "1.0") };
        let bytes = ws.make_pack(&needy, &chest, &[], Vec::new()).unwrap().to_bytes().unwrap();
        assert!(ws.install_pack(&bytes, SameId::Keep, "").unwrap_err().contains("needs not-there 2.0 or newer"));
        assert_eq!(ws.installed_packs().len(), 1);
        std::fs::write(out.join("chests-1.1.r2dpack"), b"something else").unwrap();
        assert!(download(index.newest()[0], &out.display().to_string()).is_err());
        // Forgotten, the pack is off the list; what it brought stays.
        ws.forget_pack("chests").unwrap();
        assert!(ws.installed_packs().is_empty() && ws.lib.lib.as_ref().unwrap().db.prop("chest").is_some());
        assert!(ws.make_pack(&manifest("Empty", "1.0"), &[], &[], Vec::new()).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
