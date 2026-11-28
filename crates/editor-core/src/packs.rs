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
use rustpg2d_engine::starter_update::{self, StarterHistory, StarterUpdate};
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
    /// For a pack that is installed: what this version changes, asset by asset: what it adds,
    /// what it updates (the library has it as an earlier version of the pack shipped it), what
    /// the creator changed since (replaced only when chosen), what the pack no longer has
    /// (deleted only when chosen).
    pub update: Option<StarterUpdate>,
    /// Its preview pictures (PNG).
    pub pictures: Vec<Vec<u8>>,
}

/// For a project's update from a pack: the pack as installed, the versions the project's
/// assets came as, those assets as the library has them, and the pack's history.
type UpdateParts = (Installed, Vec<String>, Bundle, StarterHistory);

/// Assets of the project that came from a pack of which the library has another version now.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectUpdate {
    pub pack: String,
    pub title: String,
    /// The versions the project's assets came as, and the one the library has.
    pub from: Vec<String>,
    pub to: String,
    /// What taking the library's version does: what it updates (the project has those as the
    /// pack shipped them), what was changed in the project too (kept unless chosen), what the
    /// updated ones need that the project doesn't have yet. (Nothing is ever deleted.)
    pub plan: StarterUpdate,
}

impl ProjectUpdate {
    pub fn line(&self) -> String {
        let names = |v: &[AssetRef]| v.iter().map(|a| a.1.as_str()).collect::<Vec<_>>().join(" ");
        format!("{} ({}) {} -> {}: updates {} [{}] changed in the project {} [{}] adds {}", self.title, self.pack, self.from.join("/"), self.to, self.plan.updated.len(), names(&self.plan.updated), self.plan.edited.len(), names(&self.plan.edited), self.plan.added.len())
    }
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
        if let Some(u) = &self.update {
            let names = |v: &[AssetRef]| v.iter().map(|a| a.1.as_str()).collect::<Vec<_>>().join(" ");
            out += &format!("; update: adds {} updates {} [{}] changed by you {} [{}] retired {} [{}]", u.added.len(), u.updated.len(), names(&u.updated), u.edited.len(), names(&u.edited), u.retired.len(), u.retired.iter().map(|r| r.asset.1.as_str()).collect::<Vec<_>>().join(" "));
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

/// What a pack shipped in the versions that were installed here (and those its own history
/// names): `packs/<id>/history.ron` in the library. An update tells by it which of the
/// library's assets are still as the pack shipped them.
fn history_path(root: &Path, id: &str) -> PathBuf {
    root.join("packs").join(id).join("history.ron")
}

fn read_history(root: &Path, id: &str) -> StarterHistory {
    std::fs::read_to_string(history_path(root, id)).ok().and_then(|t| StarterHistory::parse(&t).ok()).unwrap_or_default()
}

/// The history an update of `pack` is judged by: what was installed here, and what the pack
/// says it shipped before.
fn history_for(root: &Path, pack: &Pack) -> StarterHistory {
    let mut h = read_history(root, &pack.manifest.id);
    if let Some(own) = pack.history.as_deref().and_then(|t| StarterHistory::parse(t).ok()) {
        h.merge(&own);
    }
    h
}

/// The pack a library asset is the work of, when several packs brought one under that id:
/// the one that shipped the asset as the library has it now (of several that did, the one
/// installed last), else the one installed last.
pub(crate) fn owner_of<'a>(installed: &'a [Installed], lib: &rustpg2d_engine::defs::Database, root: &Path, asset: &AssetRef) -> Option<&'a Installed> {
    let claim: Vec<&Installed> = installed.iter().filter(|p| p.assets.iter().any(|(k, i)| k == asset.0.name() && *i == asset.1)).collect();
    match claim.as_slice() {
        [] => None,
        [one] => Some(one),
        // (Two that shipped the very same thing: the one that came in last.)
        many => many.iter().rev().find(|p| read_history(root, &p.manifest.id).untouched(lib, root, asset)).or(many.last()).copied(),
    }
}

/// The packs installed in the library at `root`.
pub(crate) fn installed_at(root: &Path) -> Vec<Installed> {
    read_record(root)
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
        let was = installed.iter().find(|p| p.manifest.id == pack.manifest.id).map(|p| p.manifest.version.clone());
        // An update is judged asset by asset. So is the starter library coming in as a pack
        // for the first time: a library has its assets already (they came by themselves
        // until 0.36), and its history knows every version of them that was shipped.
        let update = match was.as_ref().map(|_| ()).or_else(|| (pack.manifest.id == packs::STARTER_ID && pack.history.is_some()).then_some(())) {
            Some(()) => {
                let mut bundle = pack.assets.clone();
                bundle.rescale(rustpg2d_engine::project::TILE_SIZE).map_err(|e| format!("{e:#}"))?;
                Some(starter_update::plan(lib, &bundle, &history_for(&root, &pack)))
            }
            None => None,
        };
        Ok(Preview { holds: packs::holds(&pack), code: pack.code(), new, same, differing, missing, installed: was, update, pictures: pack.previews.iter().map(|(_, b)| b.clone()).collect(), manifest: pack.manifest })
    }

    /// Installs a pack into the asset library: its assets with their files and script
    /// functions, its plugins into the library's `plugins/`, its previews; `same` says what
    /// happens to assets the library has under the same id with other content. `source`: the
    /// index it came from ("" = a file). A pack that is installed already is updated instead
    /// ([`Self::update_pack`]: *replace* takes the pack's version of what the creator changed
    /// too, *keep* leaves those). Returns what was done, in words.
    pub fn install_pack(&mut self, bytes: &[u8], same: SameId, source: &str) -> Result<String, String> {
        let pack = Pack::from_bytes(bytes).map_err(|e| format!("{e:#}"))?;
        pack.manifest.fits().map_err(|e| format!("{e:#}"))?;
        let preview = self.pack_preview(bytes)?;
        if !preview.missing.is_empty() {
            return Err(format!("{} needs {}: install that first", pack.manifest.title, preview.missing.iter().map(|n| if n.version.is_empty() { n.id.clone() } else { format!("{} {} or newer", n.id, n.version) }).collect::<Vec<_>>().join(" and ")));
        }
        if let (Some(update), true) = (&preview.update, same != SameId::Copy) {
            let take = if same == SameId::Replace { update.edited.clone() } else { Vec::new() };
            return self.update_pack(bytes, &take, &[], source);
        }
        let root = self.library_root()?;
        // What the library has differently is overwritten: a copy of the library first.
        let backup = if same == SameId::Replace && !preview.differing.is_empty() { Some(starter_update::backup(&root).map_err(|e| format!("{e:#}"))?) } else { None };
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
        // What it brought, under the ids it has in the library now (a copy has a new one).
        let now = |kind: RefKind, id: &str| report.renamed.iter().find(|(k, old, _)| *k == kind && old == id).map_or(id.to_string(), |(_, _, new)| new.clone());
        let assets: Vec<(String, String)> = all.iter().map(|(k, id)| (k.name().to_string(), now(*k, id))).collect();
        let what = self.installed(&pack, assets, source)?;
        let said = format!("{what}: {}{}", report.summary(), backup.map(|b| format!(". Backup: {}", b.display())).unwrap_or_default());
        self.set_status(said.clone());
        Ok(said)
    }

    /// Updates a pack that is installed, asset by asset (after a backup of the library's
    /// folder): what the new version adds comes in; what the library still has as an earlier
    /// version shipped it is replaced; what the creator changed stays unless it is in
    /// `take_edited`; what the pack no longer has stays unless it is in `delete` (and nothing
    /// else needs it). Returns what was done, in words.
    pub fn update_pack(&mut self, bytes: &[u8], take_edited: &[AssetRef], delete: &[AssetRef], source: &str) -> Result<String, String> {
        let pack = Pack::from_bytes(bytes).map_err(|e| format!("{e:#}"))?;
        pack.manifest.fits().map_err(|e| format!("{e:#}"))?;
        let root = self.library_root()?;
        let record = read_record(&root);
        // (The starter library's first time as a pack is an update of what the library has of it.)
        let was: Vec<(String, String)> = match record.iter().find(|p| p.manifest.id == pack.manifest.id) {
            Some(p) => p.assets.clone(),
            None if pack.manifest.id == packs::STARTER_ID && pack.history.is_some() => Vec::new(),
            None => return Err(format!("{} isn't installed: install it first", pack.manifest.title)),
        };
        let history = history_for(&root, &pack);
        let lib = self.lib.lib.as_mut().ok_or("the asset library isn't available")?;
        let mut bundle = pack.assets.clone();
        bundle.rescale(rustpg2d_engine::project::TILE_SIZE).map_err(|e| format!("{e:#}"))?;
        let plan = starter_update::plan(lib, &bundle, &history);
        // (Nothing to bring in, replace or delete: the library is left as it is, without a copy.)
        let idle = plan.added.is_empty() && plan.updated.is_empty() && !take_edited.iter().any(|a| plan.edited.contains(a)) && !delete.iter().any(|a| plan.retired.iter().any(|r| r.asset == *a));
        let summary = if idle {
            if plan.edited.is_empty() { "the library has it as it is".to_string() } else { format!("nothing to change; kept your version of {}", plan.edited.iter().map(|a| a.1.as_str()).collect::<Vec<_>>().join(", ")) }
        } else {
            starter_update::apply(lib, &bundle, &plan, &history, take_edited, delete).map_err(|e| format!("{e:#}"))?.summary()
        };
        lib.db.reindex();
        // What is the pack's in the library now: what this version has, and what it retired but is kept.
        let mut assets: Vec<(String, String)> = library::all_assets(&bundle.db).iter().map(|(k, id)| (k.name().to_string(), id.clone())).collect();
        for old in &was {
            let kept = RefKind::parse(&old.0).is_some_and(|k| lib.db.has(k, &old.1));
            if kept && !assets.contains(old) {
                assets.push(old.clone());
            }
        }
        let what = self.installed(&pack, assets, source)?;
        let said = format!("{what}: {summary}");
        self.set_status(said.clone());
        Ok(said)
    }

    /// After a pack's assets are in the library: its plugins, its preview pictures, what it
    /// shipped (for the next update) and its place on the list. Returns "Installed …" /
    /// "Updated … from … to …".
    fn installed(&mut self, pack: &Pack, assets: Vec<(String, String)>, source: &str) -> Result<String, String> {
        let root = self.library_root()?;
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
        // (Its history is taken out of its folder before the folder's pictures are swapped.)
        let mut history = history_for(&root, pack);
        let mut bundle = pack.assets.clone();
        bundle.rescale(rustpg2d_engine::project::TILE_SIZE).map_err(|e| format!("{e:#}"))?;
        history.record(&bundle);
        let pics = root.join("packs").join(&pack.manifest.id);
        let _ = std::fs::remove_dir_all(&pics);
        std::fs::create_dir_all(&pics).map_err(|e| e.to_string())?;
        for (name, b) in &pack.previews {
            std::fs::write(pics.join(name), b).map_err(|e| format!("writing the preview {name}: {e}"))?;
        }
        std::fs::write(history_path(&root, &pack.manifest.id), history.to_ron().map_err(|e| format!("{e:#}"))?).map_err(|e| format!("writing the pack's history: {e}"))?;
        let mut manifest = pack.manifest.clone();
        manifest.previews = pack.previews.iter().map(|(n, _)| n.clone()).collect();
        let mut list = read_record(&root);
        let was = list.iter().find(|p| p.manifest.id == manifest.id).map(|p| p.manifest.version.clone());
        list.retain(|p| p.manifest.id != manifest.id);
        list.push(Installed { manifest: manifest.clone(), assets, plugins: pack.plugins.iter().map(|p| p.id.clone()).collect(), source: source.to_string() });
        write_record(&root, &list)?;
        self.lib.tex_dirty = true;
        let plugins = if pack.plugins.is_empty() { String::new() } else { format!(" (plugins: {})", pack.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(", ")) };
        Ok(match was {
            Some(v) if v == manifest.version => format!("Installed {} {} again{plugins}", manifest.title, manifest.version),
            Some(v) if packs::compare_versions(&manifest.version, &v) == std::cmp::Ordering::Less => format!("Went back with {} from {v} to {}{plugins}", manifest.title, manifest.version),
            Some(v) => format!("Updated {} from {v} to {}{plugins}", manifest.title, manifest.version),
            None => format!("Installed {} {}{plugins}", manifest.title, manifest.version),
        })
    }

    /// What the library has of pack `pack` for the project's assets that came from it as
    /// another version: those assets as the library has them now, and the pack's history.
    fn project_update_parts(&mut self, pack: &str) -> Result<Option<UpdateParts>, String> {
        let root = self.library_root()?;
        let Some(installed) = read_record(&root).into_iter().find(|p| p.manifest.id == pack) else { return Ok(None) };
        let lib = self.lib.lib.as_ref().ok_or("the asset library isn't available")?;
        let mut from = Vec::new();
        let mut wanted = Vec::new();
        for (key, m) in &self.meta.assets {
            let Some(o) = m.origin.as_ref().filter(|o| o.pack == pack && o.version != installed.manifest.version) else { continue };
            let Some((kind, id)) = key.split_once(':').and_then(|(k, id)| Some((RefKind::parse(k)?, id))) else { continue };
            // (Under the id it has in the pack: a copy under another id is the project's own.)
            if self.project.db.has(kind, id) && lib.db.has(kind, id) && installed.assets.iter().any(|(k, i)| k == kind.name() && i == id) {
                wanted.push((kind, id.to_string()));
                if !from.contains(&o.version) {
                    from.push(o.version.clone());
                }
            }
        }
        if wanted.is_empty() {
            return Ok(None);
        }
        let mut bundle = Bundle::collect_with_needed_scripts(&lib.db, &lib.root, rustpg2d_engine::project::TILE_SIZE, &installed.manifest.title, &wanted).map_err(|e| format!("{e:#}"))?;
        bundle.rescale(rustpg2d_engine::project::TILE_SIZE).map_err(|e| format!("{e:#}"))?;
        from.sort_by(|a, b| packs::compare_versions(a, b));
        Ok(Some((installed, from, bundle, read_history(&root, pack))))
    }

    /// The packs whose assets in the project are another version than the library has.
    /// Assets the project has exactly as the library does are noted as that version on the way.
    pub fn project_updates(&mut self) -> Vec<ProjectUpdate> {
        let packs: Vec<String> = self.packs_used().into_iter().map(|p| p.0).collect();
        let mut out = Vec::new();
        let mut settled = false;
        for pack in packs {
            let Ok(Some((installed, from, bundle, history))) = self.project_update_parts(&pack) else { continue };
            let mut plan = starter_update::plan(&self.project, &bundle, &history);
            plan.retired.clear();
            if plan.updated.is_empty() && plan.edited.is_empty() {
                // (What the project has of it is as the library has it: only the note is old.)
                for (kind, id) in library::all_assets(&bundle.db) {
                    if let Some(o) = self.meta.assets.get_mut(&crate::meta::Meta::key(kind, &id)).and_then(|m| m.origin.as_mut()).filter(|o| o.pack == pack) {
                        o.version = installed.manifest.version.clone();
                        settled = true;
                    }
                }
                continue;
            }
            out.push(ProjectUpdate { pack, title: installed.manifest.title.clone(), from, to: installed.manifest.version.clone(), plan });
        }
        if settled {
            self.save_meta();
        }
        out
    }

    /// Takes the library's version of a pack's assets into the project: those the project
    /// has as the pack shipped them, and of those changed in the project the ones in
    /// `take_edited`; what they need and the project lacks comes along. One step of the
    /// history. Pictures, sounds and models that are overwritten are kept under
    /// `editor/backup/<time>/` first. Returns what was done.
    pub fn update_project_from(&mut self, pack: &str, take_edited: &[AssetRef]) -> Result<String, String> {
        let Some((installed, _, bundle, history)) = self.project_update_parts(pack)? else { return Err(format!("the project has nothing older of the pack \"{pack}\"")) };
        let mut plan = starter_update::plan(&self.project, &bundle, &history);
        plan.retired.clear();
        // What will be overwritten is put aside (an undo brings the definitions back, not the files).
        let root = self.project.root.clone();
        let (y, mo, d, h, mi, sec) = starter_update::utc_now();
        let aside = root.join("editor").join("backup").join(format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{sec:02}"));
        let mut kept_files = 0;
        for a in plan.updated.iter().chain(take_edited.iter().filter(|a| plan.edited.contains(a))) {
            for rel in starter_update::own_files(&bundle.db, a) {
                let (have, new) = (std::fs::read(root.join(&rel)).ok(), bundle.files.get(&rel));
                if let (Some(have), Some(new)) = (have, new) {
                    if have != *new {
                        let to = aside.join(&rel);
                        std::fs::create_dir_all(to.parent().unwrap_or(&aside)).map_err(|e| e.to_string())?;
                        std::fs::write(&to, have).map_err(|e| format!("keeping {rel}: {e}"))?;
                        kept_files += 1;
                    }
                }
            }
        }
        let report = starter_update::apply_in_place(&mut self.project, &bundle, &plan, &history, take_edited, &[]).map_err(|e| format!("{e:#}"))?;
        self.project.db.reindex();
        // Everything of the pack that was looked at is this version's now (what was kept, by choice).
        for (kind, id) in library::all_assets(&bundle.db) {
            let entry = self.meta.assets.entry(crate::meta::Meta::key(kind, &id)).or_default();
            if entry.origin.as_ref().is_none_or(|o| o.pack == pack) {
                entry.origin = Some(crate::meta::Origin { pack: pack.to_string(), version: installed.manifest.version.clone() });
            }
        }
        let m = &installed.manifest;
        self.meta.packs.insert(m.id.clone(), crate::meta::PackCredit { title: m.title.clone(), author: m.author.clone(), licence: m.licence.clone(), attribution: m.attribution.clone() });
        self.save_meta();
        self.defs_dirty = true;
        self.begin(&format!("Update the project's assets of {} to {}", m.title, m.version));
        self.after_scripts_merged();
        self.reload_assets();
        self.refresh_refs();
        self.stale = true;
        let said = format!("{} {}: {}{}", m.title, m.version, report.summary(), if kept_files > 0 { format!(". The {kept_files} file(s) that were overwritten are in {}", aside.display()) } else { String::new() });
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
                RefKind::Character if self.gpu != crate::Gpu::No => db.character(id).and_then(|c| self.assets.models.bust_image(db, &c.appearance)).and_then(|img| of_image(&img, gfx::math::Rect::new(0.0, 0.0, img.width as f32, img.height as f32))),
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
        let mut pack = self.make_pack(manifest, assets, plugins, previews)?;
        // What earlier versions written into this folder shipped goes along: who updates from
        // any of them is told what they changed themselves.
        let history_file = out.join(format!("{}.history.ron", pack.manifest.id));
        let mut history = std::fs::read_to_string(&history_file).ok().and_then(|t| StarterHistory::parse(&t).ok()).unwrap_or_default();
        history.record(&pack.assets);
        let history_text = history.to_ron().map_err(|e| format!("{e:#}"))?;
        pack.history = Some(history_text.clone());
        let bytes = pack.to_bytes().map_err(|e| format!("{e:#}"))?;
        // (Read again before it leaves: what was written is a pack that installs.)
        Pack::from_bytes(&bytes).map_err(|e| format!("the pack that was written can't be read again: {e:#}"))?;
        std::fs::create_dir_all(out).map_err(|e| format!("making {}: {e}", out.display()))?;
        let file = out.join(pack.manifest.file_name());
        std::fs::write(&file, &bytes).map_err(|e| format!("writing {}: {e}", file.display()))?;
        std::fs::write(&history_file, history_text).map_err(|e| format!("writing {}: {e}", history_file.display()))?;
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
        PackManifest { author: "Anna".into(), licence: "CC BY 4.0".into(), attribution: format!("{title} by Anna"), version: version.into(), ..PackManifest::new(title) }
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
        // Added to a project from there, an asset is remembered as that pack's, with the
        // version it came as; so is a copy of it. The pack's credit line is offered for the
        // game's credits, once.
        let game = dir.join("other");
        std::fs::create_dir_all(&game).unwrap();
        crate::files::create_project(&game).unwrap();
        let lib = ws.lib.lib.take();
        let mut other = Workspace::new(game.clone(), crate::Gpu::No).unwrap();
        other.lib.lib = lib;
        other.history_frame(None);
        other.library_to_project(&[(RefKind::Prop, "chest".into())]).unwrap();
        other.history_frame(None);
        assert_eq!(other.meta.of(RefKind::Prop, "chest").and_then(|m| m.origin.clone()), Some(crate::meta::Origin { pack: "market-things".into(), version: "1.0".into() }));
        other.duplicate_def(RefKind::Prop, "chest", "strongbox").unwrap();
        other.history_frame(None);
        assert_eq!(other.meta.of(RefKind::Prop, "strongbox").and_then(|m| m.origin.as_ref()).map(|o| o.pack.as_str()), Some("market-things"));
        let used = other.packs_used();
        assert_eq!((used.len(), used[0].0.as_str(), used[0].1.licence.as_str()), (1, "market-things", "CC BY 4.0"));
        assert!(used[0].2 >= 2, "the chest, its copy and what the chest needs: {}", used[0].2);
        assert_eq!(other.credits_missing(), ["Market things by Anna"]);
        assert_eq!(other.add_pack_credits(), 1);
        assert!(other.project.config.credits.ends_with("Market things by Anna") && other.credits_missing().is_empty() && other.add_pack_credits() == 0);
        // Saved and read again, the project still knows; with the pack's assets deleted it forgets.
        other.save_all();
        let meta = crate::meta::Meta::load(&game);
        assert!(meta.packs.contains_key("market-things") && meta.assets.values().any(|m| m.origin.is_some()));
        ws.lib.lib = other.lib.lib.take();
        // The same pack again: everything in it is there as it is.
        let p = ws.pack_preview(&bytes).unwrap();
        assert_eq!((p.new.len(), p.differing.len(), p.installed.as_deref()), (0, 0, Some("1.0")));
        assert!(!p.same.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_update_replaces_what_is_untouched_and_asks_about_what_was_changed_or_retired() {
        let (mut ws, dir) = scratch("update");
        let out = dir.join("shared");
        let prop = |id: &str| (RefKind::Prop, id.to_string());
        // Version 1.0 has a chest and a barrel.
        let v1 = std::fs::read(ws.share_pack(&manifest("Chests", "1.0"), &[prop("chest"), prop("barrel")], &[], Vec::new(), &out).unwrap()).unwrap();
        ws.install_pack(&v1, SameId::Keep, "").unwrap();
        // Version 1.1: the chest has another name, a sign is new, the barrel is no longer in it.
        let mut value = ws.def_value(RefKind::Prop, "chest").unwrap();
        value["name"] = "Sea chest".into();
        ws.set_def_value(RefKind::Prop, "chest", value).unwrap();
        let v2 = std::fs::read(ws.share_pack(&manifest("Chests", "1.1"), &[prop("chest"), prop("sign")], &[], Vec::new(), &out).unwrap()).unwrap();
        assert!(Pack::from_bytes(&v2).unwrap().history.is_some(), "it carries what 1.0 shipped");
        let index = read_index(&out.display().to_string()).unwrap();
        assert_eq!(index.newest().iter().map(|e| e.manifest.version.as_str()).collect::<Vec<_>>(), ["1.1"]);
        // Looked at: the chest is updated (the library has it as 1.0 shipped it), the sign is
        // added, the barrel retired; nothing was changed here.
        let u = ws.pack_preview(&v2).unwrap().update.expect("it is installed: this is an update");
        assert!(u.updated.contains(&prop("chest")) && u.added.contains(&prop("sign")) && u.edited.is_empty(), "{u:?}");
        assert!(u.retired.iter().any(|r| r.asset == prop("barrel") && r.untouched));
        // The creator changes the library's chest: now it is theirs, and an update keeps it.
        {
            let lib = ws.lib.lib.as_mut().unwrap();
            lib.db.props.iter_mut().find(|p| p.id == "chest").unwrap().name = "My chest".into();
            lib.save_defs().unwrap();
        }
        let p = ws.pack_preview(&v2).unwrap();
        let u = p.update.as_ref().unwrap();
        assert!(u.edited.contains(&prop("chest")) && !u.updated.contains(&prop("chest")));
        assert!(p.line().contains("changed by you 1 [chest]"), "{}", p.line());
        let said = ws.install_pack(&v2, SameId::Keep, "").unwrap();
        assert!(said.starts_with("Updated Chests from 1.0 to 1.1") && said.contains("kept your version of chest") && said.contains("Backup: "), "{said}");
        let lib = ws.lib.lib.as_ref().unwrap();
        assert_eq!(lib.db.prop("chest").unwrap().name, "My chest");
        assert!(lib.db.prop("sign").is_some() && lib.db.prop("barrel").is_some(), "the sign came, the barrel stays until it is asked to go");
        assert!(dir.read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("library.bak-")), "the library was copied first");
        // A project that took the chest when the library had 1.0 is told that the library has
        // 1.1 now; nothing in it is touched before it says so.
        ws.lib.lib.as_mut().unwrap().db.props.iter_mut().find(|p| p.id == "chest").unwrap().name = "My chest".into();
        let game = dir.join("other");
        std::fs::create_dir_all(&game).unwrap();
        crate::files::create_project(&game).unwrap();
        let mut other = Workspace::new(game.clone(), crate::Gpu::No).unwrap();
        other.history_frame(None);
        // (As it was when the project took it: the library's chest of 1.0.)
        {
            let lib = ws.lib.lib.as_mut().unwrap();
            lib.db.props.iter_mut().find(|p| p.id == "chest").unwrap().name = "Chest".into();
            let mut record = read_record(&lib.root);
            record[0].manifest.version = "1.0".into();
            write_record(&lib.root, &record).unwrap();
        }
        other.lib.lib = ws.lib.lib.take();
        other.library_to_project(&[prop("chest")]).unwrap();
        other.history_frame(None);
        assert!(other.project_updates().is_empty(), "the project has the library's version");
        // The library goes on to 1.1 (the pack's chest): the project's chest is older, and untouched.
        other.update_pack(&v2, &[prop("chest")], &[], "").unwrap();
        let ups = other.project_updates();
        assert_eq!(ups.len(), 1, "{ups:?}");
        assert_eq!((ups[0].pack.as_str(), ups[0].from.as_slice(), ups[0].to.as_str(), ups[0].plan.updated.as_slice()), ("chests", &["1.0".to_string()][..], "1.1", &[prop("chest")][..]));
        assert_eq!(other.project.db.prop("chest").unwrap().name, "Chest");
        let said = other.update_project_from("chests", &[]).unwrap();
        other.history_frame(None);
        assert!(said.starts_with("Chests 1.1: ") && said.contains("1 replaced"), "{said}");
        assert_eq!(other.project.db.prop("chest").unwrap().name, "Sea chest");
        assert_eq!(other.meta.of(RefKind::Prop, "chest").and_then(|m| m.origin.as_ref()).map(|o| o.version.as_str()), Some("1.1"));
        assert!(other.project_updates().is_empty() && other.timeline.done.last().is_some_and(|c| c.label.contains("Update the project's assets of Chests to 1.1")));
        // It is a step: undone, the project has its chest of 1.0 again.
        other.undo().unwrap();
        other.history_frame(None);
        assert_eq!(other.project.db.prop("chest").unwrap().name, "Chest");
        ws.lib.lib = other.lib.lib.take();
        // (Back to where the library was: its own chest, the pack at 1.1 with the creator's version kept.)
        ws.lib.lib.as_mut().unwrap().db.props.iter_mut().find(|p| p.id == "chest").unwrap().name = "My chest".into();
        ws.lib.lib.as_mut().unwrap().save_defs().unwrap();
        // The pack's version of the chest is taken after all, and the barrel deleted.
        let said = ws.update_pack(&v2, &[prop("chest")], &[prop("barrel")], "").unwrap();
        assert!(said.starts_with("Installed Chests 1.1 again"), "{said}");
        let lib = ws.lib.lib.as_ref().unwrap();
        assert_eq!(lib.db.prop("chest").unwrap().name, "Sea chest");
        assert!(lib.db.prop("barrel").is_none());
        let packs = ws.installed_packs();
        assert!(packs[0].assets.contains(&("prop".to_string(), "sign".to_string())) && !packs[0].assets.contains(&("prop".to_string(), "barrel".to_string())));
        // The library has 1.1 as it is; only the barrel's picture, which nothing needs any
        // more, is still offered to go.
        let u = ws.pack_preview(&v2).unwrap().update.unwrap();
        assert!(u.added.is_empty() && u.updated.is_empty() && u.edited.is_empty(), "{u:?}");
        assert_eq!(u.retired.iter().map(|r| (r.asset.clone(), r.used_by.is_empty())).collect::<Vec<_>>(), [((RefKind::Sheet, "barrel_painted".to_string()), true)]);
        // The first version again as a copy: its chest beside the library's under a new id.
        ws.install_pack(&v1, SameId::Copy, "").unwrap();
        let lib = ws.lib.lib.as_ref().unwrap();
        assert!(lib.db.props.iter().any(|p| p.id != "chest" && p.name == "Chest") && lib.db.prop("chest").unwrap().name == "Sea chest");
        // A pack that needs another says so and installs nothing; a changed download is refused.
        let needy = PackManifest { needs: vec![Need { id: "not-there".into(), version: "2.0".into() }], ..manifest("Needy", "1.0") };
        let bytes = ws.make_pack(&needy, &[prop("chest")], &[], Vec::new()).unwrap().to_bytes().unwrap();
        assert!(ws.install_pack(&bytes, SameId::Keep, "").unwrap_err().contains("needs not-there 2.0 or newer"));
        assert_eq!(ws.installed_packs().len(), 1);
        std::fs::write(out.join("chests-1.1.r2dpack"), b"something else").unwrap();
        assert!(download(index.newest()[0], &out.display().to_string()).is_err());
        // A first install that replaces what the library has differently copies the library first.
        ws.forget_pack("chests").unwrap();
        assert!(ws.installed_packs().is_empty() && ws.lib.lib.as_ref().unwrap().db.prop("chest").is_some());
        let said = ws.install_pack(&v1, SameId::Replace, "").unwrap();
        assert!(said.starts_with("Installed Chests 1.0") && said.contains("Backup: "), "{said}");
        assert_eq!(ws.lib.lib.as_ref().unwrap().db.prop("chest").unwrap().name, "Chest");
        assert!(ws.make_pack(&manifest("Empty", "1.0"), &[], &[], Vec::new()).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn an_empty_library_takes_the_whole_starter_as_a_pack() {
        // The real starter library, as the editor hands it out (*Take it from the editor*).
        let dir = std::env::temp_dir().join(format!("rustpg2d-packs-whole-starter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for part in ["library", "game"] {
            std::fs::create_dir_all(dir.join(part)).unwrap();
            crate::files::create_project(&dir.join(part)).unwrap();
        }
        let mut ws = Workspace::new(dir.join("game"), crate::Gpu::No).unwrap();
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&dir.join("library")).unwrap());
        let bytes = crate::starter::as_file().expect("the starter library (the source folder)").unwrap();
        let said = ws.install_pack(&bytes, SameId::Keep, "").unwrap();
        assert!(said.starts_with("Installed Starter asset library"), "{said}");
        // Every file an asset of the library names is there: pictures, sounds, models, and the
        // 3D models items have of their own (they were left out until 0.36).
        let lib = ws.lib.lib.as_ref().unwrap();
        assert!(lib.db.items.len() > 100 && lib.db.models.len() > 50);
        let missing: Vec<String> = rustpg2d_engine::library::asset_files(&lib.db).into_iter().filter(|f| !lib.root.join(f).is_file()).collect();
        assert!(missing.is_empty(), "the library lacks files its assets name: {missing:?}");
        assert!(lib.db.items.iter().any(|i| i.model.is_some()));
        // Looked at again, there is nothing more to take.
        let p = ws.pack_preview(&bytes).unwrap();
        assert!(p.update.as_ref().is_some_and(|u| u.is_empty()), "{:?}", p.update.map(|u| u.counts()));
        // And a new game gets its basics from it.
        ws.start_with_basics("From the pack").unwrap();
        assert!(ws.project.db.character("hero").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_starter_library_comes_in_as_a_pack_over_what_a_library_has_of_it() {
        use rustpg2d_engine::starter_pack::StarterPack;
        let (mut ws, dir) = scratch("starter");
        // A small starter library: a crate with its picture. Its history knows what it shipped.
        let source = dir.join("starter");
        std::fs::create_dir_all(&source).unwrap();
        crate::files::create_project(&source).unwrap();
        let write = |name: &str, history: &mut StarterHistory, version: &str| -> Vec<u8> {
            let mut p = rustpg2d_engine::project::Project::load(&source).unwrap();
            p.db.sheets.retain(|s| s.id != "crates");
            p.db.props.retain(|s| s.id != "crate");
            p.db.sheets.push(rustpg2d_engine::defs::SheetDef::new("crates", "sprites/crates.png", 32, 32));
            p.db.props.push(rustpg2d_engine::defs::PropDef { name: name.into(), sprite: rustpg2d_engine::defs::SpriteRef::new("crates", 0), ..rustpg2d_engine::defs::PropDef::fresh("crate") });
            std::fs::create_dir_all(source.join("sprites")).unwrap();
            std::fs::write(source.join("sprites/crates.png"), rustpg2d_engine::imageio::Rgba::new(32, 32).encode().unwrap()).unwrap();
            p.save_defs().unwrap();
            let files = rustpg2d_engine::pack::read_dir_files(&source).unwrap();
            history.record(&Bundle::from_files(&files.iter().map(|(f, b)| (f.as_str(), b.as_slice())).collect::<Vec<_>>()).unwrap());
            let history_file = dir.join("starter_history.ron");
            std::fs::write(&history_file, history.to_ron().unwrap()).unwrap();
            StarterPack::from_dir(&source, &history_file, version).unwrap().to_bytes().unwrap()
        };
        let mut history = StarterHistory::default();
        let v1 = write("Crate", &mut history, "9.1");
        // The library has the starter's assets already, as they came by themselves before
        // 0.36: nothing says they are a pack's.
        copy(&source, &dir.join("library"));
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&dir.join("library")).unwrap());
        assert!(ws.installed_packs().is_empty());
        // Looked at, the file is the starter pack, and there is nothing to do; installed, it is on the list.
        let p = ws.pack_preview(&v1).unwrap();
        assert_eq!((p.manifest.id.as_str(), p.manifest.version.as_str(), p.installed.clone()), (packs::STARTER_ID, "9.1", None));
        assert!(p.update.as_ref().is_some_and(|u| u.is_empty()), "{:?}", p.update);
        let said = ws.install_pack(&v1, SameId::Keep, "").unwrap();
        assert!(said.starts_with("Installed Starter asset library 9.1") && said.ends_with("the library has it as it is"), "{said}");
        assert!(!dir.read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("library.bak-")), "nothing was replaced: no copy of the library is made");
        assert_eq!(ws.installed_packs()[0].manifest.id, packs::STARTER_ID);
        // The next version renames the crate. A library that left it alone gets the new one…
        let v2 = write("Sturdy crate", &mut history, "9.2");
        let u = ws.pack_preview(&v2).unwrap().update.unwrap();
        assert_eq!((u.updated.as_slice(), u.edited.len()), (&[(RefKind::Prop, "crate".to_string())][..], 0));
        // …one whose crate was changed keeps it, and is told.
        {
            let lib = ws.lib.lib.as_mut().unwrap();
            lib.db.props.iter_mut().find(|p| p.id == "crate").unwrap().name = "My crate".into();
            lib.save_defs().unwrap();
        }
        let u = ws.pack_preview(&v2).unwrap().update.unwrap();
        assert_eq!((u.updated.len(), u.edited.as_slice()), (0, &[(RefKind::Prop, "crate".to_string())][..]));
        let said = ws.install_pack(&v2, SameId::Keep, "").unwrap();
        assert!(said.starts_with("Updated Starter asset library from 9.1 to 9.2") && said.contains("kept your version of crate"), "{said}");
        assert_eq!(ws.lib.lib.as_ref().unwrap().db.prop("crate").unwrap().name, "My crate");
        let _ = std::fs::remove_dir_all(dir);
    }

}
