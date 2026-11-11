//! Updating the asset library from a newer starter set (*Update asset library*).
//!
//! Opening the library only adds the starter's new ids ([`Bundle::add_missing`]); changed
//! assets need this update. To replace only what the creator didn't change, the editor ships a
//! [`StarterHistory`]: for every asset the starter set ever had (every release), the
//! fingerprints of its definitions, of the files it owns and of the script functions. A library
//! asset whose definition and files are in that history is *untouched* and can be replaced; one
//! that isn't was changed by the creator, who decides. Ids the history knows but the current
//! starter set doesn't are *retired*: the creator decides whether to delete them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::defs::Database;
use crate::library::{all_assets, dependents, exists, kind_label, remove_assets, script_roots, AssetRef, Bundle, LibraryFiles, KINDS};
use crate::project::{Project, TILE_SIZE};
use crate::refactor::RefKind;
use crate::script_refs::ScriptFn;

/// 64-bit FNV-1a.
pub fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
}

/// Fingerprint of a definition: its RON text as today's structures write it (older files read
/// into the same structures compare fairly).
pub fn def_fingerprint(db: &Database, a: &AssetRef) -> Option<u64> {
    let text = crate::with_def_kind!(a.0, |T| db.get::<T>(&a.1).and_then(|d| ron::to_string(d).ok()), else => None)?;
    Some(fnv(text.as_bytes()))
}

/// The files an asset owns (a tileset's or sheet's image, a sound's file).
pub fn own_files(db: &Database, a: &AssetRef) -> Vec<String> {
    let f = match a.0 {
        RefKind::Tileset => db.tileset(&a.1).map(|d| d.image.clone()),
        RefKind::Sheet => db.sheet(&a.1).map(|d| d.image.clone()),
        RefKind::Sound => db.sound(&a.1).map(|d| d.file.clone()),
        RefKind::Model => {
            return db.model(&a.1).map(|d| std::iter::once(d.file.clone()).chain(d.clip_files.iter().cloned()).filter(|f| !f.is_empty()).collect()).unwrap_or_default();
        }
        _ => None,
    };
    f.into_iter().filter(|f| !f.is_empty()).collect()
}

fn key(a: &AssetRef) -> String {
    format!("{}:{}", a.0.name(), a.1)
}

fn parse_key(k: &str) -> Option<AssetRef> {
    let (kind, id) = k.split_once(':')?;
    Some((RefKind::parse(kind)?, id.to_string()))
}

/// What the starter set ever shipped of one asset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetHistory {
    /// Fingerprints of its definitions ([`def_fingerprint`]).
    pub defs: BTreeSet<u64>,
    /// Hashes of the contents of its files, by path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, BTreeSet<u64>>,
}

/// Everything the starter set ever shipped (`crates/editor/assets/starter_history.ron`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StarterHistory {
    /// By "kind:id" (e.g. `sheet:hero`).
    pub assets: BTreeMap<String, AssetHistory>,
    /// Hashes of the trimmed texts of the script functions, by name.
    #[serde(default)]
    pub functions: BTreeMap<String, BTreeSet<u64>>,
}

impl StarterHistory {
    pub fn parse(text: &str) -> Result<StarterHistory> {
        ron::from_str(text).context("parsing the starter history")
    }

    pub fn to_ron(&self) -> Result<String> {
        crate::project::ron_text(self)
    }

    /// Adds every asset and script function of a starter set.
    pub fn record(&mut self, b: &Bundle) {
        for a in all_assets(&b.db) {
            let h = self.assets.entry(key(&a)).or_default();
            if let Some(fp) = def_fingerprint(&b.db, &a) {
                h.defs.insert(fp);
            }
            for f in own_files(&b.db, &a) {
                if let Some(bytes) = b.files.get(&f) {
                    h.files.entry(f).or_default().insert(fnv(bytes));
                }
            }
        }
        for (name, text) in &b.scripts {
            self.functions.entry(name.clone()).or_default().insert(fnv(text.trim().as_bytes()));
        }
    }

    /// What of a starter set the history lacks ("kind:id" or "fn name"); empty = it covers it.
    pub fn missing(&self, b: &Bundle) -> Vec<String> {
        let mut probe = StarterHistory::default();
        probe.record(b);
        let mut out = Vec::new();
        for (k, h) in &probe.assets {
            let known = self.assets.get(k);
            let ok = known.is_some_and(|k| h.defs.is_subset(&k.defs) && h.files.iter().all(|(f, hs)| k.files.get(f).is_some_and(|ks| hs.is_subset(ks))));
            if !ok {
                out.push(k.clone());
            }
        }
        for (name, hs) in &probe.functions {
            if !self.functions.get(name).is_some_and(|k| hs.is_subset(k)) {
                out.push(format!("fn {name}"));
            }
        }
        out
    }

    /// Every asset the starter set ever had.
    pub fn ids(&self) -> impl Iterator<Item = AssetRef> + '_ {
        self.assets.keys().filter_map(|k| parse_key(k))
    }

    /// Is the library's `a` (in `db`, files under `root`) a version the starter set shipped?
    pub fn untouched(&self, db: &Database, root: &Path, a: &AssetRef) -> bool {
        let Some(h) = self.assets.get(&key(a)) else { return false };
        if !def_fingerprint(db, a).is_some_and(|fp| h.defs.contains(&fp)) {
            return false;
        }
        own_files(db, a).iter().all(|f| {
            let Ok(bytes) = std::fs::read(root.join(f)) else { return false };
            h.files.get(f).is_some_and(|hs| hs.contains(&fnv(&bytes)))
        })
    }

    /// Is this text of `name` one the starter set shipped?
    fn known_function(&self, name: &str, text: &str) -> bool {
        self.functions.get(name).is_some_and(|hs| hs.contains(&fnv(text.trim().as_bytes())))
    }
}

/// The starter set as a bundle at the library's tile size.
pub fn starter_bundle(files: &LibraryFiles) -> Result<Bundle> {
    let mut b = Bundle::from_files(files).context("reading the starter assets")?;
    b.rescale(TILE_SIZE)?;
    Ok(b)
}

/// An asset the starter set no longer has.
#[derive(Clone, Debug, PartialEq)]
pub struct Retired {
    pub asset: AssetRef,
    /// As the starter set once shipped it (not changed by the creator).
    pub untouched: bool,
    /// Library assets that still need it (it can't be deleted while they do).
    pub used_by: Vec<String>,
}

/// What an update would do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StarterUpdate {
    /// In the starter set, not in the library.
    pub added: Vec<AssetRef>,
    /// Changed in the starter set; the library has an earlier starter version: replaced.
    pub updated: Vec<AssetRef>,
    /// Changed in the starter set and changed by the creator: replaced only when chosen.
    pub edited: Vec<AssetRef>,
    /// No longer in the starter set: deleted only when chosen.
    pub retired: Vec<Retired>,
}

impl StarterUpdate {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.edited.is_empty() && self.retired.is_empty()
    }

    /// "added/updated/edited/retired" counts.
    pub fn counts(&self) -> String {
        format!("{}/{}/{}/{}", self.added.len(), self.updated.len(), self.edited.len(), self.retired.len())
    }
}

/// Does the library have `a` exactly as the starter set (definition and own files)?
fn same_as_starter(lib: &Project, starter: &Bundle, a: &AssetRef) -> bool {
    lib.db.same(&starter.db, a.0, &a.1)
        && own_files(&starter.db, a).iter().all(|f| starter.files.get(f).is_some_and(|b| std::fs::read(lib.root.join(f)).ok().as_ref() == Some(b)))
}

/// Compares the library with the starter set.
pub fn plan(lib: &Project, starter: &Bundle, history: &StarterHistory) -> StarterUpdate {
    let mut out = StarterUpdate::default();
    for a in all_assets(&starter.db) {
        if !exists(&lib.db, &a) {
            out.added.push(a);
        } else if same_as_starter(lib, starter, &a) {
            continue;
        } else if history.untouched(&lib.db, &lib.root, &a) {
            out.updated.push(a);
        } else {
            out.edited.push(a);
        }
    }
    let retired: BTreeSet<AssetRef> = history.ids().filter(|a| KINDS.contains(&a.0) && exists(&lib.db, a) && !exists(&starter.db, a)).collect();
    for a in &retired {
        // (Other retired assets that use it can go with it, and so can assets this update replaces
        // by a starter version that no longer uses it: the update runs before the deletions.)
        let replaced = |d: &AssetRef| out.updated.contains(d) && !starter.db.refs_of(d.0, &d.1).iter().any(|r| r == a);
        let used_by = dependents(&lib.db, a).into_iter().filter(|d| !retired.contains(d) && !replaced(d)).map(|(k, id)| format!("{} {id}", k.name())).collect();
        out.retired.push(Retired { untouched: history.untouched(&lib.db, &lib.root, a), used_by, asset: a.clone() });
    }
    out
}

/// What an update did.
#[derive(Clone, Debug, Default)]
pub struct UpdateReport {
    /// Where the library was copied first.
    pub backup: PathBuf,
    pub added: Vec<AssetRef>,
    pub replaced: Vec<AssetRef>,
    /// Changed by the creator and kept.
    pub kept: Vec<AssetRef>,
    pub deleted: Vec<AssetRef>,
    /// Chosen for deletion but still needed: (asset, why).
    pub refused: Vec<(AssetRef, String)>,
    pub scripts_added: Vec<String>,
    pub scripts_replaced: Vec<String>,
    /// Script functions the creator changed: their version stayed.
    pub scripts_kept: Vec<String>,
}

impl UpdateReport {
    pub fn summary(&self) -> String {
        let names = |v: &[AssetRef]| v.iter().map(|a| a.1.clone()).collect::<Vec<_>>().join(", ");
        let mut s = format!("{} added, {} replaced", self.added.len(), self.replaced.len());
        if !self.kept.is_empty() {
            s += &format!(", kept your version of {}", names(&self.kept));
        }
        if !self.deleted.is_empty() {
            s += &format!(", deleted {}", names(&self.deleted));
        }
        for (a, why) in &self.refused {
            s += &format!(", not deleted {} ({why})", a.1);
        }
        if !self.scripts_replaced.is_empty() {
            s += &format!(", script functions updated: {}", self.scripts_replaced.join(", "));
        }
        if !self.scripts_kept.is_empty() {
            s += &format!(", kept your script functions {}", self.scripts_kept.join(", "));
        }
        s += &format!(". Backup: {}", self.backup.display());
        s
    }
}

/// `(year, month, day, hour, minute, second)` now (UTC).
pub fn utc_now() -> (i64, u32, u32, u32, u32, u32) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month as u32, day as u32, (rem / 3600) as u32, (rem % 3600 / 60) as u32, (rem % 60) as u32)
}

/// Where a backup of the library folder `root` goes now (`<folder>.bak-YYYYMMDD-HHMMSS`).
pub fn backup_path(root: &Path) -> PathBuf {
    let (y, mo, d, h, mi, s) = utc_now();
    let name = root.file_name().map_or_else(|| "library".to_string(), |n| n.to_string_lossy().into_owned());
    let mut p = root.with_file_name(format!("{name}.bak-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}"));
    let mut n = 2;
    while p.exists() {
        p = root.with_file_name(format!("{name}.bak-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}-{n}"));
        n += 1;
    }
    p
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).with_context(|| format!("copying {}", src.display()))?;
        }
    }
    Ok(())
}

/// The starter's definitions of `wanted` (exactly those), their files and the script functions
/// they need.
fn subset(starter: &Bundle, wanted: &BTreeSet<AssetRef>) -> Bundle {
    let mut db = Database::default();
    for k in KINDS {
        for a in wanted.iter().filter(|a| a.0 == k) {
            db.copy_from(&starter.db, a.0, &a.1);
        }
    }
    let files = all_assets(&db).iter().flat_map(|a| own_files(&db, a)).filter_map(|f| starter.files.get(&f).map(|b| (f, b.clone()))).collect();
    let fns: Vec<ScriptFn> = starter.scripts.iter().map(|(name, text)| ScriptFn { name: name.clone(), params: Vec::new(), text: text.clone(), at: 0 }).collect();
    let roots: Vec<String> = all_assets(&db).iter().flat_map(|a| script_roots(&db, a)).collect();
    let needed = crate::script_refs::needed_functions(&fns, &roots);
    let scripts = starter.scripts.iter().filter(|(n, _)| needed.contains(*n)).map(|(n, t)| (n.clone(), t.clone())).collect();
    Bundle { name: starter.name.clone(), tile_size: starter.tile_size, db, files, scripts }
}

/// Applies an update to the library `lib` (a backup of its folder first): adds the new
/// assets, replaces the updated ones and those of `take_edited`, deletes `delete` (refusing
/// those other assets still need), and saves the definitions.
pub fn apply(lib: &mut Project, starter: &Bundle, plan: &StarterUpdate, history: &StarterHistory, take_edited: &[AssetRef], delete: &[AssetRef]) -> Result<UpdateReport> {
    let mut report = UpdateReport { backup: backup_path(&lib.root), ..Default::default() };
    copy_dir(&lib.root, &report.backup).context("backing up the library")?;
    let take: BTreeSet<AssetRef> = take_edited.iter().filter(|a| plan.edited.contains(a)).cloned().collect();
    report.kept = plan.edited.iter().filter(|a| !take.contains(*a)).cloned().collect();
    let wanted: BTreeSet<AssetRef> = plan.added.iter().chain(&plan.updated).chain(&take).cloned().collect();
    let mut sub = subset(starter, &wanted);
    // Script functions the creator changed stay as they are.
    let mine: BTreeMap<String, String> = crate::library::read_scripts(&lib.root).into_iter().map(|f| (f.name, f.text)).collect();
    sub.scripts.retain(|name, text| match mine.get(name) {
        Some(t) if t.trim() != text.trim() && !history.known_function(name, t) => {
            report.scripts_kept.push(name.clone());
            false
        }
        _ => true,
    });
    let merged = sub.replace_into(&mut lib.db, &lib.root)?;
    report.added = merged.added;
    report.replaced = merged.replaced;
    report.scripts_added = merged.scripts_added;
    report.scripts_replaced = merged.scripts_replaced;
    // Deletions: all together (they may need each other), else one by one.
    let chosen: Vec<AssetRef> = delete.iter().filter(|a| plan.retired.iter().any(|r| &r.asset == *a) && exists(&lib.db, a)).cloned().collect();
    if !chosen.is_empty() {
        if remove_assets(lib, &chosen).is_ok() {
            report.deleted = chosen;
        } else {
            for a in chosen {
                match remove_assets(lib, std::slice::from_ref(&a)) {
                    Ok(()) => report.deleted.push(a),
                    Err(e) => report.refused.push((a, format!("{e:#}"))),
                }
            }
        }
    }
    lib.save_defs()?;
    Ok(report)
}

/// The library's record that a newer starter set arrived and may bring updates (written by
/// [`crate::library::open_library`], removed after an update or when the creator says not now).
pub const PENDING_FILE: &str = "starter.pending";

/// Does the library at `root` wait for the creator to look at a newer starter set?
pub fn pending(root: &Path) -> bool {
    root.join(PENDING_FILE).exists()
}

/// Forgets the waiting newer starter set (updated, or not now).
pub fn clear_pending(root: &Path) {
    let _ = std::fs::remove_file(root.join(PENDING_FILE));
}

/// "sheet hero" for messages.
pub fn label(a: &AssetRef) -> String {
    format!("{} {}", kind_label(a.0).to_lowercase(), a.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{ItemDef, Ranged, SheetDef, SpriteRef, Weapon};
    use crate::events::{Action, Trigger, TriggerOn};
    use crate::imageio::Rgba;
    use crate::project::ProjectConfig;

    fn png(c: [u8; 4]) -> Vec<u8> {
        let mut img = Rgba::new(32, 16);
        img.px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| *p = c);
        img.encode().unwrap()
    }

    fn item(id: &str, name: &str) -> ItemDef {
        ItemDef { id: id.into(), name: name.into(), icon: SpriteRef::new("items", 0), ..Default::default() }
    }

    fn starter(colour: [u8; 4], items: Vec<ItemDef>, script: &str) -> Bundle {
        let db = Database { sheets: vec![SheetDef { id: "items".into(), image: "sprites/items.png".into(), frame_w: 16, frame_h: 16, ..Default::default() }], items, ..Default::default() };
        Bundle { name: "starter".into(), tile_size: TILE_SIZE, db, files: [("sprites/items.png".to_string(), png(colour))].into(), scripts: [("arrows_used".to_string(), format!("fn arrows_used() {{ say(\"{script}\"); }}"))].into() }
    }

    fn arrows(name: &str) -> ItemDef {
        ItemDef { on_use: vec![Trigger { actions: vec![Action::RunScript { function: "arrows_used".into() }], ..Trigger::new(TriggerOn::Interact) }], ..item("arrows", name) }
    }

    #[test]
    fn updates_replace_untouched_assets_and_ask_for_the_rest() {
        let a = |k: RefKind, id: &str| (k, id.to_string());
        // The starter set the library was made from, and the next one: the arrows, their picture
        // and script changed, the gem changed, the relic and the charm were dropped, a shield is new.
        let v1 = starter([255, 0, 0, 255], vec![arrows("Arrows"), item("bow", "Bow"), item("gem", "Gem"), item("relic", "Relic"), item("charm", "Charm")], "v1");
        let v2 = starter([0, 0, 255, 255], vec![arrows("Sharp arrows"), item("bow", "Bow"), item("gem", "Shiny gem"), item("shield", "Shield")], "v2");
        let mut history = StarterHistory::default();
        history.record(&v1);
        history.record(&v2);
        assert!(history.missing(&v2).is_empty());
        assert!(!history.missing(&starter([1, 2, 3, 255], vec![], "v3")).is_empty(), "a new picture and script aren't covered");

        let root = std::env::temp_dir().join(format!("rustpg2d_starter_update_{}", std::process::id())).join("library");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
        std::fs::create_dir_all(&root).unwrap();
        let mut lib = Project { root: root.clone(), config: ProjectConfig::new("Asset library"), db: Database::default() };
        v1.clone().merge_into(&mut lib.db, &root).unwrap();
        // The creator renamed the gem and the charm, made a bow that shoots relics, and changed
        // the arrows' script.
        lib.db.items.iter_mut().find(|d| d.id == "gem").unwrap().name = "My gem".into();
        lib.db.items.iter_mut().find(|d| d.id == "charm").unwrap().name = "My charm".into();
        let mut relic_bow = item("relic_bow", "Relic bow");
        relic_bow.weapon = Some(Weapon::ranged(1, 0.5, Ranged { projectile: SpriteRef::new("items", 1), speed: 100.0, range: 6.0, ammo: "relic".into() }));
        lib.db.items.push(relic_bow);
        lib.save_config().unwrap();
        lib.save_defs().unwrap();
        let script = root.join("scripts").join(crate::library::LIBRARY_SCRIPT);
        let src = std::fs::read_to_string(&script).unwrap();
        std::fs::write(&script, src.replace("say(\"v1\")", "say(\"mine\")")).unwrap();

        let p = plan(&lib, &v2, &history);
        assert_eq!(p.added, [a(RefKind::Item, "shield")]);
        assert_eq!(p.updated, [a(RefKind::Sheet, "items"), a(RefKind::Item, "arrows")]);
        assert_eq!(p.edited, [a(RefKind::Item, "gem")]);
        let retired: Vec<(&str, bool, usize)> = p.retired.iter().map(|r| (r.asset.1.as_str(), r.untouched, r.used_by.len())).collect();
        assert_eq!(retired, [("charm", false, 0), ("relic", true, 1)]);

        let report = apply(&mut lib, &v2, &p, &history, &[], &[a(RefKind::Item, "charm"), a(RefKind::Item, "relic")]).unwrap();
        assert!(report.backup.join("defs/items.ron").exists(), "backed up first");
        let name = |id: &str| lib.db.item(id).map(|d| d.name.clone());
        assert_eq!(name("arrows").as_deref(), Some("Sharp arrows"));
        assert_eq!(name("shield").as_deref(), Some("Shield"));
        assert_eq!(name("gem").as_deref(), Some("My gem"), "kept: not chosen");
        assert_eq!(name("charm"), None, "deleted");
        assert_eq!(name("relic").as_deref(), Some("Relic"), "still used by the relic bow");
        assert_eq!(report.refused.len(), 1);
        assert_eq!(std::fs::read(root.join("sprites/items.png")).unwrap(), png([0, 0, 255, 255]));
        assert_eq!(report.scripts_kept, ["arrows_used"]);
        assert!(std::fs::read_to_string(&script).unwrap().contains("say(\"mine\")"), "the creator's function stays");
        // Saved: a fresh load sees the same; only the open questions remain.
        let lib = Project::load(&root).unwrap();
        let p = plan(&lib, &v2, &history);
        assert!(p.added.is_empty() && p.updated.is_empty());
        assert_eq!(p.edited, [a(RefKind::Item, "gem")]);
        assert_eq!(p.retired.len(), 1);
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn chosen_edited_assets_take_the_starter_version() {
        let v1 = starter([255, 0, 0, 255], vec![item("gem", "Gem")], "v1");
        let v2 = starter([255, 0, 0, 255], vec![item("gem", "Shiny gem")], "v1");
        let mut history = StarterHistory::default();
        history.record(&v1);
        history.record(&v2);
        let root = std::env::temp_dir().join(format!("rustpg2d_starter_take_{}", std::process::id())).join("lib");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
        std::fs::create_dir_all(&root).unwrap();
        let mut lib = Project { root: root.clone(), config: ProjectConfig::new("Asset library"), db: Database::default() };
        v1.clone().merge_into(&mut lib.db, &root).unwrap();
        lib.db.items[0].name = "My gem".into();
        let p = plan(&lib, &v2, &history);
        assert_eq!(p.edited.len(), 1);
        let r = apply(&mut lib, &v2, &p, &history, &p.edited.clone(), &[]).unwrap();
        assert_eq!(lib.db.item("gem").unwrap().name, "Shiny gem");
        assert_eq!(r.replaced.len(), 1);
        assert!(plan(&lib, &v2, &history).is_empty());
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn retired_assets_used_only_by_replaced_versions_can_go() {
        let a = |k: RefKind, id: &str| (k, id.to_string());
        // v1's bow shoots relics; v2's bow shoots nothing and the relic is gone.
        let mut bow = item("bow", "Bow");
        bow.weapon = Some(Weapon::ranged(1, 0.5, Ranged { projectile: SpriteRef::new("items", 1), speed: 100.0, range: 6.0, ammo: "relic".into() }));
        let v1 = starter([255, 0, 0, 255], vec![bow.clone(), item("relic", "Relic")], "v1");
        bow.weapon.as_mut().unwrap().ranged.as_mut().unwrap().ammo.clear();
        let v2 = starter([255, 0, 0, 255], vec![bow], "v1");
        let mut history = StarterHistory::default();
        history.record(&v1);
        history.record(&v2);
        let root = std::env::temp_dir().join(format!("rustpg2d_starter_replaced_{}", std::process::id())).join("lib");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
        std::fs::create_dir_all(&root).unwrap();
        let mut lib = Project { root: root.clone(), config: ProjectConfig::new("Asset library"), db: Database::default() };
        v1.clone().merge_into(&mut lib.db, &root).unwrap();
        let p = plan(&lib, &v2, &history);
        assert_eq!(p.updated, [a(RefKind::Item, "bow")]);
        assert!(p.retired.iter().all(|r| r.used_by.is_empty()), "the bow that used the relic is replaced: {:?}", p.retired);
        let r = apply(&mut lib, &v2, &p, &history, &[], &[a(RefKind::Item, "relic")]).unwrap();
        assert!(r.refused.is_empty() && lib.db.item("relic").is_none());
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn backups_get_a_dated_name() {
        let p = backup_path(Path::new("/tmp/x/library"));
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("library.bak-20") && name.len() == "library.bak-YYYYMMDD-HHMMSS".len(), "{name}");
    }
}
