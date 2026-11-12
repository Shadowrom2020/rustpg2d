//! Definition editors: items, props, characters, tilesets, sprite sheets, sounds, project and scripts.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Sense, Stroke, StrokeKind};

use crate::ui::{self, ph};
use rustpg2d_engine::audio::Audio;
use rustpg2d_engine::defs::*;
use rustpg2d_engine::events::TriggerOn;
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::render::Assets;

use crate::app::Editor;
use crate::files::Purpose;
use crate::widgets::{self, Refs, Textures};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Items,
    Props,
    Characters,
    Tilesets,
    Sheets,
    Sounds,
    Quests,
    Project,
    Scripts,
    Structures,
    Skills,
    Spells,
    Recipes,
    Achievements,
    Races,
    Models,
}

/// How many definitions the list of `tab` shows (0 for the tabs without a list).
pub fn tab_len(db: &rustpg2d_engine::defs::Database, tab: Tab) -> usize {
    match tab {
        Tab::Items => db.items.len(),
        Tab::Props => db.props.len(),
        Tab::Characters => db.characters.len(),
        Tab::Tilesets => db.tilesets.len(),
        Tab::Sheets => db.sheets.len(),
        Tab::Sounds => db.sounds.len(),
        Tab::Quests => db.quests.len(),
        Tab::Structures => db.structures.len(),
        Tab::Skills => db.skills.len(),
        Tab::Spells => db.spells.len(),
        Tab::Recipes => db.recipes.len(),
        Tab::Achievements => db.achievements.len(),
        Tab::Races => db.races.len(),
        Tab::Models => db.models.len(),
        Tab::Project | Tab::Scripts => 0,
    }
}

thread_local! {
    /// *Find usages* asked for in a definition list (the id; the open tab gives the kind).
    static USAGES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The id whose usages the Database list asked for (once).
pub fn take_usages_request() -> Option<String> {
    USAGES.with(|u| u.borrow_mut().take())
}

impl Tab {
    /// The kind of definitions the tab edits (`None` for Project and Scripts).
    pub fn kind(self) -> Option<RefKind> {
        Some(match self {
            Tab::Items => RefKind::Item,
            Tab::Props => RefKind::Prop,
            Tab::Characters => RefKind::Character,
            Tab::Tilesets => RefKind::Tileset,
            Tab::Sheets => RefKind::Sheet,
            Tab::Sounds => RefKind::Sound,
            Tab::Quests => RefKind::Quest,
            Tab::Structures => RefKind::Structure,
            Tab::Skills => RefKind::Skill,
            Tab::Spells => RefKind::Spell,
            Tab::Recipes => RefKind::Recipe,
            Tab::Achievements => RefKind::Achievement,
            Tab::Races => RefKind::Race,
            Tab::Models => RefKind::Model,
            Tab::Project | Tab::Scripts => return None,
        })
    }

    /// The tab that edits definitions of `kind` (`None` for maps).
    pub fn of(kind: RefKind) -> Option<Tab> {
        Some(match kind {
            RefKind::Item => Tab::Items,
            RefKind::Prop => Tab::Props,
            RefKind::Character => Tab::Characters,
            RefKind::Tileset => Tab::Tilesets,
            RefKind::Sheet => Tab::Sheets,
            RefKind::Sound => Tab::Sounds,
            RefKind::Quest => Tab::Quests,
            RefKind::Structure => Tab::Structures,
            RefKind::Skill => Tab::Skills,
            RefKind::Spell => Tab::Spells,
            RefKind::Recipe => Tab::Recipes,
            RefKind::Achievement => Tab::Achievements,
            RefKind::Race => Tab::Races,
            RefKind::Model => Tab::Models,
            RefKind::Map => return None,
        })
    }
}

/// Inline "rename id" editor state; at most one rename is open at a time.
#[derive(Default)]
pub struct RenameUi {
    pub buf: Option<String>,
    /// (kind, old, new) requested by the user, executed after the frame's UI.
    pub request: Option<(RefKind, String, String)>,
}

#[derive(Default)]
pub struct DbState {
    pub tab: Tab,
    pub sel: usize,
    pub script_file: Option<std::path::PathBuf>,
    pub script_text: String,
    /// The open script as on disk; `script_dirty` is only set while the text differs from it.
    pub script_saved: String,
    pub script_dirty: bool,
    pub new_script: String,
    /// Character index to move the script editor's cursor to (and scroll to) next frame.
    pub script_jump: Option<usize>,
    /// Script editor (completion popup, pending cursor moves).
    pub script_ed: crate::script_editor::EditorState,
    pub rename: RenameUi,
    /// Tileset editor: animation that clicked tiles are appended to (None = toggle solidity).
    pub anim_target: Option<usize>,
    /// Prop editor: selected state (its sprite is edited below the list) and an open rename.
    pub state_sel: usize,
    pub state_rename: Option<(usize, String)>,
    /// Items / props / characters: the pixel editor is shown under the form.
    pub art_open: bool,
    /// What the embedded editor last focused (to select its frame once).
    pub art_focus: Option<(String, u32)>,
    /// Tilesets tab: clicks on the tile grid toggle light tiles.
    pub tile_light_pick: bool,
    /// Tilesets tab: clicks on the tile grid toggle ladder tiles.
    pub tile_ladder_pick: bool,
    /// Tilesets tab: clicks on the tile grid toggle low tiles (shots fly over them).
    pub tile_low_pick: bool,
    /// Editor script `dbscroll Y`: scroll the definition form there once.
    pub form_scroll: Option<f32>,
    /// Tilesets tab: the terrain whose tiles clicks into the grid assign.
    pub terrain_pick: Option<crate::terrain::TerrainPick>,
    /// The structure editor (Structures tab).
    pub st_edit: crate::structures::StructEdit,
}

/// Everything the definition forms need besides the definition itself.
struct Cx<'a> {
    tex: &'a Textures,
    assets: &'a mut Assets,
    refs: &'a Refs,
    audio: &'a mut Audio,
    rename: &'a mut RenameUi,
    anim_target: &'a mut Option<usize>,
    /// Read-only copy of the definitions (for item icons / stack sizes).
    db: &'a Database,
    browse: &'a mut Option<Purpose>,
    /// Request to open an image in the pixel editor: (path, frame size, character layout).
    /// (image, frame size, character layout, frame to select).
    paint: &'a mut Option<(String, (u32, u32), bool, u32)>,
    state_sel: &'a mut usize,
    state_rename: &'a mut Option<(usize, String)>,
    /// A renamed prop state: (prop id, old name, new name), applied to the maps afterwards.
    state_renamed: &'a mut Option<(String, String, String)>,
    art_open: &'a mut bool,
    /// Art actions handled after the form (they need the whole editor).
    art: &'a mut Option<ArtAction>,
    st_edit: &'a mut crate::structures::StructEdit,
    tile_light_pick: &'a mut bool,
    tile_ladder_pick: &'a mut bool,
    tile_low_pick: &'a mut bool,
    terrain_pick: &'a mut Option<crate::terrain::TerrainPick>,
}

/// An id not yet used in `ids`, based on `prefix`.
fn unique(prefix: &str, ids: &[String]) -> String {
    (1..).map(|n| format!("{prefix}_{n}")).find(|c| !ids.contains(c)).unwrap()
}

thread_local! {
    /// A model's rig template changed: its files are read again after the form.
    static MODEL_RELOAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The Database view belongs to the Asset Library window (set each frame by `ui`):
    /// deleting goes through the library's remove confirmation instead of `list_ui`.
    static LIBRARY_MODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Icon of a Database tab.
fn tab_icon(t: Tab) -> &'static str {
    match t {
        Tab::Items => ph::SWORD,
        Tab::Props => ph::TREASURE_CHEST,
        Tab::Characters => ph::USERS,
        Tab::Tilesets => ph::GRID_FOUR,
        Tab::Sheets => ph::IMAGE,
        Tab::Sounds => ph::SPEAKER_HIGH,
        Tab::Structures => ph::HOUSE,
        Tab::Quests => ph::SCROLL,
        Tab::Skills => ph::STAR,
        Tab::Spells => ph::MAGIC_WAND,
        Tab::Recipes => ph::HAMMER,
        Tab::Achievements => ph::TROPHY,
        Tab::Races => ph::USERS_THREE,
        Tab::Models => ph::CUBE,
        Tab::Project => ph::GEAR,
        Tab::Scripts => ph::CODE,
    }
}

/// List with add / duplicate / delete. Returns true if the list changed.
fn list_ui<T: Clone>(
    ui: &mut egui::Ui,
    items: &mut Vec<T>,
    sel: &mut usize,
    label: impl Fn(&T) -> String,
    make: impl Fn(&[String]) -> T,
    id_of: impl Fn(&mut T) -> &mut String,
) -> bool {
    let mut changed = false;
    let mut ids: Vec<String> = items.iter_mut().map(|t| id_of(t).clone()).collect();
    ui.horizontal(|ui| {
        if ui.button(ui::t(ph::PLUS, "New")).on_hover_text("A new definition with default values").clicked() {
            items.push(make(&ids));
            *sel = items.len() - 1;
            changed = true;
        }
        if ui.add_enabled(*sel < items.len(), egui::Button::new(ui::t(ph::COPY, "Duplicate"))).on_hover_text("A copy of the selected definition with a new id").clicked() {
            let mut c = items[*sel].clone();
            let id = id_of(&mut c);
            *id = unique(id, &ids);
            ids.push(id.clone());
            items.push(c);
            *sel = items.len() - 1;
            changed = true;
        }
        let library = LIBRARY_MODE.with(|m| m.get());
        if !library && ui.add_enabled(*sel < items.len(), egui::Button::new(ui::t(ph::TRASH, "Delete"))).on_hover_text("Removes the definition (references in maps are not removed)").clicked() {
            items.remove(*sel);
            *sel = sel.saturating_sub(1);
            changed = true;
        }
        if ui
            .add_enabled(*sel < ids.len(), egui::Button::new(ui::t(ph::LIST_MAGNIFYING_GLASS, "Usages")))
            .on_hover_text("Find usages: every map, trigger, definition and script that uses the selected definition (also right-click a name)")
            .clicked()
        {
            USAGES.with(|u| *u.borrow_mut() = ids.get(*sel).cloned());
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("db_list").auto_shrink(false).show(ui, |ui| {
        for (i, it) in items.iter().enumerate() {
            let r = ui.selectable_label(*sel == i, label(it));
            if r.clicked() {
                *sel = i;
            }
            r.context_menu(|ui| {
                if ui.button(ui::t(ph::LIST_MAGNIFYING_GLASS, "Find usages")).clicked() {
                    *sel = i;
                    USAGES.with(|u| *u.borrow_mut() = ids.get(i).cloned());
                    ui.close_menu();
                }
            });
        }
    });
    changed
}

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let library = ed.library_mode;
    LIBRARY_MODE.with(|m| m.set(library));
    if library && matches!(ed.db.tab, Tab::Quests | Tab::Achievements | Tab::Project | Tab::Scripts) {
        ed.db.tab = Tab::Items;
    }
    // Wraps onto a second line in narrow windows (instead of widening the forms below).
    ui.horizontal_wrapped(|ui| {
        let tabs = [
            (Tab::Items, "Items"),
            (Tab::Props, "Props"),
            (Tab::Characters, "Characters"),
            (Tab::Races, "Races"),
            (Tab::Models, "3D models"),
            (Tab::Tilesets, "Tilesets"),
            (Tab::Sheets, "Sprite sheets"),
            (Tab::Sounds, "Sounds"),
            (Tab::Structures, "Structures"),
            (Tab::Skills, "Skills"),
            (Tab::Spells, "Spells"),
            (Tab::Recipes, "Recipes"),
            (Tab::Quests, "Quests"),
            (Tab::Achievements, "Achievements"),
            (Tab::Project, "Project"),
            (Tab::Scripts, "Scripts"),
        ];
        for (t, l) in tabs {
            if library && matches!(t, Tab::Quests | Tab::Achievements | Tab::Project | Tab::Scripts) {
                continue; // the library only holds assets
            }
            if ui.selectable_label(ed.db.tab == t, ui::t(tab_icon(t), l)).clicked() && ed.db.tab != t {
                ed.db.tab = t;
                ed.db.sel = 0;
                ed.db.rename = RenameUi::default();
                ed.db.anim_target = None;
                ed.db.terrain_pick = None;
            }
        }
        ui.separator();
        let (u, r) = (ed.db_hist.can_undo(), ed.db_hist.can_redo());
        if ui::icon_button(ui, u, ph::ARROW_ARC_LEFT, "Undo", "Ctrl+Z", "Undoes the latest change to the definitions or project settings (a rename is renamed back, maps and scripts included)").clicked() {
            ed.db_undo();
        }
        if ui::icon_button(ui, r, ph::ARROW_ARC_RIGHT, "Redo", "Ctrl+Y", "Redoes the change undone last").clicked() {
            ed.db_redo();
        }
        if ui.button(ui::t(ph::FLOPPY_DISK, "Save definitions")).on_hover_text("Writes defs/*.ron (Ctrl+S saves everything)").clicked() {
            ed.save_defs();
        }
        if ui.button(ui::t(ph::ARROW_CLOCKWISE, "Reload assets")).on_hover_text("Re-reads images, sounds and scripts from disk").clicked() {
            ed.reload_assets();
        }
        ui.separator();
        // The selected asset of this tab, for the library / package buttons.
        let i = ed.db.sel;
        let db = &ed.project.db;
        let current: Option<rustpg2d_engine::library::AssetRef> = match ed.db.tab {
            Tab::Items => db.items.get(i).map(|d| (RefKind::Item, d.id.clone())),
            Tab::Props => db.props.get(i).map(|d| (RefKind::Prop, d.id.clone())),
            Tab::Characters => db.characters.get(i).map(|d| (RefKind::Character, d.id.clone())),
            Tab::Tilesets => db.tilesets.get(i).map(|d| (RefKind::Tileset, d.id.clone())),
            Tab::Sheets => db.sheets.get(i).map(|d| (RefKind::Sheet, d.id.clone())),
            Tab::Sounds => db.sounds.get(i).map(|d| (RefKind::Sound, d.id.clone())),
            Tab::Structures => db.structures.get(i).map(|d| (RefKind::Structure, d.id.clone())),
            Tab::Skills => db.skills.get(i).map(|d| (RefKind::Skill, d.id.clone())),
            Tab::Spells => db.spells.get(i).map(|d| (RefKind::Spell, d.id.clone())),
            Tab::Recipes => db.recipes.get(i).map(|d| (RefKind::Recipe, d.id.clone())),
            Tab::Races => db.races.get(i).map(|d| (RefKind::Race, d.id.clone())),
            Tab::Models => db.models.get(i).map(|d| (RefKind::Model, d.id.clone())),
            _ => None,
        };
        if !library {
            let kind = match ed.db.tab {
                Tab::Items => Some(RefKind::Item),
                Tab::Props => Some(RefKind::Prop),
                Tab::Characters => Some(RefKind::Character),
                Tab::Tilesets => Some(RefKind::Tileset),
                Tab::Sheets => Some(RefKind::Sheet),
                Tab::Sounds => Some(RefKind::Sound),
                Tab::Structures => Some(RefKind::Structure),
                Tab::Skills => Some(RefKind::Skill),
                Tab::Spells => Some(RefKind::Spell),
                Tab::Recipes => Some(RefKind::Recipe),
                Tab::Races => Some(RefKind::Race),
                Tab::Models => Some(RefKind::Model),
                _ => None,
            };
            if kind.is_some() {
                ed.add_from_library_button(ui, kind);
            }
        }
        if let Some(a) = current {
            if library {
                if ui.button(ui::t(ph::TRASH, "Delete…")).on_hover_text(format!("Removes '{}' from the library (asks first)", a.1)).clicked() {
                    ed.lib.confirm_remove = Some(vec![a.clone()]);
                }
            } else if ui.button(ui::t(ph::UPLOAD_SIMPLE, "Add to library")).on_hover_text(format!("Copies '{}' and everything it needs into the shared asset library", a.1)).clicked() {
                ed.send_project_assets(vec![a.clone()], crate::library_ui::SendTo::Library);
            }
            if ui.button(ui::t(ph::PACKAGE, "Export…")).on_hover_text(format!("Saves '{}' and everything it needs as a .r2dpack file", a.1)).clicked() {
                ed.open_file_dialog(Purpose::ExportPackage { from_library: false, assets: vec![a] });
            }
        }
        if ui.button(ui::t(ph::USER, "New character…")).on_hover_text("A new character from an existing 3D model, an imported one or one generated with AI").clicked() {
            ed.open_builder(crate::builder::Target::New);
        }
        if ui.button(ui::t(ph::SPARKLE, "New asset…")).on_hover_text("A prop, item, tileset, sprite sheet or structure (a wizard asks the details)").clicked() {
            ed.wizard = Some(crate::wizard::Wizard::new(&ed.project));
        }
    });
    ui.separator();

    let before = (ed.project.db.clone(), ed.project.config.clone());
    let mut paint: Option<(String, (u32, u32), bool, u32)> = None;
    let mut state_renamed: Option<(String, String, String)> = None;
    let mut art: Option<ArtAction> = None;
    let mut art_target: Option<(String, (u32, u32), bool, u32)> = None;
    let mut list_changed = false;
    let mut place_structure: Option<String> = None;
    let tab = ed.db.tab;
    let sel_before = ed.db.sel;
    if matches!(tab, Tab::Project | Tab::Scripts) {
        egui::CentralPanel::default().show_inside(ui, |ui| match tab {
            Tab::Project => project_ui(ed, ui),
            _ => scripts_ui(ed, ui),
        });
    } else {
        let fresh = crate::builder::default_character(&ed.project.db, "", "New character");
        let db = &mut ed.project.db;
        let sel = &mut ed.db.sel;
        egui::SidePanel::left("db_left").resizable(true).default_width(220.0).show_inside(ui, |ui| {
            list_changed = match tab {
                Tab::Items => list_ui(
                    ui,
                    &mut db.items,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| ItemDef::fresh(&unique("item", ids)),
                    |d| &mut d.id,
                ),
                Tab::Props => list_ui(
                    ui,
                    &mut db.props,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| PropDef::fresh(&unique("prop", ids)),
                    |d| &mut d.id,
                ),
                Tab::Characters => list_ui(
                    ui,
                    &mut db.characters,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| CharacterDef { id: unique("character", ids), ..fresh.clone() },
                    |d| &mut d.id,
                ),
                Tab::Tilesets => list_ui(
                    ui,
                    &mut db.tilesets,
                    sel,
                    |d| d.id.clone(),
                    |ids| TilesetDef::fresh(&unique("tileset", ids)),
                    |d| &mut d.id,
                ),
                Tab::Sheets => list_ui(
                    ui,
                    &mut db.sheets,
                    sel,
                    |d| d.id.clone(),
                    |ids| SheetDef::fresh(&unique("sheet", ids)),
                    |d| &mut d.id,
                ),
                Tab::Sounds => list_ui(
                    ui,
                    &mut db.sounds,
                    sel,
                    |d| d.id.clone(),
                    |ids| SoundDef { id: unique("sound", ids), file: "sounds/new.wav".into(), volume: 1.0 },
                    |d| &mut d.id,
                ),
                Tab::Structures => list_ui(
                    ui,
                    &mut db.structures,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| StructureDef { id: unique("structure", ids), name: "New structure".into(), w: 6, h: 5, ..Default::default() },
                    |d| &mut d.id,
                ),
                Tab::Skills => list_ui(
                    ui,
                    &mut db.skills,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::skills::SkillDef::new(&unique("skill", ids), "New skill", "", SpriteRef::default(), 5, rustpg2d_engine::skills::SkillEffect::None),
                    |d| &mut d.id,
                ),
                Tab::Spells => list_ui(
                    ui,
                    &mut db.spells,
                    sel,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::skills::SpellDef::new(&unique("spell", ids), "New spell", "", SpriteRef::default(), 5, 1.0, rustpg2d_engine::skills::SpellEffect::Heal { amount: 4 }),
                    |d| &mut d.id,
                ),
                Tab::Models => {
                    if ui.button(ui::t(ph::UPLOAD_SIMPLE, "Import model…")).on_hover_text("A rigged .glb (Meshy, Blender, Mixamo…): copied into models/, its clips matched to the animations by name").clicked() {
                        ed.browse_request = Some(Purpose::ModelFile { id: String::new(), clips: false });
                    }
                    list_ui(ui, &mut db.models, sel, |d| format!("{}  ({}){}", d.label(), d.id, if d.playable { "  - playable" } else { "" }), |ids| rustpg2d_engine::defs::ModelDef::new(&unique("model", ids), ""), |d| &mut d.id)
                }
                Tab::Races => list_ui(ui, &mut db.races, sel, |d| format!("{}  ({}){}", d.name, d.id, if d.playable { "" } else { "  - not playable" }), |ids| rustpg2d_engine::races::RaceDef::fresh(&unique("race", ids)), |d| &mut d.id),
                Tab::Recipes => {
                    // Listed by what they make.
                    let names: std::collections::HashMap<String, String> = db.items.iter().map(|d| (d.id.clone(), d.name.clone())).collect();
                    list_ui(
                        ui,
                        &mut db.recipes,
                        sel,
                        |d| {
                            let made = names.get(&d.result).cloned().unwrap_or_else(|| d.result.clone());
                            let at = if d.station.is_empty() { String::new() } else { format!(" @ {}", d.station) };
                            format!("{made}{at}  ({})", d.id)
                        },
                        |ids| rustpg2d_engine::crafting::RecipeDef::new(&unique("recipe", ids), "", 1, Vec::new(), ""),
                        |d| &mut d.id,
                    )
                }
                Tab::Achievements => list_ui(
                    ui,
                    &mut db.achievements,
                    sel,
                    |d| format!("{}{}  ({})", d.name, if d.hidden { " (hidden)" } else { "" }, d.id),
                    |ids| rustpg2d_engine::achievements::AchievementDef::new(&unique("achievement", ids), "New achievement", "", SpriteRef::default(), Vec::new()),
                    |d| &mut d.id,
                ),
                Tab::Quests => list_ui(
                    ui,
                    &mut db.quests,
                    sel,
                    |d| format!("{}  ({})", d.title, d.id),
                    |ids| rustpg2d_engine::quest::QuestDef { id: unique("quest", ids), title: "New quest".into(), description: String::new(), stages: vec!["First objective".into()], xp: 0 },
                    |d| &mut d.id,
                ),
                _ => false,
            };
        });
        if ed.db.sel != sel_before {
            ed.db.rename = RenameUi::default();
            ed.db.anim_target = None;
            ed.db.terrain_pick = None;
        }
        let db_snapshot = ed.project.db.clone();
        let mut embed: Option<(String, (u32, u32), bool)> = None;
        // A sheet's frame size or scale changed: its textures are cut / drawn anew.
        let mut sheet_changed = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Sprite sheets: a compact form on top, the pixel editor below it.
            let art_tab = matches!(tab, Tab::Items | Tab::Props | Tab::Characters) && ed.db.art_open;
            let form_height = if tab == Tab::Sheets {
                210.0
            } else if art_tab {
                (ui.available_height() * 0.45).max(220.0)
            } else {
                f32::INFINITY
            };
            let mut form_area = egui::ScrollArea::vertical().id_salt("db_form").max_height(form_height);
            if let Some(y) = ed.db.form_scroll.take() {
                form_area = form_area.vertical_scroll_offset(y);
            }
            form_area.show(ui, |ui| {
                let mut cx = Cx {
                    tex: &ed.tex,
                    assets: &mut ed.assets,
                    refs: &ed.refs,
                    audio: &mut ed.audio,
                    rename: &mut ed.db.rename,
                    anim_target: &mut ed.db.anim_target,
                    browse: &mut ed.browse_request,
                    db: &db_snapshot,
                    paint: &mut paint,
                    state_sel: &mut ed.db.state_sel,
                    state_rename: &mut ed.db.state_rename,
                    state_renamed: &mut state_renamed,
                    art_open: &mut ed.db.art_open,
                    art: &mut art,
                    st_edit: &mut ed.db.st_edit,
                    tile_light_pick: &mut ed.db.tile_light_pick,
                    tile_ladder_pick: &mut ed.db.tile_ladder_pick,
                    tile_low_pick: &mut ed.db.tile_low_pick,
                    terrain_pick: &mut ed.db.terrain_pick,
                };
                let i = ed.db.sel;
                let db = &mut ed.project.db;
                match tab {
                    Tab::Items => {
                        if let Some(d) = db.items.get_mut(i) {
                            item_form(ui, d, &mut cx);
                            art_target = art_of(&db_snapshot, &d.icon, false);
                        }
                    }
                    Tab::Props => {
                        if let Some(d) = db.props.get_mut(i) {
                            prop_form(ui, d, &mut cx);
                            let k = (*cx.state_sel).min(d.states.len());
                            art_target = art_of(&db_snapshot, d.state_look(k).0, false);
                        }
                    }
                    Tab::Characters => {
                        if let Some(d) = db.characters.get_mut(i) {
                            character_form(ui, d, &mut cx);
                            art_target = art_of(&db_snapshot, &d.portrait, false);
                        }
                    }
                    Tab::Tilesets => {
                        if let Some(d) = db.tilesets.get_mut(i) {
                            tileset_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Sheets => {
                        if let Some(d) = db.sheets.get_mut(i) {
                            let before = (d.frame_w, d.frame_h, d.scale);
                            sheet_form(ui, d, &mut cx);
                            sheet_changed = before != (d.frame_w, d.frame_h, d.scale);
                            embed = Some((d.image.clone(), (d.frame_w, d.frame_h), false));
                        }
                    }
                    Tab::Sounds => {
                        if let Some(d) = db.sounds.get_mut(i) {
                            sound_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Quests => {
                        if let Some(d) = db.quests.get_mut(i) {
                            quest_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Skills => {
                        if let Some(d) = db.skills.get_mut(i) {
                            skill_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Spells => {
                        if let Some(d) = db.spells.get_mut(i) {
                            spell_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Recipes => {
                        if let Some(d) = db.recipes.get_mut(i) {
                            recipe_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Achievements => {
                        if let Some(d) = db.achievements.get_mut(i) {
                            achievement_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Races => {
                        if let Some(d) = db.races.get_mut(i) {
                            race_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Models => {
                        if let Some(d) = db.models.get_mut(i) {
                            model_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Structures => {
                        if let Some(d) = db.structures.get_mut(i) {
                            if structure_form(ui, d, &mut cx) {
                                place_structure = Some(d.id.clone());
                            }
                        }
                    }
                    _ => {}
                }
            });
            if sheet_changed || MODEL_RELOAD.with(|r| r.replace(false)) {
                ed.reload_assets();
            }
            if let Some((rel, frame, character)) = embed.take() {
                ui.separator();
                let h = ui.available_height().max(260.0);
                let layout = if character { crate::pixel::Layout::Character } else { crate::pixel::Layout::Plain };
                ed.pixel_embedded(ui, &rel, frame, layout, h);
            }
            // Items / props / characters: their art under the form.
            if art_tab {
                ui.separator();
                match art_target.clone() {
                    Some((rel, frame, character, sel)) => {
                        let h = ui.available_height().max(260.0);
                        let layout = if character { crate::pixel::Layout::Character } else { crate::pixel::Layout::Plain };
                        ed.pixel_embedded(ui, &rel, frame, layout, h);
                        // Select the definition's frame when the focus changes.
                        let focus = Some((rel.clone(), sel));
                        if ed.db.art_focus != focus && ed.px.doc.as_ref().is_some_and(|d| d.rel == rel) {
                            ed.px.sel_frame = sel;
                            ed.db.art_focus = focus;
                        }
                    }
                    None => {
                        ui.weak("No sprite sheet chosen yet: choose a sprite, or create a blank one with the buttons above.");
                    }
                }
            }
        });
    }
    if let Some((rel, frame, character, sel)) = paint.take() {
        ed.open_pixel(&rel, frame, if character { crate::pixel::Layout::Character } else { crate::pixel::Layout::Plain });
        if ed.px.doc.as_ref().is_some_and(|d| d.rel == rel) {
            ed.px.sel_frame = sel;
        }
    }
    if let Some((prop, old, new)) = state_renamed.take() {
        ed.rename_prop_state(&prop, &old, &new);
    }
    if let Some(a) = art.take() {
        ed.art_action(a, tab);
    }
    if let Some(id) = place_structure {
        ed.mode = crate::app::Mode::Map;
        ed.arm_structure(&id);
    }
    if list_changed || before.0 != ed.project.db || before.1 != ed.project.config {
        ed.defs_dirty = true;
        ed.project.db.reindex();
        if before.0.quests != ed.project.db.quests {
            ed.refs.quests = ed.project.db.quests.iter().map(|q| q.id.clone()).collect();
            ed.refs.quest_stages = ed.project.db.quests.iter().map(|q| (q.id.clone(), q.stages.clone())).collect();
        }
        if before.0.items != ed.project.db.items || before.0.sounds != ed.project.db.sounds {
            ed.refs.items = ed.project.db.items.iter().map(|i| i.id.clone()).collect();
            ed.refs.sounds = ed.project.db.sounds.iter().map(|i| i.id.clone()).collect();
        }
    }
    if let Some((kind, old, new)) = ed.db.rename.request.take() {
        ed.rename_id(kind, &old, &new);
    }
}

/// Shows an id with a "Rename" button; renaming updates every reference in the project.
pub fn rename_row(ui: &mut egui::Ui, kind: RefKind, id: &str, rn: &mut RenameUi) {
    ui.horizontal(|ui| {
        ui.label("Id");
        match &mut rn.buf {
            None => {
                ui.monospace(id);
                if ui.small_button("Rename…").on_hover_text("Renames the id and updates all references in maps, triggers and definitions").clicked() {
                    rn.buf = Some(id.to_string());
                }
            }
            Some(buf) => {
                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(140.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Apply").clicked() || enter {
                    rn.request = Some((kind, id.to_string(), buf.trim().to_string()));
                    rn.buf = None;
                } else if ui.button("Cancel").clicked() {
                    rn.buf = None;
                }
            }
        }
    });
}

/// A multi-line text box as wide as the visible form, minus a margin for the scroll bar (the
/// form's content may be wider than the window).
fn text_box(ui: &mut egui::Ui, text: &mut String, rows: usize, hint: &str) -> egui::Response {
    let w = (ui.clip_rect().right() - ui.cursor().left() - 18.0).clamp(200.0, ui.available_width());
    ui.add(egui::TextEdit::multiline(text).desired_rows(rows).desired_width(w).hint_text(hint))
}

fn name_row(ui: &mut egui::Ui, name: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(name);
    });
}

fn hitbox(ui: &mut egui::Ui, hb: &mut [f32; 2]) {
    ui.horizontal(|ui| {
        ui.label("Hitbox w/h (px)");
        ui.add(egui::DragValue::new(&mut hb[0]).range(1.0..=256.0));
        ui.add(egui::DragValue::new(&mut hb[1]).range(1.0..=256.0));
    });
}

/// Editor for an optional weapon. Characters only use ranged weapons.
fn weapon_ui(ui: &mut egui::Ui, id: &str, w: &mut Option<Weapon>, cx: &mut Cx, character: bool) {
    let mut on = w.is_some();
    // Items choose "Weapon" in the Equipment section; characters toggle their ranged attack here.
    if character && ui.checkbox(&mut on, "Shoots projectiles").changed() {
        *w = on.then(|| {
            if character {
                Weapon::ranged(1, 1.5, Ranged { projectile: SpriteRef::default(), speed: 360.0, range: 6.0, ammo: String::new() })
            } else {
                Weapon::melee(1, 0.4)
            }
        });
    }
    let Some(w) = w else { return };
    egui::Grid::new(format!("{id}/grid")).num_columns(2).show(ui, |ui| {
        ui.label("Damage");
        ui.add(egui::DragValue::new(&mut w.damage).range(0..=999));
        ui.end_row();
        ui.label("Cooldown (s)");
        ui.add(egui::DragValue::new(&mut w.cooldown).range(0.05..=10.0).speed(0.05));
        ui.end_row();
        if !character {
            ui.label("Type");
            ui.horizontal(|ui| {
                let mut ranged = w.ranged.is_some();
                ui.radio_value(&mut ranged, false, "Melee (swing)");
                ui.radio_value(&mut ranged, true, "Ranged (projectiles)");
                if ranged != w.ranged.is_some() {
                    w.ranged = ranged.then(|| Ranged { projectile: SpriteRef::default(), speed: 440.0, range: 8.0, ammo: String::new() });
                }
            });
            ui.end_row();
        }
        if w.ranged.is_none() {
            ui.label("Reach (tiles)");
            ui.add(egui::DragValue::new(&mut w.reach).range(0.3..=6.0).speed(0.05)).on_hover_text("How far the swing reaches");
            ui.end_row();
            ui.label("Arc (degrees)");
            ui.add(egui::DragValue::new(&mut w.arc).range(5.0..=180.0)).on_hover_text("Half-angle of the swing: 20 = narrow thrust, 90 = everything in front");
            ui.end_row();
        }
        ui.label("Knockback");
        ui.add(egui::DragValue::new(&mut w.knockback).range(0.0..=5.0).speed(0.05)).on_hover_text("Multiplier");
        ui.end_row();
        if !character {
            ui.label("Hands");
            ui.checkbox(&mut w.two_handed, "Two-handed").on_hover_text("Nothing can be held in the left hand (shield, lantern) while this weapon is equipped");
            ui.end_row();
            ui.label("Critical hits");
            let mut pct = (w.crit * 100.0).round() as i32;
            if ui
                .add(egui::DragValue::new(&mut pct).range(0..=100).suffix(" %"))
                .on_hover_text("Chance that a hit of the player's with this weapon is critical: double damage, a big yellow number. 0 = never.")
                .changed()
            {
                w.crit = pct as f32 / 100.0;
            }
            ui.end_row();
        }
        if let Some(r) = &mut w.ranged {
            ui.label("Speed (px/s)");
            ui.add(egui::DragValue::new(&mut r.speed).range(10.0..=2000.0));
            ui.end_row();
            ui.label("Range (tiles)");
            ui.add(egui::DragValue::new(&mut r.range).range(0.5..=64.0).speed(0.1));
            ui.end_row();
            if !character {
                ui.label("Ammo item");
                ui.horizontal(|ui| {
                    widgets::suggest(ui, &format!("{id}/ammo"), &mut r.ammo, &cx.refs.items, 110.0);
                    ui.weak("(empty = unlimited)");
                });
                ui.end_row();
            }
        }
    });
    widgets::optional_effect_ui(
        ui,
        &format!("{id}/fx"),
        "On hit:",
        "A status effect for whoever this weapon hits (melee swings and projectiles), e.g. a poisoned dagger or a freezing bolt",
        &mut w.effect,
    );
    if let Some(r) = &mut w.ranged {
        ui.label("Projectile sprite (drawn pointing right, rotated in flight):");
        widgets::sprite_picker(ui, &format!("{id}/proj"), &mut r.projectile, cx.tex, cx.assets);
    }
}

fn item_form(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    ui.heading("Item");
    rename_row(ui, RefKind::Item, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    text_box(ui, &mut d.description, 2, "");
    ui.checkbox(&mut d.consumable, "Consumed when used (if a trigger ran)");
    ui.checkbox(&mut d.food, "Food").on_hover_text("Its 'On use' healing is raised by the player's skills with the Food effect (Cooking).");
    ui.horizontal(|ui| {
        ui.label("Stack size");
        ui.add(egui::DragValue::new(&mut d.max_stack).range(1..=9999)).on_hover_text("How many fit into one inventory slot");
        ui.label("Value");
        ui.add(egui::DragValue::new(&mut d.value).range(0..=999_999))
            .on_hover_text("Base trade price; merchants use it when their stock has no own price (0 = worthless, can't be sold)");
    });
    ui.horizontal(|ui| {
        ui.label("Use on");
        egui::ComboBox::from_id_salt("item_use_on").selected_text(d.use_on.label()).show_ui(ui, |ui| {
            for t in rustpg2d_engine::defs::UseTarget::ALL {
                ui.selectable_value(&mut d.use_on, t, t.label());
            }
        });
        ui.weak("(healing items, whose 'On use' heals: with companions around the player chooses whom to heal)");
    });
    ui.separator();
    crop_ui(ui, d, cx);
    ui.separator();
    ui.heading("Equipment");
    ui.weak("Equipment goes into a character-sheet slot when used from the inventory (weapons also with Q) instead of running 'On use' triggers.");
    let mut kind = match (&d.weapon, &d.armor) {
        _ if !d.tool.is_empty() => 6,
        (Some(_), _) => 1,
        (None, Some(a)) if a.slot == EquipSlot::Head => 2,
        (None, Some(a)) if a.slot == EquipSlot::Offhand => 4,
        (None, Some(a)) if a.slot == EquipSlot::Clothing => 5,
        (None, Some(_)) => 3,
        _ => 0,
    };
    let old = kind;
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut kind, 0, "None");
        ui.radio_value(&mut kind, 1, "Weapon");
        ui.radio_value(&mut kind, 2, "Helmet");
        ui.radio_value(&mut kind, 3, "Body armor");
        ui.radio_value(&mut kind, 4, "Left hand").on_hover_text("A shield (defense) or a light (lantern, torch: set its light below). Not usable together with a two-handed weapon.");
        ui.radio_value(&mut kind, 5, "Clothing").on_hover_text("An outfit (defense, speed); the model's art shows what a character wears.");
        ui.radio_value(&mut kind, 6, "Tool").on_hover_text("Goes into the Tool slot (a fishing rod, later pickaxes, hoes...): held in the right hand while it is used. Activities ask for it, e.g. a fishing spot's 'Needs item'.");
    });
    if kind != old {
        d.tool = if kind == 6 { if d.tool.is_empty() { "tool".into() } else { d.tool.clone() } } else { String::new() };
        d.weapon = (kind == 1).then(|| Weapon::melee(1, 0.4));
        d.armor = match kind {
            2..=5 => {
                let slot = match kind {
                    2 => EquipSlot::Head,
                    3 => EquipSlot::Body,
                    5 => EquipSlot::Clothing,
                    _ => EquipSlot::Offhand,
                };
                let defense = if slot == EquipSlot::Clothing { 0 } else { 1 };
                Some(d.armor.take().map(|a| Armor { slot, ..a }).unwrap_or(Armor { slot, defense, speed: 0.0 }))
            }
            _ => None,
        };
        if kind != 0 && kind != 4 {
            d.max_stack = 1;
        }
    }
    if d.weapon.is_some() {
        weapon_ui(ui, "item_weapon", &mut d.weapon, cx, false);
    }
    if let Some(a) = &mut d.armor {
        egui::Grid::new("armor").num_columns(2).show(ui, |ui| {
            ui.label("Defense");
            ui.add(egui::DragValue::new(&mut a.defense).range(-99..=99)).on_hover_text("Subtracted from incoming damage (1 heart = 2 HP)");
            ui.end_row();
            ui.label("Speed change");
            let mut pct = a.speed * 100.0;
            if ui.add(egui::DragValue::new(&mut pct).range(-90.0..=100.0).suffix(" %")).changed() {
                a.speed = pct / 100.0;
            }
            ui.end_row();
        });
        ui.weak("Worn items count in play (defense, speed); a character's model shows what it wears.");
    }
    if !d.tool.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Tool kind");
            let kinds: Vec<String> = {
                let mut v: Vec<String> = cx.db.items.iter().map(|i| i.tool.clone()).filter(|t| !t.is_empty()).collect();
                v.sort();
                v.dedup();
                v
            };
            widgets::suggest(ui, "item_tool", &mut d.tool, &kinds, 140.0);
            ui.weak("(what it is: \"fishing rod\", \"pickaxe\"...)");
        });
        if d.tool.trim().is_empty() {
            d.tool = "tool".into();
        }
    }
    ui.separator();
    ui.heading("Magic");
    ui.horizontal(|ui| {
        ui.label("Teaches spell");
        widgets::suggest(ui, "item_teaches", &mut d.teaches, &cx.refs.spells, 140.0);
        ui.weak("(scrolls, spell books; using the item learns it — tick 'Consumed' for one-use scrolls)");
    });
    ui.separator();
    ui.heading("Light");
    widgets::light_ui(ui, "item_light", &mut d.light, "Gives light (while equipped, e.g. in the left hand, or lying on the ground)");
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "item_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
    art_buttons(ui, cx, "icon", (1, 1));
    let holdable = d.weapon.is_some() || !d.tool.is_empty() || d.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Offhand);
    if holdable {
        ui.separator();
        item_model_ui(ui, d, cx);
        ui.separator();
        if d.model.is_some() {
            egui::CollapsingHeader::new("Picture in the hand (used without a 3D model)").id_salt("item_held_picture").show(ui, |ui| held_ui(ui, d, cx));
        } else {
            held_ui(ui, d, cx);
        }
    }
    ui.separator();
    ui.heading("On use");
    ui.weak("Runs when the item is used from the inventory and the entity in front of the player didn't react to it.");
    widgets::triggers_ui(ui, &format!("item/{}", d.id), &mut d.on_use, cx.refs, rustpg2d_engine::events::TriggerOwner::ItemUse);
    for t in &mut d.on_use {
        if !matches!(t.on, TriggerOn::UseItem { .. }) {
            t.on = TriggerOn::UseItem { item: String::new() };
        }
    }
}

/// How a weapon, left-hand item or tool looks in a figure's hand (`ItemDef.held`).
fn held_ui(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    use rustpg2d_engine::hands::{Hand, HeldSprite};
    ui.heading("Held");
    ui.weak("3D characters hold this picture at their hand bone (items with a 3D model hold that instead): its hold point goes on the hand, its orientation mark on the hand's grip axis (the line a handle runs through the fist).");
    let mut custom = d.held.is_some();
    if ui.checkbox(&mut custom, "Own marks").on_hover_text("Off: its icon, held at the lower left corner, its blade pointing up-right along the grip axis").changed() {
        d.held = custom.then(HeldSprite::default);
    }
    let by_kind = match &d.weapon {
        _ if d.armor.is_some() => Hand::Left,
        Some(w) if w.ranged.is_some() => Hand::Left,
        Some(w) if w.two_handed => Hand::Both,
        _ => Hand::Right,
    };
    let icon = d.icon.clone();
    let Some(h) = &mut d.held else { return };
    marks_ui(ui, "item_held", h, &icon, cx, Some(by_kind));
}

/// How characters carry a prop in both hands (`PropDef.carry`, *Carry prop*).
fn carry_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    use rustpg2d_engine::hands::HeldSprite;
    ui.heading("Carried");
    ui.weak("When a character carries it (the Carry prop action, carry() in scripts) it sits between the hands: its hold point there, its orientation mark along the line through both fists. It never turns upside down.");
    let mut custom = d.carry.is_some();
    if ui.checkbox(&mut custom, "Own marks").on_hover_text("Off: held at the middle of its lower half, its horizontal along the hands, at 0.6 of its size").changed() {
        d.carry = custom.then(HeldSprite::carried);
    }
    let sprite = d.sprite.clone();
    let Some(m) = &mut d.carry else { return };
    marks_ui(ui, "prop_carry", m, &sprite, cx, None);
}

/// The marks of something held (`HeldSprite`): its picture (none: `default`), the hold point (click
/// the picture) and orientation mark (drag from the hold point), size and hand (`by_kind`: an
/// item's hand when none is chosen; None: both hands, a carried prop), with previews in the hands
/// of the first figure with hand marks.
fn marks_ui(ui: &mut egui::Ui, salt: &str, h: &mut rustpg2d_engine::hands::HeldSprite, default: &SpriteRef, cx: &mut Cx, by_kind: Option<rustpg2d_engine::hands::Hand>) {
    use rustpg2d_engine::hands::Hand;
    ui.label("Picture (none: the icon / the prop's sprite)");
    widgets::sprite_field(ui, &format!("{salt}_sprite"), &mut h.sprite, cx.tex, cx.assets, cx.db, false);
    let sprite = if h.sprite.is_empty() { default.clone() } else { h.sprite.clone() };
    egui::Grid::new((salt, "grid")).num_columns(2).show(ui, |ui| {
        ui.label("Hold point");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut h.grip[0]).range(0.0..=1.0).speed(0.01).prefix("x "));
            ui.add(egui::DragValue::new(&mut h.grip[1]).range(0.0..=1.0).speed(0.01).prefix("y "));
            ui.weak("(click the picture)");
        });
        ui.end_row();
        ui.label("Orientation");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut h.points).range(-180.0..=180.0).suffix(" deg"));
            ui.weak("(drag from the hold point: the direction that lies on the hand's grip axis)");
        });
        ui.end_row();
        ui.label("Size");
        ui.add(egui::DragValue::new(&mut h.scale).range(0.1..=4.0).speed(0.02)).on_hover_text("1 = its size in the world; the hand's size multiplies it");
        ui.end_row();
        if by_kind.is_some() {
            ui.label("Hand");
            let label = |x: Option<Hand>| match x {
                None => "By its kind",
                Some(Hand::Right) => "Right",
                Some(Hand::Left) => "Left",
                Some(Hand::Both) => "Both",
            };
            egui::ComboBox::from_id_salt((salt, "hand")).selected_text(label(h.hand)).show_ui(ui, |ui| {
                for x in [None, Some(Hand::Right), Some(Hand::Left), Some(Hand::Both)] {
                    ui.selectable_value(&mut h.hand, x, label(x));
                }
            })
            .response
            .on_hover_text("By its kind: left-hand items and bows the left, two-handed weapons both, others the right");
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        // The picture: a click sets the hold point, a drag the orientation mark.
        let Some(s) = cx.assets.sheets.get(&sprite.sheet) else {
            ui.weak("(no picture)");
            return;
        };
        let (cw, ch) = sprite.cells();
        let src = s.region(sprite.frame, cw, ch);
        let k = 128.0 / src.w.max(src.h);
        let (rect, resp) = ui.allocate_exact_size(vec2(src.w * k, src.h * k), Sense::click_and_drag());
        ui.painter().rect_filled(rect, 2.0, Color32::from_gray(45));
        widgets::paint_sprite(ui, cx.tex, cx.assets, &sprite, rect);
        let g = rect.min + vec2(h.grip[0] * rect.width(), h.grip[1] * rect.height());
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                h.grip = [((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0), ((p.y - rect.min.y) / rect.height()).clamp(0.0, 1.0)];
            }
        } else if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos().filter(|p| (*p - g).length() > 4.0) {
                h.points = (p.y - g.y).atan2(p.x - g.x).to_degrees().round();
            }
        }
        let (sn, cs) = h.points.to_radians().sin_cos();
        let orange = Color32::from_rgb(255, 160, 40);
        ui.painter().arrow(g, vec2(cs, sn) * 34.0, egui::Stroke::new(2.0_f32, orange));
        ui.painter().circle(g, 4.0, orange, egui::Stroke::new(1.0_f32, Color32::BLACK));
        resp.on_hover_text("Click: where the hand holds it. Drag from there: its orientation mark");
        ui.weak("3D characters hold it at their hand bone: see it in Database › 3D models (the preview) or in a play test.");
    });
}

/// A character's drawn portrait and its mood portraits.
fn portrait_ui(ui: &mut egui::Ui, d: &mut CharacterDef, cx: &mut Cx) {
    ui.separator();
    ui.heading("Portrait");
    ui.weak("A drawn portrait for dialogs and the companions' HUD rows (any frame size, e.g. 64 x 64). None: the head and shoulders of its look.");
    widgets::sprite_field(ui, "chr_portrait", &mut d.portrait, cx.tex, cx.assets, cx.db, false);
    ui.horizontal(|ui| {
        if !d.portrait.is_empty() && ui.button("Clear (automatic)").clicked() {
            d.portrait = SpriteRef::default();
        }
    });
    art_buttons(ui, cx, "portrait", (1, 1));
    egui::CollapsingHeader::new(format!("Moods ({})", d.moods.len())).id_salt("chr_moods").show(ui, |ui| {
        ui.weak("Portraits for moods, shown while a dialog line sets one: say(\"Elder Rowan\", \"...\", #{ mood: \"worried\" }) or the Set mood action.");
        let mut list: Vec<(String, SpriteRef)> = d.moods.clone().into_iter().collect();
        let mut remove = None;
        for (k, (name, sprite)) in list.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("Mood");
                ui.add(egui::TextEdit::singleline(name).desired_width(120.0));
                if ui.button("Remove").clicked() {
                    remove = Some(k);
                }
            });
            widgets::sprite_field(ui, &format!("chr_mood_{k}"), sprite, cx.tex, cx.assets, cx.db, false);
        }
        if let Some(k) = remove {
            list.remove(k);
        }
        if ui.button("+ Mood").clicked() {
            let name = (1..).map(|n| if n == 1 { "mood".to_string() } else { format!("mood{n}") }).find(|n| !list.iter().any(|(m, _)| m == n)).unwrap_or_default();
            list.push((name, d.portrait.clone()));
        }
        d.moods = list.into_iter().collect();
    });
}

/// A seed's crop: its growth stages and the harvest.
fn crop_ui(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    use rustpg2d_engine::defs::{Crop, CropStage};
    ui.heading("Seed");
    let mut on = d.crop.is_some();
    if ui.checkbox(&mut on, "A seed: grows a crop on a farm plot").changed() {
        d.crop = on.then(|| Crop { stages: vec![CropStage { sprite: SpriteRef::default(), hours: 12.0 }, CropStage { sprite: SpriteRef::default(), hours: 0.0 }], harvest: Vec::new() });
    }
    let Some(c) = &mut d.crop else { return };
    ui.weak(format!("Stages one after another (in-game hours each); the last one is the ripe crop. Ripe after {:.0} hours ({:.1} days), faster in rain.", c.grow_hours(), c.grow_hours() / 24.0));
    let n = c.stages.len();
    let mut remove = None;
    for (k, s) in c.stages.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(if k + 1 == n { "Ripe".to_string() } else { format!("Stage {}", k + 1) });
            if k + 1 < n {
                ui.add(egui::DragValue::new(&mut s.hours).range(0.1..=10_000.0).speed(0.5).suffix(" h"));
            }
            if n > 1 && ui.small_button("x").clicked() {
                remove = Some(k);
            }
        });
        widgets::sprite_field(ui, &format!("crop_stage{k}"), &mut s.sprite, cx.tex, cx.assets, cx.db, true);
    }
    if let Some(k) = remove {
        c.stages.remove(k);
    }
    if ui.small_button("+ Stage").on_hover_text("Adds a growing stage before the ripe one").clicked() {
        let at = c.stages.len().saturating_sub(1);
        let sprite = c.stages.get(at.saturating_sub(1)).map(|s| s.sprite.clone()).unwrap_or_default();
        c.stages.insert(at, CropStage { sprite, hours: 12.0 });
    }
    ui.label("Harvest");
    widgets::loot_ui(ui, "crop_harvest", &mut c.harvest, cx.refs, cx.tex, cx.assets, cx.db);
}

/// Gathering (hits with a tool give items) and regrowing (a state changes back after a while).
fn gather_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    use rustpg2d_engine::defs::{Gather, Regrow};
    let states = d.state_names();
    ui.heading("Gathering");
    let mut on = d.gather.is_some();
    if ui.checkbox(&mut on, "Gathered with a tool (ore veins, trees, bushes)").changed() {
        d.gather = on.then(|| Gather { tool: "pickaxe".into(), hits: 3, loot: Vec::new(), then_state: states.get(1).cloned().unwrap_or_default() });
    }
    if let Some(g) = &mut d.gather {
        ui.weak("E or a melee swing at it is a hit while the player has the tool (equipped, held or carried); after the hits the loot rolls and it switches to the 'used up' state (where it can't be gathered).");
        egui::Grid::new("prop_gather").num_columns(2).show(ui, |ui| {
            ui.label("Tool kind");
            let kinds: Vec<String> = cx.db.items.iter().map(|i| i.tool.clone()).filter(|t| !t.is_empty()).collect();
            ui.horizontal(|ui| {
                widgets::suggest(ui, "gather_tool", &mut g.tool, &kinds, 140.0);
                ui.weak("(empty = bare hands)");
            });
            ui.end_row();
            ui.label("Hits");
            ui.add(egui::DragValue::new(&mut g.hits).range(1..=50)).on_hover_text("Fewer with a skill whose effect is Gathering for this tool");
            ui.end_row();
            ui.label("Used up state");
            egui::ComboBox::from_id_salt("gather_then").selected_text(if g.then_state.is_empty() { "(stays)" } else { g.then_state.as_str() }).show_ui(ui, |ui| {
                ui.selectable_value(&mut g.then_state, String::new(), "(stays: gathered again at once)");
                for s in &states {
                    ui.selectable_value(&mut g.then_state, s.clone(), s);
                }
            });
            ui.end_row();
        });
        ui.label("Gives");
        widgets::loot_ui(ui, "gather_loot", &mut g.loot, cx.refs, cx.tex, cx.assets, cx.db);
    }
    ui.heading("Farm plot");
    ui.checkbox(&mut d.plot, "Seeds are planted here (soil, a planter)").on_hover_text("E on it plants the first seed the player carries (an item with a Crop), shows how long the crop still needs, or harvests it when ripe. Crops grow over in-game hours, faster in rain.");
    ui.heading("Regrows");
    let mut on = d.regrow.is_some();
    if ui.checkbox(&mut on, "A state changes back after some in-game time").changed() {
        d.regrow = on.then(|| Regrow { from: d.gather.as_ref().map(|g| g.then_state.clone()).filter(|s| !s.is_empty()).or_else(|| states.get(1).cloned()).unwrap_or_default(), to: states[0].clone(), hours: 72.0 });
    }
    if let Some(r) = &mut d.regrow {
        egui::Grid::new("prop_regrow").num_columns(2).show(ui, |ui| {
            ui.label("From state");
            egui::ComboBox::from_id_salt("regrow_from").selected_text(r.from.as_str()).show_ui(ui, |ui| {
                for s in &states {
                    ui.selectable_value(&mut r.from, s.clone(), s);
                }
            });
            ui.end_row();
            ui.label("Into state");
            egui::ComboBox::from_id_salt("regrow_to").selected_text(r.to.as_str()).show_ui(ui, |ui| {
                for s in &states {
                    ui.selectable_value(&mut r.to, s.clone(), s);
                }
            });
            ui.end_row();
            ui.label("After");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut r.hours).range(0.1..=10_000.0).speed(1.0).suffix(" h"));
                ui.weak(format!("in-game hours ({:.1} days); also while the player is elsewhere", r.hours / 24.0));
            });
            ui.end_row();
        });
    }
}

fn prop_form(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    ui.heading("Prop");
    rename_row(ui, RefKind::Prop, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    hitbox(ui, &mut d.hitbox);
    ui.checkbox(&mut d.low, "Low (shots fly over it)").on_hover_text(
        "A low prop (a barrel, a chair, a fence) stops walking while it is solid, but arrows, bolts and the sight of enemies pass over it. Solid props that aren't low stop shots too (trees, statues, stalls).",
    );
    ui.separator();
    prop_states_ui(ui, d, cx);
    ui.separator();
    carry_ui(ui, d, cx);
    ui.separator();
    ui.heading("Light");
    ui.weak("Light per state, e.g. a lamp that is lit in its \"lit\" state and dark in \"off\". Placed props can have their own light (map inspector).");
    let names = d.state_names();
    for (i, name) in names.iter().enumerate() {
        let slot = if i == 0 { &mut d.light } else { &mut d.states[i - 1].light };
        ui.push_id(("prop_light", i), |ui| {
            widgets::light_ui(ui, "prop_light", slot, &format!("'{name}' gives light"));
        });
    }
    ui.separator();
    gather_ui(ui, d, cx);
    ui.separator();
    let states = d.state_names();
    ui.heading("Container");
    let mut on = d.container.is_some();
    if ui.checkbox(&mut on, "Lootable container (chest, barrel, ...)").changed() {
        d.container = on.then(ContainerSpec::default);
    }
    if let Some(spec) = &mut d.container {
        ui.weak("Default for every placed instance; instances can customize size and contents in the map inspector.");
        widgets::container_ui(ui, "prop_container", spec, cx.refs, cx.tex, cx.assets, cx.db, &states);
    }
    ui.separator();
    ui.heading("Lock");
    let mut on = d.lock.is_some();
    if ui.checkbox(&mut on, "Lockable (needs a key item)").changed() {
        d.lock = on.then(|| LockSpec { key: cx.refs.items.iter().find(|i| i.contains("key")).cloned().unwrap_or_default(), ..Default::default() });
    }
    let prop = d.clone();
    if let Some(lock) = &mut d.lock {
        ui.weak("Default for every placed instance; instances can use a different key or no lock in the map inspector.");
        widgets::lock_ui(ui, "prop_lock", lock, cx.refs, cx.tex, cx.assets, cx.db, Some(&prop));
    }
    ui.separator();
    use_spot_ui(ui, d, cx);
    ui.separator();
    ui.heading("Default triggers");
    ui.weak("Run for every placed instance, in addition to the instance's own triggers.");
    widgets::triggers_ui(ui, &format!("prop/{}", d.id), &mut d.triggers, cx.refs, rustpg2d_engine::events::TriggerOwner::Prop);
}

/// Furniture: where and how the player uses the prop (sits, lies, stands at it).
fn use_spot_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    use rustpg2d_engine::defs::{Pose, UseSpot};
    ui.heading("Furniture");
    let mut on = d.use_spot.is_some();
    if ui.checkbox(&mut on, "The player can use it (sit, sleep, stand at it)").changed() {
        d.use_spot = on.then(UseSpot::default);
    }
    let sprite = d.sprite.clone();
    let Some(s) = &mut d.use_spot else { return };
    let before = s.pose;
    ui.horizontal(|ui| {
        ui.label("Pose");
        for p in Pose::ALL {
            ui.selectable_value(&mut s.pose, p, p.label());
        }
    });
    if s.pose != before && s.pose == Pose::Lie {
        s.facing = rustpg2d_engine::entity::Facing::Up; // a new bed: the head at the top
    }
    egui::Grid::new("use_spot").num_columns(2).show(ui, |ui| {
        ui.label("Spot (px)");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut s.offset[0]).range(-128.0..=128.0).prefix("x "));
            ui.add(egui::DragValue::new(&mut s.offset[1]).range(-128.0..=128.0).prefix("y "));
        })
        .response
        .on_hover_text("Where the user's feet go, from the prop's feet (bottom center); in a bed, so that the head lies on the pillow");
        ui.end_row();
        // In a bed the facing is the side the head lies on.
        let lie = s.pose == Pose::Lie;
        let name = |f: rustpg2d_engine::entity::Facing| match (lie, f) {
            (true, rustpg2d_engine::entity::Facing::Up) => "Top (upright bed)",
            (true, rustpg2d_engine::entity::Facing::Left) => "Left (sideways bed)",
            (true, rustpg2d_engine::entity::Facing::Right) => "Right (sideways bed)",
            (true, rustpg2d_engine::entity::Facing::Down) => "Bottom",
            (false, f) => f.label(),
        };
        ui.label(if lie { "Head at" } else { "Facing" });
        egui::ComboBox::from_id_salt("use_spot_facing").selected_text(name(s.facing)).width(150.0).show_ui(ui, |ui| {
            for f in rustpg2d_engine::entity::Facing::ALL {
                ui.selectable_value(&mut s.facing, f, name(f));
            }
        });
        ui.end_row();
        if s.pose == Pose::Lie {
            ui.label("Sleep until");
            ui.add(egui::DragValue::new(&mut s.sleep_until).range(0.0..=23.75).speed(0.25).custom_formatter(|h, _| rustpg2d_engine::entity::hour_text(h as f32)));
            ui.end_row();
        }
    });
    // Preview: the prop with the spot (a dot at the feet, a line the way the user looks).
    let size = cx.assets.sprite_size(&sprite, 32.0);
    let k = 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size.x * k + 40.0, size.y * k + 40.0), egui::Sense::hover());
    let r = egui::Rect::from_min_size(rect.min + egui::vec2(20.0, 20.0), egui::vec2(size.x * k, size.y * k));
    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_gray(30));
    widgets::paint_sprite(ui, cx.tex, cx.assets, &sprite, r);
    let feet = egui::pos2(r.center().x, r.bottom());
    let spot = feet + egui::vec2(s.offset[0], s.offset[1]) * k;
    let (dx, dy) = s.facing.dir();
    ui.painter().circle_filled(spot, 4.0, egui::Color32::from_rgb(255, 200, 60));
    ui.painter().line_segment([spot, spot + egui::vec2(dx, dy) * 14.0], egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 200, 60)));
    if s.pose == Pose::Lie {
        ui.label("Cover (drawn over the sleeper, e.g. the blanket; same size as the bed)");
        widgets::sprite_field(ui, "use_spot_cover", &mut s.cover, cx.tex, cx.assets, cx.db, true);
    } else if s.pose == Pose::Sit {
        ui.label("Cover (drawn over the sitter, e.g. the backrest of a chair facing away; same size as the chair; optional)");
        widgets::sprite_field(ui, "use_spot_cover", &mut s.cover, cx.tex, cx.assets, cx.db, true);
    }
    ui.label("Only usable when");
    widgets::conditions_ui(ui, &format!("use_spot/{}", d.id), &mut s.conditions, cx.refs);
    if !s.conditions.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Otherwise");
            ui.add(egui::TextEdit::singleline(&mut s.refuse_text).hint_text("You can't use that now.").desired_width(260.0));
        });
    }
    ui.weak(match s.pose {
        Pose::Sit => "Interacting sits down (when the prop's own triggers did nothing), facing the chosen way (one prop per direction, e.g. chairs around a table); HP and mana come back faster while sitting. Moving, attacking or E stands up.",
        Pose::Lie => "Interacting asks \"Sleep until …?\": the night passes (HP and mana restored, effects worn off, enemies respawned, scheduled characters moved on), the bed's and the map's 'Woke up' triggers run, the game autosaves. Not with enemies nearby.",
        Pose::Stand => "Interacting steps up to it and faces it (counters, anvils, desks).",
    });
}

/// The prop's states in one place: name, sprite, whether it blocks movement.
fn prop_states_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    ui.heading("States");
    ui.weak("A prop always shows one of its states. Placed props start in the default state (or the one chosen in the map inspector); a lock, a container, the 'Set prop state' action or set_state(target, \"name\") in scripts switch to another.");
    // All states as one list (index 0 = the default state).
    let mut all: Vec<PropState> = std::iter::once(PropState { name: d.default_state.clone(), sprite: d.sprite.clone(), solid: d.solid, light: None }).chain(d.states.iter().cloned()).collect();
    let used: Vec<String> = [
        d.lock.as_ref().and_then(|l| match &l.on_unlock {
            OnUnlock::ToState { state } => Some(state.clone()),
            _ => None,
        }),
        d.container.as_ref().map(|c| c.open_state.clone()).filter(|s| !s.is_empty()),
    ]
    .into_iter()
    .flatten()
    .collect();
    if *cx.state_sel >= all.len() {
        *cx.state_sel = 0;
    }
    enum Op {
        Up(usize),
        Down(usize),
        Delete(usize),
        MakeDefault(usize),
        Rename(usize, String),
    }
    let mut op = None;
    let n = all.len();
    for (i, st) in all.iter_mut().enumerate() {
        let selected = *cx.state_sel == i;
        let frame = egui::Frame::group(ui.style()).fill(if selected { Color32::from_rgb(40, 52, 74) } else { Color32::TRANSPARENT });
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if widgets::sprite_thumb(ui, cx.tex, cx.assets, &st.sprite, 44.0, selected).on_hover_text("Select to change its sprite").clicked() {
                    *cx.state_sel = i;
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        match cx.state_rename {
                            Some((k, buf)) if *k == i => {
                                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(110.0));
                                let clean: String = buf.trim().to_lowercase().replace(' ', "_");
                                let taken = all_names_except(d, i).contains(&clean);
                                let ok = !clean.is_empty() && !taken;
                                if (ui.add_enabled(ok, egui::Button::new("OK")).clicked() || (ok && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) && clean != st.name {
                                    op = Some(Op::Rename(i, clean));
                                } else if ui.button("Cancel").clicked() {
                                    *cx.state_rename = None;
                                }
                                if taken {
                                    ui.colored_label(Color32::LIGHT_RED, "name taken");
                                }
                            }
                            _ => {
                                if ui.selectable_label(selected, egui::RichText::new(&st.name).strong()).clicked() {
                                    *cx.state_sel = i;
                                }
                                if ui.small_button("\u{E3B4}").on_hover_text("Rename (placed props using it follow)").clicked() {
                                    *cx.state_rename = Some((i, st.name.clone()));
                                }
                            }
                        }
                        if i == 0 {
                            ui.weak("default");
                        }
                        if used.contains(&st.name) {
                            ui.weak("· used by the lock / container");
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut st.solid, "blocks movement");
                        if i > 0 {
                            if ui.small_button("Make default").on_hover_text("Placed props without a chosen state start in this one").clicked() {
                                op = Some(Op::MakeDefault(i));
                            }
                            if i > 1 && ui.small_button("\u{E13C}").clicked() {
                                op = Some(Op::Up(i));
                            }
                            if i + 1 < n && ui.small_button("\u{E136}").clicked() {
                                op = Some(Op::Down(i));
                            }
                            let in_use = used.contains(&st.name);
                            if ui.add_enabled(!in_use, egui::Button::new("\u{E4F6}").small()).on_hover_text(if in_use { "Used by the lock or container" } else { "Delete this state" }).clicked() {
                                op = Some(Op::Delete(i));
                            }
                        }
                    });
                });
            });
        });
    }
    let mut renamed = None;
    match op {
        Some(Op::Up(i)) => all.swap(i, i - 1),
        Some(Op::Down(i)) => all.swap(i, i + 1),
        Some(Op::Delete(i)) => {
            all.remove(i);
            *cx.state_sel = 0;
        }
        Some(Op::MakeDefault(i)) => {
            all.swap(0, i);
            *cx.state_sel = 0;
        }
        Some(Op::Rename(i, new)) => {
            renamed = Some((all[i].name.clone(), new.clone()));
            all[i].name = new;
            *cx.state_rename = None;
        }
        None => {}
    }
    ui.horizontal(|ui| {
        if ui.button("+ Add state").on_hover_text("A new state showing the next frame of the sheet; pick its sprite below").clicked() {
            let last = all.last().unwrap();
            let sprite = SpriteRef { frame: last.sprite.frame + last.sprite.cells().0, ..last.sprite.clone() };
            let names: Vec<String> = all.iter().map(|s| s.name.clone()).collect();
            let name = (2..).map(|k| format!("state_{k}")).find(|c| !names.contains(c)).unwrap();
            all.push(PropState { name: name.clone(), sprite, solid: all[0].solid, light: None });
            *cx.state_sel = all.len() - 1;
            *cx.state_rename = Some((all.len() - 1, name));
        }
        ui.weak("e.g. closed / open / destroyed, dormant / glowing, on / off");
    });
    // The selected state's sprite.
    let sel = (*cx.state_sel).min(all.len() - 1);
    ui.add_space(4.0);
    ui.label(egui::RichText::new(format!("Sprite of '{}'", all[sel].name)).strong());
    widgets::sprite_field(ui, &format!("prop_state_{sel}"), &mut all[sel].sprite, cx.tex, cx.assets, cx.db, true);
    art_buttons(ui, cx, "frame for this state", all[sel].sprite.cells());
    // Write back.
    let first = all.remove(0);
    d.default_state = first.name;
    d.sprite = first.sprite;
    d.solid = first.solid;
    d.states = all;
    if let Some((old, new)) = renamed {
        // References inside this definition follow immediately; maps afterwards.
        if let Some(l) = &mut d.lock {
            if let OnUnlock::ToState { state } = &mut l.on_unlock {
                if *state == old {
                    *state = new.clone();
                }
            }
        }
        if let Some(c) = &mut d.container {
            if c.open_state == old {
                c.open_state = new.clone();
            }
        }
        rename_state_in_triggers(&mut d.triggers, &old, &new);
        *cx.state_renamed = Some((d.id.clone(), old, new));
    }
}

fn all_names_except(d: &PropDef, i: usize) -> Vec<String> {
    d.state_names().into_iter().enumerate().filter(|(k, _)| *k != i).map(|(_, n)| n).collect()
}

/// Renames a prop state in "Set prop state" actions and "Prop state" conditions aimed at "self".
pub fn rename_state_in_triggers(ts: &mut [rustpg2d_engine::events::Trigger], old: &str, new: &str) {
    use rustpg2d_engine::events::{Action, Cond};
    fn actions(acts: &mut [Action], old: &str, new: &str) {
        rustpg2d_engine::events::walk_actions_mut(acts, &mut |a| match a {
            Action::SetState { target, state } if (target == "self" || target.is_empty()) && state == old => *state = new.to_string(),
            Action::If { conditions, .. } => conds(conditions, old, new),
            _ => {}
        });
    }
    fn conds(cs: &mut [rustpg2d_engine::events::Condition], old: &str, new: &str) {
        for c in cs {
            if let Cond::PropState { target, state } = &mut c.cond {
                if (target == "self" || target.is_empty()) && state == old {
                    *state = new.to_string();
                }
            }
        }
    }
    for t in ts {
        conds(&mut t.conditions, old, new);
        actions(&mut t.actions, old, new);
    }
}

fn character_form(ui: &mut egui::Ui, d: &mut CharacterDef, cx: &mut Cx) {
    ui.heading("Character");
    rename_row(ui, RefKind::Character, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description").on_hover_text("What the bestiary (journal, J) says about it once the player has defeated one. Placeholders like {player} work.");
    text_box(ui, &mut d.description, 3, "Shown in the bestiary once the player has defeated one");
    portrait_ui(ui, d, cx);
    if !cx.db.races.is_empty() || !d.race.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Race");
            let label = cx.db.race(&d.race).map_or_else(|| if d.race.is_empty() { "(none)".to_string() } else { format!("{} (missing)", d.race) }, |r| r.name.clone());
            egui::ComboBox::from_id_salt("chr_race").selected_text(label).show_ui(ui, |ui| {
                ui.selectable_value(&mut d.race, String::new(), "(none)");
                for r in &cx.db.races {
                    ui.selectable_value(&mut d.race, r.id.clone(), format!("{}  ({})", r.name, r.id));
                }
            })
            .response
            .on_hover_text("What it is (Database \u{E13A} Races): for the 'Race of' condition, scripts and factions. Its size comes from its body (the race that lists it).");
        });
    }
    if !cx.refs.factions.is_empty() || !d.faction.is_empty() {
        let race_faction = cx.db.race(&d.race).map(|r| r.faction.clone()).unwrap_or_default();
        faction_combo(ui, "chr_faction", &mut d.faction, &cx.refs.factions, &race_faction, "Who it sides with (Database \u{E13A} Project \u{E13A} Factions): characters of hostile factions fight each other, and it attacks the player when the player's standing with its faction is -20 or below. Empty = its race's faction.");
    }
    // The look: assembled in the character builder (body, colors, hair, clothes, gear).
    ui.separator();
    ui.heading("Look & equipment");
    ui.allocate_ui(egui::vec2(ui.available_width().min(1100.0), 0.0), |ui| {
        ui.add(egui::Label::new(egui::RichText::new("Its 3D model in the colours of its colour features, its equipment (counts in play; weapons, left-hand items and tools show in its hands) and carried items. An equipped weapon replaces the one under Combat. (The player character: what a new game starts with.)").weak()).wrap());
    });
    {
        let bcx = crate::builder::Cx { db: cx.db, tex: cx.tex, assets: cx.assets };
        let parts = crate::builder::Parts { appearance: &mut d.appearance, equipment: &mut d.equipment, inventory: &mut d.inventory };
        let out = crate::builder::builder_ui(ui, "chr", parts, &bcx);
        if out.template {
            crate::models::request_model(crate::models::ModelTarget::Def(d.id.clone()));
        }
    }
    ui.separator();
    egui::Grid::new("chr").num_columns(2).show(ui, |ui| {
        ui.label("Speed (px/s)");
        ui.add(egui::DragValue::new(&mut d.speed).range(1.0..=500.0));
        ui.end_row();
        ui.label("Behavior");
        ui.horizontal(|ui| {
            let mut kind = match d.behavior {
                Behavior::Static => 0,
                Behavior::Wander { .. } => 1,
                Behavior::Chase { .. } => 2,
            };
            let old = kind;
            ui.radio_value(&mut kind, 0, "Static");
            ui.radio_value(&mut kind, 1, "Wander");
            ui.radio_value(&mut kind, 2, "Chase foes");
            if kind != old {
                d.behavior = match kind {
                    0 => Behavior::Static,
                    1 => Behavior::Wander { radius: 3.0 },
                    _ => Behavior::chase(4.0),
                };
            }
            match &mut d.behavior {
                Behavior::Wander { radius } => {
                    ui.label("radius");
                    ui.add(egui::DragValue::new(radius).range(0.5..=64.0).speed(0.1));
                }
                Behavior::Chase { sight, .. } => {
                    ui.label("sight");
                    ui.add(egui::DragValue::new(sight).range(0.5..=64.0).speed(0.1))
                        .on_hover_text("Tiles. It notices the player this close in a clear line, behind walls and trees only within half of it; after losing them it searches where it saw them last for a few seconds, then wanders near home.");
                }
                Behavior::Static => {}
            }
        });
        ui.end_row();
        if let Behavior::Chase { leash, flee_below, keep_distance, group, alert, .. } = &mut d.behavior {
            ui.label("Leash");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(leash).range(0.0..=256.0).speed(0.1).suffix(" tiles"))
                    .on_hover_text("Gives up and walks back home when this far from home (its start position); harmless on the way. 0 = follows the player anywhere.");
                ui.weak(if *leash > 0.0 { "from home" } else { "(no leash)" });
            });
            ui.end_row();
            ui.label("Flee below");
            ui.horizontal(|ui| {
                let mut pct = (*flee_below * 100.0).round() as i32;
                if ui
                    .add(egui::DragValue::new(&mut pct).range(0..=100).suffix(" % HP"))
                    .on_hover_text("Runs away from the player when its HP is below this share of its max HP (doesn't attack meanwhile). 0 = fights to the end.")
                    .changed()
                {
                    *flee_below = pct as f32 / 100.0;
                }
                ui.weak(if *flee_below > 0.0 { "runs away" } else { "(never flees)" });
            });
            ui.end_row();
            ui.label("Keep distance");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(keep_distance).range(0.0..=64.0).speed(0.1).suffix(" tiles"))
                    .on_hover_text("Shooters (ranged weapon): back off when the player comes closer than this. 0 = they only stop coming closer once the player is well within range.");
                ui.weak("(shooters)");
            });
            ui.end_row();
            ui.label("Group");
            ui.checkbox(group, "Fights as a group").on_hover_text(
                "At most two group fighters close in on the player (or a companion) at a time; the others circle at a distance and step in when one falls back. They spread out instead of stacking up, and the badly hurt run to their allies, catch their breath and come back.",
            );
            ui.end_row();
            ui.label("Alert");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(alert).range(0.0..=64.0).speed(0.1).suffix(" tiles"))
                    .on_hover_text("On noticing the player it alerts other characters of the same kind within this distance: they come too, even without seeing the player. 0 = never.");
                ui.weak(if *alert > 0.0 { "same kind" } else { "(alerts nobody)" });
            });
            ui.end_row();
        }
    });
    hitbox(ui, &mut d.hitbox);
    ui.separator();
    ui.heading("Combat");
    egui::Grid::new("chr_combat").num_columns(2).show(ui, |ui| {
        ui.label("Max HP");
        ui.add(egui::DragValue::new(&mut d.max_hp).range(0..=999)).on_hover_text("0 = can't be damaged");
        ui.end_row();
        ui.label("Damage");
        ui.add(egui::DragValue::new(&mut d.damage).range(0..=999));
        ui.end_row();
        ui.label("Defense");
        ui.add(egui::DragValue::new(&mut d.defense).range(-99..=99)).on_hover_text("Subtracted from incoming damage (at least 1 always gets through). Player: base defense before armor.");
        ui.end_row();
        ui.label("Hostile");
        ui.checkbox(&mut d.hostile, "attacks the player").on_hover_text(
            "With a melee weapon (its equipment) it winds up and swings it; unarmed and chasing it crouches and lunges; with a ranged weapon it shoots; otherwise it hurts by touch. With a faction it leaves players alone who are friends of that faction (standing 20 or more); without 'Hostile' it attacks when the player's standing is -20 or below.",
        );
        ui.end_row();
        ui.label("Attack wind-up");
        ui.add(egui::DragValue::new(&mut d.windup).range(0.0..=3.0).speed(0.01).suffix(" s")).on_hover_text(
            "How long it winds up before a weapon swing or a lunge: the time the player has to step out of the way. 0 = standard (0.4 s swing, 0.3 s lunge).",
        );
        ui.end_row();
        ui.label("As companion");
        ui.checkbox(&mut d.gear_locked, "can't be given gear")
            .on_hover_text("A companion's window lets the player equip it and hand it items; tick this for animals and creatures that can't wear anything (they still carry items and use potions).");
        ui.end_row();
        ui.label("Magic (companion)");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut d.max_mana).range(0..=999).prefix("mana "))
                .on_hover_text("0 = it can't learn spells. With mana, a companion learns the Heal and Bolt spells of scrolls and spell books the player gives it (or 'Teach spell') and casts them: Heal on whoever is badly hurt, Bolt at its enemy.");
            ui.add(egui::DragValue::new(&mut d.mana_regen).range(0.0..=50.0).speed(0.05).suffix(" /s")).on_hover_text("Mana it regains per second");
        });
        ui.end_row();
        ui.label("Experience");
        ui.add(egui::DragValue::new(&mut d.xp).range(0..=1_000_000).suffix(" XP"))
            .on_hover_text("What the player gains for defeating this character (Project › Experience and levels must be on). 'Defeat all enemies' gives none.");
        ui.end_row();
    });
    ui.weak("1 heart = 2 HP. For the player character, Damage is the bare-handed attack strength. Defeated characters fire 'Defeated' triggers.");
    widgets::optional_effect_ui(
        ui,
        "chr_touch_fx",
        "On touch:",
        "A status effect its touch gives the player along with the contact damage (hostile characters), e.g. a poisonous slime. Without one, a melee weapon it holds gives that weapon's effect.",
        &mut d.touch_effect,
    );
    ui.label("Ranged attack (hostile characters shoot at the player when in range and in sight):");
    weapon_ui(ui, "chr_weapon", &mut d.weapon, cx, true);
    ui.separator();
    ui.heading("Loot");
    if d.max_hp <= 0 {
        ui.weak("This character can't be defeated (Max HP 0), so its loot never drops.");
    } else {
        ui.weak("What the character can drop when defeated. Placed characters and spawners can use their own table instead.");
    }
    widgets::loot_ui(ui, "chr_loot", &mut d.loot, cx.refs, cx.tex, cx.assets, cx.db);
    ui.separator();
    ui.heading("Light");
    widgets::light_ui(ui, "chr_light", &mut d.light, "Carries a light (e.g. a lantern)");
    ui.weak("Placed characters can carry their own light instead (map inspector).");
    ui.separator();
    ui.heading("Merchant");
    let mut on = d.shop.is_some();
    if ui.checkbox(&mut on, "Merchant (trades items with the player)").changed() {
        d.shop = on.then(ShopSpec::default);
    }
    if let Some(spec) = &mut d.shop {
        ui.weak("Default stock for every placed instance; instances can have their own stock and prices in the map inspector.");
        widgets::shop_ui(ui, "chr_shop", spec, cx.refs, cx.tex, cx.assets, cx.db);
    }
    ui.separator();
    ui.heading("Default triggers");
    widgets::triggers_ui(ui, &format!("chr/{}", d.id), &mut d.triggers, cx.refs, rustpg2d_engine::events::TriggerOwner::Character);
}

fn quest_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::quest::QuestDef, _cx: &mut Cx) {
    ui.heading("Quest");
    egui::Grid::new("quest").num_columns(2).show(ui, |ui| {
        ui.label("Id");
        ui.add(egui::TextEdit::singleline(&mut d.id).desired_width(200.0))
            .on_hover_text("Used by 'Quest' actions / conditions and scripts (start_quest(\"id\")). Changing it does not update them.");
        ui.end_row();
        ui.label("Title");
        ui.add(egui::TextEdit::singleline(&mut d.title).desired_width(320.0));
        ui.end_row();
        ui.label("Experience reward");
        ui.add(egui::DragValue::new(&mut d.xp).range(0..=1_000_000).suffix(" XP"))
            .on_hover_text("Given when the quest is completed (Project › Experience and levels must be on).");
        ui.end_row();
    });
    ui.label("Description (shown in the journal)");
    text_box(ui, &mut d.description, 3, "");
    ui.separator();
    ui.heading("Stages");
    ui.weak("One objective per stage. The journal shows the objectives reached so far: earlier ones ticked, the current one open. \
             Triggers move the quest on with the Quest action (start, advance to the next stage, set stage, complete, fail) \
             and check it with the Quest condition; scripts use start_quest / advance_quest / set_quest_stage / complete_quest / fail_quest \
             and quest_stage / quest_active / quest_done. Write {name} to show the value of variable 'name', e.g. \"Defeat slimes ({slimes}/5)\".");
    let n = d.stages.len();
    let mut swap = None;
    let mut remove = None;
    for (k, s) in d.stages.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{k}"));
            ui.add(egui::TextEdit::multiline(s).desired_rows(1).desired_width(460.0));
            if ui.add_enabled(k > 0, egui::Button::new("^").small()).on_hover_text("Move up").clicked() {
                swap = Some(k - 1);
            }
            if ui.add_enabled(k + 1 < n, egui::Button::new("v").small()).on_hover_text("Move down").clicked() {
                swap = Some(k);
            }
            if ui.small_button("\u{E4F6}").on_hover_text("Remove (later stages move up: check your triggers)").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = swap {
        d.stages.swap(k, k + 1);
    }
    if let Some(k) = remove {
        d.stages.remove(k);
    }
    if ui.button("+ Stage").clicked() {
        d.stages.push(String::new());
    }
}

fn tileset_form(ui: &mut egui::Ui, d: &mut TilesetDef, cx: &mut Cx) {
    let tile = cx.assets.tile_size;
    ui.heading("Tileset");
    rename_row(ui, RefKind::Tileset, &d.id, cx.rename);
    ui.horizontal(|ui| {
        ui.label("Image");
        ui.text_edit_singleline(&mut d.image);
        if ui.button("\u{E3B4} Edit pixels").on_hover_text("Open the image in the pixel editor").clicked() {
            *cx.paint = Some((d.image.clone(), (tile as u32, tile as u32), false, 0));
        }
        if ui.button("Browse…").clicked() {
            *cx.browse = Some(Purpose::TilesetImage(d.id.clone()));
        }
    });
    ui.weak(format!("Path relative to the project; files from elsewhere are copied into tilesets/. Tiles are {tile}px squares. After editing the path by hand, use \u{E036} Reload assets."));
    let (Some(t), Some(ts)) = (cx.tex.tilesets.get(&d.id), cx.assets.tilesets.get_mut(&d.id)) else {
        ui.colored_label(Color32::LIGHT_RED, "Not loaded — reload assets.");
        return;
    };

    // ---- animations
    ui.separator();
    ui.heading("Animated tiles");
    ui.weak("Wherever the base tile is placed it cycles through the frames. Pick an animation below, then click tiles in the image to append frames.");
    let mut remove = None;
    let uv_of = |i: u32| {
        let (c, r) = (i % ts.columns, i / ts.columns);
        let (w, h) = (ts.image.width as f32, ts.image.height as f32);
        egui::Rect::from_min_max(pos2(c as f32 * tile / w, r as f32 * tile / h), pos2((c + 1) as f32 * tile / w, (r + 1) as f32 * tile / h))
    };
    let now = macroquad::time::get_time();
    for (k, a) in d.animations.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.radio_value(cx.anim_target, Some(k), "");
            let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            ui.painter().image(t.id(), rect, uv_of(a.frame_at(now)), Color32::WHITE);
            ui.label("tile");
            ui.add(egui::DragValue::new(&mut a.tile).range(0..=ts.count.saturating_sub(1)));
            ui.label("frames");
            let mut text = a.frames.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0)).changed() {
                a.frames = text.split(',').filter_map(|f| f.trim().parse().ok()).filter(|&f| f < ts.count).collect();
            }
            ui.label("fps");
            ui.add(egui::DragValue::new(&mut a.fps).range(0.1..=60.0).speed(0.1));
            if ui.small_button("\u{E4F6}").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        d.animations.remove(k);
        *cx.anim_target = None;
    }
    ui.horizontal(|ui| {
        if ui.button("+ Animation").clicked() {
            d.animations.push(TileAnim { tile: 0, frames: vec![], fps: 4.0 });
            *cx.anim_target = Some(d.animations.len() - 1);
        }
        if cx.anim_target.is_some() && ui.button("Done (back to solidity)").clicked() {
            *cx.anim_target = None;
        }
    });
    ui.ctx().request_repaint();

    // ---- light tiles
    ui.separator();
    ui.heading("Light tiles");
    ui.weak("Tiles that give light wherever they are placed (wall torches, lamps, glowing windows). Lights only show on maps that are dark (Map › Lighting).");
    let mut remove = None;
    for (k, tl) in d.lights.iter_mut().enumerate() {
        ui.push_id(("tile_light", k), |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                ui.painter().image(t.id(), rect, uv_of(tl.tile), Color32::WHITE);
                ui.label(format!("tile {}", tl.tile));
                if ui.small_button(ui::ph::TRASH).on_hover_text("No light for this tile").clicked() {
                    remove = Some(k);
                }
            });
            let mut l = Some(tl.light.clone());
            widgets::light_ui(ui, "tile_light", &mut l, "gives light");
            match l {
                Some(l) => tl.light = l,
                None => remove = Some(k),
            }
        });
    }
    if let Some(k) = remove {
        d.lights.remove(k);
    }
    if ui.selectable_label(*cx.tile_light_pick, ui::t(ui::ph::LIGHTBULB, "Pick light tiles in the grid")).on_hover_text("While on, clicking a tile below adds or removes its light").clicked() {
        *cx.tile_light_pick = !*cx.tile_light_pick;
        *cx.tile_ladder_pick = false;
        *cx.tile_low_pick = false;
        *cx.anim_target = None;
        *cx.terrain_pick = None;
    }
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*cx.tile_ladder_pick, ui::t(ui::ph::LADDER_SIMPLE, "Pick ladder tiles in the grid"))
            .on_hover_text("While on, clicking a tile below marks it as a ladder or not. The player climbs ladders: only up and down, slower, seen from behind (a ladder top can lead somewhere with a portal). Characters keep off them.")
            .clicked()
        {
            *cx.tile_ladder_pick = !*cx.tile_ladder_pick;
            *cx.tile_light_pick = false;
            *cx.tile_low_pick = false;
            *cx.anim_target = None;
            *cx.terrain_pick = None;
        }
        if !d.ladder.is_empty() {
            ui.weak(format!("{} ladder tile(s)", d.ladder.len()));
        }
    });
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*cx.tile_low_pick, ui::t(ui::ph::ARROW_FAT_LINES_DOWN, "Pick low tiles in the grid"))
            .on_hover_text("While on, clicking a tile below marks it as low or not. A low solid tile (a fence, a rock, water, a table) stops walking, but arrows, bolts and the sight of enemies pass over it. Solid tiles that aren't low stop shots too (walls, tree trunks).")
            .clicked()
        {
            *cx.tile_low_pick = !*cx.tile_low_pick;
            *cx.tile_ladder_pick = false;
            *cx.tile_light_pick = false;
            *cx.anim_target = None;
            *cx.terrain_pick = None;
        }
        if !d.low.is_empty() {
            ui.weak(format!("{} low tile(s)", d.low.len()));
        }
    });

    // ---- terrains
    ui.separator();
    let before = *cx.terrain_pick;
    crate::terrain::terrains_form(ui, d, t.id(), ts, tile, cx.terrain_pick);
    if cx.terrain_pick.is_some() && before.is_none() {
        *cx.anim_target = None;
        *cx.tile_light_pick = false;
        *cx.tile_ladder_pick = false;
        *cx.tile_low_pick = false;
    }

    // ---- tile grid
    ui.separator();
    let editing_terrain = cx.terrain_pick.and_then(|p| d.terrains.get(p.terrain).map(|t| (p, t.name.clone())));
    match (*cx.anim_target, *cx.tile_light_pick) {
        _ if *cx.tile_ladder_pick && editing_terrain.is_none() && cx.anim_target.is_none() => ui.label("Click tiles to mark or unmark ladders (green rungs = ladder)."),
        _ if *cx.tile_low_pick && editing_terrain.is_none() && cx.anim_target.is_none() => ui.label("Click tiles to mark or unmark them as low (yellow = solid but low: shots fly over it)."),
        _ if editing_terrain.is_some() => {
            let (p, name) = editing_terrain.clone().unwrap();
            let blob = d.terrains.get(p.terrain).is_some_and(|t| t.is_blob());
            ui.label(if p.block {
                format!("Click the top-left tile of a {} block for terrain {name}.", if blob { "7×7" } else { "4×4" })
            } else if p.slot == crate::terrain::BASE_SLOT {
                format!("Click the tile that erasing {name} leaves.")
            } else {
                format!("Click the tile for slot {} of terrain {name} (cyan outline = its tiles).", p.slot + 1)
            })
        }
        (Some(k), _) => ui.label(format!("Click tiles to append frames to animation #{} (the first click also sets its base tile).", k + 1)),
        (None, true) => ui.label("Click tiles to add or remove their light (yellow bulb = gives light)."),
        (None, false) => ui.label("Click tiles to toggle solidity (red = blocks movement)."),
    };
    let scale = 3.0;
    let size = vec2(ts.image.width as f32 * scale, ts.image.height as f32 * scale);
    let cell = tile * scale;
    // Only sideways (very wide images): vertically the grid is part of the form, so the form's
    // scrollbar reaches its last row. (A scroll area of its own got just the space left below
    // the sections above, often a 64 px strip.)
    egui::ScrollArea::horizontal().id_salt("ts_solid").show(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, Color32::from_gray(30));
        p.image(t.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let animated: Vec<u32> = d.animations.iter().map(|a| a.tile).collect();
        for i in 0..ts.count {
            let r = egui::Rect::from_min_size(rect.min + vec2((i % ts.columns) as f32 * cell, (i / ts.columns) as f32 * cell), vec2(cell, cell));
            p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_white_alpha(20)), StrokeKind::Inside);
            let low = d.low.contains(&i);
            if d.solid.contains(&i) {
                let c = if low { Color32::from_rgba_unmultiplied(255, 210, 0, 80) } else { Color32::from_rgba_unmultiplied(255, 0, 0, 70) };
                p.rect_filled(r, 0.0, c);
            }
            if low {
                p.text(r.right_bottom() + vec2(-2.0, -2.0), egui::Align2::RIGHT_BOTTOM, ui::ph::ARROW_FAT_LINES_DOWN, egui::FontId::proportional(14.0), Color32::from_rgb(255, 220, 60));
            }
            if animated.contains(&i) {
                p.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::LIGHT_BLUE), StrokeKind::Inside);
            }
            if editing_terrain.as_ref().is_some_and(|(p, _)| d.terrains[p.terrain].contains(i) || d.terrains[p.terrain].base == Some(i)) {
                p.rect_stroke(r.shrink(2.0), 0.0, Stroke::new(2.0_f32, Color32::from_rgb(90, 220, 255)), StrokeKind::Inside);
            }
            if d.ladder.contains(&i) {
                p.text(r.left_bottom() + vec2(2.0, -2.0), egui::Align2::LEFT_BOTTOM, ui::ph::LADDER_SIMPLE, egui::FontId::proportional(16.0), Color32::from_rgb(120, 230, 120));
            }
            if d.lights.iter().any(|l| l.tile == i) {
                p.text(r.right_top() + vec2(-3.0, 2.0), egui::Align2::RIGHT_TOP, ui::ph::LIGHTBULB, egui::FontId::proportional(16.0), Color32::YELLOW);
            }
            p.text(r.left_top() + vec2(2.0, 1.0), egui::Align2::LEFT_TOP, i.to_string(), egui::FontId::monospace(9.0), Color32::from_white_alpha(120));
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let rel = pos - rect.min;
                let i = (rel.y / cell) as u32 * ts.columns + (rel.x / cell) as u32;
                let terrain_click = i < ts.count && cx.terrain_pick.as_mut().is_some_and(|p| crate::terrain::grid_click(d, ts, p, i));
                if i < ts.count && !terrain_click {
                    match cx.anim_target.and_then(|k| d.animations.get_mut(k)) {
                        Some(a) => {
                            if a.frames.is_empty() {
                                a.tile = i;
                            }
                            a.frames.push(i);
                        }
                        None if *cx.tile_ladder_pick => match d.ladder.iter().position(|&l| l == i) {
                            Some(k) => {
                                d.ladder.remove(k);
                            }
                            None => {
                                d.ladder.push(i);
                                d.ladder.sort();
                            }
                        },
                        None if *cx.tile_low_pick => {
                            match d.low.iter().position(|&l| l == i) {
                                Some(k) => {
                                    d.low.remove(k);
                                }
                                None => {
                                    d.low.push(i);
                                    d.low.sort();
                                }
                            }
                            if let Some(v) = ts.low.get_mut(i as usize) {
                                *v = d.low.contains(&i);
                            }
                        }
                        None if *cx.tile_light_pick => match d.lights.iter().position(|l| l.tile == i) {
                            Some(k) => {
                                d.lights.remove(k);
                            }
                            None => d.lights.push(rustpg2d_engine::defs::TileLight { tile: i, light: rustpg2d_engine::defs::LightSpec::torch(4.0) }),
                        },
                        None => {
                            if let Some(k) = d.solid.iter().position(|&s| s == i) {
                                d.solid.remove(k);
                            } else {
                                d.solid.push(i);
                                d.solid.sort();
                            }
                            // Keep runtime flags in sync without a full reload.
                            if let Some(v) = ts.solid.get_mut(i as usize) {
                                *v = d.solid.contains(&i);
                            }
                        }
                    }
                }
            }
        }
    });
    if ts.anims != d.animations {
        ts.set_animations(&d.animations);
    }
}

/// Colour presets for colour features: (name, colours).
pub const COLOR_PRESETS: [(&str, &[[u8; 3]]); 3] = [("Skin tones", &widgets::SKIN_PRESETS), ("Hair", &widgets::HAIR_PRESETS), ("Cloth", &widgets::CLOTH_PRESETS)];

/// The colours of an image with their pixel counts, most used first.
pub(crate) type ColorCounts = Vec<([u8; 3], u32)>;

thread_local! {
    /// The colours of sheet images: (sheet, image size, sample hash) -> list.
    static IMAGE_COLORS: std::cell::RefCell<std::collections::HashMap<(String, usize, u64), ColorCounts>> = Default::default();
}

/// The opaque colours of an image (RGBA), most used first.
pub fn image_colors(sheet: &str, rgba: &[u8]) -> ColorCounts {
    let sample = rgba.iter().step_by(997).fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3));
    let key = (sheet.to_string(), rgba.len(), sample);
    IMAGE_COLORS.with(|m| {
        m.borrow_mut()
            .entry(key)
            .or_insert_with(|| {
                let mut count: std::collections::HashMap<[u8; 3], u32> = std::collections::HashMap::new();
                for px in rgba.as_chunks::<4>().0.iter().filter(|p| p[3] > 0) {
                    *count.entry([px[0], px[1], px[2]]).or_default() += 1;
                }
                let mut v: ColorCounts = count.into_iter().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                v
            })
            .clone()
    })
}

/// Colours of an image, most used first (cached per `key`).
pub(crate) fn image_colors_of(key: &str, rgba: &[u8]) -> ColorCounts {
    image_colors(key, rgba)
}

/// The colour features of a figure or a model: pick each part's shades from `image` (its
/// colours, most used first) and the colours it offers. Returns the selected feature.
pub(crate) fn features_editor(ui: &mut egui::Ui, key: &str, features: &mut Vec<rustpg2d_engine::defs::ColorFeature>, image: &ColorCounts) -> usize {
    use rustpg2d_engine::defs::ColorFeature;
    let d = Features { id: key, features };
    let sel_id = egui::Id::new(("feature_sel", &d.id));
    let mut sel: usize = ui.data_mut(|m| m.get_temp(sel_id).unwrap_or(0));
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (k, f) in d.features.iter().enumerate() {
            if ui.selectable_label(sel == k, if f.name.is_empty() { "(unnamed)" } else { f.name.as_str() }).clicked() {
                sel = k;
            }
        }
        for name in ["skin", "hair", "clothing", "eyes"] {
            if !d.features.iter().any(|f| f.name == name) && ui.button(ui::t(ui::ph::PLUS, name)).on_hover_text("Add this colour feature").clicked() {
                let colors = match name {
                    "skin" => widgets::SKIN_PRESETS[..7].to_vec(),
                    "hair" => widgets::HAIR_PRESETS.to_vec(),
                    "clothing" => widgets::CLOTH_PRESETS.to_vec(),
                    _ => Vec::new(),
                };
                d.features.push(ColorFeature { name: name.into(), shades: Vec::new(), colors });
                sel = d.features.len() - 1;
            }
        }
        if ui.button(ui::t(ui::ph::PLUS, "Other")).clicked() {
            d.features.push(ColorFeature { name: format!("part_{}", d.features.len() + 1), shades: Vec::new(), colors: Vec::new() });
            sel = d.features.len() - 1;
        }
    });
    sel = sel.min(d.features.len().saturating_sub(1));
    if let Some(f) = d.features.get_mut(sel) {
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(120.0)).on_hover_text("What characters, scripts and the creator call it (\"skin\", \"hair\"…)");
            if ui.small_button(ui::t(ui::ph::TRASH, "Remove")).clicked() {
                remove = Some(sel);
            }
        });
        ui.label(format!("Its shades ({}): click a colour of the image below to add or remove it", f.shades.len()));
        ui.horizontal_wrapped(|ui| {
            let mut drop = None;
            for (k, c) in f.shades.iter().enumerate() {
                if crate::builder::swatch(ui, *c, true).on_hover_text("Click to remove").clicked() {
                    drop = Some(k);
                }
            }
            if let Some(k) = drop {
                f.shades.remove(k);
            }
        });
        ui.label("Colours offered (the character creator's swatches; click one to remove it)");
        ui.horizontal_wrapped(|ui| {
            let mut drop = None;
            for (k, c) in f.colors.iter().enumerate() {
                if crate::builder::swatch(ui, *c, false).clicked() {
                    drop = Some(k);
                }
            }
            if let Some(k) = drop {
                f.colors.remove(k);
            }
            let id = egui::Id::new(("feature_new_color", sel));
            let mut c: [u8; 3] = ui.data_mut(|m| m.get_temp(id).unwrap_or([200, 160, 120]));
            ui.color_edit_button_srgb(&mut c);
            ui.data_mut(|m| m.insert_temp(id, c));
            if ui.small_button(ui::t(ui::ph::PLUS, "Add")).clicked() && !f.colors.contains(&c) {
                f.colors.push(c);
            }
            for (name, list) in COLOR_PRESETS {
                if ui.small_button(name).on_hover_text("Add these colours").clicked() {
                    for c in list {
                        if !f.colors.contains(c) {
                            f.colors.push(*c);
                        }
                    }
                }
            }
        });
    }
    if let Some(k) = remove {
        d.features.remove(k);
        sel = sel.saturating_sub(1);
    }
    // Shades in two features can't tell which part they belong to.
    let mut shared: Vec<[u8; 3]> = Vec::new();
    for (i, a) in d.features.iter().enumerate() {
        for b in &d.features[i + 1..] {
            shared.extend(a.shades.iter().filter(|s| b.shades.contains(s)));
        }
    }
    if !shared.is_empty() {
        ui.colored_label(egui::Color32::YELLOW, format!("{} shade(s) belong to two features: the first feature takes them.", shared.len()));
    }
    if let Some(f) = d.features.get_mut(sel) {
        ui.label(format!("Colours of the image ({}; most used first). Framed: in \"{}\"; dimmed: in another feature.", image.len(), f.name));
        let others: Vec<[u8; 3]> = Vec::new();
        let _ = others;
        egui::ScrollArea::vertical().id_salt(("image_colors", &d.id)).max_height(120.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                for (c, n) in image {
                    let on = f.shades.contains(c);
                    let resp = crate::builder::swatch(ui, *c, on).on_hover_text(format!("{n} pixels"));
                    if resp.clicked() {
                        if on {
                            f.shades.retain(|s| s != c);
                        } else {
                            f.shades.push(*c);
                        }
                    }
                }
            });
        });
    }
    ui.data_mut(|m| m.insert_temp(sel_id, sel));
    sel
}

/// The colour features being edited and whose they are (`features_editor`).
struct Features<'a> {
    id: &'a str,
    features: &'a mut Vec<rustpg2d_engine::defs::ColorFeature>,
}

fn sheet_form(ui: &mut egui::Ui, d: &mut SheetDef, cx: &mut Cx) {
    ui.heading("Sprite sheet");
    rename_row(ui, RefKind::Sheet, &d.id, cx.rename);
    egui::Grid::new("sheet").num_columns(2).show(ui, |ui| {
        ui.label("Kind");
        egui::ComboBox::from_id_salt("sheet_kind")
            .selected_text(d.kind.label())
            .show_ui(ui, |ui| {
                for k in rustpg2d_engine::defs::SheetKind::ALL {
                    ui.selectable_value(&mut d.kind, k, k.label());
                }
            })
            .response
            .on_hover_text("Emotions: icons over heads. Sprite: anything else (characters are 3D models: Database › 3D models).");
        ui.end_row();
        ui.label("Image");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut d.image);
            if ui.button("\u{E3B4} Edit pixels").on_hover_text("Open the image in the pixel editor").clicked() {
                *cx.paint = Some((d.image.clone(), (d.frame_w, d.frame_h), false, 0));
            }
            if ui.button("Browse…").clicked() {
                *cx.browse = Some(Purpose::SheetImage(d.id.clone()));
            }
        });
        ui.end_row();
        ui.label("Frame size");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut d.frame_w).range(1..=512));
            ui.add(egui::DragValue::new(&mut d.frame_h).range(1..=512));
            ui.weak("image pixels");
        });
        ui.end_row();
        ui.label("Scale");
        ui.horizontal(|ui| {
            for k in 1..=4u32 {
                ui.radio_value(&mut d.scale, k, format!("{k}\u{00D7}"));
            }
        })
        .response
        .on_hover_text("Image pixels per world pixel. 2\u{00D7} = high resolution: 64\u{00D7}96 frames fill the place of a 32\u{00D7}48 character, with twice the detail. Hitboxes and positions don't change.");
        ui.end_row();
        let k = d.scale.max(1);
        ui.label("");
        ui.weak(format!("In the world: {}\u{00D7}{} px per frame", d.frame_w as f32 / k as f32, d.frame_h as f32 / k as f32));
        ui.end_row();
    });
    // Named animations (from Aseprite tags, or added here): Effect / Fly actions and
    // effect(…, #{ tag }) play them by name; on an Emotions sheet each one is an emotion.
    if d.kind == rustpg2d_engine::defs::SheetKind::Emotions {
        ui.weak("Every animation of this sheet is an emotion shown over characters' heads: its name is what 'Show emotion', emote(target, \"name\") and say(..., #{ emote: \"name\" }) use.");
    }
    egui::CollapsingHeader::new(format!("Animations ({})", d.tags.len())).id_salt(("sheet_tags", &d.id)).default_open(!d.tags.is_empty()).show(ui, |ui| {
        let mut remove = None;
        for (k, t) in d.tags.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(110.0).hint_text("name"));
                ui.label("frames");
                ui.add(egui::DragValue::new(&mut t.first).range(0..=9999));
                ui.label("to");
                ui.add(egui::DragValue::new(&mut t.last).range(t.first..=9999));
                ui.add(egui::DragValue::new(&mut t.fps).range(0.1..=60.0).speed(0.2).suffix(" fps"));
                if ui.small_button(ui::ph::TRASH).clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            d.tags.remove(k);
        }
        if ui.button(ui::t(ui::ph::PLUS, "Animation")).on_hover_text("A named frame range, offered by the Effect / Fly actions and effect(…, #{ tag: \"name\" })").clicked() {
            d.tags.push(rustpg2d_engine::defs::SheetTag { name: format!("anim_{}", d.tags.len() + 1), first: 0, last: 0, fps: 10.0 });
        }
    });
    // Who uses this sheet.
    let db = cx.db;
    let mut users: Vec<String> = Vec::new();
    users.extend(db.items.iter().filter(|i| i.icon.sheet == d.id).map(|i| format!("item {}", i.name)));
    users.extend(db.props.iter().filter(|p| p.sprite.sheet == d.id || p.states.iter().any(|s| s.sprite.sheet == d.id)).map(|p| format!("prop {}", p.name)));
    users.extend(db.characters.iter().filter(|c| c.appearance.body == d.id).map(|c| format!("character {}", c.name)));
    ui.horizontal_wrapped(|ui| {
        ui.label("Used by:");
        if users.is_empty() {
            ui.weak("nothing yet");
        } else {
            ui.weak(users.join(", "));
        }
    });
    ui.weak("Edit the image below: draw, add frames (Canvas … Resize), copy / mirror frames. Ctrl+S saves it and updates it everywhere.");
}

/// Structure form: id, name, category, a one-line summary and the structure editor. Returns true
/// when "Place on map" was clicked.
fn structure_form(ui: &mut egui::Ui, d: &mut StructureDef, cx: &mut Cx) -> bool {
    let library = LIBRARY_MODE.with(|m| m.get());
    let mut place = false;
    ui.horizontal(|ui| {
        ui.heading(if d.name.is_empty() { d.id.as_str() } else { d.name.as_str() });
        ui.add_space(8.0);
        rename_row(ui, RefKind::Structure, &d.id, cx.rename);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(220.0));
        ui.add_space(12.0);
        ui.label("Category");
        ui.add(egui::TextEdit::singleline(&mut d.category).desired_width(140.0).hint_text("e.g. Buildings"))
            .on_hover_text("Groups structures in the map palette, e.g. Buildings, Walls & fences, Rooms, Outdoor");
        let mut cats: Vec<String> = cx.db.structures.iter().map(|s| s.category.clone()).filter(|c| !c.is_empty()).collect();
        cats.sort();
        cats.dedup();
        egui::ComboBox::from_id_salt("st_form_cat").selected_text("").width(24.0).show_ui(ui, |ui| {
            for c in cats {
                ui.selectable_value(&mut d.category, c.clone(), c);
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        let missing: Vec<&String> = d.tilesets.iter().filter(|t| cx.db.tileset(t).is_none()).collect();
        let tilesets = if d.tilesets.is_empty() { "no tiles yet".to_string() } else { format!("tilesets: {}", d.tilesets.join(", ")) };
        let props = match d.entities.len() {
            0 => String::new(),
            1 => " · 1 prop".to_string(),
            n => format!(" · {n} props"),
        };
        ui.weak(format!("{} × {} tiles · {tilesets}{props}", d.w, d.h));
        if !missing.is_empty() {
            ui.colored_label(Color32::YELLOW, format!("\u{E4E0} not in this project: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        if !library {
            place = ui.button("\u{E31A} Place on map").on_hover_text("Switches to the map with the Structures tool and this structure picked").clicked();
        }
    });
    ui.separator();
    crate::structures::structure_editor(ui, d, cx.st_edit, cx.db, cx.tex, cx.assets);
    if !library {
        ui.weak("You can also capture an area of a map: Structures tool (H), \u{E69C} New from map area…");
    }
    place
}

fn sound_form(ui: &mut egui::Ui, d: &mut SoundDef, cx: &mut Cx) {
    ui.heading("Sound");
    rename_row(ui, RefKind::Sound, &d.id, cx.rename);
    egui::Grid::new("sound").num_columns(2).show(ui, |ui| {
        ui.label("File");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut d.file);
            if ui.button("Browse…").clicked() {
                *cx.browse = Some(Purpose::SoundFile(d.id.clone()));
            }
        });
        ui.end_row();
        ui.label("Volume");
        ui.add(egui::Slider::new(&mut d.volume, 0.0..=1.0));
        ui.end_row();
    });
    ui.weak("WAV or Ogg Vorbis (.ogg), path relative to the project; 44.1 kHz plays best. Use \u{E036} Reload assets after changing the file or volume.");
    ui.horizontal(|ui| {
        if cx.audio.has(&d.id) {
            if ui.button("\u{E3D0} Play").on_hover_text("Plays the file once (a new preview stops the previous one)").clicked() {
                cx.audio.play_preview(&d.id);
            }
            // Stop covers the preview and the map music preview; a short effect may already be over.
            let playing = cx.audio.previewing() == Some(d.id.as_str()) || cx.audio.current_music() == Some(d.id.as_str());
            if ui.add_enabled(playing, egui::Button::new("\u{E46C} Stop")).on_hover_text("Stops this sound").clicked() {
                cx.audio.stop(&d.id);
            }
            if playing {
                ui.weak("playing");
            }
        } else {
            ui.colored_label(Color32::LIGHT_RED, "Not loaded — reload assets (see terminal for errors).");
        }
    });
    ui.separator();
    ui.weak("Use sounds with the 'Play sound' / 'Play music' actions, as map music (map properties), or for built-in game events (Project tab).");
}

fn project_ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let mut area = egui::ScrollArea::vertical().id_salt("project_scroll").auto_shrink(false);
    // Editor script `dbscroll Y` (screenshots of the lower sections).
    if let Some(y) = ed.db.form_scroll.take() {
        area = area.vertical_scroll_offset(y);
    }
    area.show(ui, |ui| project_form(ed, ui));
}

fn project_form(ed: &mut Editor, ui: &mut egui::Ui) {
    let c = &mut ed.project.config;
    ui.heading("Project");
    egui::Grid::new("proj").num_columns(2).show(ui, |ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut c.name);
        ui.end_row();
        ui.label("Tile size");
        ui.weak(format!("{} px (fixed)", rustpg2d_engine::project::TILE_SIZE));
        ui.end_row();
        ui.label("Start map");
        widgets::suggest(ui, "start_map", &mut c.start_map, &ed.refs.maps, 140.0);
        ui.end_row();
        ui.label("Start spawn");
        let spawns = ed.refs.spawns.get(&c.start_map).cloned().unwrap_or_default();
        widgets::suggest(ui, "start_spawn", &mut c.start_spawn, &spawns, 140.0);
        ui.end_row();
        ui.label("Pixel scale");
        ui.add(egui::DragValue::new(&mut c.scale).range(1.0..=8.0).speed(0.1));
        ui.end_row();
        ui.label("Inventory slots");
        ui.add(egui::DragValue::new(&mut c.inventory_size).range(1..=200));
        ui.end_row();
        ui.label("Day length");
        ui.add(egui::DragValue::new(&mut c.day_minutes).range(0.0..=1440.0).speed(0.5).suffix(" min"))
            .on_hover_text("Real minutes one in-game day lasts on outdoor maps (0 = the clock stands still)");
        ui.end_row();
        ui.label("New games start at");
        ui.add(egui::DragValue::new(&mut c.start_hour).range(0.0..=23.99).speed(0.25).suffix(" h"));
        ui.end_row();
        ui.label("Player character");
        let ids: Vec<String> = ed.project.db.characters.iter().map(|d| d.id.clone()).collect();
        widgets::suggest(ui, "player", &mut c.player, &ids, 140.0);
        ui.end_row();
        ui.label("Dialog portraits");
        ui.checkbox(&mut c.dialog_portraits, "show the speaker's portrait in dialogs")
            .on_hover_text("Found from the speaker's name: the player (\"{player}\" or the player's name), the trigger's owner ({self}), a character on the map or a character definition with that name. Narration and signs (no matching character) have none.");
        ui.end_row();
        ui.label("Character pose memory");
        let atlas = egui::DragValue::new(&mut c.model_atlas_mb).range(rustpg2d_engine::render::model::ATLAS_MB).speed(1.0).suffix(" MB");
        if ui.add(atlas).on_hover_text("GPU memory for the rendered poses of 3D characters (pages of 4 MB). When it is full, the poses drawn longest ago are rendered again when needed. Raise it for many different characters on screen at once; lower it for small GPUs. Default 32 MB.").changed() {
            ed.assets.models.set_atlas_mb(c.model_atlas_mb);
        }
        ui.end_row();
        ui.label("Eight facings");
        ui.checkbox(&mut c.eight_facings, "characters walking diagonally turn diagonally")
            .on_hover_text("3D characters are drawn turned halfway between two facings while they walk along a diagonal. Off: they show the four facings only (fewer poses to render).");
        ui.end_row();
        ui.label("Automatic emotions");
        ui.checkbox(&mut c.auto_emotions, "characters show emotions by themselves").on_hover_text(
            "Enemies show 'surprise' when they notice the player, 'confused' when they lose them and 'fear' when they flee; sleepers 'sleepy'; companions 'joy' when they learn a spell. Needs those emotions (tags) in an Emotions sprite sheet.",
        );
        ui.end_row();
        ui.label("Player portrait");
        ui.vertical(|ui| {
            widgets::sprite_field(ui, "player_portrait", &mut c.player_portrait, &ed.tex, &ed.assets, &ed.project.db, false);
            ui.weak("Drawn portrait of the player in dialogs (none: the player character's portrait, else the head and shoulders of the player's look).");
        });
        ui.end_row();
        ui.label("Companions at 0 HP");
        ui.horizontal(|ui| {
            use rustpg2d_engine::project::DownRule;
            egui::ComboBox::from_id_salt("companion_down").selected_text(c.companion_down.label()).show_ui(ui, |ui| {
                for r in [DownRule::StaysDown, DownRule::GetsUp { seconds: 30.0 }, DownRule::GoesHome] {
                    let on = std::mem::discriminant(&c.companion_down) == std::mem::discriminant(&r);
                    if ui.selectable_label(on, r.label()).clicked() && !on {
                        c.companion_down = r;
                    }
                }
            })
            .response
            .on_hover_text("A knocked-down companion lies where it fell, enemies leave it alone; any heal (potion, spell, Heal character) gets it up. Leaving the map asks whether to leave it behind (it then goes home).");
            if let DownRule::GetsUp { seconds } = &mut c.companion_down {
                ui.add(egui::DragValue::new(seconds).range(1.0..=600.0).suffix(" s"));
            }
        });
        ui.end_row();
    });
    ui.weak("Tile size changes need a restart of the editor.");
    ui.separator();
    languages_form(ed, ui);
    let c = &mut ed.project.config;
    ui.separator();
    ui.heading("Skills & magic");
    {
        let s = &mut c.skills;
        ui.checkbox(&mut s.enabled, "Skills (K opens the Skills screen; skill points buy levels)").on_hover_text("Define skills in Database › Skills. Points come from the 'Give skill points' action or give_skill_points() in scripts.");
        ui.add_enabled_ui(s.enabled, |ui| {
            ui.checkbox(&mut s.magic, "Magic (spells learned from items, cast with mana)").on_hover_text("Define spells in Database › Spells; items with 'Teaches spell' are scrolls and books.");
            egui::Grid::new("skills_cfg").num_columns(2).show(ui, |ui| {
                ui.label("Starting skill points");
                ui.add(egui::DragValue::new(&mut s.start_points).range(0..=99));
                ui.end_row();
                if s.magic {
                    ui.label("Max mana");
                    ui.add(egui::DragValue::new(&mut s.max_mana).range(0..=9999)).on_hover_text("Before skill bonuses");
                    ui.end_row();
                    ui.label("Mana regeneration");
                    ui.add(egui::DragValue::new(&mut s.mana_regen).range(0.0..=100.0).speed(0.05).suffix(" / s"));
                    ui.end_row();
                }
            });
        });
    }
    ui.separator();
    ui.heading("Experience and levels");
    {
        let p = &mut c.progression;
        ui.checkbox(&mut p.enabled, "Experience and levels (defeated characters and completed quests give XP)")
            .on_hover_text("Set the XP per character (Characters › Combat) and per quest (Quests › Experience reward). The HUD shows the level and an XP bar.");
        ui.add_enabled_ui(p.enabled, |ui| {
            egui::Grid::new("progression_cfg").num_columns(2).show(ui, |ui| {
                ui.label("XP for level 2");
                ui.add(egui::DragValue::new(&mut p.base_xp).range(1..=1_000_000));
                ui.end_row();
                ui.label("Growth per level");
                ui.add(egui::DragValue::new(&mut p.growth).range(1.0..=10.0).speed(0.01).prefix("x "))
                    .on_hover_text("Each level needs this many times the XP of the one before");
                ui.end_row();
                ui.label("Highest level");
                ui.add(egui::DragValue::new(&mut p.max_level).range(1..=999));
                ui.end_row();
                ui.label("Max HP per level");
                ui.add(egui::DragValue::new(&mut p.hp_per_level).range(0..=99)).on_hover_text("1 heart = 2 HP");
                ui.end_row();
                ui.label("Mana per level");
                ui.add(egui::DragValue::new(&mut p.mana_per_level).range(0..=999)).on_hover_text("With magic");
                ui.end_row();
                ui.label("Skill points per level");
                ui.add(egui::DragValue::new(&mut p.skill_points_per_level).range(0..=99)).on_hover_text("With skills");
                ui.end_row();
            });
            let steps: Vec<String> = (2..=p.max_level.clamp(1, 6)).map(|l| format!("level {l}: {} XP", p.xp_for_level(l))).collect();
            if !steps.is_empty() {
                ui.weak(format!("Total experience needed: {}", steps.join(", ")));
            }
        });
    }
    ui.separator();
    ui.heading("Game sounds");
    ui.weak("Played by built-in events. Leave empty for silence.");
    egui::Grid::new("sfx").num_columns(2).show(ui, |ui| {
        for (label, v) in c.sfx.fields_mut() {
            ui.label(label);
            widgets::suggest(ui, &format!("sfx_{label}"), v, &ed.refs.sounds, 140.0);
            ui.end_row();
        }
    });
    ui.separator();
    ui.separator();
    ui.heading("Character creator");
    let cc = &mut c.creation;
    ui.checkbox(&mut cc.enabled, "Show the character creator when a new game starts")
        .on_hover_text("The player types a name, chooses male / female, a race and model when there is a choice, and a colour for each colour feature of the model. Play-tests from the editor skip it and use the default name.");
    ui.horizontal(|ui| {
        ui.label("Player character's sex");
        egui::ComboBox::from_id_salt("creation_sex").selected_text(cc.sex.label()).show_ui(ui, |ui| {
            for v in rustpg2d_engine::project::SexChoice::ALL {
                ui.selectable_value(&mut cc.sex, v, v.label());
            }
        });
        ui.weak(match cc.sex {
            rustpg2d_engine::project::SexChoice::Both => "chosen in the character creator (male when it is off)",
            _ => "fixed by the game; the creator uses this sex's body",
        });
    });
    ui.horizontal(|ui| {
        ui.label("Default name");
        ui.add(egui::TextEdit::singleline(&mut cc.default_name).hint_text("(player character's name)").desired_width(160.0));
    });
    // The player's models (the colour rows come from each model's colour features).
    {
        let bcx = crate::builder::Cx { db: &ed.project.db, tex: &ed.tex, assets: &ed.assets };
        let base: Appearance = Appearance::default();
        ui.label(format!("Playable models (click to switch on / off; {} = playable): players choose among those of their sex (races list their own).", ph::GAME_CONTROLLER));
        if let Some(b) = crate::builder::model_cards(ui, &bcx, &base, "") {
            let on = !ed.project.db.model(&b).is_some_and(|m| m.playable);
            crate::builder::request_playable(&b, on);
        }
        ui.label("Default model for each sex (shown first; empty = the player character's own):");
        for (label, female, v) in [("Male", false, &mut cc.body_male), ("Female", true, &mut cc.body_female)] {
            let ids: Vec<String> = ed.project.db.models.iter().filter(|m| m.playable && m.female == female).map(|m| m.id.clone()).collect();
            ui.horizontal_top(|ui| {
                ui.label(label);
                if let Some(p) = crate::builder::model_cards_of(ui, &bcx, &ids, Some("Own model"), &base, v) {
                    *v = p;
                }
            });
            if !v.is_empty() && !ids.contains(v) {
                ui.colored_label(Color32::YELLOW, format!("The default {} model '{v}' isn't marked playable (it is still offered).", label.to_lowercase()));
            }
        }
        ui.weak("The creator shows a colour row for each colour feature of the chosen model that offers colours (Database \u{E13A} 3D models \u{E13A} Colour features).");
        ui.weak("New games give the player the player character's equipment (Database \u{E13A} Characters).");
    }
    ui.weak("Dialogs address the player with {player} (name) and {sir|madam} (male|female); conditions have 'Player sex'; scripts have player_name(), player_sex(), is_female(), is_male(), text(\"...\").");
    ui.separator();
    factions_form(ui, &mut ed.project.config.factions);
    ui.separator();
    ui.label(format!("Project folder: {}", ed.project.root.display()));
}

/// A *Faction* field: the project's factions; `inherited` = what "(none)" stands for.
fn faction_combo(ui: &mut egui::Ui, salt: &str, v: &mut String, factions: &[String], inherited: &str, help: &str) {
    ui.horizontal(|ui| {
        ui.label("Faction");
        let none = if inherited.is_empty() { "(none)".to_string() } else { format!("(its race's: {inherited})") };
        let label = if v.is_empty() { none.clone() } else if factions.contains(v) { v.clone() } else { format!("{v} (missing)") };
        egui::ComboBox::from_id_salt(salt).selected_text(label).show_ui(ui, |ui| {
            ui.selectable_value(v, String::new(), none);
            for f in factions {
                ui.selectable_value(v, f.clone(), f);
            }
        })
        .response
        .on_hover_text(help);
    });
}

/// *Factions* in Database › Project: the list (ids are fixed once added: triggers and scripts
/// name them) and the attitude table, symmetric, every cell cycling hostile / neutral / friendly.
fn factions_form(ui: &mut egui::Ui, f: &mut rustpg2d_engine::factions::Factions) {
    use rustpg2d_engine::factions::{Attitude, Faction};
    ui.heading("Factions");
    ui.weak("Who is hostile to whom. Characters belong to their own Faction (Database \u{E13A} Characters) or else their race's; characters of hostile factions fight each other. The player starts friendly with their own faction (their race's) and with the others as the table says; at a standing of -20 or below a faction's characters attack the player ('Change standing' action, change_standing() in scripts). 'Hostile' on a character makes it attack the player unless the player is a friend of its faction (standing 20 or more).");
    let mut remove = None;
    egui::Grid::new("factions").num_columns(3).striped(true).show(ui, |ui| {
        for (k, fa) in f.list.iter_mut().enumerate() {
            ui.monospace(&fa.id).on_hover_text("The id triggers and scripts use (fixed)");
            ui.add_sized([180.0, 20.0], egui::TextEdit::singleline(&mut fa.name).hint_text("Name"));
            if crate::ui::icon_button(ui, true, ph::TRASH, "Delete faction", "", "Characters and races that name it no longer belong to a faction").clicked() {
                remove = Some(k);
            }
            ui.end_row();
        }
    });
    if let Some(k) = remove {
        let id = f.list.remove(k).id;
        f.relations.retain(|r| r.a != id && r.b != id);
    }
    ui.horizontal(|ui| {
        let key = egui::Id::new("new_faction_id");
        let mut text: String = ui.data_mut(|d| d.get_temp(key).unwrap_or_default());
        ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0).hint_text("new faction id"));
        let id: String = text.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let ok = !id.is_empty() && f.get(&id).is_none();
        if ui.add_enabled(ok, egui::Button::new(crate::ui::t(ph::PLUS, "Add faction"))).clicked() {
            let mut name = id.replace('_', " ");
            if let Some(c) = name.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            f.list.push(Faction { id: id.clone(), name });
            text.clear();
        }
        ui.data_mut(|d| d.insert_temp(key, text));
    });
    if f.list.len() < 2 {
        return;
    }
    ui.label("Attitudes (click a cell to change it; the table reads both ways):");
    egui::ScrollArea::horizontal().id_salt("faction_table").show(ui, |ui| {
        egui::Grid::new("faction_attitudes").num_columns(f.list.len() + 1).spacing([6.0, 4.0]).show(ui, |ui| {
            ui.label("");
            for b in &f.list {
                ui.strong(&b.name);
            }
            ui.end_row();
            let ids: Vec<(String, String)> = f.list.iter().map(|x| (x.id.clone(), x.name.clone())).collect();
            for (i, (a, an)) in ids.iter().enumerate() {
                ui.strong(an);
                for (j, (b, bn)) in ids.iter().enumerate() {
                    if i == j {
                        ui.weak("(itself)");
                        continue;
                    }
                    let at = f.attitude(a, b);
                    let color = match at {
                        Attitude::Hostile => egui::Color32::from_rgb(220, 90, 80),
                        Attitude::Neutral => ui.visuals().weak_text_color(),
                        Attitude::Friendly => egui::Color32::from_rgb(110, 190, 110),
                    };
                    let r = ui.add(egui::Button::new(egui::RichText::new(at.label()).color(color)).min_size(egui::vec2(70.0, 0.0)));
                    if r.on_hover_text(format!("{an} and {bn}: click for the next attitude")).clicked() {
                        let next = Attitude::ALL[(Attitude::ALL.iter().position(|x| *x == at).unwrap_or(1) + 1) % 3];
                        f.set_attitude(a, b, next);
                    }
                }
                ui.end_row();
            }
        });
    });
}

/// A cut-scene function to start from (Scripts tab, *+ Cutscene*).
fn cutscene_template(name: &str) -> String {
    format!(
        r#"
// A cut-scene: play it with the trigger action "Play cutscene" (or cutscene("{name}") in a
// script). While it plays the HUD is hidden and everyone else waits. Positions are tiles.
fn {name}() {{
    place("player", 5, 5);            // put the player on a tile at once
    walk_to("player", 8, 5);          // walk onto a tile (the scene waits until it's there)
    approach("guard", "player");      // walk up to someone; both face each other
    say("Guard", "Hello, {{player}}!");
    face("guard", "down");
    wait(0.5);                        // seconds
}}
"#
    )
}

/// `fn name(` definitions in a script, with the character index where each starts.
pub(crate) fn script_functions(text: &str) -> Vec<(String, usize)> {
    rustpg2d_engine::script_refs::functions(text).into_iter().map(|f| (f.name, text[..f.at].chars().count())).collect()
}

impl Editor {
    /// Opens a script in Database → Scripts (saving the open one first if it was edited),
    /// optionally with the cursor at character `jump`.
    pub fn open_script(&mut self, f: &std::path::Path, jump: Option<usize>) {
        if self.db.script_file.as_deref() != Some(f) {
            self.save_script_if_dirty();
            self.db.script_text = std::fs::read_to_string(f).unwrap_or_default();
            self.db.script_saved = self.db.script_text.clone();
            self.db.script_file = Some(f.to_path_buf());
            self.db.script_dirty = false;
        }
        self.db.script_jump = jump;
    }

    fn save_script_if_dirty(&mut self) {
        if !self.db.script_dirty {
            return;
        }
        if let Some(f) = self.db.script_file.clone() {
            match std::fs::write(&f, &self.db.script_text) {
                Ok(()) => {
                    self.db.script_dirty = false;
                    self.db.script_saved = self.db.script_text.clone();
                    self.problems.stale = true;
                    self.set_status(format!("Saved {}", f.file_name().unwrap_or_default().to_string_lossy()));
                }
                Err(e) => self.set_status(format!("Saving script failed: {e}")),
            }
        }
    }
}

fn scripts_ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let dir = ed.project.scripts_dir();
    let files: Vec<std::path::PathBuf> = ed.project.script_files();
    // Always show a script: the first one when none (or a deleted one) is open.
    if ed.db.script_file.as_ref().is_none_or(|f| !files.contains(f)) {
        if let Some(f) = files.first().cloned() {
            ed.db.script_dirty = false;
            ed.open_script(&f, None);
        }
    }
    let mut open: Option<(std::path::PathBuf, Option<usize>)> = None;
    let mut create: Option<std::path::PathBuf> = None;
    let mut save = false;
    // Ids used in API calls that don't exist (checked live, the open file from the editor text).
    let texts: Vec<(std::path::PathBuf, String)> = files
        .iter()
        .map(|f| (f.clone(), if ed.db.script_file.as_ref() == Some(f) { ed.db.script_text.clone() } else { std::fs::read_to_string(f).unwrap_or_default() }))
        .collect();
    let issues: Vec<(std::path::PathBuf, usize, String, String)> = texts
        .iter()
        .flat_map(|(f, text)| {
            rustpg2d_engine::script_refs::check_script(text, &ed.project.db, &ed.refs.maps, &ed.refs.spawns)
                .into_iter()
                .map(move |i| (f.clone(), i.line, i.message, text.clone()))
        })
        .collect();
    egui::SidePanel::left("scripts_left").default_width(220.0).show_inside(ui, |ui| {
        let st = &mut ed.db;
        ui.weak("Click a file or one of its functions to edit it.");
        egui::ScrollArea::vertical().id_salt("script_files").auto_shrink(false).max_height(ui.available_height() - 40.0).show(ui, |ui| {
            for f in &files {
                let name = f.file_name().unwrap().to_string_lossy().to_string();
                let current = st.script_file.as_ref() == Some(f);
                let row = ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| ui.selectable_label(current, format!("\u{E23A} {name}")));
                if row.inner.clicked() {
                    open = Some((f.clone(), None));
                }
                // Its functions (from the editor text for the open file, so new ones show at once).
                let text = if current { st.script_text.clone() } else { std::fs::read_to_string(f).unwrap_or_default() };
                for (func, at) in script_functions(&text) {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        let label = egui::RichText::new(format!("{func}()")).monospace();
                        let r = ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| ui.selectable_label(false, label));
                        if r.inner.on_hover_text(format!("Go to fn {func} in {name}")).clicked() {
                            open = Some((f.clone(), Some(at)));
                        }
                    });
                }
                ui.add_space(4.0);
            }
            if files.is_empty() {
                ui.weak("No scripts yet.");
            }
            ui.separator();
            if issues.is_empty() {
                ui.weak("\u{E182} No unknown ids in script calls");
            } else {
                ui.colored_label(Color32::YELLOW, format!("Problems ({})", issues.len()))
                    .on_hover_text("Ids passed to script functions (give_item, teleport, start_quest, play_sound, set_state, ...) that don't exist in this project");
                for (f, line, msg, text) in &issues {
                    let name = f.file_name().unwrap_or_default().to_string_lossy();
                    let label = egui::RichText::new(format!("{name}:{line}  {msg}")).small().color(Color32::LIGHT_RED);
                    if ui.add(egui::Label::new(label).sense(egui::Sense::click())).on_hover_text("Go to this line").clicked() {
                        // Character index of the line's start.
                        let at: usize = text.split_inclusive('\n').take(line - 1).map(|l| l.chars().count()).sum();
                        open = Some((f.clone(), Some(at)));
                    }
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut st.new_script).hint_text("new file name").desired_width(120.0));
            if ui.button("+ New").on_hover_text("Creates scripts/<name>.rhai (without a name: script_N.rhai)").clicked() {
                let base = st.new_script.trim().trim_end_matches(".rhai").to_string();
                let name = if base.is_empty() {
                    (1..).map(|n| format!("script_{n}")).find(|n| !dir.join(format!("{n}.rhai")).exists()).unwrap()
                } else {
                    base
                };
                create = Some(dir.join(format!("{name}.rhai")));
                st.new_script.clear();
            }
        });
    });
    if let Some(p) = create {
        if !p.exists() {
            if let Err(e) = rustpg2d_engine::sys::write(&p, b"// New script\n\nfn my_function() {\n    say(\"Hello!\");\n}\n") {
                eprintln!("[editor] cannot create {}: {e}", p.display());
            }
        }
        ed.refresh_refs();
        open = Some((p, None));
    }
    if let Some((f, jump)) = open {
        ed.open_script(&f, jump);
    }
    // What completion offers: names from triggers and maps plus those used in scripts.
    let mut flags = ed.refs.all_flags.clone();
    let mut vars = ed.refs.vars.clone();
    let mut functions: Vec<String> = Vec::new();
    let mut signatures: Vec<(String, Vec<String>)> = Vec::new();
    for (_, text) in &texts {
        functions.extend(crate::script_editor::defined_functions(text));
        signatures.extend(crate::script_editor::defined_signatures(text));
        for a in rustpg2d_engine::script_refs::string_args(text) {
            // Flag and variable arguments of any API function (the table says which).
            let kinds: Vec<rustpg2d_engine::script_api::Arg> = rustpg2d_engine::script_api::arg_kinds(&a.func, a.arg).collect();
            if kinds.contains(&rustpg2d_engine::script_api::Arg::Flag) {
                flags.push(a.value);
            } else if kinds.contains(&rustpg2d_engine::script_api::Arg::Var) {
                vars.push(a.value);
            }
        }
    }
    for v in [&mut flags, &mut vars, &mut functions] {
        v.sort();
        v.dedup();
    }
    let mut targets = vec!["self".to_string(), "player".to_string()];
    targets.extend(ed.refs.entity_names.iter().cloned());
    let problem_spans: Vec<(usize, usize)> = rustpg2d_engine::script_refs::check_script(&ed.db.script_text, &ed.project.db, &ed.refs.maps, &ed.refs.spawns)
        .into_iter()
        .map(|i| i.span)
        .collect();
    let mut goto: Option<String> = None;
    let st = &mut ed.db;
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(f) = st.script_file.clone() else {
            ui.label("No script files yet: create one on the left. Functions can be called from triggers with the 'Run script' action, or used as 'Script' conditions (returning bool).");
            return;
        };
        ui.horizontal(|ui| {
            ui.strong(f.file_name().unwrap().to_string_lossy());
            if st.script_dirty {
                ui.colored_label(Color32::YELLOW, "• edited");
            }
            save = ui.button("\u{E248} Save & check").on_hover_text("Ctrl+S").clicked();
            ui.separator();
            // Cut-scenes: a template to start from, and a preview of any function as one.
            if ui.button(ui::t(ph::FILM_SLATE, "+ Cutscene")).on_hover_text("Adds a cut-scene template function at the end of this script").clicked() {
                let names: Vec<String> = script_functions(&st.script_text).into_iter().map(|f| f.0).collect();
                let name = (1..).map(|n| if n == 1 { "my_cutscene".to_string() } else { format!("my_cutscene_{n}") }).find(|n| !names.contains(n)).unwrap();
                let at = st.script_text.chars().count();
                if !st.script_text.ends_with('\n') && !st.script_text.is_empty() {
                    st.script_text.push('\n');
                }
                st.script_text.push_str(&cutscene_template(&name));
                st.script_dirty = true;
                st.script_jump = Some(at);
            }
            ui.menu_button(ui::t(ph::PLAY, "Play as cutscene"), |ui| {
                ui.label("Play-test a function of this script as a cut-scene on the open map (saves first):");
                let funcs = script_functions(&st.script_text);
                if funcs.is_empty() {
                    ui.weak("No functions in this script.");
                }
                for (func, _) in funcs {
                    if ui.button(format!("{func}()")).clicked() {
                        crate::widgets::request_play_cutscene(&func);
                        ui.close_menu();
                    }
                }
            });
        });
        if let Some(ci) = st.script_jump.take() {
            st.script_ed.set_cursor = Some(ci);
            st.script_ed.scroll_to = Some(ci);
        }
        let known = crate::script_editor::Known {
            db: &ed.project.db,
            maps: &ed.refs.maps,
            spawns: &ed.refs.spawns,
            targets: &targets,
            flags: &flags,
            vars: &vars,
            functions: &functions,
            signatures: &signatures,
            factions: &ed.refs.factions,
        };
        let res = crate::script_editor::show(ui, &mut st.script_text, &mut st.script_ed, &known, &problem_spans);
        if res.changed {
            st.script_dirty = st.script_text != st.script_saved;
        }
        goto = res.goto;
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            save = true;
        }
    });
    // Ctrl+click on a function: open its file at the definition.
    if let Some(name) = goto {
        let found = texts.iter().find_map(|(f, text)| {
            let text = if ed.db.script_file.as_ref() == Some(f) { ed.db.script_text.clone() } else { text.clone() };
            let at = script_functions(&text).into_iter().find(|(n, _)| *n == name)?.1;
            Some((f.clone(), at))
        });
        if let Some((f, at)) = found {
            ed.open_script(&f, Some(at));
        }
    }
    if save {
        ed.db.script_dirty = true;
        ed.save_script_if_dirty();
        if !ed.db.script_dirty {
            let mut host = rustpg2d_engine::script::ScriptHost::new();
            host.load_dir(&dir);
            ed.refresh_refs();
            let unknown = if issues.is_empty() { String::new() } else { format!("; {} unknown id(s), see Problems", issues.len()) };
            if host.errors.is_empty() {
                ed.set_status(format!("Script saved, no errors{unknown}"));
            } else {
                ed.set_status(format!("Script error: {}{unknown}", host.errors.join("; ")));
            }
        }
    }
}

/// Art actions of the item / prop / character forms.
pub enum ArtAction {
    /// Give the current item icon / prop state a blank frame (`cells` big) to draw in.
    NewFrame { cells: (u32, u32) },
}

/// The image the embedded editor should show for a sprite: (path, frame size, character
/// layout, frame to select).
fn art_of(db: &Database, sprite: &SpriteRef, character: bool) -> Option<(String, (u32, u32), bool, u32)> {
    let s = db.sheet(&sprite.sheet)?;
    Some((s.image.clone(), (s.frame_w, s.frame_h), character, sprite.frame))
}

/// "Edit art" toggle and "New blank …" for item icons and prop states.
fn art_buttons(ui: &mut egui::Ui, cx: &mut Cx, what: &str, cells: (u32, u32)) {
    ui.horizontal(|ui| {
        let mut on = *cx.art_open;
        if ui.toggle_value(&mut on, "\u{E6C8} Edit art").on_hover_text("Show the pixel editor under this form, at this sprite").changed() {
            *cx.art_open = on;
        }
        if ui.button(format!("+ New blank {what}")).on_hover_text("Uses an empty frame of the sheet (or adds a row of frames) and opens it for drawing").clicked() {
            *cx.art = Some(ArtAction::NewFrame { cells });
        }
    });
}

impl Editor {
    /// Carries out an art action of the item / prop / character forms.
    pub fn art_action(&mut self, a: ArtAction, tab: Tab) {
        let i = self.db.sel;
        let res: anyhow::Result<String> = (|| match a {
            ArtAction::NewFrame { cells } => {
                let cur = match tab {
                    Tab::Items => self.project.db.items.get(i).map(|d| d.icon.clone()),
                    Tab::Props => self.project.db.props.get(i).map(|d| d.state_look(self.db.state_sel.min(d.states.len())).0.clone()),
                    Tab::Characters => self.project.db.characters.get(i).map(|d| d.portrait.clone()),
                    _ => None,
                };
                let Some(cur) = cur else { anyhow::bail!("nothing selected") };
                let sheet = if self.project.db.sheet(&cur.sheet).is_some() { cur.sheet.clone() } else { self.default_sheet(tab)? };
                let frame = self.blank_frame(&sheet, cells)?;
                let sprite = if cells == (1, 1) { SpriteRef::new(&sheet, frame) } else { SpriteRef::block(&sheet, frame, cells.0, cells.1) };
                match tab {
                    Tab::Items => self.project.db.items[i].icon = sprite,
                    Tab::Characters => self.project.db.characters[i].portrait = sprite,
                    _ => {
                        let d = &mut self.project.db.props[i];
                        let k = self.db.state_sel.min(d.states.len());
                        if k == 0 {
                            d.sprite = sprite;
                        } else {
                            d.states[k - 1].sprite = sprite;
                        }
                    }
                }
                self.defs_dirty = true;
                self.db.art_open = true;
                self.db.art_focus = None;
                Ok(format!("Blank frame {frame} of sheet '{sheet}' is ready: draw it below, Ctrl+S saves the image"))
            }
        })();
        match res {
            Ok(m) => self.set_status(m),
            Err(e) => self.set_status(format!("Could not create the art: {e:#}")),
        }
    }

    /// A sheet for new item icons / prop frames when the definition has none yet: an existing
    /// "items" / "props" sheet, else a new blank one.
    fn default_sheet(&mut self, tab: Tab) -> anyhow::Result<String> {
        // Portraits: 64 x 64 frames on a sheet of their own.
        if tab == Tab::Characters {
            const SIZE: u32 = 64;
            if self.project.db.sheet("portraits").is_none() {
                let rel = "sprites/portraits.png";
                let path = self.project.root.join(rel);
                std::fs::create_dir_all(path.parent().unwrap())?;
                if !path.exists() {
                    std::fs::write(&path, Rgba::new(SIZE * 4, SIZE).encode()?)?;
                }
                self.project.db.sheets.push(SheetDef::new("portraits", rel, SIZE, SIZE));
                self.defs_dirty = true;
                self.reload_assets();
            }
            return Ok("portraits".into());
        }
        let want = if tab == Tab::Items { "items" } else { "props" };
        if self.project.db.sheet(want).is_some() {
            return Ok(want.to_string());
        }
        let id = if tab == Tab::Items { "icons" } else { "prop_art" };
        if self.project.db.sheet(id).is_none() {
            let ts = rustpg2d_engine::project::TILE_SIZE;
            let rel = format!("sprites/{id}.png");
            let path = self.project.root.join(&rel);
            std::fs::create_dir_all(path.parent().unwrap())?;
            if !path.exists() {
                std::fs::write(&path, Rgba::new(ts * 8, ts).encode()?)?;
            }
            self.project.db.sheets.push(SheetDef::new(id, &rel, ts, ts));
            self.defs_dirty = true;
            self.reload_assets();
        }
        Ok(id.to_string())
    }

    /// First fully transparent `cells`-sized frame block of a sheet; if there is none, a row
    /// of frames is added to the image. Returns the block's top-left frame.
    fn blank_frame(&mut self, sheet: &str, cells: (u32, u32)) -> anyhow::Result<u32> {
        let def = self.project.db.sheet(sheet).cloned().ok_or_else(|| anyhow::anyhow!("unknown sheet {sheet}"))?;
        if self.px.doc.as_ref().is_some_and(|d| d.rel == def.image && d.is_dirty()) {
            anyhow::bail!("save or revert the open image {} first", def.image);
        }
        let path = self.project.root.join(&def.image);
        let mut img = Rgba::decode(&std::fs::read(&path)?)?;
        let (fw, fh) = (def.frame_w.max(1), def.frame_h.max(1));
        let (cols, rows) = ((img.w / fw).max(1), img.h / fh);
        let (cw, ch) = (cells.0.max(1), cells.1.max(1));
        if cw > cols {
            anyhow::bail!("the sheet is only {cols} frames wide");
        }
        let empty = |img: &Rgba, c: u32, r: u32| {
            (0..ch * fh).all(|y| (0..cw * fw).all(|x| img.get((c * fw + x) as i32, (r * fh + y) as i32)[3] == 0))
        };
        for r in 0..rows.saturating_sub(ch - 1) {
            for c in 0..=cols - cw {
                if empty(&img, c, r) {
                    return Ok(r * cols + c);
                }
            }
        }
        // Full: add rows at the bottom.
        img = img.with_canvas(img.w, (rows + ch) * fh);
        std::fs::write(&path, img.encode()?)?;
        self.reload_assets();
        if let Some(d) = self.px.doc.as_ref().filter(|d| d.rel == def.image) {
            let (rel, frame, layout) = (d.rel.clone(), d.frame, d.layout);
            self.load_pixel(&rel, frame, layout);
        }
        Ok(rows * cols)
    }
}

fn skill_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::skills::SkillDef, cx: &mut Cx) {
    use rustpg2d_engine::skills::SkillEffect;
    ui.heading("Skill");
    rename_row(ui, RefKind::Skill, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    text_box(ui, &mut d.description, 2, "");
    ui.horizontal(|ui| {
        ui.label("Max level");
        ui.add(egui::DragValue::new(&mut d.max_level).range(1..=99));
    });
    ui.separator();
    ui.heading("Effect");
    let mut idx = d.effect.index();
    ui.horizontal_wrapped(|ui| {
        for (i, (name, tip)) in SkillEffect::LABELS.iter().enumerate() {
            ui.selectable_value(&mut idx, i, *name).on_hover_text(*tip);
        }
    });
    if idx != d.effect.index() {
        d.effect = SkillEffect::template(idx);
    }
    egui::Grid::new("skill_effect").num_columns(2).show(ui, |ui| match &mut d.effect {
        SkillEffect::None | SkillEffect::Lockpick => {}
        SkillEffect::MeleeDamage { percent } | SkillEffect::RangedDamage { percent } | SkillEffect::SpellDamage { percent } | SkillEffect::Speed { percent } | SkillEffect::Haggle { percent } | SkillEffect::FoodBonus { percent } => {
            ui.label("Per level");
            ui.add(egui::DragValue::new(percent).range(0.0..=100.0).speed(0.5).suffix(" %"));
            ui.end_row();
        }
        SkillEffect::Defense { points } => {
            ui.label("Per level");
            ui.add(egui::DragValue::new(points).range(0..=99).suffix(" defense"));
            ui.end_row();
        }
        SkillEffect::Mana { points } => {
            ui.label("Per level");
            ui.add(egui::DragValue::new(points).range(0..=999).suffix(" mana"));
            ui.end_row();
        }
        SkillEffect::ManaRegen { per_second } => {
            ui.label("Per level");
            ui.add(egui::DragValue::new(per_second).range(0.0..=100.0).speed(0.05).suffix(" mana / s"));
            ui.end_row();
        }
        SkillEffect::Script { function } => {
            ui.label("Function");
            widgets::suggest(ui, "skill_fn", function, &cx.refs.functions, 160.0);
            ui.end_row();
        }
        SkillEffect::Gather { tool, percent } => {
            ui.label("Tool kind");
            let kinds: Vec<String> = cx.db.items.iter().map(|i| i.tool.clone()).filter(|t| !t.is_empty()).collect();
            widgets::suggest(ui, "skill_tool", tool, &kinds, 140.0);
            ui.end_row();
            ui.label("Per level");
            ui.add(egui::DragValue::new(percent).range(0.0..=80.0).speed(0.5).suffix(" % fewer hits"));
            ui.end_row();
        }
    });
    if let SkillEffect::Script { function } = &d.effect {
        ui.weak(format!("Called as {}(level) whenever the level changes (and when a game starts or loads with the skill at level 1 or more). The script applies the effect itself, e.g. with set_var or flags that other scripts and triggers read.", if function.is_empty() { "fn" } else { function.as_str() }));
    }
    ui.weak(format!("At level {}: {}", d.max_level, d.effect_text(d.max_level)));
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "skill_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
}

fn achievement_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::achievements::AchievementDef, cx: &mut Cx) {
    ui.heading("Achievement");
    egui::Grid::new("achievement").num_columns(2).show(ui, |ui| {
        ui.label("Id");
        ui.add(egui::TextEdit::singleline(&mut d.id).desired_width(200.0))
            .on_hover_text("Used by the 'Unlock achievement' action and scripts (unlock_achievement(\"id\")). Changing it does not update them, and players' unlocks are kept by id.");
        ui.end_row();
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(320.0));
        ui.end_row();
        ui.label("Hidden");
        ui.checkbox(&mut d.hidden, "a secret: shown as \"???\" until unlocked");
        ui.end_row();
    });
    ui.label("Description (shown in the Achievements page of the menus)");
    text_box(ui, &mut d.description, 2, "");
    ui.separator();
    ui.heading("Unlocked when");
    ui.weak("All of these hold (checked twice a second while playing): quests, flags, variables, defeated characters (the bestiary counts), level, items, a script condition... Without conditions only the 'Unlock achievement' action or unlock_achievement(id) unlocks it.");
    widgets::conditions_ui(ui, &format!("ach_{}", d.id), &mut d.conditions, cx.refs);
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "ach_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
    ui.weak("Unlocks belong to the player, not to a save game: they are kept in achievements.ron next to the game's settings (play-tests use the same file; Reset achievements in the game's menu clears it).");
}

fn race_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::races::RaceDef, cx: &mut Cx) {
    ui.heading("Race");
    rename_row(ui, RefKind::Race, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description").on_hover_text("Shown in the character creator under the race.");
    text_box(ui, &mut d.description, 2, "Shown in the character creator");
    ui.checkbox(&mut d.playable, "Playable: offered in the character creator");
    if !cx.refs.factions.is_empty() || !d.faction.is_empty() {
        faction_combo(ui, "race_faction", &mut d.faction, &cx.refs.factions, "", "The faction of its characters (those without their own) and of players of this race: they start friendly with it and with its friends, and are attacked by the factions hostile to it.");
    }
    ui.separator();
    ui.heading("Models");
    ui.weak("The creator offers the models of the chosen sex; characters with one of these models get the race's size. Click a model to add or remove it.");
    {
        let bcx = crate::builder::Cx { db: cx.db, tex: cx.tex, assets: cx.assets };
        for (label, list) in [("Men", &mut d.bodies_male), ("Women", &mut d.bodies_female)] {
            ui.label(format!("{label}: {}", if list.is_empty() { "(none: the project's playable models)".to_string() } else { list.join(", ") }));
            let base = Appearance::default();
            if let Some(b) = crate::builder::model_cards(ui, &bcx, &base, "") {
                if list.contains(&b) {
                    list.retain(|x| *x != b);
                } else {
                    list.push(b);
                }
            }
        }
    }
    ui.separator();
    ui.heading("A player of this race");
    egui::Grid::new("race_stats").num_columns(2).show(ui, |ui| {
        ui.label("Max HP");
        ui.add(egui::DragValue::new(&mut d.max_hp).range(-50..=100)).on_hover_text("Added to the player's max HP (negative: less)");
        ui.end_row();
        ui.label("Speed");
        ui.add(egui::DragValue::new(&mut d.speed).range(-50.0..=100.0).suffix(" %")).on_hover_text("Faster (or slower, negative) than other players");
        ui.end_row();
        ui.label("Defense");
        ui.add(egui::DragValue::new(&mut d.defense).range(-20..=50)).on_hover_text("Added to the player's defense");
        ui.end_row();
        ui.label("Starting skills");
        ui.vertical(|ui| {
            let mut remove = None;
            for (k, s) in d.skills.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    widgets::suggest(ui, &format!("race_skill{k}"), &mut s.skill, &cx.refs.skills, 120.0);
                    ui.label("level");
                    ui.add(egui::DragValue::new(&mut s.level).range(1..=20));
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                d.skills.remove(k);
            }
            if ui.small_button("+ Skill").on_hover_text("A skill level the player starts with (Database \u{E13A} Skills)").clicked() {
                d.skills.push(rustpg2d_engine::races::RaceSkill { skill: cx.refs.skills.first().cloned().unwrap_or_default(), level: 1 });
            }
        });
        ui.end_row();
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Size");
        ui.add(egui::Slider::new(&mut d.size, 0.5..=2.0).step_by(0.05)).on_hover_text("Characters with this race's bodies are drawn this much larger (and their hitbox grows with them): trolls 1.5");
    });
}

fn recipe_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::crafting::RecipeDef, cx: &mut Cx) {
    ui.heading("Recipe");
    rename_row(ui, RefKind::Recipe, &d.id, cx.rename);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
        if let Some(it) = cx.db.item(&d.result) {
            widgets::paint_sprite(ui, cx.tex, cx.assets, &it.icon, rect);
        }
        ui.label("Makes");
        ui.add(egui::DragValue::new(&mut d.count).range(1..=999));
        ui.label("x");
        widgets::suggest(ui, "recipe_result", &mut d.result, &cx.refs.items, 140.0);
    });
    if !d.result.is_empty() && cx.db.item(&d.result).is_none() {
        ui.colored_label(egui::Color32::YELLOW, format!("There is no item '{}'.", d.result));
    }
    ui.separator();
    ui.heading("Ingredients");
    ui.weak("Used up when the recipe is made (from the inventory, then what the player wears).");
    widgets::carried_ui(ui, "recipe_ing", &mut d.ingredients, &cx.refs.items);
    ui.separator();
    egui::Grid::new("recipe_where").num_columns(2).show(ui, |ui| {
        ui.label("Station");
        ui.horizontal(|ui| {
            widgets::suggest(ui, "recipe_station", &mut d.station, &cx.refs.stations, 120.0);
            if d.station.is_empty() {
                ui.weak("(none)");
            }
        })
        .response
        .on_hover_text("Where it is made: the 'Open crafting' action / open_crafting(station) shows the recipes of a station, e.g. on a workbench prop's Interact trigger");
        ui.end_row();
        ui.label("Requires skill");
        ui.horizontal(|ui| {
            widgets::suggest(ui, "recipe_skill", &mut d.skill, &cx.refs.skills, 120.0);
            if !d.skill.is_empty() {
                ui.label("at level");
                ui.add(egui::DragValue::new(&mut d.level).range(0..=99));
            } else {
                ui.weak("(none)");
            }
        });
        ui.end_row();
        ui.label("Tools");
        ui.vertical(|ui| {
            let kinds: Vec<String> = {
                let mut v: Vec<String> = cx.db.items.iter().map(|i| i.tool.clone()).filter(|t| !t.is_empty()).collect();
                v.sort();
                v.dedup();
                v
            };
            let mut remove = None;
            for (k, t) in d.tools.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    widgets::suggest(ui, &format!("recipe_tool{k}"), t, &kinds, 120.0);
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                d.tools.remove(k);
            }
            if ui.small_button("+ Tool").on_hover_text("A tool kind (an item's 'Tool kind', e.g. hammer) the player must have: not used up").clicked() {
                d.tools.push(kinds.first().cloned().unwrap_or_else(|| "hammer".into()));
            }
        });
        ui.end_row();
        ui.label("Station state");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.station_state).desired_width(100.0).hint_text("e.g. lit"));
            ui.weak("(the station's prop must be in this state, e.g. a cooking fire \"lit\"; empty = any)");
        });
        ui.end_row();
        ui.label("Better results");
        ui.vertical(|ui| {
            if d.skill.is_empty() {
                ui.weak("(choose a required skill first: higher levels of it can make better items)");
            }
            let mut remove = None;
            for (k, q) in d.quality.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label("from level");
                    ui.add(egui::DragValue::new(&mut q.level).range(1..=99));
                    ui.label("makes");
                    widgets::suggest(ui, &format!("recipe_quality{k}"), &mut q.result, &cx.refs.items, 130.0);
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                d.quality.remove(k);
            }
            if ui.add_enabled(!d.skill.is_empty(), egui::Button::new("+ Better result").small()).on_hover_text("From a higher level of the required skill on, the recipe makes another item (e.g. a Fine Iron Sword)").clicked() {
                let level = d.quality.iter().map(|q| q.level).max().unwrap_or(d.level) + 1;
                d.quality.push(rustpg2d_engine::crafting::QualityStep { level, result: d.result.clone() });
            }
        });
        ui.end_row();
    });
    ui.weak("Players make it in the crafting window: give a prop (a workbench, an anvil, an alchemy table) an Interact trigger with the 'Open crafting' action for this station. Scripts: open_crafting(station[, title]), can_craft(recipe).");
}

fn spell_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::skills::SpellDef, cx: &mut Cx) {
    use rustpg2d_engine::skills::SpellEffect;
    ui.heading("Spell");
    rename_row(ui, RefKind::Spell, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    text_box(ui, &mut d.description, 2, "");
    egui::Grid::new("spell_cost").num_columns(2).show(ui, |ui| {
        ui.label("Mana");
        ui.add(egui::DragValue::new(&mut d.mana).range(0..=9999));
        ui.end_row();
        ui.label("Cooldown");
        ui.add(egui::DragValue::new(&mut d.cooldown).range(0.0..=600.0).speed(0.1).suffix(" s"));
        ui.end_row();
        ui.label("Requires skill");
        ui.horizontal(|ui| {
            widgets::suggest(ui, "spell_skill", &mut d.skill, &cx.refs.skills, 120.0);
            if !d.skill.is_empty() {
                ui.label("at level");
                ui.add(egui::DragValue::new(&mut d.level).range(0..=99));
            } else {
                ui.weak("(none)");
            }
        });
        ui.end_row();
    });
    ui.separator();
    ui.heading("Effect");
    let mut idx = d.effect.index();
    ui.horizontal_wrapped(|ui| {
        for (i, (name, tip)) in SpellEffect::LABELS.iter().enumerate() {
            ui.selectable_value(&mut idx, i, *name).on_hover_text(*tip);
        }
    });
    if idx != d.effect.index() {
        d.effect = SpellEffect::template(idx);
    }
    let mut projectile: Option<&mut SpriteRef> = None;
    egui::Grid::new("spell_effect").num_columns(2).show(ui, |ui| match &mut d.effect {
        SpellEffect::Heal { amount } => {
            ui.label("Heals");
            ui.add(egui::DragValue::new(amount).range(1..=999).suffix(" HP")).on_hover_text("1 heart = 2 HP");
            ui.end_row();
        }
        SpellEffect::Bolt { damage, projectile: p, speed, range } => {
            ui.label("Damage");
            ui.add(egui::DragValue::new(damage).range(0..=999));
            ui.end_row();
            ui.label("Speed (px/s)");
            ui.add(egui::DragValue::new(speed).range(10.0..=2000.0));
            ui.end_row();
            ui.label("Range (tiles)");
            ui.add(egui::DragValue::new(range).range(0.5..=64.0).speed(0.1));
            ui.end_row();
            projectile = Some(p);
        }
        SpellEffect::Blast { damage, radius } => {
            ui.label("Damage");
            ui.add(egui::DragValue::new(damage).range(0..=999));
            ui.end_row();
            ui.label("Radius (tiles)");
            ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1));
            ui.end_row();
        }
        SpellEffect::Light { radius, seconds } => {
            ui.label("Radius (tiles)");
            ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1));
            ui.end_row();
            ui.label("Lasts");
            ui.add(egui::DragValue::new(seconds).range(0.5..=3600.0).speed(0.5).suffix(" s"));
            ui.end_row();
        }
        SpellEffect::Script { function } => {
            ui.label("Function");
            widgets::suggest(ui, "spell_fn", function, &cx.refs.functions, 160.0);
            ui.end_row();
        }
    });
    if let Some(p) = projectile {
        ui.label("Projectile sprite (drawn pointing right, rotated in flight):");
        widgets::sprite_picker(ui, "spell_proj", p, cx.tex, cx.assets);
    }
    if matches!(d.effect, SpellEffect::Script { .. }) {
        ui.weak("Called when the spell is cast (mana and cooldown are already taken). The function can use the whole script API: say, heal, give_item, damage_around, player_light, set_flag ...");
    }
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "spell_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
    ui.weak("Spells are learned from items with 'Teaches spell' (Database › Items) or the 'Learn spell' action; the player casts them from the Magic screen (K) or the quick bar (1-8).");
}

/// *Languages* of the Project form: the game's own language, its translations (`lang/<code>.ron`,
/// see `rustpg2d_engine::lang`), template export and the play-test language.
fn languages_form(ed: &mut Editor, ui: &mut egui::Ui) {
    use rustpg2d_engine::lang;
    ui.heading("Languages");
    ui.weak("The game is written in one language; translations are files lang/CODE.ron in the project (original text -> translation). Export a template, fill in the translations (any text editor), and players choose the language in Options. R(ust)PG2D's own texts (menus, windows) come translated for German.");
    let mut export = None;
    let mut remove = None;
    egui::Grid::new("langs").num_columns(2).show(ui, |ui| {
        ui.label("Written in");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut ed.project.config.language).desired_width(50.0)).on_hover_text("Language code of the texts in the project (en, de, fr, ...)");
            ui.weak(lang::language_name(&ed.project.config.language));
        });
        ui.end_row();
        for (k, code) in ed.project.config.languages.clone().iter().enumerate() {
            ui.label(if k == 0 { "Translations" } else { "" });
            ui.horizontal(|ui| {
                let file = lang::load_file(&ed.project.root, code);
                let (n, open) = file.as_ref().map_or((0, 0), |f| (f.texts.len(), f.texts.values().filter(|v| v.is_empty()).count()));
                let name = file.as_ref().map(|f| f.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| lang::language_name(code));
                ui.label(format!("{code}  {name}"));
                if file.is_some() {
                    ui.weak(format!("{} of {n} translated", n - open));
                } else {
                    ui.colored_label(egui::Color32::from_rgb(240, 190, 90), "no lang file yet");
                }
                if ui.small_button(ui::t(ph::EXPORT, "Update template")).on_hover_text(format!("Writes lang/{code}.ron with every text of the game: translations already there stay, new texts are added empty")).clicked() {
                    export = Some(code.clone());
                }
                if ui.small_button(ph::TRASH).on_hover_text("No longer offer this language (the file stays)").clicked() {
                    remove = Some(k);
                }
            });
            ui.end_row();
        }
        ui.label("Add");
        ui.horizontal(|ui| {
            let id = egui::Id::new("new_lang_code");
            let mut code: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
            ui.add(egui::TextEdit::singleline(&mut code).desired_width(50.0).hint_text("de"));
            if ui.button(ui::t(ph::EXPORT, "Export translation template")).on_hover_text("Adds the language and writes lang/CODE.ron with every text of the game to translate").clicked() && !code.trim().is_empty() {
                export = Some(code.clone());
                code.clear();
            }
            ui.data_mut(|d| d.insert_temp(id, code));
        });
        ui.end_row();
        ui.label("Play-test in");
        let mut options = vec![(String::new(), "the player's choice".to_string())];
        options.extend(lang::offered(&ed.project.root, &ed.project.config).into_iter().map(|(c, n)| (c.clone(), format!("{c} {n}"))));
        let current = options.iter().find(|(c, _)| *c == ed.play_lang).map_or_else(|| ed.play_lang.clone(), |(_, n)| n.clone());
        egui::ComboBox::from_id_salt("play_lang").selected_text(current).show_ui(ui, |ui| {
            for (c, n) in options {
                ui.selectable_value(&mut ed.play_lang, c, n);
            }
        });
        ui.end_row();
    });
    if let Some(m) = &ed.lang_msg {
        ui.weak(m);
    }
    if let Some(k) = remove {
        ed.project.config.languages.remove(k);
    }
    if let Some(code) = export {
        let r = ed.export_translation(&code);
        ed.lang_msg = Some(r.unwrap_or_else(|e| format!("Not exported: {e}")));
    }
}

/// A 3D character model: its files, height, the clip each animation plays (with a preview), its
/// colour features (picked from its texture) and whether players may choose it.
/// The *Rig template* section of an unrigged model: the template, the turn, and the fitted
/// skeleton from the side and the front, its joints dragged to where they belong.
fn rig_template_ui(ui: &mut egui::Ui, d: &mut rustpg2d_engine::defs::ModelDef, cx: &mut Cx) {
    use rustpg2d_engine::defs::JointNudge;
    ui.separator();
    ui.heading("Rig template");
    ui.weak("This model has no skeleton of its own (Meshy rigs only people). A rig template lends it one: its bones are stretched over the model, and the template's walk, idle, attack and hurt play. The model must face the front (toward the viewer in the preview's down view): turn it if it walks sideways.");
    let current = d.rig.as_ref().map(|r| r.template.clone()).unwrap_or_default();
    let mut pick = current.clone();
    let templates: Vec<(String, String)> = cx.db.models.iter().filter(|m| m.id != d.id && cx.assets.models.info(&m.id).is_some_and(|i| i.rigged)).map(|m| (m.id.clone(), m.label().to_string())).collect();
    egui::Grid::new("rig_template").num_columns(2).show(ui, |ui| {
        ui.label("Template");
        egui::ComboBox::from_id_salt("rig_template_pick").width(220.0).selected_text(if pick.is_empty() { "(none)".to_string() } else { cx.db.model(&pick).map_or(pick.clone(), |m| m.label().to_string()) }).show_ui(ui, |ui| {
            ui.selectable_value(&mut pick, String::new(), "(none)");
            for (id, label) in &templates {
                ui.selectable_value(&mut pick, id.clone(), label);
            }
        });
        ui.end_row();
        if let Some(rig) = &mut d.rig {
            ui.label("Turn");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut rig.turn).range(-180.0..=180.0).suffix(" deg")).on_hover_text("Turns the model about its up axis before the fit");
                if ui.small_button(ui::t(ph::ARROW_CLOCKWISE, "90")).on_hover_text("A quarter turn").clicked() {
                    rig.turn = if rig.turn + 90.0 > 180.0 { rig.turn - 270.0 } else { rig.turn + 90.0 };
                }
            });
            ui.end_row();
        }
    });
    if pick != current {
        crate::models_tab::choose_template(d, cx.db, &pick);
        MODEL_RELOAD.with(|r| r.set(true));
        return;
    }
    let Some(rig) = &mut d.rig else { return };
    let Some(view) = cx.assets.models.rig_view(&d.id) else { return };
    let height = (view.hi.y - view.lo.y).max(1e-5);
    let did = egui::Id::new(("rig_drag", &d.id));
    let mut mirror: bool = ui.data_mut(|m| m.get_temp(did.with("mirror")).unwrap_or(true));
    let mut dragged: Option<usize> = ui.data_mut(|m| m.get_temp(did));
    let mut moved: Option<(usize, macroquad::math::Vec3)> = None;
    ui.horizontal(|ui| {
        // (horizontal axis of the view as a model axis and its sign, title)
        for (side, title) in [(true, "Side (front to the left)"), (false, "Front")] {
            ui.vertical(|ui| {
                ui.weak(title);
                let (r, resp) = ui.allocate_exact_size(egui::vec2(240.0, 200.0), egui::Sense::click_and_drag());
                let p = ui.painter_at(r);
                p.rect_filled(r, 4.0, egui::Color32::from_gray(32));
                let across = |v: macroquad::math::Vec3| if side { -v.z } else { v.x };
                let (a0, a1) = (across(view.lo).min(across(view.hi)), across(view.lo).max(across(view.hi)));
                let k = ((r.width() - 24.0) / (a1 - a0).max(1e-5)).min((r.height() - 24.0) / height);
                let mid = (a0 + a1) / 2.0;
                let to_screen = |v: macroquad::math::Vec3| egui::pos2(r.center().x + (across(v) - mid) * k, r.bottom() - 12.0 - (v.y - view.lo.y) * k);
                for q in &view.points {
                    let s = to_screen(*q);
                    p.rect_filled(egui::Rect::from_center_size(s, egui::vec2(1.5, 1.5)), 0.0, egui::Color32::from_gray(120));
                }
                for (_, pos, parent) in &view.joints {
                    if let Some(pp) = parent {
                        p.line_segment([to_screen(view.joints[*pp].1), to_screen(*pos)], egui::Stroke::new(1.5f32, egui::Color32::from_rgb(230, 190, 90)));
                    }
                }
                let hover = resp.hover_pos().and_then(|m| view.joints.iter().enumerate().map(|(i, j)| (i, to_screen(j.1).distance(m))).filter(|(_, d)| *d < 9.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i));
                for (i, (_, pos, _)) in view.joints.iter().enumerate() {
                    let hot = Some(i) == hover || Some(i) == dragged;
                    p.circle_filled(to_screen(*pos), if hot { 5.0 } else { 3.5 }, if hot { egui::Color32::from_rgb(255, 120, 80) } else { egui::Color32::from_rgb(240, 210, 120) });
                }
                if let Some(i) = hover.or(dragged) {
                    p.text(r.left_top() + egui::vec2(6.0, 4.0), egui::Align2::LEFT_TOP, &view.joints[i].0, egui::FontId::proportional(13.0), egui::Color32::WHITE);
                }
                if resp.drag_started() {
                    dragged = hover;
                }
                if let (Some(i), true) = (dragged, resp.dragged()) {
                    let dlt = resp.drag_delta() / k;
                    let v = if side { macroquad::math::Vec3::new(0.0, -dlt.y, -dlt.x) } else { macroquad::math::Vec3::new(dlt.x, -dlt.y, 0.0) };
                    moved = Some((i, v / height));
                }
                if resp.drag_stopped() {
                    dragged = None;
                }
            });
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut mirror, "Mirror left and right").on_hover_text("Moving a joint ending in _L also moves its _R twin (and back)");
        if ui.add_enabled(!rig.joints.is_empty(), egui::Button::new(ui::t(ph::ARROW_COUNTER_CLOCKWISE, "Reset joints"))).on_hover_text("Back to the template's joints, stretched over the model").clicked() {
            rig.joints.clear();
        }
        ui.weak("Drag a joint (the dots) to where it belongs.");
    });
    if let Some((i, v)) = moved {
        let mut nudge = |name: &str, v: macroquad::math::Vec3| match rig.joints.iter_mut().find(|j| j.joint == name) {
            Some(j) => j.offset = (macroquad::math::Vec3::from(j.offset) + v).to_array(),
            None => rig.joints.push(JointNudge { joint: name.to_string(), offset: v.to_array() }),
        };
        let name = view.joints[i].0.clone();
        nudge(&name, v);
        let twin = name.strip_suffix("_L").map(|b| format!("{b}_R")).or_else(|| name.strip_suffix("_R").map(|b| format!("{b}_L")));
        if let Some(t) = twin.filter(|t| mirror && view.joints.iter().any(|j| j.0 == *t)) {
            nudge(&t, macroquad::math::Vec3::new(-v.x, v.y, v.z));
        }
    }
    ui.data_mut(|m| {
        m.insert_temp(did.with("mirror"), mirror);
        match dragged {
            Some(i) => m.insert_temp(did, i),
            None => m.remove::<usize>(did),
        }
    });
}

fn model_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::defs::ModelDef, cx: &mut Cx) {
    use rustpg2d_engine::defs::{Appearance, ClipMap};
    use rustpg2d_engine::entity::Facing;
    use rustpg2d_engine::render::model::{Gear, PoseReq};
    ui.heading("3D model");
    rename_row(ui, RefKind::Model, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    egui::Grid::new("model").num_columns(2).show(ui, |ui| {
        ui.label("File");
        ui.horizontal(|ui| {
            ui.monospace(if d.file.is_empty() { "(none)" } else { d.file.as_str() });
            if ui.small_button("Choose…").on_hover_text("A rigged .glb (copied into models/)").clicked() {
                *cx.browse = Some(Purpose::ModelFile { id: d.id.clone(), clips: false });
            }
        });
        ui.end_row();
        ui.label("Clip files");
        ui.vertical(|ui| {
            let mut drop = None;
            for (k, f) in d.clip_files.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.monospace(f);
                    if ui.small_button(ui::t(ph::TRASH, "")).on_hover_text("Stop using this file's clips").clicked() {
                        drop = Some(k);
                    }
                });
            }
            if let Some(k) = drop {
                d.clip_files.remove(k);
            }
            if ui.small_button(ui::t(ph::PLUS, "Add…")).on_hover_text("A .glb with more clips of the same rig (Meshy writes animations as separate files)").clicked() {
                *cx.browse = Some(Purpose::ModelFile { id: d.id.clone(), clips: true });
            }
        });
        ui.end_row();
        ui.label("Height");
        ui.add(egui::DragValue::new(&mut d.height).range(8.0..=256.0).suffix(" px")).on_hover_text("Standing height in world pixels (a person: 40; a tile is 32)");
        ui.end_row();
        ui.label("Character creator");
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.playable, "Playable").on_hover_text("Players may choose it in the character creator");
            ui.checkbox(&mut d.female, "Female").on_hover_text("Offered to female player characters");
        });
        ui.end_row();
    });
    let info = cx.assets.models.info(&d.id);
    match &info {
        Some(i) => {
            ui.weak(format!(
                "{} triangles, {} bones, {} clips{}{}",
                i.triangles,
                i.bones.len(),
                i.clips.len(),
                i.texture.map_or(" - no texture".to_string(), |(w, h)| format!(", texture {w} x {h}")),
                if i.hands { "" } else { " - no hand bones: it can't hold things" }
            ));
            if i.triangles > 8000 {
                ui.colored_label(egui::Color32::YELLOW, "Many triangles: it renders fine, but files and the web game grow. About 5000 is plenty for a character this small.");
            }
        }
        None => {
            let err = cx.assets.models.errors.iter().find(|e| e.contains(&format!("model {}:", d.id))).cloned();
            ui.colored_label(egui::Color32::LIGHT_RED, err.unwrap_or_else(|| "Not loaded yet: choose a file (or Reload assets).".into()));
        }
    }
    if info.as_ref().is_some_and(|i| !i.rigged) || d.rig.is_some() {
        rig_template_ui(ui, d, cx);
    }
    // The renderer takes the edited definition at once.
    cx.assets.models.set_def(d);
    // Animations: the clip each plays.
    let clips: Vec<(String, f32)> = info.as_ref().map(|i| i.clips.clone()).unwrap_or_default();
    ui.separator();
    ui.heading("Animations");
    ui.weak("The clip each animation plays (12 frames a second unless set). Characters without a clip for an animation play idle; walking, idle, attacks and being hit are used by everyone, sitting and sleeping on furniture, the rest by activities.");
    egui::Grid::new("model_clips").num_columns(3).show(ui, |ui| {
        for anim in rustpg2d_engine::model::ANIMS {
            ui.label(anim);
            let cur = d.clip_of(anim).map(|c| c.clip.clone()).unwrap_or_default();
            let mut pick = cur.clone();
            egui::ComboBox::from_id_salt(("model_clip", anim)).width(220.0).selected_text(if pick.is_empty() { "(none: idle)".to_string() } else { pick.clone() }).show_ui(ui, |ui| {
                ui.selectable_value(&mut pick, String::new(), "(none: idle)");
                for (c, secs) in &clips {
                    ui.selectable_value(&mut pick, c.clone(), format!("{c}  ({secs:.1} s)"));
                }
            });
            if pick != cur {
                d.clips.retain(|c| c.anim != anim);
                if !pick.is_empty() {
                    d.clips.push(ClipMap { anim: anim.into(), clip: pick, fps: 12.0 });
                }
            }
            match d.clips.iter_mut().find(|c| c.anim == anim) {
                Some(c) => {
                    ui.add(egui::DragValue::new(&mut c.fps).range(1.0..=30.0).suffix(" fps"));
                }
                None => {
                    ui.label("");
                }
            }
            ui.end_row();
        }
    });
    // Preview: turning (or one facing), playing the chosen animation, and the four facings.
    ui.separator();
    let pid = egui::Id::new(("model_preview", &d.id));
    let (mut anim_k, mut facing_k, mut turning): (usize, usize, bool) = ui.data_mut(|m| m.get_temp(pid).unwrap_or((0, 0, true)));
    let t = macroquad::time::get_time();
    let facings = [Facing::Down, Facing::Left, Facing::Up, Facing::Right];
    let facing = if turning { facings[(t / 1.5) as usize % 4] } else { facings[facing_k % 4] };
    let anim = rustpg2d_engine::model::ANIMS[anim_k % rustpg2d_engine::model::ANIMS.len()];
    let look = Appearance { model: d.id.clone(), ..Default::default() };
    let gear = Gear::default();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(150.0, 180.0), egui::Sense::hover());
        ui.painter().rect_filled(r, 4.0, egui::Color32::from_gray(40));
        let req = PoseReq { look: &look, gear: &gear, anim, time: t as f32, once: false, facing, turn: 0, bust: false };
        if !widgets::paint_model(ui, cx.assets, cx.db, &req, r.shrink(4.0)) {
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "no model", egui::FontId::default(), egui::Color32::LIGHT_RED);
        }
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, a) in rustpg2d_engine::model::ANIMS.iter().enumerate() {
                    if (k < 4 || d.clip_of(a).is_some()) && ui.selectable_label(anim_k == k, *a).clicked() {
                        anim_k = k;
                    }
                }
            });
            ui.horizontal(|ui| {
                for (k, label) in [(0, ph::ARROW_DOWN), (1, ph::ARROW_LEFT), (2, ph::ARROW_UP), (3, ph::ARROW_RIGHT)] {
                    if ui.selectable_label(!turning && facing_k == k, label).clicked() {
                        facing_k = k;
                        turning = false;
                    }
                }
                ui.toggle_value(&mut turning, ph::ARROWS_CLOCKWISE).on_hover_text("Turn around");
            });
            ui.horizontal(|ui| {
                for f in facings {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(48.0, 64.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 3.0, egui::Color32::from_gray(36));
                    let req = PoseReq { look: &look, gear: &gear, anim: "idle", time: 0.0, once: false, facing: f, turn: 0, bust: false };
                    widgets::paint_model(ui, cx.assets, cx.db, &req, r.shrink(2.0));
                }
                let (r, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 3.0, egui::Color32::from_gray(36));
                let req = PoseReq { look: &look, gear: &gear, anim: "idle", time: 0.0, once: true, facing: Facing::Down, turn: 0, bust: true };
                widgets::paint_model(ui, cx.assets, cx.db, &req, r.shrink(2.0));
            });
            ui.weak("The four facings standing, and its portrait (dialogs).");
        });
    });
    ui.data_mut(|m| m.insert_temp(pid, (anim_k, facing_k, turning)));
    ui.ctx().request_repaint();
    // Colour features on its texture.
    ui.separator();
    ui.heading(format!("Colour features ({})", d.features.len()));
    ui.weak("Parts that can take other colours (skin, hair, clothing…): pick the shades of each part from the model's texture; characters and the character creator replace them by a chosen colour, keeping light and shadow.");
    let image = cx.assets.models.texture_image(&d.id).map(|t| image_colors_of(&format!("model:{}", d.id), &t.bytes)).unwrap_or_default();
    let sel = features_editor(ui, &format!("model:{}", d.id), &mut d.features, &image);
    if let Some(f) = d.features.get(sel).filter(|f| !f.shades.is_empty()) {
        cx.assets.models.set_def(d);
        let f = f.clone();
        ui.horizontal_wrapped(|ui| {
            for c in std::iter::once(None).chain(f.colors.iter().take(8).map(|c| Some(*c))) {
                let mut look = Appearance { model: d.id.clone(), ..Default::default() };
                if let Some(c) = c {
                    look.colors.insert(f.name.clone(), c);
                }
                let (r, _) = ui.allocate_exact_size(egui::vec2(48.0, 64.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 3.0, egui::Color32::from_gray(36));
                let req = PoseReq { look: &look, gear: &gear, anim: "idle", time: 0.0, once: false, facing: Facing::Down, turn: 0, bust: false };
                widgets::paint_model(ui, cx.assets, cx.db, &req, r.shrink(2.0));
            }
        });
    }
}

thread_local! {
    /// *Render icon* asked for this item (handled after the form: it needs the whole editor).
    static ICON_REQUEST: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The item whose icon the form asked to render from its model (once).
pub fn take_icon_request() -> Option<String> {
    ICON_REQUEST.with(|r| r.borrow_mut().take())
}

thread_local! {
    /// The item form's preview: animation, playing, the model holding it ("" = the first
    /// playable one).
    static HOLD_PREVIEW: std::cell::RefCell<(&'static str, bool, String)> = const { std::cell::RefCell::new(("idle", false, String::new())) };
}

/// Sets the item form's preview (`itempreview` script command): `anim` idle / walk / attack,
/// playing or still, held by `model` ("" = the first playable model).
pub fn set_hold_preview(anim: &str, play: bool, model: &str) {
    let anim = ["idle", "walk", "attack"].into_iter().find(|a| *a == anim).unwrap_or("idle");
    HOLD_PREVIEW.with(|p| *p.borrow_mut() = (anim, play, model.to_string()));
}

/// A held item's 3D model (`ItemDef.model`): the file, how it is held (style, length, how far
/// along), buttons that turn it, and a big preview in a character's hands; the numbers behind
/// them folded away.
fn item_model_ui(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    use crate::models_tab::{hold_fraction, hold_op, HoldOp};
    use rustpg2d_engine::defs::{Appearance, HoldStyle};
    use rustpg2d_engine::entity::Facing;
    use rustpg2d_engine::render::model::{Gear, PoseReq};
    ui.heading("3D model");
    let before = d.model.clone();
    // The item without its model: what its kind says about holding it.
    let kind = ItemDef { model: None, on_use: Vec::new(), ..d.clone() };
    let Some(m) = &mut d.model else {
        ui.weak("Characters hold the item's 3D model; without one, its picture (below).");
        if ui.button(ui::t(ph::CUBE, "Choose a model…")).on_hover_text("A .glb of the item (copied into models/); how it is held is set from the item's kind and its shape").clicked() {
            *cx.browse = Some(Purpose::ItemModel(d.id.clone()));
        }
        return;
    };
    ui.weak("Pick how it is held, then turn it with the buttons until it sits right in the preview.");
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.monospace(&m.file);
        if ui.small_button("Choose…").clicked() {
            *cx.browse = Some(Purpose::ItemModel(d.id.clone()));
        }
        if ui.small_button(ui::t(ph::TRASH, "Remove")).clicked() {
            remove = true;
        }
    });
    let points = cx.assets.models.item_points(&m.file);
    if points.is_none() {
        let err = cx.assets.models.errors.iter().find(|e| e.contains(&m.file)).cloned();
        ui.colored_label(egui::Color32::LIGHT_RED, err.unwrap_or_else(|| "Not loaded (Reload assets).".into()));
    }
    let pts = points.map(|p| p.0).unwrap_or_default();
    let indices = cx.assets.models.item_indices(&m.file);
    let mut op = None;
    let style_help = |s: HoldStyle| match s {
        HoldStyle::Grip => "In the fist: the handle through it, the front (blade's edge, axe head) where the knuckles point. Swords, axes, tools, bows; two-handed along both fists.",
        HoldStyle::Upright => "Held up, the handle pointing up whatever the arm does: torches, staffs.",
        HoldStyle::Hang => "Hanging from the hand by its top: lanterns, buckets.",
        HoldStyle::Strap => "On the forearm, its face turned out and forward, its top up: shields.",
    };
    egui::Grid::new("item_model").num_columns(2).show(ui, |ui| {
        ui.label("Held as");
        ui.horizontal(|ui| {
            for s in HoldStyle::ALL {
                if ui.selectable_label(m.hold == s, s.label()).on_hover_text(style_help(s)).clicked() && m.hold != s {
                    op = Some(HoldOp::Style(s));
                }
            }
        });
        ui.end_row();
        ui.label("Length");
        ui.add(egui::Slider::new(&mut m.size, 4.0..=80.0).suffix(" px").step_by(1.0))
            .on_hover_text("Its whole length in the world: a character is about 40 tall (a dagger 14, a sword 24, a spear 44)");
        ui.end_row();
        ui.label("Hold at");
        ui.horizontal(|ui| {
            let (mut t, _, _) = hold_fraction(m, &pts);
            ui.weak("handle end");
            if ui.add(egui::Slider::new(&mut t, 0.0..=1.0).show_value(false)).on_hover_text("Where along its length the hand holds it").changed() {
                op = Some(HoldOp::At(t));
            }
            ui.weak(if m.hold == HoldStyle::Hang { "bottom" } else { "far end" });
        });
        ui.end_row();
    });
    ui.horizontal_wrapped(|ui| {
        let on = !pts.is_empty();
        let mut button = |ui: &mut egui::Ui, icon: &str, text: &str, tip: &str, o: HoldOp| {
            if ui.add_enabled(on, egui::Button::new(ui::t(icon, text))).on_hover_text(tip).clicked() {
                op = Some(o);
            }
        };
        button(ui, ph::MAGIC_WAND, "Estimate", "Hold point, handle and front guessed from its shape again (for this way of holding)", HoldOp::Estimate);
        button(ui, ph::ARROW_CLOCKWISE, "Turn handle", "The handle runs along the model's next axis: press until it lies in the hand", HoldOp::TurnHandle);
        button(ui, ph::ARROWS_CLOCKWISE, "Turn front", "A quarter turn about the handle: the blade's edge, the axe head or the shield's face points another way", HoldOp::TurnFront);
        button(ui, ph::ARROWS_DOWN_UP, "Flip ends", "The other end is the handle", HoldOp::Flip);
        if ui.add_enabled(on, egui::Button::new(ui::t(ph::IMAGE, "Render icon"))).on_hover_text("Its inventory icon drawn from the model (into the icons_3d sheet)").clicked() {
            ICON_REQUEST.with(|r| *r.borrow_mut() = Some(d.id.clone()));
        }
    });
    if let Some(o) = op {
        let at = crate::models_tab::kind_at(&kind, &pts);
        hold_op(m, &o, &pts, &indices, at);
    }
    // The preview: four facings, an animation, the model holding it.
    let (mut anim, mut play, mut holder) = HOLD_PREVIEW.with(|p| p.borrow().clone());
    let models: Vec<(String, String)> = cx.db.models.iter().filter(|m| m.playable).chain(cx.db.models.iter().filter(|m| !m.playable)).filter(|m| m.rig.is_some() || cx.assets.models.has(&m.id)).map(|m| (m.id.clone(), m.name.clone())).collect();
    if !models.iter().any(|(id, _)| *id == holder) {
        holder = models.first().map(|m| m.0.clone()).unwrap_or_default();
    }
    if holder.is_empty() {
        ui.weak("(no 3D character model to preview it with)");
    } else {
        ui.horizontal(|ui| {
            ui.label("Preview");
            for a in ["idle", "walk", "attack"] {
                ui.selectable_value(&mut anim, a, match a {
                    "idle" => "Standing",
                    "walk" => "Walking",
                    _ => "Attacking",
                });
            }
            ui.checkbox(&mut play, "Play");
            ui.label("held by");
            let name = models.iter().find(|m| m.0 == holder).map(|m| m.1.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("item_hold_by").selected_text(name).height(300.0).show_ui(ui, |ui| {
                for (id, name) in &models {
                    ui.selectable_value(&mut holder, id.clone(), name);
                }
            });
        });
        HOLD_PREVIEW.with(|p| *p.borrow_mut() = (anim, play, holder.clone()));
        let look = Appearance { model: holder, ..Default::default() };
        let offhand = d.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Offhand);
        let gear = if offhand { Gear { offhand: Some(d.id.clone()), ..Default::default() } } else { Gear { weapon: Some(d.id.clone()), ..Default::default() } };
        let clock = ui.input(|i| i.time) as f32;
        let time = match (play, anim) {
            (true, "attack") => (clock * 0.9).fract(),
            (true, _) => clock,
            (false, "attack") => 0.42,
            (false, "walk") => 0.3,
            _ => 0.0,
        };
        if play {
            ui.ctx().request_repaint();
        }
        ui.horizontal(|ui| {
            for facing in [Facing::Down, Facing::Left, Facing::Right, Facing::Up] {
                let (r, _) = ui.allocate_exact_size(egui::vec2(170.0, 200.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 3.0, egui::Color32::from_rgb(58, 74, 52));
                let req = PoseReq { look: &look, gear: &gear, anim, time, once: anim == "attack", facing, turn: 0, bust: false };
                widgets::paint_model_whole(ui, cx.assets, cx.db, &req, r.shrink(4.0));
            }
        });
    }
    egui::CollapsingHeader::new("Numbers").id_salt("item_model_numbers").show(ui, |ui| {
        ui.weak("In the model's own axes: the hold point, the direction from it toward the far end, and the front (0 0 0: the wider side across the handle).");
        egui::Grid::new("item_model_numbers_grid").num_columns(2).show(ui, |ui| {
            for (label, v, speed) in [("Hold point", &mut m.grip, 0.005), ("Handle toward", &mut m.axis, 0.01), ("Front", &mut m.front, 0.01)] {
                ui.label(label);
                ui.horizontal(|ui| {
                    for (k, n) in ["x", "y", "z"].iter().enumerate() {
                        ui.add(egui::DragValue::new(&mut v[k]).speed(speed).prefix(format!("{n} ")));
                    }
                });
                ui.end_row();
            }
        });
    });
    if remove {
        d.model = None;
    }
    if d.model != before {
        cx.assets.models.clear_poses();
    }
}
