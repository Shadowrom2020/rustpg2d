//! What the editor keeps about a project's assets beside their definitions: the creator's own
//! order. A folder, tags, notes and a favourite mark per definition or map, in
//! `editor/meta.ron` of the project. The game never reads the file, and exports leave it out.
//!
//! It is written at once when it changes (no "unsaved" state of its own), and its changes are
//! steps of the one history. Entries follow renames; an entry of something deleted stays until
//! the next writing (so undoing the deletion finds it again) and is left out then.

use std::collections::BTreeMap;
use std::path::Path;

use rustpg2d_engine::refactor::RefKind;
use serde::{Deserialize, Serialize};

use crate::history::Extra;
use crate::workspace::Workspace;

/// The file, in the project's folder.
pub const FILE: &str = "editor/meta.ron";

fn is_false(b: &bool) -> bool {
    !*b
}

/// What the creator noted about one asset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetMeta {
    /// Where the creator keeps it, parts separated by `/` (`Village/People`); "" = nowhere.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub favorite: bool,
    /// A map: where its card lies in the World view (`None`: where the view puts it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<[f32; 2]>,
    /// The pack it came from (its id; [`Meta::packs`] says what that pack is) and the version
    /// it came as: whose work it is, and what an update of the pack compares with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
}

/// Where an asset of the project came from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub pack: String,
    pub version: String,
}

/// A pack some of the project's assets came from, as the project remembers it (the library
/// it was installed in may be another by the time the game is published).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackCredit {
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub licence: String,
    /// The line a game that uses it shows in its credits.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribution: String,
}

impl PackCredit {
    /// The line for the game's credits: the pack's own, else made from what is known of it.
    pub fn line(&self) -> String {
        if !self.attribution.trim().is_empty() {
            return self.attribution.trim().to_string();
        }
        let mut line = self.title.clone();
        if !self.author.is_empty() {
            line += &format!(" by {}", self.author);
        }
        if !self.licence.is_empty() {
            line += &format!(" ({})", self.licence);
        }
        line
    }
}

impl AssetMeta {
    pub fn is_empty(&self) -> bool {
        *self == AssetMeta::default()
    }
}

/// Every asset's [`AssetMeta`], by `kind:id` (`item:potion`, `map:village`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub assets: BTreeMap<String, AssetMeta>,
    /// The packs assets came from, by pack id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub packs: BTreeMap<String, PackCredit>,
}

/// A tag or folder as it is kept: without spaces around it, a folder without empty parts.
pub fn tidy_tag(tag: &str) -> String {
    tag.trim().trim_start_matches('#').trim().to_string()
}

pub fn tidy_folder(folder: &str) -> String {
    folder.split('/').map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("/")
}

impl Meta {
    pub fn key(kind: RefKind, id: &str) -> String {
        format!("{}:{id}", kind.name())
    }

    /// Reads the project's file; nothing noted when there is none (or it can't be read: said
    /// on the console, the file stays as it is until something is noted).
    pub fn load(root: &Path) -> Meta {
        let path = root.join(FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => ron::from_str(&text).unwrap_or_else(|e| {
                eprintln!("[editor] {}: {e}", path.display());
                Meta::default()
            }),
            Err(_) => Meta::default(),
        }
    }

    /// Writes the file: the entries that say something about something `exists`.
    pub fn save(&self, root: &Path, exists: impl Fn(&str) -> bool) -> anyhow::Result<()> {
        let assets: BTreeMap<String, AssetMeta> = self.assets.iter().filter(|(key, m)| !m.is_empty() && exists(key)).map(|(k, m)| (k.clone(), m.clone())).collect();
        // (Only the packs something still comes from.)
        let packs = self.packs.iter().filter(|(id, _)| assets.values().any(|m| m.origin.as_ref().is_some_and(|o| o.pack == **id))).map(|(k, v)| (k.clone(), v.clone())).collect();
        let kept = Meta { assets, packs };
        let path = root.join(FILE);
        if kept.assets.is_empty() && !path.exists() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        rustpg2d_engine::project::write_ron(&path, &kept)
    }

    pub fn of(&self, kind: RefKind, id: &str) -> Option<&AssetMeta> {
        self.assets.get(&Meta::key(kind, id))
    }

    /// The ids of `kind` with something noted, with it.
    pub fn of_kind(&self, kind: RefKind) -> impl Iterator<Item = (&str, &AssetMeta)> {
        let prefix = format!("{}:", kind.name());
        self.assets.iter().filter_map(move |(key, m)| key.strip_prefix(prefix.as_str()).map(|id| (id, m)))
    }

    /// Every tag in use, in the alphabet's order (one spelling of each: `Act 1` and `act 1`
    /// are the same tag).
    pub fn tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.assets.values().flat_map(|m| m.tags.iter().cloned()).collect();
        tags.sort_by_key(|t| (t.to_lowercase(), t.clone()));
        tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        tags
    }

    /// Whether an asset has `tag` (in any spelling).
    pub fn has_tag(&self, kind: RefKind, id: &str, tag: &str) -> bool {
        self.of(kind, id).is_some_and(|m| m.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
    }

    /// Every folder in use, in the alphabet's order.
    pub fn folders(&self) -> Vec<String> {
        let mut folders: Vec<String> = self.assets.values().map(|m| m.folder.clone()).filter(|f| !f.is_empty()).collect();
        folders.sort_by_key(|f| f.to_lowercase());
        folders.dedup();
        folders
    }
}

impl Workspace {
    fn meta_exists(&self, key: &str) -> bool {
        let Some((kind, id)) = key.split_once(':') else { return false };
        // (A note on a flag or a variable: kept while the creator keeps it.)
        if crate::names::NameKind::ALL.iter().any(|k| k.key() == kind) {
            return true;
        }
        match RefKind::parse(kind) {
            Some(RefKind::Map) => self.maps.iter().any(|m| m == id),
            Some(k) => self.project.db.has(k, id),
            None => false,
        }
    }

    /// Writes `editor/meta.ron` (a failure is said in the status line).
    pub fn save_meta(&mut self) {
        if !self.project_open {
            return;
        }
        if let Err(e) = self.meta.save(&self.project.root, |key| self.meta_exists(key)) {
            self.set_status(format!("Writing {FILE} failed: {e:#}"));
        }
    }

    /// Notes where map `map`'s card lies in the World view (`None`: where the view puts it).
    /// Written at once; no step of the history (arranging the view changes nothing of the game).
    pub fn set_card_place(&mut self, map: &str, place: Option<[f32; 2]>) {
        let key = Meta::key(RefKind::Map, map);
        let entry = self.meta.assets.entry(key.clone()).or_default();
        if entry.card == place {
            return;
        }
        entry.card = place;
        if entry.is_empty() {
            self.meta.assets.remove(&key);
        }
        self.save_meta();
    }

    /// Changes what is noted about an asset: a step of the history, written at once. False
    /// when `edit` changed nothing.
    pub fn meta_edit(&mut self, kind: RefKind, id: &str, edit: impl FnOnce(&mut AssetMeta)) -> bool {
        let before = self.meta.clone();
        let key = Meta::key(kind, id);
        let entry = self.meta.assets.entry(key.clone()).or_default();
        edit(entry);
        entry.folder = tidy_folder(&entry.folder);
        entry.tags = entry.tags.iter().map(|t| tidy_tag(t)).filter(|t| !t.is_empty()).collect();
        let mut seen = std::collections::HashSet::new();
        entry.tags.retain(|t| seen.insert(t.to_lowercase()));
        if entry.is_empty() {
            self.meta.assets.remove(&key);
        }
        if self.meta == before {
            return false;
        }
        self.db_hist.record(Extra::Meta(Box::new(before)));
        self.begin_unnamed("Tags, folders and notes");
        self.save_meta();
        self.stale = true;
        true
    }

    /// An asset was renamed: what is noted about it follows (the rename's step undoes both).
    pub(crate) fn meta_renamed(&mut self, kind: RefKind, old: &str, new: &str) {
        if let Some(m) = self.meta.assets.remove(&Meta::key(kind, old)) {
            self.meta.assets.insert(Meta::key(kind, new), m);
            self.save_meta();
        }
    }

    /// The packs the project's assets came from, with how many of each pack's there are now,
    /// by title.
    pub fn packs_used(&self) -> Vec<(String, PackCredit, usize)> {
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for (key, m) in &self.meta.assets {
            if let Some(o) = m.origin.as_ref().filter(|_| self.meta_exists(key)) {
                *count.entry(o.pack.as_str()).or_default() += 1;
            }
        }
        let mut out: Vec<(String, PackCredit, usize)> = count.into_iter().map(|(id, n)| (id.to_string(), self.meta.packs.get(id).cloned().unwrap_or_else(|| PackCredit { title: id.to_string(), ..Default::default() }), n)).collect();
        out.sort_by_key(|p| p.1.title.to_lowercase());
        out
    }

    /// The credit lines of the packs in use that the game's credits don't hold yet.
    pub fn credits_missing(&self) -> Vec<String> {
        let credits = self.project.config.credits.to_lowercase();
        self.packs_used().into_iter().map(|(_, p, _)| p.line()).filter(|l| !credits.contains(&l.to_lowercase())).collect()
    }

    /// Puts the credit lines that are missing at the end of the game's credits (one step).
    /// Returns how many.
    pub fn add_pack_credits(&mut self) -> usize {
        let missing = self.credits_missing();
        if missing.is_empty() {
            return 0;
        }
        self.begin("Add the packs' credit lines to the credits");
        let credits = &mut self.project.config.credits;
        if !credits.trim().is_empty() && !credits.ends_with('\n') {
            credits.push('\n');
        }
        *credits += &missing.join("\n");
        self.defs_dirty = true;
        self.stale = true;
        missing.len()
    }

    /// A copy of an asset starts in the same folder with the same tags (not a favourite, no notes).
    pub(crate) fn meta_copied(&mut self, kind: RefKind, from: &str, to: &str) {
        let Some(m) = self.meta.of(kind, from).cloned() else { return };
        // (And it is still that pack's work: the origin goes with it.)
        self.meta_edit(kind, to, |new| {
            new.folder = m.folder;
            new.tags = m.tags;
            new.origin = m.origin;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_and_folders_are_kept_tidy() {
        assert_eq!(tidy_tag("  #boss "), "boss");
        assert_eq!(tidy_folder(" Village / People //"), "Village/People");
        let mut meta = Meta::default();
        meta.assets.insert(Meta::key(RefKind::Item, "potion"), AssetMeta { tags: vec!["healing".into(), "Act 1".into()], folder: "Shop".into(), ..Default::default() });
        meta.assets.insert(Meta::key(RefKind::Map, "village"), AssetMeta { tags: vec!["act 1".into(), "Act 1".into()], ..Default::default() });
        assert_eq!(meta.tags(), ["Act 1", "healing"]);
        assert!(meta.has_tag(RefKind::Map, "village", "ACT 1") && !meta.has_tag(RefKind::Map, "village", "healing"));
        assert_eq!(meta.folders(), ["Shop"]);
        assert_eq!(meta.of_kind(RefKind::Item).map(|(id, _)| id).collect::<Vec<_>>(), ["potion"]);
    }

    #[test]
    fn the_file_keeps_what_says_something_about_something_there() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-meta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut meta = Meta::default();
        meta.save(&dir, |_| true).unwrap();
        assert!(!dir.join(FILE).exists(), "nothing noted: no file");
        meta.assets.insert("item:potion".into(), AssetMeta { favorite: true, ..Default::default() });
        meta.assets.insert("item:gone".into(), AssetMeta { notes: "was here".into(), ..Default::default() });
        meta.assets.insert("item:plain".into(), AssetMeta::default());
        meta.save(&dir, |key| key != "item:gone").unwrap();
        let read = Meta::load(&dir);
        assert_eq!(read.assets.keys().collect::<Vec<_>>(), ["item:potion"]);
        assert!(read.assets["item:potion"].favorite);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
