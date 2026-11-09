//! Asset bundles and the shared asset library.
//!
//! A [`Bundle`] is a set of definitions (tilesets, sprite sheets, sounds, items, props,
//! characters) together with the image / sound files they need. Bundles are
//! - collected from a project with all their dependencies ([`Bundle::collect`]),
//! - stored as `.r2dpack` packages (a zip with `manifest.ron` and `files/…`),
//! - rescaled between tile sizes (e.g. 32 px library art into a 16 px project),
//! - merged into a project or the library: identical assets are reused, clashing ids and
//!   file names get a numeric suffix and every reference inside the bundle follows.
//!
//! The library itself is an ordinary project folder (without maps) under
//! `~/.local/share/rustpg2d/library`, so it is loaded and saved like any project.
//!
//! Script functions travel with the assets that need them (a spell or skill with a *Script*
//! effect, triggers that run scripts): bundles carry whole functions by name, and merging
//! appends them to `scripts/library.rhai` of the target (functions it already has are kept).

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::defs::*;
use crate::imageio::Rgba;
use crate::project::{Project, ProjectConfig, TILE_SIZE};
use crate::refactor::{self, RefKind};
use crate::script_refs::ScriptFn;

pub const PACK_EXT: &str = "r2dpack";
const PACK_VERSION: u32 = 1;

/// Kind and id of one asset.
pub type AssetRef = (RefKind, String);

/// Asset kinds in dependency order (things later in the list may refer to earlier ones).
pub const KINDS: [RefKind; 10] = [RefKind::Tileset, RefKind::Sheet, RefKind::Sound, RefKind::Item, RefKind::Prop, RefKind::Character, RefKind::Structure, RefKind::Skill, RefKind::Spell, RefKind::Recipe];

pub fn kind_label(k: RefKind) -> &'static str {
    match k {
        RefKind::Tileset => "Tileset",
        RefKind::Sheet => "Sprite sheet",
        RefKind::Sound => "Sound",
        RefKind::Item => "Item",
        RefKind::Prop => "Prop",
        RefKind::Character => "Character",
        RefKind::Map => "Map",
        RefKind::Structure => "Structure",
        RefKind::Skill => "Skill",
        RefKind::Spell => "Spell",
        RefKind::Recipe => "Recipe",
    }
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    name: String,
    tile_size: u32,
    #[serde(default)]
    tilesets: Vec<TilesetDef>,
    #[serde(default)]
    sheets: Vec<SheetDef>,
    #[serde(default)]
    sounds: Vec<SoundDef>,
    #[serde(default)]
    items: Vec<ItemDef>,
    #[serde(default)]
    props: Vec<PropDef>,
    #[serde(default)]
    characters: Vec<CharacterDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    structures: Vec<StructureDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skills: Vec<crate::skills::SkillDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    spells: Vec<crate::skills::SpellDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recipes: Vec<crate::crafting::RecipeDef>,
    /// Script functions by name (their whole source text).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    scripts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Bundle {
    pub name: String,
    /// Tile size the art was made for.
    pub tile_size: u32,
    pub db: Database,
    /// Files by project-relative path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// Script functions the assets need, by name (whole source text).
    pub scripts: BTreeMap<String, String>,
}

/// What a merge did.
#[derive(Clone, Debug, Default)]
pub struct MergeReport {
    pub added: Vec<AssetRef>,
    /// Already present with identical content.
    pub existing: Vec<AssetRef>,
    /// (kind, id in the bundle, id it got).
    pub renamed: Vec<(RefKind, String, String)>,
    /// Files written.
    pub files: Vec<String>,
    /// Script functions appended to `scripts/library.rhai`.
    pub scripts_added: Vec<String>,
    /// Script functions the target already had with different code (its version was kept).
    pub scripts_kept: Vec<String>,
    /// Existing images replaced by a larger version that only adds frames / tiles (starter
    /// updates): (kind, id).
    pub upgraded: Vec<AssetRef>,
    /// Assets whose existing version was replaced ([`Bundle::replace_into`]).
    pub replaced: Vec<AssetRef>,
    /// New assets left out because they need newer art than the target has (a structure
    /// whose tileset the target has with other tiles).
    pub skipped: Vec<AssetRef>,
    /// Script functions whose existing code was replaced ([`Bundle::replace_into`]).
    pub scripts_replaced: Vec<String>,
}

impl MergeReport {
    pub fn summary(&self) -> String {
        let mut s = format!("{} added", self.added.len());
        if !self.added.is_empty() && self.added.len() <= 8 {
            // (Few: name them, so a report of one send says what arrived.)
            let list: Vec<&str> = self.added.iter().map(|a| a.1.as_str()).collect();
            s += &format!(" ({})", list.join(", "));
        }
        s += &format!(", {} already present", self.existing.len());
        if !self.renamed.is_empty() {
            let list: Vec<String> = self.renamed.iter().map(|(_, a, b)| format!("{a} → {b}")).collect();
            s += &format!(", renamed (id taken): {}", list.join(", "));
        }
        if !self.scripts_added.is_empty() {
            s += &format!(", script functions added to scripts/{LIBRARY_SCRIPT}: {}", self.scripts_added.join(", "));
        }
        if !self.upgraded.is_empty() {
            let list: Vec<&str> = self.upgraded.iter().map(|a| a.1.as_str()).collect();
            s += &format!(", images extended: {}", list.join(", "));
        }
        if !self.skipped.is_empty() {
            let list: Vec<&str> = self.skipped.iter().map(|a| a.1.as_str()).collect();
            s += &format!(", left out (they need newer tileset art; rebuild the library to get them): {}", list.join(", "));
        }
        if !self.replaced.is_empty() {
            let list: Vec<&str> = self.replaced.iter().map(|a| a.1.as_str()).collect();
            s += &format!(", replaced: {}", list.join(", "));
        }
        if !self.scripts_replaced.is_empty() {
            s += &format!(", script functions replaced: {}", self.scripts_replaced.join(", "));
        }
        if !self.scripts_kept.is_empty() {
            s += &format!(", kept the existing (different) script functions {}", self.scripts_kept.join(", "));
        }
        s
    }
}

/// Do `a` and `b` have the same definition of `asset` (both missing counts as the same)?
pub fn same_def(a: &Database, b: &Database, asset: &AssetRef) -> bool {
    let id = asset.1.as_str();
    match asset.0 {
        RefKind::Tileset => a.tileset(id) == b.tileset(id),
        RefKind::Sheet => a.sheet(id) == b.sheet(id),
        RefKind::Sound => a.sounds.iter().find(|d| d.id == id) == b.sounds.iter().find(|d| d.id == id),
        RefKind::Item => a.item(id) == b.item(id),
        RefKind::Prop => a.prop(id) == b.prop(id),
        RefKind::Character => a.character(id) == b.character(id),
        RefKind::Structure => a.structure(id) == b.structure(id),
        RefKind::Skill => a.skill(id) == b.skill(id),
        RefKind::Spell => a.spell(id) == b.spell(id),
        RefKind::Recipe => a.recipe(id) == b.recipe(id),
        RefKind::Map => true,
    }
}

/// Puts `from`'s definition of `asset` into `to`: replaces the one with the same id in place,
/// or appends it.
fn put_def(to: &mut Database, from: &Database, asset: &AssetRef) {
    macro_rules! put {
        ($list:ident) => {
            if let Some(d) = from.$list.iter().find(|d| d.id == asset.1) {
                match to.$list.iter().position(|o| o.id == asset.1) {
                    Some(i) => to.$list[i] = d.clone(),
                    None => to.$list.push(d.clone()),
                }
            }
        };
    }
    match asset.0 {
        RefKind::Tileset => put!(tilesets),
        RefKind::Sheet => put!(sheets),
        RefKind::Sound => put!(sounds),
        RefKind::Item => put!(items),
        RefKind::Prop => put!(props),
        RefKind::Character => put!(characters),
        RefKind::Structure => put!(structures),
        RefKind::Skill => put!(skills),
        RefKind::Spell => put!(spells),
        RefKind::Recipe => put!(recipes),
        RefKind::Map => {}
    }
}

pub fn exists(db: &Database, a: &AssetRef) -> bool {
    let id = a.1.as_str();
    match a.0 {
        RefKind::Tileset => db.tileset(id).is_some(),
        RefKind::Sheet => db.sheet(id).is_some(),
        RefKind::Sound => db.sounds.iter().any(|d| d.id == id),
        RefKind::Item => db.item(id).is_some(),
        RefKind::Prop => db.prop(id).is_some(),
        RefKind::Character => db.character(id).is_some(),
        RefKind::Structure => db.structure(id).is_some(),
        RefKind::Skill => db.skill(id).is_some(),
        RefKind::Spell => db.spell(id).is_some(),
        RefKind::Recipe => db.recipe(id).is_some(),
        RefKind::Map => false,
    }
}

/// Assets `a` refers to directly (only kinds that live in a bundle).
pub fn deps(db: &Database, a: &AssetRef) -> Vec<AssetRef> {
    let refs = match a.0 {
        RefKind::Item => db.item(&a.1).map(refactor::refs_of_item),
        RefKind::Prop => db.prop(&a.1).map(refactor::refs_of_prop),
        RefKind::Character => db.character(&a.1).map(refactor::refs_of_character),
        RefKind::Structure => db.structure(&a.1).map(refactor::refs_of_structure),
        RefKind::Skill => db.skill(&a.1).map(refactor::refs_of_skill),
        RefKind::Spell => db.spell(&a.1).map(refactor::refs_of_spell),
        RefKind::Recipe => db.recipe(&a.1).map(refactor::refs_of_recipe),
        _ => None,
    };
    refs.unwrap_or_default().into_iter().filter(|r| r.0 != RefKind::Map).collect()
}

/// All assets in a database, in dependency order.
pub fn all_assets(db: &Database) -> Vec<AssetRef> {
    let mut out = Vec::new();
    out.extend(db.tilesets.iter().map(|d| (RefKind::Tileset, d.id.clone())));
    out.extend(db.sheets.iter().map(|d| (RefKind::Sheet, d.id.clone())));
    out.extend(db.sounds.iter().map(|d| (RefKind::Sound, d.id.clone())));
    out.extend(db.items.iter().map(|d| (RefKind::Item, d.id.clone())));
    out.extend(db.props.iter().map(|d| (RefKind::Prop, d.id.clone())));
    out.extend(db.characters.iter().map(|d| (RefKind::Character, d.id.clone())));
    out.extend(db.structures.iter().map(|d| (RefKind::Structure, d.id.clone())));
    out.extend(db.skills.iter().map(|d| (RefKind::Skill, d.id.clone())));
    out.extend(db.spells.iter().map(|d| (RefKind::Spell, d.id.clone())));
    out.extend(db.recipes.iter().map(|d| (RefKind::Recipe, d.id.clone())));
    out
}

/// File (in `scripts/`) that receives script functions merged from bundles.
pub const LIBRARY_SCRIPT: &str = "library.rhai";

/// Script functions an asset calls itself: a skill's or spell's *Script* effect and the
/// functions its triggers run (*Run script* actions, *Script* conditions, *Aim* / *Fly*).
pub fn script_roots(db: &Database, a: &AssetRef) -> Vec<String> {
    use crate::events::{Action, Cond, Trigger};
    fn actions(acts: &[Action], out: &mut Vec<String>) {
        for a in acts {
            match a {
                Action::RunScript { function } | Action::Aim { function } | Action::Fly { function, .. } | Action::Cutscene { function } => out.push(function.clone()),
                Action::If { conditions, then, otherwise } => {
                    out.extend(conditions.iter().filter_map(|c| match &c.cond {
                        Cond::Script { function } => Some(function.clone()),
                        _ => None,
                    }));
                    actions(then, out);
                    actions(otherwise, out);
                }
                _ => {}
            }
        }
    }
    let triggers = |ts: &[Trigger], out: &mut Vec<String>| {
        for t in ts {
            out.extend(t.conditions.iter().filter_map(|c| match &c.cond {
                Cond::Script { function } => Some(function.clone()),
                _ => None,
            }));
            actions(&t.actions, out);
        }
    };
    let mut out = Vec::new();
    let id = a.1.as_str();
    match a.0 {
        RefKind::Skill => {
            if let Some(crate::skills::SkillEffect::Script { function }) = db.skill(id).map(|d| &d.effect) {
                out.push(function.clone());
            }
        }
        RefKind::Spell => {
            if let Some(crate::skills::SpellEffect::Script { function }) = db.spell(id).map(|d| &d.effect) {
                out.push(function.clone());
            }
        }
        RefKind::Item => triggers(db.item(id).map_or(&[][..], |d| &d.on_use), &mut out),
        RefKind::Prop => triggers(db.prop(id).map_or(&[][..], |d| &d.triggers), &mut out),
        RefKind::Character => triggers(db.character(id).map_or(&[][..], |d| &d.triggers), &mut out),
        _ => {}
    }
    out.retain(|f| !f.is_empty());
    out.sort();
    out.dedup();
    out
}

/// The functions of every `*.rhai` file in `<root>/scripts` (files in name order).
pub fn read_scripts(root: &Path) -> Vec<ScriptFn> {
    let Ok(rd) = std::fs::read_dir(root.join("scripts")) else { return Vec::new() };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect();
    files.sort();
    files.iter().filter_map(|f| std::fs::read_to_string(f).ok()).flat_map(|src| crate::script_refs::functions(&src)).collect()
}

/// Assets that script functions refer to by id (sprite sheets, sounds, items, skills, spells).
pub fn script_asset_refs<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<AssetRef> {
    let mut out: Vec<AssetRef> = texts
        .into_iter()
        .flat_map(crate::script_refs::id_refs)
        .filter_map(|(a, k)| k.ref_kind().filter(|k| KINDS.contains(k)).map(|k| (k, a.value)))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Assets in `db` that refer to `a`.
pub fn dependents(db: &Database, a: &AssetRef) -> Vec<AssetRef> {
    all_assets(db).into_iter().filter(|b| deps(db, b).contains(a)).collect()
}

fn with_suffix(path: &str, n: u32) -> String {
    match path.rfind('.') {
        Some(dot) if !path[dot..].contains('/') => format!("{}_{n}{}", &path[..dot], &path[dot..]),
        _ => format!("{path}_{n}"),
    }
}

/// Paths inside a package must stay inside the project.
fn safe_rel(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.contains('\\') && p.split('/').all(|c| !c.is_empty() && c != "." && c != "..")
}

impl Bundle {
    /// Collects `wanted` plus everything they depend on from a project-like store.
    pub fn collect(db: &Database, root: &Path, tile_size: u32, name: &str, wanted: &[AssetRef]) -> Result<Bundle> {
        let mut seen: BTreeSet<AssetRef> = BTreeSet::new();
        let mut stack: Vec<AssetRef> = wanted.to_vec();
        while let Some(a) = stack.pop() {
            if a.0 == RefKind::Map || seen.contains(&a) || !exists(db, &a) {
                continue;
            }
            stack.extend(deps(db, &a));
            seen.insert(a);
        }
        let has = |k: RefKind, id: &str| seen.contains(&(k, id.to_string()));
        let out = Database {
            tilesets: db.tilesets.iter().filter(|d| has(RefKind::Tileset, &d.id)).cloned().collect(),
            sheets: db.sheets.iter().filter(|d| has(RefKind::Sheet, &d.id)).cloned().collect(),
            sounds: db.sounds.iter().filter(|d| has(RefKind::Sound, &d.id)).cloned().collect(),
            items: db.items.iter().filter(|d| has(RefKind::Item, &d.id)).cloned().collect(),
            props: db.props.iter().filter(|d| has(RefKind::Prop, &d.id)).cloned().collect(),
            characters: db.characters.iter().filter(|d| has(RefKind::Character, &d.id)).cloned().collect(),
            structures: db.structures.iter().filter(|d| has(RefKind::Structure, &d.id)).cloned().collect(),
            skills: db.skills.iter().filter(|d| has(RefKind::Skill, &d.id)).cloned().collect(),
            spells: db.spells.iter().filter(|d| has(RefKind::Spell, &d.id)).cloned().collect(),
            recipes: db.recipes.iter().filter(|d| has(RefKind::Recipe, &d.id)).cloned().collect(),
            quests: Vec::new(),
            achievements: Vec::new(),
        };
        let mut files = BTreeMap::new();
        let paths = out.tilesets.iter().map(|d| &d.image).chain(out.sheets.iter().map(|d| &d.image)).chain(out.sounds.iter().map(|d| &d.file));
        for rel in paths {
            if rel.is_empty() || files.contains_key(rel) {
                continue;
            }
            let bytes = std::fs::read(root.join(rel)).with_context(|| format!("reading {rel}"))?;
            files.insert(rel.clone(), bytes);
        }
        Ok(Bundle { name: name.to_string(), tile_size, db: out, files, scripts: BTreeMap::new() })
    }

    /// Like [`collect`](Self::collect), plus the script functions `fns` and the assets they
    /// refer to by id (e.g. the sprite sheets a spell's script flies and explodes).
    pub fn collect_with_scripts(db: &Database, root: &Path, tile_size: u32, name: &str, wanted: &[AssetRef], fns: &[ScriptFn]) -> Result<Bundle> {
        let mut all = wanted.to_vec();
        all.extend(script_asset_refs(fns.iter().map(|f| f.text.as_str())));
        let mut b = Self::collect(db, root, tile_size, name, &all)?;
        b.scripts = fns.iter().map(|f| (f.name.clone(), f.text.clone())).collect();
        Ok(b)
    }

    /// Collects `wanted` from a store whose scripts are already curated (the library), with
    /// every script function the collected assets need.
    pub fn collect_with_needed_scripts(db: &Database, root: &Path, tile_size: u32, name: &str, wanted: &[AssetRef]) -> Result<Bundle> {
        Self::collect_for_library(db, root, tile_size, name, wanted, false)
    }

    /// Like [`collect_with_needed_scripts`](Self::collect_with_needed_scripts); with
    /// `drop_script_triggers`, the triggers of items, props and characters that run script
    /// functions are left out first (a game's story scripts stay with the game, as in the
    /// editor's starter library), so only the functions of skills, spells and the remaining
    /// triggers travel.
    pub fn collect_for_library(db: &Database, root: &Path, tile_size: u32, name: &str, wanted: &[AssetRef], drop_script_triggers: bool) -> Result<Bundle> {
        let fns = read_scripts(root);
        let collect = |fns: &[ScriptFn]| -> Result<Bundle> {
            let mut b = Self::collect_with_scripts(db, root, tile_size, name, wanted, fns)?;
            if drop_script_triggers {
                b.drop_script_triggers();
            }
            Ok(b)
        };
        let mut b = collect(&[])?;
        // Functions bring assets, whose own scripts may need more functions: repeat until stable.
        loop {
            let roots: Vec<String> = all_assets(&b.db).iter().flat_map(|a| script_roots(&b.db, a)).collect();
            let needed = crate::script_refs::needed_functions(&fns, &roots);
            let chosen: Vec<ScriptFn> = fns.iter().filter(|f| needed.contains(&f.name)).cloned().collect();
            let next = collect(&chosen)?;
            let done = all_assets(&next.db).len() == all_assets(&b.db).len() && next.scripts.len() == b.scripts.len();
            b = next;
            if done {
                return Ok(b);
            }
        }
    }

    /// Removes the triggers of items (on use), props and characters that run a script function.
    pub fn drop_script_triggers(&mut self) {
        let scripted = |t: &crate::events::Trigger| t.actions.iter().any(|a| matches!(a, crate::events::Action::RunScript { .. }));
        for d in &mut self.db.items {
            d.on_use.retain(|t| !scripted(t));
        }
        for d in &mut self.db.props {
            d.triggers.retain(|t| !scripted(t));
        }
        for d in &mut self.db.characters {
            d.triggers.retain(|t| !scripted(t));
        }
    }

    pub fn is_empty(&self) -> bool {
        all_assets(&self.db).is_empty() && self.scripts.is_empty()
    }

    /// Writes a `.r2dpack` (zip) package.
    pub fn to_pack(&self) -> Result<Vec<u8>> {
        let m = Manifest {
            version: PACK_VERSION,
            name: self.name.clone(),
            tile_size: self.tile_size,
            tilesets: self.db.tilesets.clone(),
            sheets: self.db.sheets.clone(),
            sounds: self.db.sounds.clone(),
            items: self.db.items.clone(),
            props: self.db.props.clone(),
            characters: self.db.characters.clone(),
            structures: self.db.structures.clone(),
            skills: self.db.skills.clone(),
            spells: self.db.spells.clone(),
            recipes: self.db.recipes.clone(),
            scripts: self.scripts.clone(),
        };
        let cfg = ron::ser::PrettyConfig::default().struct_names(false).new_line("\n");
        let manifest = ron::ser::to_string_pretty(&m, cfg)?;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zw.start_file("manifest.ron", opts)?;
        zw.write_all(manifest.as_bytes())?;
        for (rel, bytes) in &self.files {
            zw.start_file(format!("files/{rel}"), opts)?;
            zw.write_all(bytes)?;
        }
        Ok(zw.finish()?.into_inner())
    }

    /// Reads a `.r2dpack` package.
    pub fn from_pack(bytes: &[u8]) -> Result<Bundle> {
        let mut za = zip::ZipArchive::new(Cursor::new(bytes)).context("not an asset package")?;
        let mut manifest = String::new();
        za.by_name("manifest.ron").context("package has no manifest.ron")?.read_to_string(&mut manifest)?;
        let m: Manifest = ron::from_str(&manifest).context("parsing manifest.ron")?;
        if m.version > PACK_VERSION {
            bail!("package version {} is newer than this editor supports ({PACK_VERSION})", m.version);
        }
        let mut files = BTreeMap::new();
        for i in 0..za.len() {
            let mut f = za.by_index(i)?;
            let name = f.name()?.to_string();
            let Some(rel) = name.strip_prefix("files/").map(str::to_string) else { continue };
            if f.is_dir() {
                continue;
            }
            if !safe_rel(&rel) {
                bail!("unsafe path in package: {rel}");
            }
            let mut data = Vec::new();
            f.read_to_end(&mut data)?;
            files.insert(rel, data);
        }
        let db = Database { tilesets: m.tilesets, sheets: m.sheets, sounds: m.sounds, items: m.items, props: m.props, quests: Vec::new(), characters: m.characters, structures: m.structures, skills: m.skills, spells: m.spells, recipes: m.recipes, achievements: Vec::new() };
        let b = Bundle { name: m.name, tile_size: m.tile_size, db, files, scripts: m.scripts };
        for rel in b.db.tilesets.iter().map(|d| &d.image).chain(b.db.sheets.iter().map(|d| &d.image)).chain(b.db.sounds.iter().map(|d| &d.file)) {
            if !rel.is_empty() && !b.files.contains_key(rel) {
                bail!("package is missing {rel}");
            }
        }
        Ok(b)
    }

    /// Reads the files of a library folder (`project.ron`, `defs/*.ron`, `scripts/*.rhai` and
    /// the art / sounds the definitions name), e.g. the starter library embedded in the editor.
    /// Paths are relative to the folder with `/`; quests and achievements are left out, as in
    /// packages.
    pub fn from_files(files: &[(&str, &[u8])]) -> Result<Bundle> {
        let text = |rel: &str| -> Result<Option<String>> {
            files.iter().find(|(p, _)| *p == rel).map(|(_, b)| String::from_utf8(b.to_vec()).with_context(|| format!("{rel} is not UTF-8"))).transpose()
        };
        let config: ProjectConfig = ron::from_str(&text("project.ron")?.context("the library has no project.ron")?).context("parsing project.ron")?;
        let mut db = crate::project::parse_defs(|n| text(&format!("defs/{n}.ron")))?;
        db.quests.clear();
        db.achievements.clear();
        let wanted = files_of(&db);
        let mut out = BTreeMap::new();
        for (rel, bytes) in files {
            if wanted.contains(*rel) {
                if !safe_rel(rel) {
                    bail!("unsafe path in the library: {rel}");
                }
                out.insert(rel.to_string(), bytes.to_vec());
            }
        }
        if let Some(rel) = wanted.iter().find(|r| !r.is_empty() && !out.contains_key(*r)) {
            bail!("the library is missing {rel}");
        }
        let mut scripts = BTreeMap::new();
        let mut rhai: Vec<&(&str, &[u8])> = files.iter().filter(|(p, _)| p.starts_with("scripts/") && p.ends_with(".rhai")).collect();
        rhai.sort_by_key(|(p, _)| *p);
        for (rel, _) in rhai {
            for f in crate::script_refs::functions(&text(rel)?.unwrap_or_default()) {
                scripts.entry(f.name).or_insert(f.text);
            }
        }
        Ok(Bundle { name: config.name, tile_size: config.tile_size, db, files: out, scripts })
    }

    /// The bundle as the files of a library folder (the reverse of [`from_files`](Self::from_files)).
    pub fn to_files(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let mut config = ProjectConfig::new(&self.name);
        config.tile_size = self.tile_size;
        let mut out = vec![("project.ron".to_string(), crate::project::ron_text(&config)?.into_bytes())];
        let db = &self.db;
        let defs: [(&str, String); 10] = [
            ("tilesets", crate::project::ron_text(&db.tilesets)?),
            ("sheets", crate::project::ron_text(&db.sheets)?),
            ("items", crate::project::ron_text(&db.items)?),
            ("props", crate::project::ron_text(&db.props)?),
            ("characters", crate::project::ron_text(&db.characters)?),
            ("sounds", crate::project::ron_text(&db.sounds)?),
            ("structures", crate::project::ron_text(&db.structures)?),
            ("skills", crate::project::ron_text(&db.skills)?),
            ("spells", crate::project::ron_text(&db.spells)?),
            ("recipes", crate::project::ron_text(&db.recipes)?),
        ];
        out.extend(defs.into_iter().map(|(n, t)| (format!("defs/{n}.ron"), t.into_bytes())));
        if !self.scripts.is_empty() {
            let mut src = LIBRARY_SCRIPT_HEADER.to_string();
            for text in self.scripts.values() {
                src += &format!("\n{}\n", text.trim_end());
            }
            out.push((format!("scripts/{LIBRARY_SCRIPT}"), src.into_bytes()));
        }
        out.extend(self.files.iter().map(|(p, b)| (p.clone(), b.clone())));
        out.sort();
        Ok(out)
    }

    /// Rescales the art and pixel-based values (frame sizes, hitboxes, speeds) for a project
    /// with tile size `to` (nearest-neighbour, so 16 → 32 is lossless).
    pub fn rescale(&mut self, to: u32) -> Result<()> {
        if to == 0 || self.tile_size == 0 || self.tile_size == to {
            self.tile_size = to.max(self.tile_size);
            return Ok(());
        }
        let f = to as f32 / self.tile_size as f32;
        let sc = |v: u32| ((v as f32 * f).round() as u32).max(1);
        let images: BTreeSet<String> = self.db.tilesets.iter().map(|d| d.image.clone()).chain(self.db.sheets.iter().map(|d| d.image.clone())).collect();
        for rel in images {
            let Some(bytes) = self.files.get(&rel) else { continue };
            let img = Rgba::decode(bytes).with_context(|| format!("rescaling {rel} (only PNG images can be rescaled)"))?;
            let out = img.resized(sc(img.w), sc(img.h));
            self.files.insert(rel, out.encode()?);
        }
        for s in &mut self.db.sheets {
            s.frame_w = sc(s.frame_w);
            s.frame_h = sc(s.frame_h);
        }
        let ranged = |w: &mut Option<Weapon>| {
            if let Some(r) = w.as_mut().and_then(|w| w.ranged.as_mut()) {
                r.speed *= f;
            }
        };
        for d in &mut self.db.items {
            ranged(&mut d.weapon);
        }
        for d in &mut self.db.props {
            d.hitbox = [d.hitbox[0] * f, d.hitbox[1] * f];
        }
        for e in self.db.structures.iter_mut().flat_map(|d| d.entities.iter_mut()) {
            e.x *= f;
            e.y *= f;
        }
        for d in &mut self.db.characters {
            d.hitbox = [d.hitbox[0] * f, d.hitbox[1] * f];
            d.speed *= f;
            ranged(&mut d.weapon);
        }
        self.tile_size = to;
        Ok(())
    }

    /// Renames an asset inside the bundle (its definition and every reference to it).
    fn rename(&mut self, kind: RefKind, old: &str, new: &str) {
        let set = |id: &mut String| {
            if id == old {
                *id = new.to_string();
            }
        };
        match kind {
            RefKind::Tileset => self.db.tilesets.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Sheet => self.db.sheets.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Sound => self.db.sounds.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Item => self.db.items.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Prop => self.db.props.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Character => self.db.characters.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Structure => self.db.structures.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Skill => self.db.skills.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Spell => self.db.spells.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Recipe => self.db.recipes.iter_mut().for_each(|d| set(&mut d.id)),
            RefKind::Map => {}
        }
        let mut cfg = ProjectConfig::new("");
        refactor::rename_in_db(&mut self.db, &mut cfg, kind, old, new);
        // Ids passed to API calls in the bundle's script functions follow too.
        for text in self.scripts.values_mut() {
            *text = crate::script_refs::rename_in_script(text, kind, old, new).0;
        }
    }

    /// Is the bundle's version of `a` identical to the one in `db`?
    fn same_as(&self, db: &Database, a: &AssetRef) -> bool {
        same_def(db, &self.db, a)
    }

    /// Adds only the assets whose ids `db` doesn't have yet (existing assets, possibly edited
    /// by the user, are left alone; new assets referring to them use the existing ones).
    pub fn add_missing(mut self, db: &mut Database, root: &Path) -> Result<MergeReport> {
        // Images the newer set only extends (more frames or tiles, the existing part unchanged)
        // are upgraded: its new assets may use the added frames (e.g. spell icons on "items").
        // Images the user changed differ in the existing part and stay as they are.
        let mut upgraded = Vec::new();
        let pairs = self.db.sheets.iter().filter_map(|n| {
            let o = db.sheet(&n.id)?;
            ((o.frame_w, o.frame_h) == (n.frame_w, n.frame_h)).then(|| ((RefKind::Sheet, n.id.clone()), o.image.clone(), n.image.clone()))
        });
        let pairs: Vec<_> = pairs.chain(self.db.tilesets.iter().filter_map(|n| Some(((RefKind::Tileset, n.id.clone()), db.tileset(&n.id)?.image.clone(), n.image.clone())))).collect();
        // Tiles of upgraded tilesets that were empty before (their flags come along).
        let mut fresh: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for (a, old_rel, new_rel) in pairs {
            let (Some(new_bytes), true) = (self.files.get(&new_rel), safe_rel(&old_rel)) else { continue };
            let path = root.join(&old_rel);
            let Ok(old_bytes) = std::fs::read(&path) else { continue };
            if extends(&old_bytes, new_bytes) {
                if a.0 == RefKind::Tileset {
                    fresh.insert(a.1.clone(), new_tiles(&old_bytes, new_bytes, TILE_SIZE));
                }
                std::fs::write(&path, new_bytes).with_context(|| format!("writing {old_rel}"))?;
                upgraded.push(a);
            }
        }
        // The new tiles of an upgraded tileset bring their flags (solid, ladder, light,
        // animation); flags of the tiles that were there stay as the library has them.
        for n in &self.db.tilesets {
            let (Some(tiles), Some(o)) = (fresh.get(&n.id), db.tilesets.iter_mut().find(|o| o.id == n.id)) else { continue };
            for (new, old) in [(&n.solid, &mut o.solid), (&n.ladder, &mut o.ladder)] {
                let add: Vec<u32> = new.iter().copied().filter(|t| tiles.contains(t) && !old.contains(t)).collect();
                old.extend(add);
                old.sort();
            }
            let anims: Vec<_> = n.animations.iter().filter(|a| tiles.contains(&a.tile) && !o.animations.iter().any(|x| x.tile == a.tile)).cloned().collect();
            o.animations.extend(anims);
            let lights: Vec<_> = n.lights.iter().filter(|l| tiles.contains(&l.tile) && !o.lights.iter().any(|x| x.tile == l.tile)).cloned().collect();
            o.lights.extend(lights);
        }
        // Body sheets the newer set marks playable get the flag when they are otherwise the
        // same (the flag was added later; an edited sheet stays as it is).
        for n in self.db.sheets.iter().filter(|n| n.playable) {
            if let Some(o) = db.sheets.iter_mut().find(|o| o.id == n.id && !o.playable) {
                if (SheetDef { playable: true, ..o.clone() }) == *n {
                    o.playable = true;
                    upgraded.push((RefKind::Sheet, n.id.clone()));
                }
            }
        }
        // Tilesets the newer set gives auto-tiling terrains get them (with the solid flags and
        // animations of the terrain tiles) when they have none yet and use the same (or an
        // upgraded) image.
        for n in self.db.tilesets.iter().filter(|n| !n.terrains.is_empty()) {
            let Some(o) = db.tilesets.iter_mut().find(|o| o.id == n.id && o.terrains.is_empty()) else { continue };
            let key = (RefKind::Tileset, n.id.clone());
            let same_image = upgraded.contains(&key) || (safe_rel(&o.image) && std::fs::read(root.join(&o.image)).ok().as_ref() == self.files.get(&n.image));
            if !same_image {
                continue;
            }
            o.terrains = n.terrains.clone();
            for &t in n.solid.iter().filter(|&&t| n.terrains.iter().any(|tr| tr.contains(t))) {
                if !o.solid.contains(&t) {
                    o.solid.push(t);
                }
            }
            o.solid.sort();
            // Animated terrain tiles (water insides cycling through the water frames).
            for a in n.animations.iter().filter(|a| n.terrains.iter().any(|tr| tr.contains(a.tile))) {
                if !o.animations.iter().any(|x| x.tile == a.tile) {
                    o.animations.push(a.clone());
                }
            }
            if !upgraded.contains(&key) {
                upgraded.push(key);
            }
        }
        // New structures place tiles by index: they need the tilesets as the set has them. A
        // tileset the target has with other art (not upgraded above) would get wrong tiles.
        let other_art: BTreeSet<String> = self
            .db
            .tilesets
            .iter()
            .filter(|n| !upgraded.contains(&(RefKind::Tileset, n.id.clone())))
            .filter_map(|n| {
                let o = db.tileset(&n.id)?;
                let mine = safe_rel(&o.image).then(|| std::fs::read(root.join(&o.image)).ok()).flatten();
                let same = match (mine.as_deref().map(Rgba::decode), self.files.get(&n.image).map(|b| Rgba::decode(b))) {
                    (Some(Ok(a)), Some(Ok(b))) => a == b,
                    _ => true,
                };
                (!same).then(|| n.id.clone())
            })
            .collect();
        let mut skipped = Vec::new();
        self.db.structures.retain(|d| {
            let ok = exists(db, &(RefKind::Structure, d.id.clone())) || !d.tilesets.iter().any(|t| other_art.contains(t));
            if !ok {
                skipped.push((RefKind::Structure, d.id.clone()));
            }
            ok
        });
        let new = |k: RefKind, id: &str| !exists(db, &(k, id.to_string()));
        self.db.tilesets.retain(|d| new(RefKind::Tileset, &d.id));
        self.db.sheets.retain(|d| new(RefKind::Sheet, &d.id));
        self.db.sounds.retain(|d| new(RefKind::Sound, &d.id));
        self.db.items.retain(|d| new(RefKind::Item, &d.id));
        self.db.props.retain(|d| new(RefKind::Prop, &d.id));
        self.db.characters.retain(|d| new(RefKind::Character, &d.id));
        self.db.structures.retain(|d| new(RefKind::Structure, &d.id));
        self.db.skills.retain(|d| new(RefKind::Skill, &d.id));
        self.db.spells.retain(|d| new(RefKind::Spell, &d.id));
        self.db.recipes.retain(|d| new(RefKind::Recipe, &d.id));
        let used = files_of(&self.db);
        self.files.retain(|rel, _| used.contains(rel));
        let mut report = self.merge_into(db, root)?;
        report.upgraded = upgraded;
        report.skipped = skipped;
        Ok(report)
    }

    /// Adds the bundle to a store at `root`, replacing the store's version of every asset it
    /// has (definition in place, files at the bundle's paths, script functions of the same name
    /// in `scripts/library.rhai`): how updated assets go back to the library they belong to,
    /// e.g. the example project's into the editor's starter library. Files are written right
    /// away; the caller saves the definitions.
    pub fn replace_into(self, db: &mut Database, root: &Path) -> Result<MergeReport> {
        let mut report = MergeReport::default();
        for (rel, bytes) in &self.files {
            if !safe_rel(rel) {
                bail!("unsafe path: {rel}");
            }
            let p = root.join(rel);
            if std::fs::read(&p).ok().as_deref() != Some(bytes.as_slice()) {
                if let Some(dir) = p.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&p, bytes).with_context(|| format!("writing {rel}"))?;
                report.files.push(rel.clone());
            }
        }
        let file_of = |a: &AssetRef| match a.0 {
            RefKind::Tileset => self.db.tileset(&a.1).map(|d| d.image.clone()),
            RefKind::Sheet => self.db.sheet(&a.1).map(|d| d.image.clone()),
            RefKind::Sound => self.db.sounds.iter().find(|d| d.id == a.1).map(|d| d.file.clone()),
            _ => None,
        };
        for a in all_assets(&self.db) {
            if !exists(db, &a) {
                report.added.push(a.clone());
            } else if same_def(db, &self.db, &a) && !file_of(&a).is_some_and(|f| report.files.contains(&f)) {
                report.existing.push(a);
                continue;
            } else {
                report.replaced.push(a.clone());
            }
            put_def(db, &self.db, &a);
        }
        if !self.scripts.is_empty() {
            let path = root.join("scripts").join(LIBRARY_SCRIPT);
            let old = std::fs::read_to_string(&path).ok();
            let mut src = old.clone().unwrap_or_else(|| LIBRARY_SCRIPT_HEADER.to_string());
            let here = crate::script_refs::functions(&src);
            let elsewhere: BTreeSet<String> = read_scripts(root).into_iter().map(|f| f.name).filter(|n| !here.iter().any(|h| h.name == *n)).collect();
            for (name, text) in &self.scripts {
                match here.iter().find(|f| f.name == *name) {
                    Some(f) if f.text.trim() == text.trim() => {}
                    Some(f) => {
                        src = src.replacen(f.text.trim_end(), text.trim_end(), 1);
                        report.scripts_replaced.push(name.clone());
                    }
                    // Defined in another script file of the store: that one stays.
                    None if elsewhere.contains(name) => report.scripts_kept.push(name.clone()),
                    None => {
                        src += &format!("\n{}\n", text.trim_end());
                        report.scripts_added.push(name.clone());
                    }
                }
            }
            if old.as_deref() != Some(src.as_str()) {
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(&path, src).with_context(|| format!("writing {}", path.display()))?;
            }
        }
        Ok(report)
    }

    /// Adds the bundle to a project (or the library) at `root`. Files are written right away;
    /// the caller saves the definitions.
    pub fn merge_into(mut self, db: &mut Database, root: &Path) -> Result<MergeReport> {
        let mut report = MergeReport::default();
        // 1. Files: reuse identical files, never overwrite different ones.
        let mut moved: BTreeMap<String, String> = BTreeMap::new();
        for (rel, bytes) in &self.files {
            if !safe_rel(rel) {
                bail!("unsafe path: {rel}");
            }
            let mut target = rel.clone();
            let mut n = 2;
            loop {
                let p = root.join(&target);
                if !p.exists() {
                    if let Some(dir) = p.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&p, bytes).with_context(|| format!("writing {target}"))?;
                    report.files.push(target.clone());
                    break;
                }
                if std::fs::read(&p)? == *bytes {
                    break;
                }
                target = with_suffix(rel, n);
                n += 1;
            }
            if target != *rel {
                moved.insert(rel.clone(), target);
            }
        }
        let fix = |p: &mut String| {
            if let Some(n) = moved.get(p) {
                *p = n.clone();
            }
        };
        self.db.tilesets.iter_mut().for_each(|d| fix(&mut d.image));
        self.db.sheets.iter_mut().for_each(|d| fix(&mut d.image));
        self.db.sounds.iter_mut().for_each(|d| fix(&mut d.file));

        // 2. Ids: an id that exists with different content is renamed (and the rename is
        // followed by the bundle's references). Repeat until stable, since a rename can make
        // dependents differ too.
        let mut renamed_to: BTreeSet<AssetRef> = BTreeSet::new();
        loop {
            let mut changed = false;
            for a in all_assets(&self.db) {
                if renamed_to.contains(&a) || !exists(db, &a) || self.same_as(db, &a) {
                    continue;
                }
                let mut n = 2;
                let new = loop {
                    let cand = format!("{}_{n}", a.1);
                    let r = (a.0, cand.clone());
                    if !exists(db, &r) && !exists(&self.db, &r) {
                        break cand;
                    }
                    n += 1;
                };
                self.rename(a.0, &a.1, &new);
                report.renamed.push((a.0, a.1.clone(), new.clone()));
                renamed_to.insert((a.0, new));
                changed = true;
            }
            if !changed {
                break;
            }
        }

        // 3. Add what is new.
        for a in all_assets(&self.db) {
            if exists(db, &a) {
                report.existing.push(a);
                continue;
            }
            let id = a.1.as_str();
            match a.0 {
                RefKind::Tileset => db.tilesets.extend(self.db.tileset(id).cloned()),
                RefKind::Sheet => db.sheets.extend(self.db.sheet(id).cloned()),
                RefKind::Sound => db.sounds.extend(self.db.sounds.iter().find(|d| d.id == id).cloned()),
                RefKind::Item => db.items.extend(self.db.item(id).cloned()),
                RefKind::Prop => db.props.extend(self.db.prop(id).cloned()),
                RefKind::Character => db.characters.extend(self.db.character(id).cloned()),
                RefKind::Structure => db.structures.extend(self.db.structure(id).cloned()),
                RefKind::Skill => db.skills.extend(self.db.skill(id).cloned()),
                RefKind::Spell => db.spells.extend(self.db.spell(id).cloned()),
                RefKind::Recipe => db.recipes.extend(self.db.recipe(id).cloned()),
                RefKind::Map => {}
            }
            report.added.push(a);
        }

        // 4. Script functions: append new ones to scripts/library.rhai; a function the target
        // already has stays as it is (the user may have changed it).
        if !self.scripts.is_empty() {
            let have: BTreeMap<String, String> = read_scripts(root).into_iter().map(|f| (f.name, f.text)).collect();
            let mut add = String::new();
            for (name, text) in &self.scripts {
                match have.get(name) {
                    Some(t) if t.trim() == text.trim() => {}
                    Some(_) => report.scripts_kept.push(name.clone()),
                    None => {
                        add += &format!("\n{}\n", text.trim_end());
                        report.scripts_added.push(name.clone());
                    }
                }
            }
            if !add.is_empty() {
                let path = root.join("scripts").join(LIBRARY_SCRIPT);
                let mut src = std::fs::read_to_string(&path).unwrap_or_else(|_| LIBRARY_SCRIPT_HEADER.to_string());
                src += &add;
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(&path, src).with_context(|| format!("writing {}", path.display()))?;
            }
        }
        Ok(report)
    }
}

const LIBRARY_SCRIPT_HEADER: &str = "// Script functions that came with assets from the asset library or asset packages\n// (spells, skills, triggers). Edit them like any other script.\n";

/// Removes assets from a project-like store and deletes files nothing refers to anymore.
/// Refuses (with the list of users) if other remaining assets depend on them.
pub fn remove_assets(project: &mut Project, assets: &[AssetRef]) -> Result<()> {
    let gone: BTreeSet<&AssetRef> = assets.iter().collect();
    for a in assets {
        let users: Vec<String> = dependents(&project.db, a).into_iter().filter(|u| !gone.contains(u)).map(|u| u.1).collect();
        if !users.is_empty() {
            bail!("{} '{}' is used by: {}", kind_label(a.0), a.1, users.join(", "));
        }
    }
    let files_before: BTreeSet<String> = files_of(&project.db);
    let db = &mut project.db;
    let keep = |k: RefKind, id: &str| !gone.contains(&(k, id.to_string()));
    db.tilesets.retain(|d| keep(RefKind::Tileset, &d.id));
    db.sheets.retain(|d| keep(RefKind::Sheet, &d.id));
    db.sounds.retain(|d| keep(RefKind::Sound, &d.id));
    db.items.retain(|d| keep(RefKind::Item, &d.id));
    db.props.retain(|d| keep(RefKind::Prop, &d.id));
    db.characters.retain(|d| keep(RefKind::Character, &d.id));
    db.structures.retain(|d| keep(RefKind::Structure, &d.id));
    db.skills.retain(|d| keep(RefKind::Skill, &d.id));
    db.spells.retain(|d| keep(RefKind::Spell, &d.id));
    db.recipes.retain(|d| keep(RefKind::Recipe, &d.id));
    let files_after = files_of(&project.db);
    for f in files_before.difference(&files_after) {
        if safe_rel(f) {
            let _ = std::fs::remove_file(project.root.join(f));
        }
    }
    prune_library_script(project)?;
    project.save_defs()
}

/// Drops the functions of `scripts/library.rhai` that no remaining asset needs anymore.
fn prune_library_script(project: &Project) -> Result<()> {
    let path = project.root.join("scripts").join(LIBRARY_SCRIPT);
    let Ok(src) = std::fs::read_to_string(&path) else { return Ok(()) };
    let roots: Vec<String> = all_assets(&project.db).iter().flat_map(|a| script_roots(&project.db, a)).collect();
    let needed = crate::script_refs::needed_functions(&read_scripts(&project.root), &roots);
    let fns = crate::script_refs::functions(&src);
    if fns.iter().all(|f| needed.contains(&f.name)) {
        return Ok(());
    }
    let mut out = LIBRARY_SCRIPT_HEADER.to_string();
    for f in fns.iter().filter(|f| needed.contains(&f.name)) {
        out += &format!("\n{}\n", f.text.trim_end());
    }
    std::fs::write(&path, out).with_context(|| format!("writing {}", path.display()))
}

/// Is `new` a larger version of image `old` that keeps all of `old`'s pixels in place?
fn extends(old: &[u8], new: &[u8]) -> bool {
    let (Ok(o), Ok(n)) = (Rgba::decode(old), Rgba::decode(new)) else { return false };
    if n.w < o.w || n.h < o.h || old == new {
        return false;
    }
    // Every pixel the old image has is still there; new art only where it was empty
    // (more frames or tiles, or art in cells that were blank).
    let mut changed = (n.w, n.h) != (o.w, o.h);
    for y in 0..o.h as usize {
        for x in 0..o.w as usize {
            let (a, b) = ((y * o.w as usize + x) * 4, (y * n.w as usize + x) * 4);
            let (op, np) = (&o.px[a..a + 4], &n.px[b..b + 4]);
            if op[3] == 0 {
                changed |= np[3] != 0;
            } else if op != np {
                return false;
            }
        }
    }
    changed
}

/// Tile indices (in the new image's columns) whose cells were empty or missing in `old` and
/// have art in `new`.
fn new_tiles(old: &[u8], new: &[u8], ts: u32) -> BTreeSet<u32> {
    let (Ok(o), Ok(n)) = (Rgba::decode(old), Rgba::decode(new)) else { return BTreeSet::new() };
    let blank = |img: &Rgba, c: u32, r: u32| {
        (r * ts..((r + 1) * ts).min(img.h)).all(|y| (c * ts..((c + 1) * ts).min(img.w)).all(|x| img.px[((y * img.w + x) * 4 + 3) as usize] == 0))
    };
    let cols = n.w / ts.max(1);
    let mut out = BTreeSet::new();
    for r in 0..n.h / ts.max(1) {
        for c in 0..cols {
            let was_empty = (c + 1) * ts > o.w || (r + 1) * ts > o.h || blank(&o, c, r);
            if was_empty && !blank(&n, c, r) {
                out.insert(r * cols + c);
            }
        }
    }
    out
}

fn files_of(db: &Database) -> BTreeSet<String> {
    db.tilesets.iter().map(|d| d.image.clone()).chain(db.sheets.iter().map(|d| d.image.clone())).chain(db.sounds.iter().map(|d| d.file.clone())).collect()
}

/// Folder of the shared asset library: `$RUSTPG2D_LIBRARY`, else
/// `$XDG_DATA_HOME/rustpg2d/library`, else `~/.local/share/rustpg2d/library` (Windows:
/// `%LOCALAPPDATA%\rustpg2d\library`).
pub fn library_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("RUSTPG2D_LIBRARY") {
        return PathBuf::from(p);
    }
    let data = crate::paths::data_base().unwrap_or_else(|| PathBuf::from("."));
    let dir = data.join("rustpg2d/library");
    crate::legacy::adopt(&dir, &data.join(crate::legacy::OLD_NAME).join("library"));
    dir
}

/// The files of a library folder embedded in a program: paths relative to the folder (`/`
/// separators) and contents. The editor embeds its starter library this way
/// (`crates/editor/assets/starter_library`).
pub type LibraryFiles<'a> = [(&'a str, &'a [u8])];

/// Opens the library at `dir`, creating it (filled with the `starter` library's assets) if
/// needed. A changed starter set (a newer editor) adds the assets the library doesn't have yet.
pub fn open_library(dir: &Path, starter: Option<&LibraryFiles>) -> Result<Project> {
    let stamp_file = dir.join(STARTER_STAMP);
    // The stamp includes how starter sets are applied, so libraries get a changed rule once.
    let stamp = starter.map(|f| format!("{STARTER_RULES}:{:016x}", files_hash(f)));
    if !dir.join("project.ron").exists() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let bundle = starter.map(Bundle::from_files).transpose().context("reading the starter assets")?;
        let mut lib = Project { root: dir.to_path_buf(), config: ProjectConfig::new("Asset library"), db: Database::default() };
        if let Some(mut b) = bundle {
            b.rescale(TILE_SIZE)?;
            b.merge_into(&mut lib.db, dir)?;
        }
        lib.save_config()?;
        lib.save_defs()?;
        if let Some(s) = &stamp {
            std::fs::write(&stamp_file, s)?;
        }
    }
    // Libraries from before tiles were fixed at 32 px.
    crate::migrate::upgrade(dir, None)?;
    let mut lib = Project::load(dir)?;
    // A newer starter set (new editor version): add the assets the library doesn't have yet.
    if let (Some(files), Some(s)) = (starter, &stamp) {
        if std::fs::read_to_string(&stamp_file).ok().as_deref() != Some(s.as_str()) {
            let mut b = Bundle::from_files(files).context("reading the starter assets")?;
            b.rescale(TILE_SIZE)?;
            let report = b.add_missing(&mut lib.db, dir)?;
            if !report.added.is_empty() || !report.upgraded.is_empty() {
                lib.save_defs()?;
            }
            std::fs::write(&stamp_file, s)?;
        }
    }
    Ok(lib)
}

/// File in the library recording which starter set was last applied.
const STARTER_STAMP: &str = "starter.stamp";
/// Version of the rules for applying starter sets (2: images that only grew are upgraded;
/// 4: bodies get the playable flag; 5: tilesets get their auto-tiling terrains; 6: the terrain
/// sheet is 16 columns wide with a 47-tile set per terrain — not an extension of the old
/// sheet, so existing libraries keep their art until they are rebuilt; 7: an image of the
/// same size that only fills empty cells is an upgrade too, and the new tiles bring their
/// solid / ladder / light / animation flags).
const STARTER_RULES: u32 = 7;

/// 64-bit FNV-1a hash of every path and content (a cheap fingerprint of the starter set that
/// only changes when the files do).
fn files_hash(files: &LibraryFiles) -> u64 {
    let mut sorted: Vec<&(&str, &[u8])> = files.iter().collect();
    sorted.sort_by_key(|(p, _)| *p);
    let step = |h: u64, b: &u8| (h ^ *b as u64).wrapping_mul(0x100000001b3);
    sorted.iter().fold(0xcbf29ce484222325u64, |h, (p, b)| {
        let h = p.as_bytes().iter().chain(&[0u8]).fold(h, step);
        let h = (b.len() as u64).to_le_bytes().iter().fold(h, step);
        b.iter().fold(h, step)
    })
}

/// Adds a bundle to the library (rescaled to the library's tile size) and saves it.
pub fn add_to_library(lib: &mut Project, mut bundle: Bundle) -> Result<MergeReport> {
    bundle.rescale(TILE_SIZE)?;
    let root = lib.root.clone();
    let report = bundle.merge_into(&mut lib.db, &root)?;
    lib.save_defs()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustpg2d_lib_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn borrowed(files: &[(String, Vec<u8>)]) -> Vec<(&str, &[u8])> {
        files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect()
    }

    fn png(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
        let mut img = Rgba::new(w, h);
        img.px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| *p = c);
        img.encode().unwrap()
    }

    /// A source project: an "arrows" item, a "bow" using them, both on sheet "items".
    fn source(test: &str) -> (PathBuf, Database) {
        let root = tmp(&format!("src_{test}"));
        std::fs::create_dir_all(root.join("sprites")).unwrap();
        std::fs::write(root.join("sprites/items.png"), png(32, 16, [255, 0, 0, 255])).unwrap();
        let item = |id: &str| ItemDef {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            icon: SpriteRef::new("items", 0),
            consumable: false,
            on_use: vec![],
            weapon: None,
            armor: None,
            max_stack: 99,
            value: 0,
        light: None,
        teaches: String::new(),
        held: String::new(),
        tool: String::new(),
        use_on: Default::default(),
        crop: None,
        food: false,
        };
        let mut bow = item("bow");
        bow.weapon = Some(Weapon::ranged(1, 0.5, Ranged { projectile: SpriteRef::new("items", 1), speed: 100.0, range: 6.0, ammo: "arrows".into() }));
        let db = Database {
            sheets: vec![SheetDef { id: "items".into(), image: "sprites/items.png".into(), frame_w: 16, frame_h: 16, kind: Default::default(), female: false, playable: false, tags: vec![] }],
            items: vec![item("arrows"), bow],
            ..Default::default()
        };
        (root, db)
    }

    #[test]
    fn replace_updates_existing_assets_in_place() {
        let (root, db) = source("replace");
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts/main.rhai"), "fn arrows_used() { say(\"new\"); }\nfn story() { say(\"tale\"); }\n").unwrap();
        let dst = tmp("replace_dst");
        let _ = std::fs::remove_dir_all(&dst);
        let mut lib = Database::default();
        Bundle::collect(&db, &root, 32, "", &all_assets(&db)).unwrap().merge_into(&mut lib, &dst).unwrap();
        std::fs::create_dir_all(dst.join("scripts")).unwrap();
        std::fs::write(dst.join("scripts/library.rhai"), format!("{LIBRARY_SCRIPT_HEADER}\nfn arrows_used() {{ say(\"old\"); }}\n")).unwrap();
        // The project changed the arrows, their picture and script; the bow is new to the store.
        let mut db2 = db.clone();
        db2.items[0].name = "Better arrows".into();
        db2.items[0].on_use = vec![crate::events::Trigger { actions: vec![crate::events::Action::RunScript { function: "arrows_used".into() }], ..crate::events::Trigger::new(crate::events::TriggerOn::Interact) }];
        db2.items[1].on_use = vec![crate::events::Trigger { actions: vec![crate::events::Action::RunScript { function: "story".into() }], ..crate::events::Trigger::new(crate::events::TriggerOn::Interact) }];
        lib.items.retain(|d| d.id != "bow");
        std::fs::write(root.join("sprites/items.png"), png(32, 16, [0, 0, 255, 255])).unwrap();
        let b = Bundle::collect_for_library(&db2, &root, 32, "", &all_assets(&db2), false).unwrap();
        assert_eq!(b.scripts.keys().collect::<Vec<_>>(), ["arrows_used", "story"]);
        let report = b.replace_into(&mut lib, &dst).unwrap();
        assert_eq!(lib.items.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["Better arrows", "bow"], "replaced in place, new one appended");
        assert_eq!(report.added, vec![(RefKind::Item, "bow".to_string())]);
        assert!(report.replaced.contains(&(RefKind::Sheet, "items".to_string())), "its picture changed");
        assert_eq!(std::fs::read(dst.join("sprites/items.png")).unwrap(), std::fs::read(root.join("sprites/items.png")).unwrap());
        let src = std::fs::read_to_string(dst.join("scripts/library.rhai")).unwrap();
        assert!(src.contains("say(\"new\")") && !src.contains("say(\"old\")") && src.contains("fn story()"), "{src}");
        assert_eq!(report.scripts_replaced, ["arrows_used"]);
        // Story triggers can stay behind.
        let b = Bundle::collect_for_library(&db2, &root, 32, "", &all_assets(&db2), true).unwrap();
        assert!(b.db.items.iter().all(|d| d.on_use.is_empty()) && b.scripts.is_empty());
        let _ = (std::fs::remove_dir_all(root), std::fs::remove_dir_all(dst));
    }

    #[test]
    fn collect_pack_and_merge() {
        let (root, db) = source("merge");
        // Collecting the bow brings its ammo and the sheet along.
        let b = Bundle::collect(&db, &root, 16, "bow", &[(RefKind::Item, "bow".into())]).unwrap();
        assert_eq!(b.db.items.len(), 2);
        assert_eq!(b.db.sheets.len(), 1);
        assert!(b.files.contains_key("sprites/items.png"));
        let b = Bundle::from_pack(&b.to_pack().unwrap()).unwrap();
        assert_eq!(b.db.items.len(), 2);

        // Into an empty project: everything is added.
        let dest = tmp("dest");
        let mut d = Database::default();
        let r = b.clone().merge_into(&mut d, &dest).unwrap();
        assert_eq!(r.added.len(), 3);
        assert!(dest.join("sprites/items.png").exists());
        // Again: all identical, nothing new.
        let r = b.clone().merge_into(&mut d, &dest).unwrap();
        assert_eq!((r.added.len(), r.existing.len()), (0, 3));

        // A project with a different "items" sheet image: the file and the sheet get new
        // names, and the items (which now differ) are renamed with their references.
        let other = tmp("other");
        std::fs::create_dir_all(other.join("sprites")).unwrap();
        std::fs::write(other.join("sprites/items.png"), png(32, 16, [0, 255, 0, 255])).unwrap();
        let mut d2 = Database { sheets: db.sheets.clone(), items: db.items.clone(), ..Default::default() };
        let r = b.merge_into(&mut d2, &other).unwrap();
        assert!(other.join("sprites/items_2.png").exists());
        assert_eq!(d2.sheet("items_2").unwrap().image, "sprites/items_2.png");
        let bow2 = d2.item("bow_2").unwrap();
        assert_eq!(bow2.icon.sheet, "items_2");
        assert_eq!(bow2.weapon.as_ref().unwrap().ranged.as_ref().unwrap().ammo, "arrows_2");
        assert_eq!(r.renamed.len(), 3);
        for p in [root, dest, other] {
            let _ = std::fs::remove_dir_all(p);
        }
    }

    #[test]
    fn recipes_bring_their_items() {
        let (root, mut db) = source("recipes");
        db.recipes.push(crate::crafting::RecipeDef::new("fletch", "arrows", 10, vec![crate::inventory::ItemStack::new("bow", 1)], "bench"));
        let r = (RefKind::Recipe, "fletch".to_string());
        let b = Bundle::collect(&db, &root, 16, "x", std::slice::from_ref(&r)).unwrap();
        assert!(b.db.item("arrows").is_some() && b.db.item("bow").is_some() && b.db.sheet("items").is_some());
        let b = Bundle::from_pack(&b.to_pack().unwrap()).unwrap();
        assert_eq!(b.db.recipe("fletch"), db.recipe("fletch"));
        // Renaming an item in a project follows into its recipes.
        let mut cfg = ProjectConfig::new("p");
        assert_eq!(refactor::rename_in_db(&mut db, &mut cfg, RefKind::Item, "bow", "longbow"), 1, "only the recipe ingredient refers to the bow");
        assert_eq!(db.recipe("fletch").unwrap().ingredients[0].item, "longbow");
    }

    #[test]
    fn script_functions_travel_with_spells() {
        use crate::skills::{SpellDef, SpellEffect};
        let (root, mut db) = source("scripts");
        std::fs::write(root.join("sprites/boom.png"), png(16, 16, [9, 9, 9, 255])).unwrap();
        db.sheets.push(SheetDef { id: "boom".into(), image: "sprites/boom.png".into(), frame_w: 16, frame_h: 16, kind: Default::default(), female: false, playable: false, tags: vec![] });
        db.spells.push(SpellDef::new("fireball", "Fireball", "", SpriteRef::new("items", 0), 5, 1.0, SpellEffect::Script { function: "cast_fireball".into() }));
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        let src = "// Fireball: aim first.\n// (two comment lines)\nfn cast_fireball() {\n    aim(\"fireball_at\");\n}\n\nfn fireball_at(x, y) { if x > 0 { helper(); } effect(\"boom\", x, y); }\n\n// unrelated\nfn helper() { say(\"hi\"); }\n\nfn shop_talk() { say(\"{ not a brace }\"); }\n";
        std::fs::write(root.join("scripts/main.rhai"), src).unwrap();
        let fns = read_scripts(&root);
        assert_eq!(fns.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["cast_fireball", "fireball_at", "helper", "shop_talk"]);
        assert!(fns[0].text.starts_with("// Fireball: aim first.\n// (two comment lines)\nfn cast_fireball()"));
        assert_eq!(fns[1].params, ["x", "y"]);
        assert!(fns[2].text.starts_with("// unrelated"));
        let spell = (RefKind::Spell, "fireball".to_string());
        assert_eq!(script_roots(&db, &spell), ["cast_fireball"]);
        // The call graph follows calls and function names passed as strings, nothing else.
        let needed = crate::script_refs::needed_functions(&fns, &script_roots(&db, &spell));
        assert_eq!(needed.into_iter().collect::<Vec<_>>(), ["cast_fireball", "fireball_at", "helper"]);

        // Chosen functions travel with the bundle, and bring the sheet they use by id.
        let chosen: Vec<ScriptFn> = fns.iter().filter(|f| f.name != "shop_talk").cloned().collect();
        let b = Bundle::collect_with_scripts(&db, &root, 16, "x", std::slice::from_ref(&spell), &chosen).unwrap();
        assert!(b.db.sheet("boom").is_some());
        let b = Bundle::from_pack(&b.to_pack().unwrap()).unwrap();
        assert_eq!(b.scripts.keys().collect::<Vec<_>>(), ["cast_fireball", "fireball_at", "helper"]);

        // Into a library: appended to scripts/library.rhai, and found again from there.
        let lib_dir = tmp("scripts_lib");
        let mut lib = Project { root: lib_dir.clone(), config: ProjectConfig::new("lib"), db: Database::default() };
        lib.save_config().unwrap();
        let r = add_to_library(&mut lib, b.clone()).unwrap();
        assert_eq!(r.scripts_added.len(), 3);
        let again = Bundle::collect_with_needed_scripts(&lib.db, &lib_dir, 32, "lib", std::slice::from_ref(&spell)).unwrap();
        assert_eq!(again.scripts.len(), 3);
        // A second time: nothing new; a project with its own different helper() keeps it.
        assert!(add_to_library(&mut lib, b.clone()).unwrap().scripts_added.is_empty());
        let proj = tmp("scripts_proj");
        std::fs::create_dir_all(proj.join("scripts")).unwrap();
        std::fs::write(proj.join("scripts/main.rhai"), "fn helper() { say(\"mine\"); }\n").unwrap();
        let mut pdb = Database::default();
        let r = again.clone().merge_into(&mut pdb, &proj).unwrap();
        assert_eq!((r.scripts_added.len(), r.scripts_kept.clone()), (2, vec!["helper".to_string()]));
        // A clashing sheet id is renamed, and the script follows.
        let other = tmp("scripts_other");
        std::fs::create_dir_all(other.join("sprites")).unwrap();
        std::fs::write(other.join("sprites/boom.png"), png(8, 8, [1, 1, 1, 255])).unwrap();
        let mut odb = Database { sheets: vec![SheetDef { id: "boom".into(), image: "sprites/boom.png".into(), frame_w: 8, frame_h: 8, kind: Default::default(), female: false, playable: false, tags: vec![] }], ..Default::default() };
        again.merge_into(&mut odb, &other).unwrap();
        let merged = std::fs::read_to_string(other.join("scripts").join(LIBRARY_SCRIPT)).unwrap();
        assert!(merged.contains("effect(\"boom_2\", x, y)"), "{merged}");
        // Removing the spell drops the functions nothing needs anymore.
        remove_assets(&mut lib, &[spell]).unwrap();
        assert!(read_scripts(&lib_dir).is_empty());
        for p in [root, lib_dir, proj, other] {
            let _ = std::fs::remove_dir_all(p);
        }
    }

    #[test]
    fn rescale_doubles_art_and_sizes() {
        let (root, mut db) = source("rescale");
        db.props.push(PropDef {
            id: "crate".into(),
            name: "Crate".into(),
            sprite: SpriteRef::new("items", 0),
            solid: true,
            hitbox: [12.0, 6.0],
            triggers: vec![],
            container: None,
            lock: None,
            default_state: default_state_name(),
            states: vec![],
            light: None,
            use_spot: None,
            gather: None,
            regrow: None,
            plot: false,
        });
        let mut b = Bundle::collect(&db, &root, 16, "x", &all_assets(&db)).unwrap();
        b.rescale(32).unwrap();
        let img = Rgba::decode(&b.files["sprites/items.png"]).unwrap();
        assert_eq!((img.w, img.h), (64, 32));
        assert_eq!(b.db.sheets[0].frame_w, 32);
        assert_eq!(b.db.props[0].hitbox, [24.0, 12.0]);
        assert_eq!(b.db.item("bow").unwrap().weapon.as_ref().unwrap().ranged.as_ref().unwrap().speed, 200.0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn newer_sets_add_terrains_to_existing_tilesets() {
        let src = tmp("terrain_src");
        let dst = tmp("terrain_dst");
        for d in [&src, &dst] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d.join("tilesets")).unwrap();
            std::fs::write(d.join("tilesets/t.png"), png(64, 64, [10, 200, 10, 255])).unwrap();
        }
        let mut pond = crate::autotile::Terrain { name: "Pond".into(), base: Some(0), ..Default::default() };
        pond.fill_from_block(0, 4);
        let ts = |solid: Vec<u32>, terrains| TilesetDef { id: "t".into(), image: "tilesets/t.png".into(), solid, ladder: vec![], animations: vec![], lights: vec![], terrains };
        let mut newer = Database { tilesets: vec![ts(vec![3, 5, 99], vec![pond.clone()])], ..Default::default() };
        newer.tilesets[0].animations = vec![TileAnim { tile: 15, frames: vec![15, 16], fps: 2.0 }, TileAnim { tile: 40, frames: vec![40, 41], fps: 2.0 }];
        let pack = Bundle::collect(&newer, &src, 32, "starter", &all_assets(&newer)).unwrap();
        let mut db = Database { tilesets: vec![ts(vec![3], vec![])], ..Default::default() };
        let report = pack.add_missing(&mut db, &dst).unwrap();
        assert_eq!(db.tilesets[0].terrains, vec![pond.clone()]);
        assert_eq!(db.tilesets[0].solid, vec![3, 5], "solid flags of terrain tiles only");
        assert_eq!(db.tilesets[0].animations.iter().map(|a| a.tile).collect::<Vec<_>>(), vec![15], "animations of terrain tiles only");
        assert_eq!(report.upgraded, vec![(RefKind::Tileset, "t".to_string())]);
        // A user's own image (different pixels) keeps its definition as it is.
        std::fs::write(dst.join("tilesets/t.png"), png(64, 64, [1, 2, 3, 255])).unwrap();
        let mut db = Database { tilesets: vec![ts(vec![], vec![])], ..Default::default() };
        let pack = Bundle::collect(&newer, &src, 32, "starter", &all_assets(&newer)).unwrap();
        pack.add_missing(&mut db, &dst).unwrap();
        assert!(db.tilesets[0].terrains.is_empty());
        let _ = (std::fs::remove_dir_all(src), std::fs::remove_dir_all(dst));
    }

    #[test]
    fn filled_cells_upgrade_with_their_flags() {
        // 64x64 tileset: tile 0 drawn; the newer one also draws tile 3 (a ladder) and tile 1 (solid).
        let img = |tiles: &[u32], c: u8| {
            let mut im = Rgba::new(64, 64);
            for &t in tiles {
                let (tx, ty) = ((t % 2) * 32, (t / 2) * 32);
                for y in ty..ty + 32 {
                    for x in tx..tx + 32 {
                        let k = ((y * 64 + x) * 4) as usize;
                        im.px[k..k + 4].copy_from_slice(&[c, 100, 50, 255]);
                    }
                }
            }
            im.encode().unwrap()
        };
        let (old, newer) = (img(&[0], 10), img(&[0, 1, 3], 10));
        assert!(extends(&old, &newer), "art only where it was empty");
        assert!(!extends(&old, &img(&[0, 1], 99)), "the existing tile changed");
        assert!(!extends(&old, &old), "nothing new");
        assert_eq!(new_tiles(&old, &newer, 32), BTreeSet::from([1, 3]));
        let src = tmp("fill_src");
        let dst = tmp("fill_dst");
        for (d, png) in [(&src, &newer), (&dst, &old)] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d.join("tilesets")).unwrap();
            std::fs::write(d.join("tilesets/t.png"), png).unwrap();
        }
        let ts = |solid: Vec<u32>, ladder: Vec<u32>| TilesetDef { id: "t".into(), image: "tilesets/t.png".into(), solid, ladder, animations: vec![], lights: vec![], terrains: vec![] };
        let newer_db = Database { tilesets: vec![ts(vec![0, 1], vec![3])], ..Default::default() };
        let pack = Bundle::collect(&newer_db, &src, 32, "starter", &all_assets(&newer_db)).unwrap();
        // The library's own tile 0 is walkable: that stays.
        let mut db = Database { tilesets: vec![ts(vec![], vec![])], ..Default::default() };
        let report = pack.add_missing(&mut db, &dst).unwrap();
        assert_eq!(std::fs::read(dst.join("tilesets/t.png")).unwrap(), newer);
        assert_eq!((db.tilesets[0].solid.clone(), db.tilesets[0].ladder.clone()), (vec![1], vec![3]));
        assert_eq!(report.upgraded, vec![(RefKind::Tileset, "t".to_string())]);
        let _ = (std::fs::remove_dir_all(src), std::fs::remove_dir_all(dst));
    }

    #[test]
    fn structures_wait_for_their_tileset_art() {
        let src = tmp("st_src");
        let dst = tmp("st_dst");
        for (d, c) in [(&src, [10, 200, 10, 255]), (&dst, [1, 2, 3, 255])] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d.join("tilesets")).unwrap();
            std::fs::write(d.join("tilesets/t.png"), png(64, 64, c)).unwrap();
        }
        let ts = TilesetDef { id: "t".into(), image: "tilesets/t.png".into(), solid: vec![], ladder: vec![], animations: vec![], lights: vec![], terrains: vec![] };
        let st = StructureDef { id: "tower".into(), name: "Tower".into(), w: 1, h: 1, tilesets: vec!["t".into()], layers: vec![vec![1]], collision: vec![0], ..Default::default() };
        let newer = Database { tilesets: vec![ts.clone()], structures: vec![st], ..Default::default() };
        // The library's tileset has other art (not an extension): the structure would get wrong tiles.
        let mut db = Database { tilesets: vec![ts.clone()], ..Default::default() };
        let report = Bundle::collect(&newer, &src, 32, "starter", &all_assets(&newer)).unwrap().add_missing(&mut db, &dst).unwrap();
        assert!(db.structures.is_empty());
        assert_eq!(report.skipped, vec![(RefKind::Structure, "tower".to_string())]);
        // Same art: it comes along.
        std::fs::write(dst.join("tilesets/t.png"), png(64, 64, [10, 200, 10, 255])).unwrap();
        Bundle::collect(&newer, &src, 32, "starter", &all_assets(&newer)).unwrap().add_missing(&mut db, &dst).unwrap();
        assert_eq!(db.structures.len(), 1);
        let _ = (std::fs::remove_dir_all(src), std::fs::remove_dir_all(dst));
    }

    #[test]
    fn library_and_removal() {
        let (root, db) = source("lib");
        let pack = Bundle::collect(&db, &root, 16, "starter", &all_assets(&db)).unwrap().to_files().unwrap();
        let dir = tmp("lib");
        let _ = std::fs::remove_dir_all(&dir);
        let mut lib = open_library(&dir, Some(&borrowed(&pack))).unwrap();
        // The 16px pack was enlarged to the fixed 32px tiles.
        assert_eq!(lib.config.tile_size, 32);
        assert_eq!(lib.db.sheet("items").unwrap().frame_w, 32);
        assert_eq!(lib.db.items.len(), 2);
        // The sheet is still used by the items.
        assert!(remove_assets(&mut lib, &[(RefKind::Sheet, "items".into())]).is_err());
        // A newer starter set: only assets with new ids are added, existing ones stay as they are.
        let mut db2 = db.clone();
        db2.items[0].name = "Changed arrows".into();
        db2.sheets.push(SheetDef { id: "extra".into(), image: "sprites/extra.png".into(), frame_w: 16, frame_h: 16, kind: Default::default(), female: false, playable: false, tags: vec![] });
        std::fs::write(root.join("sprites/extra.png"), png(16, 16, [1, 2, 3, 255])).unwrap();
        let pack2 = Bundle::collect(&db2, &root, 16, "starter", &all_assets(&db2)).unwrap().to_files().unwrap();
        let lib = open_library(&dir, Some(&borrowed(&pack2))).unwrap();
        assert!(lib.db.sheet("extra").is_some(), "new sheet added");
        // A newer set marking a body playable flags the library's (otherwise unchanged) copy.
        let mut db3 = db2.clone();
        for s in &mut db3.sheets {
            s.playable = s.id == "extra";
        }
        let pack3 = Bundle::collect(&db3, &root, 16, "starter", &all_assets(&db3)).unwrap().to_files().unwrap();
        let lib = open_library(&dir, Some(&borrowed(&pack3))).unwrap();
        assert!(lib.db.sheet("extra").unwrap().playable, "playable flag added");
        assert!(!lib.db.sheet("items").unwrap().playable);
        // A newer set whose "items" image only grew (frames added on the right) upgrades it;
        // one that changed existing pixels doesn't.
        let old = Rgba::decode(&std::fs::read(dir.join("sprites/items.png")).unwrap()).unwrap();
        let row = old.w as usize * 4;
        let mut wider = old.resized(old.w * 2, old.h);
        for y in 0..old.h as usize {
            wider.px[y * row * 2..y * row * 2 + row].copy_from_slice(&old.px[y * row..y * row + row]);
        }
        let src3 = tmp("src3");
        std::fs::create_dir_all(src3.join("sprites")).unwrap();
        std::fs::write(src3.join("sprites/items.png"), wider.encode().unwrap()).unwrap();
        let mut db3 = Database { sheets: lib.db.sheets.iter().filter(|s| s.id == "items").cloned().collect(), items: lib.db.items.clone(), ..Default::default() };
        db3.items.push(ItemDef { id: "gem".into(), icon: SpriteRef::new("items", 3), ..db3.items[0].clone() });
        let pack3 = Bundle::collect(&db3, &src3, 32, "starter", &all_assets(&db3)).unwrap().to_files().unwrap();
        let lib3 = open_library(&dir, Some(&borrowed(&pack3))).unwrap();
        assert_eq!(Rgba::decode(&std::fs::read(dir.join("sprites/items.png")).unwrap()).unwrap().w, old.w * 2, "extended image upgraded");
        assert!(lib3.db.item("gem").is_some());
        assert!(extends(&old.encode().unwrap(), &wider.encode().unwrap()));
        let mut changed = wider.clone();
        changed.px[0] ^= 0xff;
        assert!(!extends(&old.encode().unwrap(), &changed.encode().unwrap()), "edited pixels are not an extension");
        let _ = std::fs::remove_dir_all(src3);
        let mut lib = Project::load(&dir).unwrap();
        assert_eq!(lib.db.item("arrows").unwrap().name, "arrows", "existing item untouched");
        assert_eq!(lib.db.items.len(), 3, "arrows, bow and the new gem");
        let everything = all_assets(&lib.db);
        remove_assets(&mut lib, &everything).unwrap();
        assert!(!dir.join("sprites/items.png").exists());
        assert!(Project::load(&dir).unwrap().db.items.is_empty());
        assert!(!safe_rel("../x") && !safe_rel("/etc/x") && safe_rel("sprites/a.png"));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(dir);
    }
}
