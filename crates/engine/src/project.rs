//! A project directory: config, definitions, maps and scripts.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::defs::Database;
use crate::map::{io as map_io, Map};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    /// Always [`TILE_SIZE`]. Projects saved with another size are converted by
    /// [`crate::migrate::upgrade`] (the editor does this when opening them).
    #[serde(default = "default_tile_size")]
    pub tile_size: u32,
    pub start_map: String,
    pub start_spawn: String,
    /// Pixel scale used by the game window.
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Player character definition id.
    pub player: String,
    /// Sounds played by built-in game events.
    #[serde(default)]
    pub sfx: GameSfx,
    /// Number of player inventory slots.
    #[serde(default = "default_inventory_size")]
    pub inventory_size: u32,
    /// Character creator shown when a new game starts.
    #[serde(default)]
    pub creation: CharacterCreation,
    /// Real minutes one in-game day lasts (day / night on outdoor maps).
    #[serde(default = "default_day_minutes")]
    pub day_minutes: f32,
    /// Time of day a new game starts at (hours, 0..24).
    #[serde(default = "default_start_hour")]
    pub start_hour: f32,
    /// Factions and their attitudes (who fights whom); empty = only `hostile` decides.
    #[serde(default, skip_serializing_if = "crate::factions::Factions::is_empty")]
    pub factions: crate::factions::Factions,
    /// Skills and magic (both off by default).
    #[serde(default)]
    pub skills: crate::skills::SkillSettings,
    /// Experience and levels (off by default).
    #[serde(default)]
    pub progression: crate::progression::Progression,
    /// Dialog boxes show the speaker's portrait (on by default).
    #[serde(default = "yes")]
    pub dialog_portraits: bool,
    /// Characters show emotions by themselves (on by default): enemies noticing the player
    /// (surprise), losing them (confused), fleeing (fear); sleepers (sleepy); companions learning
    /// a spell (joy). Needs those emotions in an *Emotions* sprite sheet.
    #[serde(default = "yes")]
    pub auto_emotions: bool,
    /// What happens to a companion at 0 HP.
    #[serde(default)]
    pub companion_down: DownRule,
    /// The player's portrait in dialogs (empty = the player character's, else the head and
    /// shoulders of the player's model).
    #[serde(default, skip_serializing_if = "crate::defs::SpriteRef::is_empty")]
    pub player_portrait: crate::defs::SpriteRef,
    /// The language the game is written in (code, e.g. "en"): the texts in the project.
    #[serde(default = "default_language")]
    pub language: String,
    /// Translations players can choose (codes; each has `lang/<code>.ron`, see `lang`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// GPU memory for the rendered poses of 3D characters, in MB (the pose atlas: pages of 4 MB;
    /// 8 to 128, see `render::model::atlas_pages`). Big casts on screen at once need more.
    #[serde(default = "default_atlas_mb", skip_serializing_if = "is_default_atlas_mb")]
    pub model_atlas_mb: u32,
}

fn default_atlas_mb() -> u32 {
    crate::render::model::DEFAULT_ATLAS_MB
}

fn is_default_atlas_mb(mb: &u32) -> bool {
    *mb == default_atlas_mb()
}

fn default_language() -> String {
    "en".into()
}

/// What happens to a companion knocked down to 0 HP (`ProjectConfig::companion_down`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum DownRule {
    /// It lies where it fell until healed (a potion, a spell, `heal_npc`).
    #[default]
    StaysDown,
    /// It lies there, then gets up with 1 HP after this many seconds (healing gets it up sooner).
    GetsUp { seconds: f32 },
    /// It goes back to where it is placed at once, healed (as before 0.6).
    GoesHome,
}

impl DownRule {
    pub fn label(&self) -> &'static str {
        match self {
            DownRule::StaysDown => "Stays down until healed",
            DownRule::GetsUp { .. } => "Gets up after a while",
            DownRule::GoesHome => "Goes home",
        }
    }
}

fn yes() -> bool {
    true
}

fn default_day_minutes() -> f32 {
    24.0
}
pub fn default_start_hour() -> f32 {
    8.0
}

/// The character creator: the player chooses a name, sex and look before a new game.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterCreation {
    pub enabled: bool,
    /// The player character's sex: chosen by the player, or fixed by the game.
    #[serde(default)]
    pub sex: SexChoice,
    /// Name suggested in the creator (and used when it is skipped); "" = the player
    /// character definition's name.
    #[serde(default)]
    pub default_name: String,
    /// The figure of a male / female player ("" = the player character's own figure). The
    /// creator offers a colour row for each of the figure's colour features.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_male: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_female: String,
}

impl CharacterCreation {
    /// Body sheet of a player of `sex` ("" = the player character's own body).
    pub fn body(&self, sex: crate::state::Sex) -> &str {
        match sex {
            crate::state::Sex::Male => &self.body_male,
            crate::state::Sex::Female => &self.body_female,
        }
    }
}

/// Whether the player may choose the player character's sex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SexChoice {
    /// The player chooses in the character creator (male if the creator is off).
    #[default]
    Both,
    Male,
    Female,
}

impl SexChoice {
    pub const ALL: [SexChoice; 3] = [SexChoice::Both, SexChoice::Male, SexChoice::Female];
    pub fn label(self) -> &'static str {
        match self {
            SexChoice::Both => "Player chooses",
            SexChoice::Male => "Always male",
            SexChoice::Female => "Always female",
        }
    }
    /// The sex every player character has, if it is fixed.
    pub fn fixed(self) -> Option<crate::state::Sex> {
        match self {
            SexChoice::Both => None,
            SexChoice::Male => Some(crate::state::Sex::Male),
            SexChoice::Female => Some(crate::state::Sex::Female),
        }
    }
}

/// Edge length of every tile in pixels. Sprites, hitboxes and speeds are designed for it.
pub const TILE_SIZE: u32 = 32;

fn default_tile_size() -> u32 {
    TILE_SIZE
}

impl ProjectConfig {
    /// A fresh configuration with the given name.
    pub fn new(name: &str) -> Self {
        ProjectConfig {
            name: name.to_string(),
            tile_size: TILE_SIZE,
            start_map: "start".into(),
            start_spawn: "start".into(),
            scale: default_scale(),
            player: String::new(),
            sfx: GameSfx::default(),
            inventory_size: default_inventory_size(),
            creation: CharacterCreation::default(),
            day_minutes: default_day_minutes(),
            start_hour: default_start_hour(),
            factions: Default::default(),
            skills: Default::default(),
            progression: Default::default(),
            dialog_portraits: true,
            auto_emotions: true,
            companion_down: Default::default(),
            player_portrait: Default::default(),
            language: default_language(),
            languages: Vec::new(),
            model_atlas_mb: default_atlas_mb(),
        }
    }
}

fn default_inventory_size() -> u32 {
    crate::inventory::DEFAULT_CAPACITY as u32
}

/// Sound ids for built-in game events (empty = silent).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSfx {
    pub pickup: String,
    pub attack: String,
    pub hit: String,
    pub hurt: String,
    pub defeat: String,
    pub dialog: String,
    pub portal: String,
    pub open_container: String,
    pub unlock: String,
}

impl GameSfx {
    pub fn fields_mut(&mut self) -> [(&'static str, &mut String); 9] {
        [
            ("pickup", &mut self.pickup),
            ("attack", &mut self.attack),
            ("hit", &mut self.hit),
            ("hurt", &mut self.hurt),
            ("defeat", &mut self.defeat),
            ("dialog", &mut self.dialog),
            ("portal", &mut self.portal),
            ("open container", &mut self.open_container),
            ("unlock", &mut self.unlock),
        ]
    }
}

fn default_scale() -> f32 {
    1.5
}

static SAVES_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Keeps save games in `dir` instead of the project folder (set once, at startup).
pub fn set_saves_dir(dir: PathBuf) {
    let _ = SAVES_DIR.set(dir);
}

pub struct Project {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub db: Database,
}

pub const DEFS: [&str; 14] = ["tilesets", "sheets", "items", "props", "characters", "sounds", "quests", "structures", "skills", "spells", "recipes", "achievements", "races", "models"];

/// A `defs/<name>.ron` list. The whole file is tried first; when that fails, the list is read
/// entry by entry (`RawValue` keeps each entry's text, so struct and enum syntax survive) and
/// only the entries that don't parse are left out, each reported through `warn` (a hand-edit
/// typo, an enum variant of a newer version). The list syntax itself must be intact.
fn parse_list<T: DeserializeOwned>(text: Option<String>, name: &str, warn: &mut dyn FnMut(String)) -> Result<Vec<T>> {
    let Some(s) = text else { return Ok(Vec::new()) };
    let whole = match ron::from_str::<Vec<T>>(&s) {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    let raws: Vec<Box<ron::value::RawValue>> = ron::from_str(&s).with_context(|| format!("parsing defs/{name}.ron: {whole}"))?;
    let mut out = Vec::with_capacity(raws.len());
    for (i, raw) in raws.iter().enumerate() {
        match raw.into_rust::<T>() {
            Ok(d) => out.push(d),
            Err(e) => warn(format!("defs/{name}.ron entry {} ({}) can't be read and is left out: {e}", i + 1, field_hint(raw.get_ron(), "id"))),
        }
    }
    Ok(out)
}

/// `field: value` of an entry's RON text (`id "sword"`, `uid 7`), for messages about entries
/// that can't be read.
pub fn field_hint(raw: &str, field: &str) -> String {
    let Some(i) = raw.find(&format!("{field}:")) else { return format!("no {field}") };
    let value = raw[i + field.len() + 1..].trim_start();
    let end = value.find([',', ')', '\n']).unwrap_or(value.len());
    format!("{field} {}", value[..end].trim())
}

/// The definitions from the texts of `defs/*.ron`: `read("items")` returns the text of
/// `defs/items.ron`, `None` when the file is missing (an empty list). Used for project folders
/// and for folders embedded in a program (the editor's starter library). Entries that can't be
/// read are left out and logged (`[defs]`); [`parse_defs_with`] hands them to the caller.
pub fn parse_defs(read: impl Fn(&str) -> Result<Option<String>>) -> Result<Database> {
    parse_defs_with(read, &mut |w| eprintln!("[defs] {w}"))
}

/// [`parse_defs`], reporting every entry that was left out through `warn`.
pub fn parse_defs_with(read: impl Fn(&str) -> Result<Option<String>>, warn: &mut dyn FnMut(String)) -> Result<Database> {
    let r = |i: usize| read(DEFS[i]);
    let mut db = Database {
        tilesets: parse_list(r(0)?, DEFS[0], warn)?,
        sheets: parse_list(r(1)?, DEFS[1], warn)?,
        items: parse_list(r(2)?, DEFS[2], warn)?,
        props: parse_list(r(3)?, DEFS[3], warn)?,
        characters: parse_list(r(4)?, DEFS[4], warn)?,
        sounds: parse_list(r(5)?, DEFS[5], warn)?,
        quests: parse_list(r(6)?, DEFS[6], warn)?,
        structures: parse_list(r(7)?, DEFS[7], warn)?,
        skills: parse_list(r(8)?, DEFS[8], warn)?,
        spells: parse_list(r(9)?, DEFS[9], warn)?,
        recipes: parse_list(r(10)?, DEFS[10], warn)?,
        achievements: parse_list(r(11)?, DEFS[11], warn)?,
        races: parse_list(r(12)?, DEFS[12], warn)?,
        models: parse_list(r(13)?, DEFS[13], warn)?,
        ..Default::default()
    };
    db.upgrade_prop_states();
    db.reindex();
    Ok(db)
}

/// The `*.rhai` files of a folder, in name order (the order scripts are loaded in).
pub fn script_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = crate::sys::files_in(dir).unwrap_or_default().into_iter().filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect();
    files.sort();
    files
}

/// A project-relative file path that stays inside the project: no leading `/`, no `\\`, no `.` or `..` parts.
pub fn safe_rel(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.contains('\\') && p.split('/').all(|c| !c.is_empty() && c != "." && c != "..")
}

/// `value` as the RON text project files are written with (pretty, LF line endings).
pub fn ron_text<T: Serialize>(value: &T) -> Result<String> {
    let cfg = ron::ser::PrettyConfig::default().struct_names(false).new_line("\n");
    Ok(ron::ser::to_string_pretty(value, cfg)?)
}

pub fn write_ron<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    // LF on every platform: projects look the same written on Windows (git, diffs).
    let s = ron_text(value)?;
    crate::sys::write(path, s.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

impl Project {
    /// Reads a project folder. Definition entries that can't be read are left out and logged
    /// (`[defs]`); [`Project::load_with_warnings`] hands them to the caller instead.
    pub fn load(root: &Path) -> Result<Project> {
        let (p, warnings) = Project::load_with_warnings(root)?;
        for w in warnings {
            eprintln!("[defs] {w}");
        }
        Ok(p)
    }

    /// [`Project::load`] with the entries that were left out: `defs/<file>.ron entry N (id "x")
    /// can't be read and is left out: <why>` (the editor shows them and backs the file up before
    /// it saves without them).
    pub fn load_with_warnings(root: &Path) -> Result<(Project, Vec<String>)> {
        let root = root.to_path_buf();
        let cfg_path = root.join("project.ron");
        let config: ProjectConfig = ron::from_str(
            &crate::sys::read_to_string(&cfg_path).with_context(|| format!("reading {}", cfg_path.display()))?,
        )
        .with_context(|| format!("parsing {}", cfg_path.display()))?;
        let defs = root.join("defs");
        let mut warnings = Vec::new();
        let db = parse_defs_with(
            |n| {
                let path = defs.join(format!("{n}.ron"));
                if !crate::sys::exists(&path) {
                    return Ok(None);
                }
                crate::sys::read_to_string(&path).map(Some).with_context(|| format!("reading {}", path.display()))
            },
            &mut |w| warnings.push(w),
        )
        .with_context(|| format!("in {}", root.display()))?;
        Ok((Project { root, config, db }, warnings))
    }

    /// Re-reads `project.ron` and the definitions from disk (another program changed them).
    pub fn reload(&mut self) -> Result<()> {
        *self = Project::load(&self.root)?;
        Ok(())
    }

    /// Every trigger list of the project: the definitions' ([`crate::events::walk_db_triggers`]),
    /// then every map's read from disk ([`crate::events::walk_map_triggers`]; a map that doesn't
    /// load is skipped). Translations, the project check and the editor's name collectors use it.
    pub fn for_each_trigger(&self, f: &mut dyn for<'a> FnMut(crate::events::TriggerSite<'a>, &'a [crate::events::Trigger])) {
        crate::events::walk_db_triggers(&self.db, f);
        for name in self.list_maps() {
            let Ok(map) = self.load_map(&name) else { continue };
            crate::events::walk_map_triggers(&name, &map, f);
        }
    }

    /// The project's `scripts/*.rhai`, in name order.
    pub fn script_files(&self) -> Vec<PathBuf> {
        script_files_in(&self.scripts_dir())
    }

    pub fn save_config(&self) -> Result<()> {
        write_ron(&self.root.join("project.ron"), &self.config)
    }

    pub fn save_defs(&self) -> Result<()> {
        let d = |n: &str| self.root.join("defs").join(format!("{n}.ron"));
        write_ron(&d(DEFS[0]), &self.db.tilesets)?;
        write_ron(&d(DEFS[1]), &self.db.sheets)?;
        write_ron(&d(DEFS[2]), &self.db.items)?;
        write_ron(&d(DEFS[3]), &self.db.props)?;
        write_ron(&d(DEFS[4]), &self.db.characters)?;
        write_ron(&d(DEFS[5]), &self.db.sounds)?;
        write_ron(&d(DEFS[6]), &self.db.quests)?;
        // Only projects that have structures get the file (older editors ignore it anyway).
        if !self.db.structures.is_empty() || d(DEFS[7]).exists() {
            write_ron(&d(DEFS[7]), &self.db.structures)?;
        }
        if !self.db.skills.is_empty() || d(DEFS[8]).exists() {
            write_ron(&d(DEFS[8]), &self.db.skills)?;
        }
        if !self.db.spells.is_empty() || d(DEFS[9]).exists() {
            write_ron(&d(DEFS[9]), &self.db.spells)?;
        }
        if !self.db.recipes.is_empty() || d(DEFS[10]).exists() {
            write_ron(&d(DEFS[10]), &self.db.recipes)?;
        }
        if !self.db.achievements.is_empty() || d(DEFS[11]).exists() {
            write_ron(&d(DEFS[11]), &self.db.achievements)?;
        }
        if !self.db.races.is_empty() || d(DEFS[12]).exists() {
            write_ron(&d(DEFS[12]), &self.db.races)?;
        }
        if !self.db.models.is_empty() || d(DEFS[13]).exists() {
            write_ron(&d(DEFS[13]), &self.db.models)?;
        }
        Ok(())
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    pub fn maps_dir(&self) -> PathBuf {
        self.root.join("maps")
    }
    pub fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }
    /// `<root>/saves`, unless [`set_saves_dir`] chose another folder (released games).
    pub fn saves_dir(&self) -> PathBuf {
        SAVES_DIR.get().cloned().unwrap_or_else(|| self.root.join("saves"))
    }
    pub fn map_path(&self, name: &str) -> PathBuf {
        self.maps_dir().join(format!("{name}.map"))
    }

    pub fn list_maps(&self) -> Vec<String> {
        let mut out: Vec<String> = crate::sys::files_in(&self.maps_dir())
            .into_iter()
            .flatten()
            .filter_map(|p| {
                if p.extension()? != "map" {
                    return None;
                }
                Some(p.file_stem()?.to_string_lossy().into_owned())
            })
            .collect();
        out.sort();
        out
    }

    pub fn load_map(&self, name: &str) -> Result<Map> {
        let mut m = map_io::load(&self.map_path(name))?;
        m.name = name.to_string();
        for e in &mut m.entities {
            self.db.upgrade_instance(e);
        }
        Ok(m)
    }

    pub fn save_map(&self, map: &Map) -> Result<()> {
        map_io::save(map, &self.map_path(&map.name))
    }

    /// Renames a map file (references are updated with `refactor`).
    pub fn rename_map_file(&self, old: &str, new: &str) -> Result<()> {
        let (from, to) = (self.map_path(old), self.map_path(new));
        if to.exists() {
            anyhow::bail!("a map named '{new}' already exists");
        }
        std::fs::rename(from, to)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One entry that can't be read (an unknown enum variant, a typo) is left out with a
    /// warning; the rest of the file loads. A broken list is still an error.
    #[test]
    fn unreadable_entries_are_left_out() {
        let text = r#"[
            (id: "a", name: "A", icon: (sheet: "s", frame: 0)),
            (id: "b", name: "B", icon: (sheet: "s", frame: 1), use_on: NoSuchVariant),
            (id: "c", name: "C", icon: (sheet: "s", frame: 2)),
        ]"#;
        let mut warnings = Vec::new();
        let items: Vec<crate::defs::ItemDef> = parse_list(Some(text.into()), "items", &mut |w| warnings.push(w)).unwrap();
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("defs/items.ron entry 2 (id \"b\") can't be read"), "{}", warnings[0]);
        let whole = parse_list::<crate::defs::ItemDef>(Some("[(id: \"a\"".into()), "items", &mut |_| panic!("no entry warning for a broken list"));
        assert!(whole.is_err());
        assert_eq!(field_hint("(uid: 7, def: \"x\")", "uid"), "uid 7");
        assert_eq!(field_hint("(uid: 7)", "def"), "no def");
        // A whole healthy file parses in one go (nothing reported).
        let ok: Vec<crate::defs::ItemDef> = parse_list(Some(r#"[(id: "a", name: "A", icon: (sheet: "s", frame: 0))]"#.into()), "items", &mut |_| panic!("healthy")).unwrap();
        assert_eq!(ok.len(), 1);
    }

    /// Projects of earlier versions have no pose atlas size: the default, and saving the default
    /// leaves project.ron as it was.
    #[test]
    fn the_atlas_size_defaults_and_stays_out_of_the_file() {
        let c: ProjectConfig = ron::from_str(include_str!("../tests/fixtures/project-0.16.0/project.ron")).unwrap();
        assert_eq!(c.model_atlas_mb, crate::render::model::DEFAULT_ATLAS_MB);
        assert!(!ron::to_string(&c).unwrap().contains("model_atlas_mb"));
        let big = ProjectConfig { model_atlas_mb: 64, ..c };
        assert!(ron::to_string(&big).unwrap().contains("model_atlas_mb:64"));
    }
}
