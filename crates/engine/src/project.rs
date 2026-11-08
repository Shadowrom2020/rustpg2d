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
    /// Skills and magic (both off by default).
    #[serde(default)]
    pub skills: crate::skills::SkillSettings,
    /// Experience and levels (off by default).
    #[serde(default)]
    pub progression: crate::progression::Progression,
    /// Dialog boxes show the speaker's portrait (on by default).
    #[serde(default = "yes")]
    pub dialog_portraits: bool,
    /// What happens to a companion at 0 HP.
    #[serde(default)]
    pub companion_down: DownRule,
    /// The player's portrait in dialogs (empty = the player character's, else the head and
    /// shoulders of the player's model).
    #[serde(default, skip_serializing_if = "crate::defs::SpriteRef::is_empty")]
    pub player_portrait: crate::defs::SpriteRef,
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
    /// The body sheets of a male / female player ("" = the player character's own body). The
    /// player picks skin, hair style, hair color and (men) a beard from the lists below.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_male: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body_female: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skin_colors: Vec<[u8; 3]>,
    /// Hair sheets ("" = no hair).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hair_styles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hair_colors: Vec<[u8; 3]>,
    /// Beard sheets offered to male players ("" = no beard).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beards: Vec<String>,
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
            skills: Default::default(),
            progression: Default::default(),
            dialog_portraits: true,
            companion_down: Default::default(),
            player_portrait: Default::default(),
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

const DEFS: [&str; 12] = ["tilesets", "sheets", "items", "props", "characters", "sounds", "quests", "structures", "skills", "spells", "recipes", "achievements"];

fn read_ron<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if !crate::sys::exists(path) {
        return Ok(T::default());
    }
    let s = crate::sys::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&s).with_context(|| format!("parsing {}", path.display()))
}

pub fn write_ron<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let cfg = ron::ser::PrettyConfig::default().struct_names(false);
    let s = ron::ser::to_string_pretty(value, cfg)?;
    crate::sys::write(path, s.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

impl Project {
    pub fn load(root: &Path) -> Result<Project> {
        let root = root.to_path_buf();
        let cfg_path = root.join("project.ron");
        let config: ProjectConfig = ron::from_str(
            &crate::sys::read_to_string(&cfg_path).with_context(|| format!("reading {}", cfg_path.display()))?,
        )
        .with_context(|| format!("parsing {}", cfg_path.display()))?;
        let d = |n: &str| root.join("defs").join(format!("{n}.ron"));
        let db = Database {
            tilesets: read_ron(&d(DEFS[0]))?,
            sheets: read_ron(&d(DEFS[1]))?,
            items: read_ron(&d(DEFS[2]))?,
            props: read_ron(&d(DEFS[3]))?,
            characters: read_ron(&d(DEFS[4]))?,
            sounds: read_ron(&d(DEFS[5]))?,
            quests: read_ron(&d(DEFS[6]))?,
            structures: read_ron(&d(DEFS[7]))?,
            skills: read_ron(&d(DEFS[8]))?,
            spells: read_ron(&d(DEFS[9]))?,
            recipes: read_ron(&d(DEFS[10]))?,
            achievements: read_ron(&d(DEFS[11]))?,
        };
        let mut db = db;
        db.upgrade_prop_states();
        db.upgrade_characters();
        Ok(Project { root, config, db })
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
