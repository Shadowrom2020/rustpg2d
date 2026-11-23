//! Unused assets: the 3D models, sprite sheets, sounds, materials and fonts a project defines but
//! nothing uses, and the files in its asset folders that no definition names. The editor's
//! *Clean up unused files* window offers to delete them (when a project closes, and from the
//! Project menu); exports leave them out ([`slim`]).
//!
//! "Used" is decided on the safe side: an id counts as used when it appears as a string anywhere
//! in the project's other definitions, its settings or its maps' objects, anywhere in the text of
//! a script or a plugin's files (also inside a longer string), or in a used asset's definition (a
//! model's rig template, a material's footstep sound). Models the character creator offers
//! (playable bodies), pieces that fit a used body (or any body), the mannequin (drawn for looks
//! without a model) and *Emotions* sheets (found by their kind) are used too. Rig templates no
//! model uses are editor tools: the cleanup leaves them, exports don't take them. Only pictures,
//! models, sounds and fonts count as stray files (a folder's credits stay).

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use anyhow::{Context, Result};

use crate::defs::Database;
use crate::project::{ron_text, Project, ProjectConfig, DEFS};
use crate::refactor::RefKind;

/// The kinds of definitions the cleanup looks at: those with files.
pub const KINDS: [RefKind; 5] = [RefKind::Model, RefKind::Sheet, RefKind::Sound, RefKind::Material, RefKind::Font];

/// Folders whose files belong to definitions: a file there that nothing names is unused.
pub const ASSET_DIRS: [&str; 6] = ["sprites", "models", "sounds", "materials", "fonts", "tilesets"];

/// The file types of assets (stray files of other types, such as credits, stay).
const ASSET_TYPES: [&str; 14] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "glb", "gltf", "wav", "ogg", "mp3", "flac", "ttf", "otf"];

fn is_asset_file(rel: &str) -> bool {
    rel.rsplit_once('.').is_some_and(|(_, ext)| ASSET_TYPES.contains(&ext.to_ascii_lowercase().as_str()))
}

/// The `defs/` file of each kind in [`KINDS`].
fn kind_file(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Model => "models",
        RefKind::Sheet => "sheets",
        RefKind::Sound => "sounds",
        RefKind::Material => "materials",
        _ => "fonts",
    }
}

/// An unused definition and the files only it uses.
#[derive(Clone, Debug, PartialEq)]
pub struct UnusedDef {
    pub kind: RefKind,
    pub id: String,
    /// Its files (project-relative) that nothing else uses: deleted with it.
    pub files: Vec<String>,
    /// Their size in bytes.
    pub bytes: u64,
    /// An editor tool (a rig template): left out of exports, not offered for deletion.
    pub tool: bool,
}

/// What a project has but doesn't use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unused {
    pub defs: Vec<UnusedDef>,
    /// Files in the asset folders that no definition, setting or script names, with their sizes.
    pub files: Vec<(String, u64)>,
}

impl Unused {
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty() && self.files.is_empty()
    }

    /// Without the editor's tools: what the cleanup offers to delete.
    pub fn deletable(&self) -> Unused {
        Unused { defs: self.defs.iter().filter(|d| !d.tool).cloned().collect(), files: self.files.clone() }
    }

    /// The bytes all of it takes.
    pub fn bytes(&self) -> u64 {
        self.defs.iter().map(|d| d.bytes).sum::<u64>() + self.files.iter().map(|f| f.1).sum::<u64>()
    }

    /// Whether the definition `kind` / `id` is unused.
    pub fn has(&self, kind: RefKind, id: &str) -> bool {
        self.defs.iter().any(|d| d.kind == kind && d.id == id)
    }
}

/// The strings of a RON text (the contents of its `"…"`), added to `out`.
fn strings_of(text: &str, out: &mut HashSet<String>) {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(e) = chars.next() {
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            o => o,
                        });
                    }
                }
                o => s.push(o),
            }
        }
        out.insert(s);
    }
}

/// The RON text of the definition list `name` (one of [`DEFS`]).
fn list_text(db: &Database, name: &str) -> Result<String> {
    Ok(match name {
        "sheets" => ron_text(&db.sheets)?,
        "items" => ron_text(&db.items)?,
        "props" => ron_text(&db.props)?,
        "characters" => ron_text(&db.characters)?,
        "sounds" => ron_text(&db.sounds)?,
        "quests" => ron_text(&db.quests)?,
        "structures" => ron_text(&db.structures)?,
        "spells" => ron_text(&db.spells)?,
        "recipes" => ron_text(&db.recipes)?,
        "achievements" => ron_text(&db.achievements)?,
        "races" => ron_text(&db.races)?,
        "models" => ron_text(&db.models)?,
        "materials" => ron_text(&db.materials)?,
        "scatter" => ron_text(&db.scatter)?,
        "fonts" => ron_text(&db.fonts)?,
        "skins" => ron_text(&db.skins)?,
        "common_events" => ron_text(&db.common_events)?,
        "backgrounds" => ron_text(&db.backgrounds)?,
        "classes" => ron_text(&db.classes)?,
        "diseases" => ron_text(&db.diseases)?,
        "cutscenes" => ron_text(&db.cutscenes)?,
        "animations" => ron_text(&db.animations)?,
        "visual_scripts" => ron_text(&db.visual_scripts)?,
        other => anyhow::bail!("no definition list '{other}'"),
    })
}

/// One definition of the kinds the cleanup looks at.
struct Candidate {
    kind: RefKind,
    id: String,
    /// The strings of its definition (without its own id).
    strings: HashSet<String>,
    /// Used without being named: offered by the character creator, the mannequin, an emotions
    /// sheet.
    playable: bool,
    /// A rig template (its id starts with `rig_`).
    tool: bool,
    /// A piece: the bodies it fits (empty = any).
    fits: Option<Vec<String>>,
}

fn candidates(db: &Database) -> Result<Vec<Candidate>> {
    let mut out = Vec::new();
    let mut add = |kind: RefKind, id: &str, text: String, playable: bool, fits: Option<Vec<String>>| {
        let mut strings = HashSet::new();
        strings_of(&text, &mut strings);
        strings.remove(id);
        let tool = kind == RefKind::Model && id.starts_with("rig_");
        out.push(Candidate { kind, id: id.to_string(), strings, playable, tool, fits });
    };
    for d in &db.models {
        add(RefKind::Model, &d.id, ron_text(d)?, d.playable || d.id == crate::render::model::MANNEQUIN, d.part.as_ref().map(|p| p.fits.clone()));
    }
    for d in &db.sheets {
        add(RefKind::Sheet, &d.id, ron_text(d)?, d.kind == crate::defs::SheetKind::Emotions, None);
    }
    for d in &db.sounds {
        add(RefKind::Sound, &d.id, ron_text(d)?, false, None);
    }
    for d in &db.materials {
        add(RefKind::Material, &d.id, ron_text(d)?, false, None);
    }
    for d in &db.fonts {
        add(RefKind::Font, &d.id, ron_text(d)?, false, None);
    }
    Ok(out)
}

/// Everything of a project that can use an asset, apart from the assets themselves.
pub struct Sources<'a> {
    pub root: &'a Path,
    pub db: &'a Database,
    pub config: &'a ProjectConfig,
    /// The RON text of each map's objects and settings ([`crate::map::io::Meta`]) with its
    /// materials and decoration props.
    pub maps: Vec<String>,
    /// The text of the scripts and of the plugins' files.
    pub texts: Vec<String>,
}

impl<'a> Sources<'a> {
    /// Reads a project's maps, scripts and plugin files. Fails when a map can't be read (then
    /// nothing can be called unused).
    pub fn of(project: &'a Project) -> Result<Sources<'a>> {
        // An old tile project's maps name their tilesets in ways this doesn't read.
        anyhow::ensure!(!crate::convert_tiles::needs_conversion(&project.root), "the project still has tiles: open it in the editor to convert it first");
        let mut maps = Vec::new();
        for m in project.list_maps() {
            let map = project.load_map(&m).with_context(|| format!("reading the map '{m}'"))?;
            maps.push(map_text(&map)?);
        }
        let mut texts = Vec::new();
        for f in project.script_files() {
            texts.push(std::fs::read_to_string(&f).with_context(|| format!("reading {}", f.display()))?);
        }
        let plugins = project.root.join(crate::plugins::PLUGINS_DIR);
        for f in files_under(&plugins) {
            if let Ok(t) = std::fs::read_to_string(plugins.join(&f)) {
                texts.push(t);
            }
        }
        Ok(Sources { root: &project.root, db: &project.db, config: &project.config, maps, texts })
    }
}

/// What a map names: its objects and settings as RON, its materials and decoration props.
pub fn map_text(map: &crate::map::Map) -> Result<String> {
    let mut t = ron_text(&crate::map::io::Meta::of(map))?;
    t.push_str(&ron_text(&map.materials)?);
    t.push_str(&ron_text(&map.decor_defs)?);
    Ok(t)
}

/// The files under `dir` (paths relative to it with `/`), without hidden files and folders.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((d, prefix)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push((e.path(), rel)),
                Ok(t) if t.is_file() => out.push(rel),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// What `src` doesn't use.
pub fn find(src: &Sources) -> Result<Unused> {
    let kinds: Vec<&str> = KINDS.iter().map(|k| kind_file(*k)).collect();
    // The strings everything but the assets names.
    let mut named = HashSet::new();
    for name in DEFS.iter().filter(|n| !kinds.contains(n)) {
        strings_of(&list_text(src.db, name)?, &mut named);
    }
    strings_of(&ron_text(src.config)?, &mut named);
    for m in &src.maps {
        strings_of(m, &mut named);
    }
    let text = src.texts.join("\n");
    let cands = candidates(src.db)?;
    // Used assets add what they name, until nothing changes.
    let mut used = vec![false; cands.len()];
    let mut bodies: HashSet<&str> = HashSet::new();
    loop {
        let mut changed = false;
        for (k, c) in cands.iter().enumerate() {
            if used[k] {
                continue;
            }
            let fits = c.fits.as_ref().is_some_and(|f| f.is_empty() || f.iter().any(|b| bodies.contains(b.as_str())));
            if c.playable || fits || named.contains(&c.id) || (!c.id.is_empty() && text.contains(&c.id)) {
                used[k] = true;
                named.extend(c.strings.iter().cloned());
                if c.kind == RefKind::Model {
                    bodies.insert(&c.id);
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let size = |rel: &str| std::fs::metadata(src.root.join(rel)).map_or(0, |m| m.len());
    let is_file = |s: &str| crate::project::safe_rel(s) && s.contains('/') && src.root.join(s).is_file();
    let mut out = Unused::default();
    for (_, c) in cands.iter().enumerate().filter(|(k, _)| !used[*k]) {
        let mut files: Vec<String> = c.strings.iter().filter(|s| is_file(s) && !named.contains(*s) && !text.contains(s.as_str())).cloned().collect();
        files.sort();
        let bytes = files.iter().map(|f| size(f)).sum();
        out.defs.push(UnusedDef { kind: c.kind, id: c.id.clone(), files, bytes, tool: c.tool });
    }
    // Files no definition (used or not), setting or script names.
    let mut all = named;
    for c in &cands {
        all.extend(c.strings.iter().cloned());
    }
    for dir in ASSET_DIRS {
        for f in files_under(&src.root.join(dir)) {
            let rel = format!("{dir}/{f}");
            if is_asset_file(&rel) && !all.contains(&rel) && !text.contains(&rel) {
                out.files.push((rel.clone(), size(&rel)));
            }
        }
    }
    out.defs.sort_by(|a, b| (KINDS.iter().position(|k| *k == a.kind), &a.id).cmp(&(KINDS.iter().position(|k| *k == b.kind), &b.id)));
    Ok(out)
}

/// [`find`] for a project as it is on disk and in memory.
pub fn find_in(project: &Project) -> Result<Unused> {
    find(&Sources::of(project)?)
}

/// `db` without the definitions `unused` lists (an export's definitions).
pub fn slim(db: &Database, unused: &Unused) -> Database {
    let mut db = db.clone();
    db.models.retain(|d| !unused.has(RefKind::Model, &d.id));
    db.sheets.retain(|d| !unused.has(RefKind::Sheet, &d.id));
    db.sounds.retain(|d| !unused.has(RefKind::Sound, &d.id));
    db.materials.retain(|d| !unused.has(RefKind::Material, &d.id));
    db.fonts.retain(|d| !unused.has(RefKind::Font, &d.id));
    db
}

/// The `defs/` files whose text [`slim`] changes, with their new text.
pub fn slim_defs(db: &Database, unused: &Unused) -> Result<Vec<(String, String)>> {
    let slim = slim(db, unused);
    let mut out = Vec::new();
    for kind in KINDS {
        if unused.defs.iter().any(|d| d.kind == kind) {
            let name = kind_file(kind);
            out.push((format!("defs/{name}.ron"), list_text(&slim, name)?));
        }
    }
    Ok(out)
}

/// What a cleanup removed.
#[derive(Debug, Default)]
pub struct Cleaned {
    pub defs: usize,
    pub files: usize,
    pub bytes: u64,
    /// Where the removed files and the definition files as they were went.
    pub backup: Option<std::path::PathBuf>,
}

/// Deletes the definitions `defs` (kind, id) and the files `files` of `project`, after copying
/// them (and the definition files as they were) into `backup` (keeping their paths). Only what
/// `unused` lists is touched; the definitions are saved.
pub fn delete(project: &mut Project, unused: &Unused, defs: &BTreeSet<(RefKind, String)>, files: &BTreeSet<String>, backup: &Path) -> Result<Cleaned> {
    let picked: Vec<&UnusedDef> = unused.defs.iter().filter(|d| defs.contains(&(d.kind, d.id.clone()))).collect();
    let mut gone: Vec<String> = picked.iter().flat_map(|d| d.files.iter().cloned()).collect();
    gone.extend(unused.files.iter().filter(|(f, _)| files.contains(f)).map(|(f, _)| f.clone()));
    gone.sort();
    gone.dedup();
    if picked.is_empty() && gone.is_empty() {
        return Ok(Cleaned::default());
    }
    let kinds: BTreeSet<RefKind> = picked.iter().map(|d| d.kind).collect();
    let def_files: Vec<String> = kinds.iter().map(|k| format!("defs/{}.ron", kind_file(*k))).filter(|f| project.root.join(f).is_file()).collect();
    for rel in gone.iter().chain(&def_files) {
        let dst = backup.join(rel);
        std::fs::create_dir_all(dst.parent().expect("a backup path has a parent")).with_context(|| format!("creating {}", backup.display()))?;
        std::fs::copy(project.root.join(rel), &dst).with_context(|| format!("backing up {rel}"))?;
    }
    let mut cleaned = Cleaned { defs: picked.len(), files: gone.len(), bytes: 0, backup: Some(backup.to_path_buf()) };
    let sub = Unused { defs: picked.into_iter().cloned().collect(), files: Vec::new() };
    project.db = slim(&project.db, &sub);
    project.save_defs()?;
    for rel in &gone {
        let p = project.root.join(rel);
        cleaned.bytes += std::fs::metadata(&p).map_or(0, |m| m.len());
        std::fs::remove_file(&p).with_context(|| format!("deleting {rel}"))?;
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{CharacterDef, ModelDef, ModelPart, SheetDef, SoundDef};

    fn sound(id: &str, file: &str) -> SoundDef {
        SoundDef { id: id.into(), file: file.into(), volume: 1.0, caption: String::new() }
    }

    fn project(name: &str) -> Project {
        let root = std::env::temp_dir().join(format!("rustpg2d_unused_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in ["models/used.glb", "models/unused.glb", "models/rig.glb", "models/hair.glb", "models/stray.glb", "sprites/coin.png", "sprites/spare.png", "sounds/drum.ogg", "sounds/script.ogg", "models/shared.glb"] {
            std::fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
            std::fs::write(root.join(f), b"12345").unwrap();
        }
        let mut used = ModelDef::new("used", "models/used.glb");
        used.rig = Some(crate::defs::RigFit { template: "rig".into(), ..Default::default() });
        let hair = ModelDef { part: Some(ModelPart { slot: "hair".into(), fits: vec!["used".into()], ..Default::default() }), ..ModelDef::new("hair", "models/hair.glb") };
        let other_hair = ModelDef { part: Some(ModelPart { slot: "hair".into(), fits: vec!["unused".into()], ..Default::default() }), ..ModelDef::new("other_hair", "models/hair.glb") };
        let db = Database {
            models: vec![used, ModelDef::new("unused", "models/unused.glb"), ModelDef::new("rig", "models/rig.glb"), hair, other_hair, ModelDef::new("shares", "models/shared.glb"), ModelDef::new("shared", "models/shared.glb")],
            sheets: vec![SheetDef { id: "coin_icon".into(), image: "sprites/coin.png".into(), ..Default::default() }, SheetDef { id: "spare_icon".into(), image: "sprites/spare.png".into(), ..Default::default() }],
            sounds: vec![sound("drum", "sounds/drum.ogg"), sound("chime", "sounds/script.ogg")],
            characters: vec![CharacterDef { appearance: crate::defs::Appearance { model: "used".into(), ..Default::default() }, ..CharacterDef::fresh("guard") }],
            items: vec![crate::defs::ItemDef { icon: crate::defs::SpriteRef { sheet: "coin_icon".into(), ..Default::default() }, ..crate::defs::ItemDef::fresh("coin") }],
            ..Default::default()
        };
        let config = ProjectConfig::new("T");
        let mut db = db;
        db.characters.push(CharacterDef { appearance: crate::defs::Appearance { model: "shared".into(), ..Default::default() }, ..CharacterDef::fresh("twin") });
        Project { root, config, db }
    }

    #[test]
    fn finds_what_nothing_uses() {
        let p = project("find");
        let src = Sources { root: &p.root, db: &p.db, config: &p.config, maps: vec![], texts: vec!["fn ring() { play_sound(\"chi\" + \"me\"); play_sound(\"chime\"); }".into()] };
        let u = find(&src).unwrap();
        let ids: Vec<(RefKind, &str)> = u.defs.iter().map(|d| (d.kind, d.id.as_str())).collect();
        assert_eq!(ids, vec![(RefKind::Model, "other_hair"), (RefKind::Model, "shares"), (RefKind::Model, "unused"), (RefKind::Sheet, "spare_icon"), (RefKind::Sound, "drum")], "used: by a character, a rig template, a piece fitting a used body, an item's icon, a script's text");
        let unused = u.defs.iter().find(|d| d.id == "unused").unwrap();
        assert_eq!((unused.files.as_slice(), unused.bytes), (&["models/unused.glb".to_string()][..], 5));
        assert!(u.defs.iter().find(|d| d.id == "shares").unwrap().files.is_empty(), "a file a used definition shares stays");
        assert!(u.defs.iter().find(|d| d.id == "other_hair").unwrap().files.is_empty(), "the hair file is the used piece's too");
        assert_eq!(u.files, vec![("models/stray.glb".to_string(), 5)], "a file no definition names");
        // A map's character and a material's footsteps make them used.
        let mut map = crate::map::Map::new("m", 4, 4);
        map.music = "drum".into();
        let src = Sources { maps: vec![map_text(&map).unwrap()], ..src };
        assert!(!find(&src).unwrap().has(RefKind::Sound, "drum"), "map music");
        let _ = std::fs::remove_dir_all(&p.root);
    }

    #[test]
    fn every_definition_list_is_read() {
        let db = Database::default();
        for name in DEFS {
            assert!(list_text(&db, name).is_ok(), "{name}: add it to list_text, or its references are missed");
        }
    }

    #[test]
    fn deletes_with_a_backup_and_slims() {
        let mut p = project("delete");
        p.save_config().unwrap();
        p.save_defs().unwrap();
        let u = find_in(&p).unwrap();
        let s = slim_defs(&p.db, &u).unwrap();
        assert!(s.iter().any(|(f, t)| f == "defs/models.ron" && !t.contains("\"unused\"") && t.contains("\"used\"")), "exports leave unused definitions out");
        let backup = p.root.with_extension("bak");
        let _ = std::fs::remove_dir_all(&backup);
        let defs = BTreeSet::from([(RefKind::Model, "unused".to_string()), (RefKind::Sheet, "spare_icon".to_string())]);
        let c = delete(&mut p, &u, &defs, &BTreeSet::from(["models/stray.glb".to_string()]), &backup).unwrap();
        assert_eq!((c.defs, c.files, c.bytes), (2, 3, 15));
        assert!(!p.root.join("models/unused.glb").exists() && !p.root.join("models/stray.glb").exists() && !p.root.join("sprites/spare.png").exists());
        assert!(backup.join("models/unused.glb").is_file() && backup.join("defs/models.ron").is_file() && backup.join("defs/sheets.ron").is_file());
        let back = Project::load(&p.root).unwrap();
        assert!(back.db.model("unused").is_none() && back.db.model("shares").is_some() && back.db.sheet("spare_icon").is_none() && back.db.sound("drum").is_some(), "only the picked ones went");
        let _ = std::fs::remove_dir_all(&p.root);
        let _ = std::fs::remove_dir_all(&backup);
    }
}
