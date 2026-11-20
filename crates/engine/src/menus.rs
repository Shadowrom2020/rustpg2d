//! The project's title screen and pause menu (`ProjectConfig::title`, `ProjectConfig::menu`,
//! `ProjectConfig::credits`): the title's background (a map or a picture), logo and music, and
//! which entries both menus show, in which order, with custom entries that run a script
//! function. Empty settings = the built-in menus. `runtime/menu.rs` builds its rows from them.

use serde::{Deserialize, Serialize};

/// An entry of the title screen or the pause menu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MenuEntry {
    /// Title: start a new game (the character creator first, when it is on).
    NewGame,
    /// Title: load the newest save.
    Continue,
    /// Title and pause menu: the Load page.
    Load,
    /// Title and pause menu: the Options page.
    Options,
    /// Title and pause menu: the Achievements page (shown only when the game has achievements).
    Achievements,
    /// Title and pause menu: the Credits page (`ProjectConfig::credits`).
    Credits,
    /// Title: close the game (not in web games).
    Exit,
    /// Pause menu: back to the game.
    Resume,
    /// Pause menu: photo mode.
    Photo,
    /// Pause menu: the Save page.
    Save,
    /// Pause menu: save to the game's slot, back to the title screen.
    SaveAndQuit,
    /// Pause menu: back to the title screen without saving (asks first).
    QuitToTitle,
    /// Runs a script function. `label` is shown (and translated), `detail` under it.
    Custom {
        label: String,
        function: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        detail: String,
    },
}

impl MenuEntry {
    /// The entries the title screen can show.
    pub const TITLE: [MenuEntry; 7] = [MenuEntry::NewGame, MenuEntry::Continue, MenuEntry::Load, MenuEntry::Options, MenuEntry::Achievements, MenuEntry::Credits, MenuEntry::Exit];
    /// The entries the pause menu can show.
    pub const PAUSE: [MenuEntry; 9] =
        [MenuEntry::Resume, MenuEntry::Photo, MenuEntry::Save, MenuEntry::Load, MenuEntry::Options, MenuEntry::Achievements, MenuEntry::Credits, MenuEntry::SaveAndQuit, MenuEntry::QuitToTitle];

    /// The name shown in the editor (custom entries: their label).
    pub fn label(&self) -> &str {
        match self {
            MenuEntry::NewGame => "New game",
            MenuEntry::Continue => "Continue",
            MenuEntry::Load => "Load game",
            MenuEntry::Options => "Options",
            MenuEntry::Achievements => "Achievements",
            MenuEntry::Credits => "Credits",
            MenuEntry::Exit => "Exit",
            MenuEntry::Resume => "Resume",
            MenuEntry::Photo => "Photo mode",
            MenuEntry::Save => "Save game",
            MenuEntry::SaveAndQuit => "Save and quit",
            MenuEntry::QuitToTitle => "Quit",
            MenuEntry::Custom { label, .. } => label,
        }
    }

    /// Whether the title screen (`true`) or the pause menu (`false`) can show it.
    pub fn fits(&self, title: bool) -> bool {
        match self {
            MenuEntry::Custom { .. } | MenuEntry::Load | MenuEntry::Options | MenuEntry::Achievements | MenuEntry::Credits => true,
            MenuEntry::NewGame | MenuEntry::Continue | MenuEntry::Exit => title,
            MenuEntry::Resume | MenuEntry::Photo | MenuEntry::Save | MenuEntry::SaveAndQuit | MenuEntry::QuitToTitle => !title,
        }
    }
}

/// Where a menu sits on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuAlign {
    Left,
    #[default]
    Center,
    Right,
}

impl MenuAlign {
    pub const ALL: [MenuAlign; 3] = [MenuAlign::Left, MenuAlign::Center, MenuAlign::Right];

    pub fn label(self) -> &'static str {
        match self {
            MenuAlign::Left => "Left",
            MenuAlign::Center => "Center",
            MenuAlign::Right => "Right",
        }
    }
}

/// What the title screen shows behind its menu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TitleBackground {
    /// A map: the camera on tile `x`, `y` (`None` = the start spawn), the clock held at `hour`
    /// (`None` = the project's start hour), slowly drifting with `drift`. An empty `map` = the
    /// start map.
    Map {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        map: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<(f32, f32)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hour: Option<f32>,
        #[serde(default = "yes")]
        drift: bool,
    },
    /// A sprite sheet drawn whole, covering the window.
    Picture { sheet: String },
}

impl Default for TitleBackground {
    fn default() -> Self {
        TitleBackground::Map { map: String::new(), at: None, hour: None, drift: true }
    }
}

/// A logo picture on the title screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TitleLogo {
    /// Sprite sheet id (drawn whole).
    pub sheet: String,
    /// Its middle, as parts of the window's width and height.
    #[serde(default = "half")]
    pub x: f32,
    #[serde(default = "quarter")]
    pub y: f32,
    /// Screen pixels per picture pixel.
    #[serde(default = "one")]
    pub scale: f32,
}

impl TitleLogo {
    pub fn new(sheet: &str) -> Self {
        TitleLogo { sheet: sheet.to_string(), x: 0.5, y: 0.25, scale: 1.0 }
    }
}

/// The title screen (`ProjectConfig::title`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TitleScreen {
    #[serde(default, skip_serializing_if = "is_default_bg")]
    pub background: TitleBackground,
    /// Without a logo the game's name is drawn above the menu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<TitleLogo>,
    /// Sound id; empty = the background map's music.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub music: String,
    /// The entries in order; empty = the built-in ones ([`TitleScreen::DEFAULT`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<MenuEntry>,
    #[serde(default, skip_serializing_if = "is_center")]
    pub align: MenuAlign,
    /// The rows sit on a panel (off: they float over the background).
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub panel: bool,
    /// How much the background is darkened behind the menu, 0 to 1 (`None`: 0.55 over a map,
    /// 0.2 over a picture).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<f32>,
}

impl Default for TitleScreen {
    fn default() -> Self {
        TitleScreen { background: TitleBackground::default(), logo: None, music: String::new(), entries: Vec::new(), align: MenuAlign::Center, panel: true, dim: None }
    }
}

impl TitleScreen {
    /// The built-in title entries.
    pub const DEFAULT: [MenuEntry; 6] = [MenuEntry::NewGame, MenuEntry::Continue, MenuEntry::Load, MenuEntry::Options, MenuEntry::Achievements, MenuEntry::Exit];

    pub fn is_default(&self) -> bool {
        *self == TitleScreen::default()
    }

    /// The entries shown (the built-in ones when none are set).
    pub fn entries(&self) -> Vec<MenuEntry> {
        if self.entries.is_empty() {
            Self::DEFAULT.to_vec()
        } else {
            self.entries.clone()
        }
    }
}

/// An entry of the action bar outside fights (`ProjectConfig::action_bar`): each opens a list or
/// acts at once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BarEntry {
    /// The usable items in the bag: potions, food, scrolls, items with *Use* triggers.
    Use,
    /// The spells the hero can cast now.
    Cast,
    /// The hero's class actions that work outside fights (Second Wind, Lay on Hands).
    Class,
    /// A short rest, or a long one in camp (with the camp item: `RulesSettings::camp_item`).
    Rest,
    /// Sneaking on / off.
    Sneak,
    /// The companions' orders (only with companions).
    Party,
    /// Runs a script function. `label` is shown (and translated), `detail` above the favorites
    /// while the entry has the focus.
    Custom {
        label: String,
        function: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        detail: String,
    },
}

impl BarEntry {
    /// The built-in entries, in order (an empty `action_bar`).
    pub const DEFAULT: [BarEntry; 6] = [BarEntry::Use, BarEntry::Cast, BarEntry::Class, BarEntry::Rest, BarEntry::Sneak, BarEntry::Party];

    /// The name on the bar (custom entries: their label, untranslated).
    pub fn label(&self) -> &str {
        match self {
            BarEntry::Use => "Use",
            BarEntry::Cast => "Cast",
            BarEntry::Class => "Class",
            BarEntry::Rest => "Rest",
            BarEntry::Sneak => "Sneak",
            BarEntry::Party => "Party",
            BarEntry::Custom { label, .. } => label,
        }
    }

    /// The id autoplay's `bar ENTRY` and `status` use (custom entries: their function).
    pub fn id(&self) -> &str {
        match self {
            BarEntry::Use => "use",
            BarEntry::Cast => "cast",
            BarEntry::Class => "class",
            BarEntry::Rest => "rest",
            BarEntry::Sneak => "sneak",
            BarEntry::Party => "party",
            BarEntry::Custom { function, .. } => function,
        }
    }
}

/// The pause menu (`ProjectConfig::menu`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PauseMenu {
    /// The entries in order; empty = the built-in ones ([`PauseMenu::DEFAULT`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<MenuEntry>,
    #[serde(default, skip_serializing_if = "is_center")]
    pub align: MenuAlign,
}

impl PauseMenu {
    /// The built-in pause entries.
    pub const DEFAULT: [MenuEntry; 8] = [MenuEntry::Resume, MenuEntry::Photo, MenuEntry::Save, MenuEntry::Load, MenuEntry::Options, MenuEntry::Achievements, MenuEntry::SaveAndQuit, MenuEntry::QuitToTitle];

    pub fn is_default(&self) -> bool {
        *self == PauseMenu::default()
    }

    pub fn entries(&self) -> Vec<MenuEntry> {
        if self.entries.is_empty() {
            Self::DEFAULT.to_vec()
        } else {
            self.entries.clone()
        }
    }
}

fn yes() -> bool {
    true
}
fn is_yes(v: &bool) -> bool {
    *v
}
fn half() -> f32 {
    0.5
}
fn quarter() -> f32 {
    0.25
}
fn one() -> f32 {
    1.0
}
fn is_center(a: &MenuAlign) -> bool {
    *a == MenuAlign::Center
}
fn is_default_bg(b: &TitleBackground) -> bool {
    *b == TitleBackground::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_bar_entries_round_trip() {
        let mut cfg = crate::project::ProjectConfig::new("p");
        let text = ron::to_string(&cfg).unwrap();
        assert!(!text.contains("action_bar"), "the built-in bar isn't written");
        cfg.action_bar = vec![BarEntry::Rest];
        let back: crate::project::ProjectConfig = ron::from_str(&ron::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back.action_bar, vec![BarEntry::Rest]);
        let list = vec![BarEntry::Use, BarEntry::Sneak, BarEntry::Custom { label: "Pray".into(), function: "pray".into(), detail: String::new() }];
        let text = ron::to_string(&list).unwrap();
        assert_eq!(ron::from_str::<Vec<BarEntry>>(&text).unwrap(), list);
        assert_eq!(BarEntry::DEFAULT.map(|e| e.id().to_string()), ["use", "cast", "class", "rest", "sneak", "party"]);
        assert_eq!(list[2].id(), "pray");
    }

    #[test]
    fn empty_settings_are_the_built_in_menus() {
        let t: TitleScreen = ron::from_str("()").unwrap();
        assert!(t.is_default());
        assert_eq!(t.entries(), TitleScreen::DEFAULT.to_vec());
        assert_eq!(ron::to_string(&t).unwrap(), "()");
        let p: PauseMenu = ron::from_str("()").unwrap();
        assert_eq!(p.entries(), PauseMenu::DEFAULT.to_vec());
        assert!(TitleScreen::DEFAULT.iter().all(|e| e.fits(true)) && PauseMenu::DEFAULT.iter().all(|e| e.fits(false)));
    }

    #[test]
    fn a_designed_title_round_trips() {
        let t = TitleScreen {
            background: TitleBackground::Map { map: "village".into(), at: Some((34.0, 25.0)), hour: Some(19.5), drift: false },
            logo: Some(TitleLogo::new("logo")),
            music: "music_title".into(),
            entries: vec![MenuEntry::NewGame, MenuEntry::Custom { label: "Website".into(), function: "open_site".into(), detail: String::new() }, MenuEntry::Exit],
            align: MenuAlign::Left,
            panel: false,
            dim: Some(0.3),
        };
        let text = ron::to_string(&t).unwrap();
        assert_eq!(ron::from_str::<TitleScreen>(&text).unwrap(), t);
        let pic: TitleScreen = ron::from_str(r#"(background: Picture(sheet: "art"))"#).unwrap();
        assert_eq!(pic.background, TitleBackground::Picture { sheet: "art".into() });
        assert!(!MenuEntry::Resume.fits(true) && !MenuEntry::NewGame.fits(false));
    }
}
