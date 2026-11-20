//! Definition editors: items, props, characters, materials, sprite sheets, sounds, project and scripts.

use egui_macroquad::egui::{self, vec2, Color32, Sense};

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
    Sheets,
    Sounds,
    Quests,
    Project,
    Scripts,
    Structures,
    Spells,
    Recipes,
    Achievements,
    Races,
    Models,
    Materials,
    Scatter,
    Fonts,
    Skins,
    CommonEvents,
    Backgrounds,
    Factions,
    Classes,
}

/// How many definitions the list of `tab` shows (0 for the tabs without a list).
pub fn tab_len(db: &rustpg2d_engine::defs::Database, tab: Tab) -> usize {
    match tab {
        Tab::Items => db.items.len(),
        Tab::Props => db.props.len(),
        Tab::Characters => db.characters.len(),
        Tab::Sheets => db.sheets.len(),
        Tab::Sounds => db.sounds.len(),
        Tab::Quests => db.quests.len(),
        Tab::Structures => db.structures.len(),
        Tab::Spells => db.spells.len(),
        Tab::Recipes => db.recipes.len(),
        Tab::Achievements => db.achievements.len(),
        Tab::Races => db.races.len(),
        Tab::Models => db.models.len(),
        Tab::Materials => db.materials.len(),
        Tab::Scatter => db.scatter.len(),
        Tab::Fonts => db.fonts.len(),
        Tab::Skins => db.skins.len(),
        Tab::CommonEvents => db.common_events.len(),
        Tab::Backgrounds => db.backgrounds.len(),
        Tab::Classes => db.classes.len(),
        Tab::Project | Tab::Scripts | Tab::Factions => 0,
    }
}

thread_local! {
    /// The Database list's search text (`DbState::search` while the list draws).
    static LIST_SEARCH: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// What the list showed of how many.
    static LIST_SHOWN: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    /// *Find usages* asked for in a definition list (the id; the open tab gives the kind).
    static USAGES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The id whose usages the Database list asked for (once).
pub fn take_usages_request() -> Option<String> {
    USAGES.with(|u| u.borrow_mut().take())
}

thread_local! {
    /// A link in a form asked to open another definition (kind, id).
    static JUMP: std::cell::RefCell<Option<(RefKind, String)>> = const { std::cell::RefCell::new(None) };
}

/// Opens definition `id` of `kind` after this frame's form (links between forms).
pub fn request_jump(kind: RefKind, id: &str) {
    JUMP.with(|j| *j.borrow_mut() = Some((kind, id.to_string())));
}

impl Tab {
    /// The kind of definitions the tab edits (`None` for Project and Scripts).
    pub fn kind(self) -> Option<RefKind> {
        Some(match self {
            Tab::Items => RefKind::Item,
            Tab::Props => RefKind::Prop,
            Tab::Characters => RefKind::Character,
            Tab::Sheets => RefKind::Sheet,
            Tab::Sounds => RefKind::Sound,
            Tab::Quests => RefKind::Quest,
            Tab::Structures => RefKind::Structure,
            Tab::Spells => RefKind::Spell,
            Tab::Recipes => RefKind::Recipe,
            Tab::Achievements => RefKind::Achievement,
            Tab::Races => RefKind::Race,
            Tab::Models => RefKind::Model,
            Tab::Materials => RefKind::Material,
            Tab::Scatter => RefKind::Scatter,
            Tab::Fonts => RefKind::Font,
            Tab::Skins => RefKind::Skin,
            Tab::CommonEvents => RefKind::CommonEvent,
            Tab::Backgrounds => RefKind::Background,
            Tab::Classes => RefKind::Class,
            Tab::Project | Tab::Scripts | Tab::Factions => return None,
        })
    }

    /// The tab that edits definitions of `kind` (`None` for maps).
    pub fn of(kind: RefKind) -> Option<Tab> {
        Some(match kind {
            RefKind::Item => Tab::Items,
            RefKind::Prop => Tab::Props,
            RefKind::Character => Tab::Characters,
            RefKind::Sheet => Tab::Sheets,
            RefKind::Sound => Tab::Sounds,
            RefKind::Quest => Tab::Quests,
            RefKind::Structure => Tab::Structures,
            RefKind::Spell => Tab::Spells,
            RefKind::Recipe => Tab::Recipes,
            RefKind::Achievement => Tab::Achievements,
            RefKind::Race => Tab::Races,
            RefKind::Model => Tab::Models,
            RefKind::Material => Tab::Materials,
            RefKind::Scatter => Tab::Scatter,
            RefKind::Font => Tab::Fonts,
            RefKind::Skin => Tab::Skins,
            RefKind::CommonEvent => Tab::CommonEvents,
            RefKind::Background => Tab::Backgrounds,
            RefKind::Class => Tab::Classes,
            RefKind::Map | RefKind::Skill => return None,
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
    /// The list's search text (name or id; cleared with another tab).
    pub search: String,
    /// The Items tab's category filter.
    pub item_filter: crate::item_category::ItemFilter,
    /// How many definitions the list shows of how many (status `dblist=`).
    pub list_shown: (usize, usize),
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
    /// Prop editor: selected state (its sprite is edited below the list) and an open rename.
    pub state_sel: usize,
    pub state_rename: Option<(usize, String)>,
    /// Items / props / characters: the pixel editor is shown under the form.
    pub art_open: bool,
    /// What the embedded editor last focused (to select its frame once).
    pub art_focus: Option<(String, u32)>,
    /// Editor script `dbscroll Y`: scroll the definition form there once.
    pub form_scroll: Option<f32>,
    /// The structure editor (Structures tab).
    pub st_edit: crate::structures::StructEdit,
    /// The material form's preview (Materials tab).
    pub mat_preview: crate::material_preview::MaterialPreview,
    pub skin_preview: crate::skin_preview::SkinPreview,
}

/// Everything the definition forms need besides the definition itself.
struct Cx<'a> {
    tex: &'a Textures,
    assets: &'a mut Assets,
    refs: &'a Refs,
    audio: &'a mut Audio,
    rename: &'a mut RenameUi,
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
    mat_preview: &'a mut crate::material_preview::MaterialPreview,
    /// The picture open in the pixel editor: (path, pixels, change counter).
    drawing: Option<(&'a str, &'a rustpg2d_engine::imageio::Rgba, u64)>,
    /// The project folder (font previews read the font files).
    root: &'a std::path::Path,
    skin_preview: &'a mut crate::skin_preview::SkinPreview,
    /// The project's font roles (the skin preview draws with them).
    fonts: &'a rustpg2d_engine::fonts::FontRoles,
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
        Tab::Sheets => ph::IMAGE,
        Tab::Sounds => ph::SPEAKER_HIGH,
        Tab::Structures => ph::HOUSE,
        Tab::Quests => ph::SCROLL,
        Tab::Spells => ph::MAGIC_WAND,
        Tab::Recipes => ph::HAMMER,
        Tab::Achievements => ph::TROPHY,
        Tab::Races => ph::USERS_THREE,
        Tab::Models => ph::CUBE,
        Tab::Materials => ph::PAINT_BRUSH,
        Tab::Scatter => ph::TREE_EVERGREEN,
        Tab::Fonts => ph::TEXT_AA,
        Tab::Skins => ph::FRAME_CORNERS,
        Tab::CommonEvents => ph::LIGHTNING,
        Tab::Backgrounds => ph::IDENTIFICATION_CARD,
        Tab::Classes => ph::SWORD,
        Tab::Factions => ph::FLAG,
        Tab::Project => ph::GEAR,
        Tab::Scripts => ph::CODE,
    }
}

/// List with add / duplicate / delete. Returns true if the list changed.
fn list_ui<T: Clone>(
    ui: &mut egui::Ui,
    items: &mut Vec<T>,
    sel: &mut usize,
    keep: impl Fn(&T) -> bool,
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
    // The search (name or id) and the tab's own filter.
    let shown: Vec<usize> = LIST_SEARCH.with(|q| {
        let mut q = q.borrow_mut();
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.add(egui::TextEdit::singleline(&mut *q).hint_text("search").desired_width(f32::INFINITY));
        });
        let needle = q.trim().to_lowercase();
        (0..items.len()).filter(|&i| keep(&items[i]) && (needle.is_empty() || label(&items[i]).to_lowercase().contains(&needle))).collect()
    });
    LIST_SHOWN.with(|n| n.set((shown.len(), items.len())));
    // (The selection follows the search and filter: the first match when it is hidden.)
    if shown.len() < items.len() && !shown.is_empty() && !shown.contains(sel) {
        *sel = shown[0];
    }
    if shown.len() < items.len() {
        ui.weak(format!("{} of {} shown", shown.len(), items.len()));
    }
    ui.separator();
    egui::ScrollArea::vertical().id_salt("db_list").auto_shrink(false).show(ui, |ui| {
        for &i in &shown {
            let it = &items[i];
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
    if library && matches!(ed.db.tab, Tab::Quests | Tab::Achievements | Tab::Project | Tab::Scripts | Tab::Factions) {
        ed.db.tab = Tab::Items;
    }
    // Wraps onto a second line in narrow windows (instead of widening the forms below).
    ui.horizontal_wrapped(|ui| {
        let tabs = [
            (Tab::Items, "Items"),
            (Tab::Props, "Props"),
            (Tab::Characters, "Characters"),
            (Tab::Races, "Races"),
            (Tab::Backgrounds, "Backgrounds"),
            (Tab::Classes, "Classes"),
            (Tab::Factions, "Factions"),
            (Tab::Models, "3D models"),
            (Tab::Materials, "Materials"),
            (Tab::Scatter, "Scatter sets"),
            (Tab::Sheets, "Sprite sheets"),
            (Tab::Sounds, "Sounds"),
            (Tab::Fonts, "Fonts"),
            (Tab::Skins, "UI skins"),
            (Tab::Structures, "Structures"),
            (Tab::Spells, "Spells"),
            (Tab::Recipes, "Recipes"),
            (Tab::Quests, "Quests"),
            (Tab::Achievements, "Achievements"),
            (Tab::CommonEvents, "Common events"),
            (Tab::Project, "Project"),
            (Tab::Scripts, "Scripts"),
        ];
        for (t, l) in tabs {
            if library && matches!(t, Tab::Quests | Tab::Achievements | Tab::Project | Tab::Scripts | Tab::Factions) {
                continue; // the library only holds assets
            }
            if ui.selectable_label(ed.db.tab == t, ui::t(tab_icon(t), l)).clicked() && ed.db.tab != t {
                ed.db.tab = t;
                ed.db.sel = 0;
                ed.db.search.clear();
                ed.db.rename = RenameUi::default();
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
            Tab::Sheets => db.sheets.get(i).map(|d| (RefKind::Sheet, d.id.clone())),
            Tab::Sounds => db.sounds.get(i).map(|d| (RefKind::Sound, d.id.clone())),
            Tab::Structures => db.structures.get(i).map(|d| (RefKind::Structure, d.id.clone())),
            Tab::Spells => db.spells.get(i).map(|d| (RefKind::Spell, d.id.clone())),
            Tab::Recipes => db.recipes.get(i).map(|d| (RefKind::Recipe, d.id.clone())),
            Tab::Races => db.races.get(i).map(|d| (RefKind::Race, d.id.clone())),
            Tab::Models => db.models.get(i).map(|d| (RefKind::Model, d.id.clone())),
            Tab::Materials => db.materials.get(i).map(|d| (RefKind::Material, d.id.clone())),
            Tab::Scatter => db.scatter.get(i).map(|d| (RefKind::Scatter, d.id.clone())),
            Tab::Fonts => db.fonts.get(i).map(|d| (RefKind::Font, d.id.clone())),
            Tab::Skins => db.skins.get(i).map(|d| (RefKind::Skin, d.id.clone())),
            Tab::CommonEvents => db.common_events.get(i).map(|d| (RefKind::CommonEvent, d.id.clone())),
            Tab::Backgrounds => db.backgrounds.get(i).map(|d| (RefKind::Background, d.id.clone())),
            Tab::Classes => db.classes.get(i).map(|d| (RefKind::Class, d.id.clone())),
            _ => None,
        };
        if !library {
            let kind = match ed.db.tab {
                Tab::Items => Some(RefKind::Item),
                Tab::Props => Some(RefKind::Prop),
                Tab::Characters => Some(RefKind::Character),
                Tab::Sheets => Some(RefKind::Sheet),
                Tab::Sounds => Some(RefKind::Sound),
                Tab::Structures => Some(RefKind::Structure),
                Tab::Spells => Some(RefKind::Spell),
                Tab::Recipes => Some(RefKind::Recipe),
                Tab::Races => Some(RefKind::Race),
                Tab::Models => Some(RefKind::Model),
                Tab::Materials => Some(RefKind::Material),
                Tab::Scatter => Some(RefKind::Scatter),
                Tab::Fonts => Some(RefKind::Font),
                Tab::Skins => Some(RefKind::Skin),
                Tab::CommonEvents => Some(RefKind::CommonEvent),
                Tab::Backgrounds => Some(RefKind::Background),
                Tab::Classes => Some(RefKind::Class),
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
        if ui.button(ui::t(ph::SPARKLE, "New asset…")).on_hover_text("A prop, item, sprite sheet or structure (a wizard asks the details)").clicked() {
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
    if matches!(tab, Tab::Project | Tab::Scripts | Tab::Factions) {
        egui::CentralPanel::default().show_inside(ui, |ui| match tab {
            Tab::Project => project_ui(ed, ui),
            Tab::Factions => factions_ui(ed, ui),
            _ => scripts_ui(ed, ui),
        });
    } else {
        let fresh = crate::builder::default_character(&ed.project.db, "", "New character");
        let db = &mut ed.project.db;
        let sel = &mut ed.db.sel;
        let item_filter = &mut ed.db.item_filter;
        LIST_SEARCH.with(|q| *q.borrow_mut() = std::mem::take(&mut ed.db.search));
        egui::SidePanel::left("db_left").resizable(true).default_width(220.0).show_inside(ui, |ui| {
            list_changed = match tab {
                Tab::Items => {
                    // The SRD's categories.
                    let ammo = crate::item_category::ammo_of(db);
                    let cats: Vec<crate::item_category::ItemCat> = db.items.iter().map(|d| crate::item_category::category(d, &ammo)).collect();
                    crate::item_category::picker(ui, "db_item_filter", item_filter, &cats);
                    let f = *item_filter;
                    list_ui(
                    ui,
                    &mut db.items,
                    sel, |d| f.keeps(crate::item_category::category(d, &ammo)),
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| ItemDef::fresh(&unique("item", ids)),
                    |d| &mut d.id,
                )
                }
                Tab::Props => list_ui(
                    ui,
                    &mut db.props,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| PropDef::fresh(&unique("prop", ids)),
                    |d| &mut d.id,
                ),
                Tab::Characters => list_ui(
                    ui,
                    &mut db.characters,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| CharacterDef { id: unique("character", ids), ..fresh.clone() },
                    |d| &mut d.id,
                ),
                Tab::Sheets => list_ui(
                    ui,
                    &mut db.sheets,
                    sel, |_| true,
                    |d| d.id.clone(),
                    |ids| SheetDef::fresh(&unique("sheet", ids)),
                    |d| &mut d.id,
                ),
                Tab::Sounds => list_ui(
                    ui,
                    &mut db.sounds,
                    sel, |_| true,
                    |d| d.id.clone(),
                    |ids| SoundDef { id: unique("sound", ids), file: "sounds/new.wav".into(), volume: 1.0, caption: String::new() },
                    |d| &mut d.id,
                ),
                Tab::Structures => list_ui(
                    ui,
                    &mut db.structures,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| StructureDef { id: unique("structure", ids), name: "New structure".into(), w: 6, h: 5, ..Default::default() },
                    |d| &mut d.id,
                ),
                Tab::Spells => list_ui(
                    ui,
                    &mut db.spells,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::skills::SpellDef::new(&unique("spell", ids), "New spell", "", SpriteRef::default(), 5, 1.0, rustpg2d_engine::skills::SpellEffect::Heal { amount: 4 }),
                    |d| &mut d.id,
                ),
                Tab::Models => {
                    if ui.button(ui::t(ph::UPLOAD_SIMPLE, "Import model…")).on_hover_text("A rigged .glb (Meshy, Blender, Mixamo…): copied into models/, its clips matched to the animations by name").clicked() {
                        ed.browse_request = Some(Purpose::ModelFile { id: String::new(), clips: false });
                    }
                    list_ui(ui, &mut db.models, sel, |_| true, |d| format!("{}  ({}){}", d.label(), d.id, if d.playable { "  - playable" } else { "" }), |ids| rustpg2d_engine::defs::ModelDef::new(&unique("model", ids), ""), |d| &mut d.id)
                }
                Tab::Races => list_ui(ui, &mut db.races, sel, |_| true, |d| format!("{}  ({}){}", d.name, d.id, if d.playable { "" } else { "  - not playable" }), |ids| rustpg2d_engine::races::RaceDef::fresh(&unique("race", ids)), |d| &mut d.id),
                Tab::Recipes => {
                    // Listed by what they make.
                    let names: std::collections::HashMap<String, String> = db.items.iter().map(|d| (d.id.clone(), d.name.clone())).collect();
                    list_ui(
                        ui,
                        &mut db.recipes,
                        sel, |_| true,
                        |d| {
                            let made = names.get(&d.result).cloned().unwrap_or_else(|| d.result.clone());
                            let at = if d.station.is_empty() { String::new() } else { format!(" @ {}", d.station) };
                            format!("{made}{at}  ({})", d.id)
                        },
                        |ids| rustpg2d_engine::crafting::RecipeDef::new(&unique("recipe", ids), "", 1, Vec::new(), ""),
                        |d| &mut d.id,
                    )
                }
                Tab::Scatter => list_ui(
                    ui,
                    &mut db.scatter,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::decor::ScatterDef::fresh(&unique("scatter", ids)),
                    |d| &mut d.id,
                ),
                Tab::Fonts => list_ui(
                    ui,
                    &mut db.fonts,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::fonts::FontDef::fresh(&unique("font", ids)),
                    |d| &mut d.id,
                ),
                Tab::Skins => list_ui(
                    ui,
                    &mut db.skins,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::skin::SkinDef::fresh(&unique("skin", ids)),
                    |d| &mut d.id,
                ),
                Tab::CommonEvents => list_ui(
                    ui,
                    &mut db.common_events,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::common_events::CommonEventDef::fresh(&unique("common_event", ids)),
                    |d| &mut d.id,
                ),
                Tab::Backgrounds => list_ui(
                    ui,
                    &mut db.backgrounds,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::backgrounds::BackgroundDef::fresh(&unique("background", ids)),
                    |d| &mut d.id,
                ),
                Tab::Classes => list_ui(
                    ui,
                    &mut db.classes,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::classes::ClassDef::fresh(&unique("class", ids)),
                    |d| &mut d.id,
                ),
                Tab::Materials => list_ui(
                    ui,
                    &mut db.materials,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.name, d.id),
                    |ids| rustpg2d_engine::ground::MaterialDef::fresh(&unique("material", ids)),
                    |d| &mut d.id,
                ),
                Tab::Achievements => list_ui(
                    ui,
                    &mut db.achievements,
                    sel, |_| true,
                    |d| format!("{}{}  ({})", d.name, if d.hidden { " (hidden)" } else { "" }, d.id),
                    |ids| rustpg2d_engine::achievements::AchievementDef::new(&unique("achievement", ids), "New achievement", "", SpriteRef::default(), Vec::new()),
                    |d| &mut d.id,
                ),
                Tab::Quests => list_ui(
                    ui,
                    &mut db.quests,
                    sel, |_| true,
                    |d| format!("{}  ({})", d.title, d.id),
                    |ids| rustpg2d_engine::quest::QuestDef { id: unique("quest", ids), title: "New quest".into(), stages: vec!["First objective".into()], ..Default::default() },
                    |d| &mut d.id,
                ),
                _ => false,
            };
        });
        ed.db.search = LIST_SEARCH.with(|q| std::mem::take(&mut *q.borrow_mut()));
        ed.db.list_shown = LIST_SHOWN.with(|n| n.get());
        if ed.db.sel != sel_before {
            ed.db.rename = RenameUi::default();
        }
        let db_snapshot = ed.project.db.clone();
        let mut embed: Option<(String, (u32, u32), crate::pixel::Layout)> = None;
        // A sheet's frame size or scale changed: its textures are cut / drawn anew.
        let mut sheet_changed = false;
        // A material's animation frames changed: (texture, old count, new count).
        let mut refit: Option<(String, u32, u32)> = None;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Sprite sheets: a compact form on top, the pixel editor below it.
            let art_tab = matches!(tab, Tab::Items | Tab::Props | Tab::Characters) && ed.db.art_open;
            let form_height = if tab == Tab::Sheets {
                210.0
            } else if tab == Tab::Materials && ed.project.db.materials.get(ed.db.sel).is_some_and(|m| !m.texture.is_empty()) {
                // The texture is drawn under the form.
                (ui.available_height() * 0.5).max(300.0)
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
                    browse: &mut ed.browse_request,
                    db: &db_snapshot,
                    paint: &mut paint,
                    state_sel: &mut ed.db.state_sel,
                    state_rename: &mut ed.db.state_rename,
                    state_renamed: &mut state_renamed,
                    art_open: &mut ed.db.art_open,
                    art: &mut art,
                    st_edit: &mut ed.db.st_edit,
                    mat_preview: &mut ed.db.mat_preview,
                    drawing: ed.px.doc.as_ref().map(|d| (d.rel.as_str(), &d.img, d.version)),
                    root: &ed.project.root,
                    skin_preview: &mut ed.db.skin_preview,
                    fonts: &ed.project.config.fonts,
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
                    Tab::Sheets => {
                        if let Some(d) = db.sheets.get_mut(i) {
                            let before = (d.frame_w, d.frame_h, d.scale);
                            sheet_form(ui, d, &mut cx);
                            sheet_changed = before != (d.frame_w, d.frame_h, d.scale);
                            embed = Some((d.image.clone(), (d.frame_w, d.frame_h), crate::pixel::Layout::Plain));
                        }
                    }
                    Tab::Sounds => {
                        if let Some(d) = db.sounds.get_mut(i) {
                            sound_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Quests => {
                        if let Some(d) = db.quests.get_mut(i) {
                            crate::quest_form::quest_form(ui, d, cx.refs, cx.db);
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
                    Tab::Materials => {
                        if let Some(d) = db.materials.get_mut(i) {
                            let frames = d.frames;
                            material_form(ui, d, &mut cx);
                            if d.frames != frames {
                                refit = Some((d.texture.clone(), frames.max(1) as u32, d.frames.max(1) as u32));
                            }
                            if !d.texture.is_empty() {
                                embed = Some((d.texture.clone(), (4096, 4096), crate::pixel::Layout::Seamless(d.frames.max(1) as u32)));
                            }
                        }
                    }
                    Tab::Scatter => {
                        if let Some(d) = db.scatter.get_mut(i) {
                            scatter_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Fonts => {
                        if let Some(d) = db.fonts.get_mut(i) {
                            font_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Skins => {
                        if let Some(d) = db.skins.get_mut(i) {
                            skin_form(ui, d, &mut cx);
                        }
                    }
                    Tab::CommonEvents => {
                        if let Some(d) = db.common_events.get_mut(i) {
                            common_event_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Backgrounds => {
                        if let Some(d) = db.backgrounds.get_mut(i) {
                            background_form(ui, d, &mut cx);
                        }
                    }
                    Tab::Classes => {
                        if let Some(d) = db.classes.get_mut(i) {
                            class_form(ui, d, &mut cx);
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
            if let Some((rel, old, new)) = refit.take() {
                ed.px.refit_frames(&rel, old, new);
            }
            if let Some((rel, frame, layout)) = embed.take() {
                ui.separator();
                let h = ui.available_height().max(260.0);
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
    if let Some((kind, id)) = JUMP.with(|j| j.borrow_mut().take()) {
        ed.jump_to(&crate::search::Target::Def(kind, id));
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
pub(crate) fn text_box(ui: &mut egui::Ui, text: &mut String, rows: usize, hint: &str) -> egui::Response {
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
                Weapon::ranged(1, 1.5, Ranged { projectile: SpriteRef::default(), speed: 360.0, range: 6.0, ammo: String::new(), long_range: 0.0 })
            } else {
                Weapon::melee(1, 0.4)
            }
        });
    }
    let Some(w) = w else { return };
    egui::Grid::new(format!("{id}/grid")).num_columns(2).show(ui, |ui| {
        ui.label("Damage dice");
        ui.horizontal(|ui| {
            let hint = Weapon::melee(w.damage, 1.0).damage_dice();
            ui.add(egui::TextEdit::singleline(&mut w.dice).desired_width(70.0).hint_text(hint)).on_hover_text("The SRD's damage dice of the weapon (1d6, 1d8, 2d6); the attacker's STR (melee) or DEX (ranged) modifier is added. Empty = from the old damage value: 1 = 1d4, 2 = 1d6, 3 = 1d8, 4 = 1d10, 5+ = 1d12.");
            widgets::damage_type_combo(ui, &format!("{id}/dt"), &mut w.damage_type, "untyped");
            ui.weak("(the type: resistances halve it, immunities stop it)");
        });
        ui.end_row();
        if !w.dice.trim().is_empty() {
            if let Err(e) = rustpg2d_engine::srd::Expr::parse(&w.dice) {
                ui.label("");
                ui.colored_label(egui::Color32::from_rgb(230, 120, 90), e);
                ui.end_row();
            }
        }
        ui.label("Old damage");
        ui.add(egui::DragValue::new(&mut w.damage).range(0..=999)).on_hover_text("The damage of 0.28 and older (scripted damage outside fights, and the dice when Damage dice is empty)");
        ui.end_row();
        ui.label("Swing time (s)");
        ui.add(egui::DragValue::new(&mut w.cooldown).range(0.05..=10.0).speed(0.05)).on_hover_text("Outside fights: how long a swing at a tree or a vein takes");
        ui.end_row();
        if !character {
            ui.label("Type");
            ui.horizontal(|ui| {
                let mut ranged = w.ranged.is_some();
                ui.radio_value(&mut ranged, false, "Melee (swing)");
                ui.radio_value(&mut ranged, true, "Ranged (projectiles)");
                if ranged != w.ranged.is_some() {
                    w.ranged = ranged.then(|| Ranged { projectile: SpriteRef::default(), speed: 440.0, range: 8.0, ammo: String::new(), long_range: 0.0 });
                }
            });
            ui.end_row();
        }
        if w.ranged.is_none() {
            ui.label("Reach (tiles)");
            ui.add(egui::DragValue::new(&mut w.reach).range(0.3..=6.0).speed(0.05)).on_hover_text("How far the swing reaches outside fights and how it is carried: above 1.4 tiles in both hands like a spear (and never at the hip). In fights a weapon reaches 5 ft, 10 ft with the Reach property");
            ui.end_row();
            ui.label("Arc (degrees)");
            ui.add(egui::DragValue::new(&mut w.arc).range(5.0..=180.0)).on_hover_text("Half-angle of the swing: 20 = narrow thrust, 90 = everything in front");
            ui.end_row();
        }
        ui.label("Knockback");
        ui.add(egui::DragValue::new(&mut w.knockback).range(0.0..=5.0).speed(0.05)).on_hover_text("Multiplier");
        ui.end_row();
        if !character {
            weapon_srd_rows(ui, id, w);
            ui.label("Critical hits");
            let mut keen = w.crit >= 0.1;
            if ui.checkbox(&mut keen, "On a 19 or 20").on_hover_text("A keen weapon: a natural 19 is a critical hit too (else only a 20); a critical hit rolls the damage dice twice").changed() {
                w.crit = if keen { 0.1 } else { 0.0 };
            }
            ui.end_row();
        }
        if let Some(r) = &mut w.ranged {
            ui.label("Speed (px/s)");
            ui.add(egui::DragValue::new(&mut r.speed).range(10.0..=2000.0));
            ui.end_row();
            ui.label("Range (tiles)");
            ui.add(egui::DragValue::new(&mut r.range).range(0.5..=64.0).speed(0.1)).on_hover_text("Normal range (a tile = 5 ft); farther shots up to the long range have disadvantage");
            ui.end_row();
            ui.label("Long range (tiles)");
            ui.add(egui::DragValue::new(&mut r.long_range).range(0.0..=128.0).speed(0.1)).on_hover_text("The farthest a shot reaches, with disadvantage beyond the range (0 = twice the range)");
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
        "A condition (and / or damage every round) for whoever this weapon hits (melee swings and projectiles), e.g. a poisoned dagger: Poisoned, a CON save keeps it off",
        &mut w.effect,
    );
    if let Some(r) = &mut w.ranged {
        ui.label("Projectile sprite (drawn pointing right, rotated in flight):");
        widgets::sprite_picker(ui, &format!("{id}/proj"), &mut r.projectile, cx.tex, cx.assets);
    }
}

/// The SRD row of an item: what SRD item it is, its cost in coins, weight and (money) coin worth.
fn srd_item_ui(ui: &mut egui::Ui, d: &mut ItemDef) {
    ui.horizontal_wrapped(|ui| {
        ui.label("SRD item");
        ui.add(egui::TextEdit::singleline(&mut d.srd).desired_width(110.0).hint_text("none"))
            .on_hover_text("The SRD's equipment this is (longsword, chain-mail, thieves-tools): weapon proficiency by name, and the tools rules (picking locks needs thieves-tools, casting spells with material components a component-pouch or a focus)");
        ui.label("Cost");
        let (mut gp, mut sp, mut cp) = (d.cost_cp / 100, d.cost_cp % 100 / 10, d.cost_cp % 10);
        let mut changed = ui.add(egui::DragValue::new(&mut gp).range(0..=99_999).suffix(" gp")).on_hover_text("What shops whose currency is a coin ask for it (the SRD's prices); 0 = its value in the shop's currency").changed();
        changed |= ui.add(egui::DragValue::new(&mut sp).range(0..=9).suffix(" sp")).changed();
        changed |= ui.add(egui::DragValue::new(&mut cp).range(0..=9).suffix(" cp")).changed();
        if changed {
            d.cost_cp = gp * 100 + sp * 10 + cp;
        }
        ui.label("Weight");
        ui.add(egui::DragValue::new(&mut d.weight).range(0.0..=99_999.0).speed(0.1).suffix(" lb")).on_hover_text("Its weight (carrying limits come with a later version)");
        ui.label("Money");
        const WORTH: [(u32, &str); 6] = [(0, "not money"), (1, "copper piece"), (10, "silver piece"), (50, "electrum piece"), (100, "gold piece"), (1000, "platinum piece")];
        let label = WORTH.iter().find(|(w, _)| *w == d.coin_cp).map_or_else(|| format!("{} cp", d.coin_cp), |(_, l)| l.to_string());
        egui::ComboBox::from_id_salt("item_coin").selected_text(label).show_ui(ui, |ui| {
            for (w, l) in WORTH {
                ui.selectable_value(&mut d.coin_cp, w, l);
            }
        })
        .response
        .on_hover_text("A coin: shops whose currency is a coin count every coin the player carries by its worth and give change");
    });
    ui.horizontal(|ui| {
        ui.label("Uses");
        ui.add(egui::DragValue::new(&mut d.uses).range(0..=999))
            .on_hover_text("How many times it can be used, like a magic item's charges (a healer's kit: 10; it is used up after the last); 0 = no count. Items with uses don't stack.");
        ui.checkbox(&mut d.silvered, "Silvered").on_hover_text(
            "A silvered weapon or ammunition gets through resistances to nonmagical attacks 'that aren't silvered' (a werewolf's). Magic weapons get through every resistance to nonmagical attacks.",
        );
    });
}

/// The Magic item section: rarity, attunement, its +N bonus, charges and their recharge at dawn,
/// spells cast from it, passive effects while worn (as RON), what using it does (spell parts).
fn magic_item_ui(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    use rustpg2d_engine::srd::magic_items::{ItemSpell, MagicItem, Rarity};
    let mut on = d.magic.is_some();
    ui.checkbox(&mut on, "Magic item").on_hover_text("Rarity, attunement, a +1 to +3 bonus, charges, spells cast from it, effects while worn, what using it does (the SRD's magic items)");
    if on != d.magic.is_some() {
        d.magic = on.then(MagicItem::default);
    }
    let id = d.id.clone();
    let Some(m) = d.magic.as_mut() else { return };
    egui::Grid::new("magic_item").num_columns(2).show(ui, |ui| {
        ui.label("Rarity");
        egui::ComboBox::from_id_salt("magic_rarity").selected_text(m.rarity.label()).show_ui(ui, |ui| {
            for r in Rarity::ALL {
                ui.selectable_value(&mut m.rarity, r, r.label());
            }
        });
        ui.end_row();
        ui.label("Attunement");
        ui.horizontal(|ui| {
            ui.checkbox(&mut m.attunement, "required").on_hover_text("It works only while the hero is attuned to it (at most three items; attuning takes an hour, outside fights)");
            if m.attunement {
                ui.label("by");
                let mut t = m.attune_by.join(", ");
                if ui.add(egui::TextEdit::singleline(&mut t).desired_width(160.0).hint_text("anyone")).on_hover_text("Class or race ids, or spellcaster (comma separated); empty = anyone").changed() {
                    m.attune_by = t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                }
            }
        });
        ui.end_row();
        ui.label("Bonus");
        ui.add(egui::DragValue::new(&mut m.bonus).range(-3..=3).prefix("+")).on_hover_text("A weapon's or ammunition's +N to attack and damage rolls, armor's or a shield's to armor class");
        ui.end_row();
        ui.label("Charges");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut m.charges).range(0..=50)).on_hover_text("0 = none");
            if m.charges > 0 {
                ui.label("regains at dawn");
                ui.add(egui::TextEdit::singleline(&mut m.recharge).desired_width(70.0).hint_text("none")).on_hover_text("Dice (1d6+1), all, or empty = none");
                ui.checkbox(&mut m.destroy_on_last, "may crumble").on_hover_text("Spending the last charge rolls a d20: on a 1 the item is destroyed");
            }
        });
        ui.end_row();
    });
    ui.label("Spells cast from it");
    let mut remove = None;
    for (k, sp) in m.spells.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button(crate::ui::ph::TRASH).on_hover_text("Remove this spell").clicked() {
                remove = Some(k);
            }
            widgets::suggest(ui, &format!("{id}/mspell{k}"), &mut sp.spell, &cx.refs.spells, 140.0);
            ui.add(egui::DragValue::new(&mut sp.charges).range(0..=50).suffix(" charges")).on_hover_text("0 = at will");
            ui.add(egui::DragValue::new(&mut sp.slot).range(0..=9).prefix("slot ")).on_hover_text("The slot level it is cast at (0 = the spell's own)");
            ui.add(egui::DragValue::new(&mut sp.upcast_to).range(0..=9).prefix("up to ")).on_hover_text("One more charge per slot level, up to this one (0 = not)");
            ui.add(egui::DragValue::new(&mut sp.dc).range(0..=30).prefix("DC ")).on_hover_text("The item's save DC (its attack bonus = DC - 8); 0 = the user's own");
        });
    }
    if let Some(k) = remove {
        m.spells.remove(k);
    }
    if ui.small_button("Add spell").clicked() {
        m.spells.push(ItemSpell { charges: 1, ..Default::default() });
    }
    ui.label("Effects while worn or held").on_hover_text("The class features' effects (RON): Bonus(to: Ac, amount: 1), Bonus(to: Saves, amount: 1), AbilityScore(ability: Str, score: 21), Resistance(types: [\"fire\"]), Speed(feet: 10, unarmored: false), CheckAdvantage(skill: \"stealth\"), Darkvision(feet: 60), SaveAdvantage(ability: Dex), Evasion…");
    let key = egui::Id::new(("magic_effects", id.clone()));
    let current = ron::ser::to_string(&m.effects).unwrap_or_default();
    let mut text: String = ui.data_mut(|d| d.get_temp::<(String, String)>(key)).filter(|(base, _)| *base == current).map_or(current.clone(), |(_, t)| t);
    let r = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(2).desired_width(f32::INFINITY).code_editor());
    if r.changed() {
        match ron::from_str::<Vec<rustpg2d_engine::classes::FeatureEffect>>(&text) {
            Ok(v) => {
                m.effects = v;
                let now = ron::ser::to_string(&m.effects).unwrap_or_default();
                ui.data_mut(|d| d.insert_temp(key, (now, text.clone())));
            }
            Err(_) => ui.data_mut(|d| d.insert_temp(key, (current.clone(), text.clone()))),
        }
    }
    if ron::from_str::<Vec<rustpg2d_engine::classes::FeatureEffect>>(&text).is_err() {
        ui.colored_label(egui::Color32::LIGHT_RED, "Not valid: the effects keep their last valid value");
    }
    ui.label("Using it").on_hover_text("What using it does to its user, or whom it is used on (a potion's healing, a buff for its duration): spell parts");
    spell_parts_ui(ui, &mut m.use_parts, cx);
    if !m.use_parts.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Lasts");
            let mut min = m.use_seconds / 60.0;
            if ui.add(egui::DragValue::new(&mut min).range(0.0..=1440.0).speed(1.0).suffix(" min")).on_hover_text("How long its boons and conditions last (0 = only the instant parts)").changed() {
                m.use_seconds = min * 60.0;
            }
        });
    }
    if m.story_only() {
        ui.weak("No rules effect yet: a story item (its effect is up to triggers and scripts).");
    }
}

/// The SRD's rows of an item's weapon: category, properties, versatile dice, thrown range.
fn weapon_srd_rows(ui: &mut egui::Ui, id: &str, w: &mut Weapon) {
    ui.label("Category");
    ui.horizontal(|ui| {
        let label = |c: Option<WeaponCategory>| match c {
            Some(WeaponCategory::Simple) => "Simple",
            Some(WeaponCategory::Martial) => "Martial",
            None => "none",
        };
        egui::ComboBox::from_id_salt(format!("{id}/cat")).selected_text(label(w.category)).show_ui(ui, |ui| {
            for c in [Some(WeaponCategory::Simple), Some(WeaponCategory::Martial)] {
                ui.selectable_value(&mut w.category, c, label(c));
            }
        });
        ui.weak("(proficiency: simple-weapons / martial-weapons, or the weapon's own name)");
    });
    ui.end_row();
    ui.label("Properties");
    ui.horizontal_wrapped(|ui| {
        for p in WeaponProperty::ALL {
            let mut on = w.has(p);
            if ui.checkbox(&mut on, p.label()).on_hover_text(property_tip(p)).changed() {
                if on {
                    w.properties.push(p);
                } else {
                    w.properties.retain(|x| *x != p);
                }
                if p == WeaponProperty::TwoHanded {
                    w.two_handed = on;
                }
            }
        }
    });
    ui.end_row();
    if w.has(WeaponProperty::Versatile) {
        ui.label("Two-handed dice");
        ui.add(egui::TextEdit::singleline(&mut w.versatile).desired_width(70.0).hint_text("1d10")).on_hover_text("Versatile: the damage dice with both hands on it (the off hand free)");
        ui.end_row();
    }
    if w.has(WeaponProperty::Thrown) {
        ui.label("Thrown range (ft)");
        let (mut n, mut l) = w.thrown_ft.unwrap_or((20, 60));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut n).range(5..=600).suffix(" ft")).on_hover_text("Normal range");
            ui.add(egui::DragValue::new(&mut l).range(5..=1200).suffix(" ft")).on_hover_text("Long range (disadvantage beyond the normal one)");
        });
        w.thrown_ft = Some((n, l.max(n)));
        ui.end_row();
    }
}

fn property_tip(p: WeaponProperty) -> &'static str {
    match p {
        WeaponProperty::Ammunition => "Shoots its ammo item (below), one per attack",
        WeaponProperty::Finesse => "Attacks with STR or DEX, the better (and Sneak Attack works with it)",
        WeaponProperty::Heavy => "Small heroes (halflings, gnomes) attack with disadvantage",
        WeaponProperty::Light => "Two light weapons make the off-hand attack (two-weapon fighting)",
        WeaponProperty::Loading => "One attack with it per action, however many attacks the wielder has",
        WeaponProperty::Reach => "Reaches 10 ft (two tiles)",
        WeaponProperty::Thrown => "Can be thrown: a ranged attack with the same ability (STR, or DEX if finesse)",
        WeaponProperty::TwoHanded => "Needs both hands: nothing can be held in the left hand (shield, lantern)",
        WeaponProperty::Versatile => "With the off hand free it rolls its two-handed dice",
    }
}

/// The SRD's rows of armor: category, armor class, DEX cap, STR minimum, Stealth disadvantage.
fn armor_srd_rows(ui: &mut egui::Ui, a: &mut Armor) {
    ui.label("Category");
    let label = |c: Option<ArmorCategory>| c.map_or("none (old defense)", |c| c.label());
    egui::ComboBox::from_id_salt("armor_cat").selected_text(label(a.category)).show_ui(ui, |ui| {
        for c in ArmorCategory::ALL {
            if ui.selectable_value(&mut a.category, Some(c), c.label()).changed() {
                (a.base_ac, a.dex_max) = match c {
                    ArmorCategory::Light => (11, None),
                    ArmorCategory::Medium => (14, Some(2)),
                    ArmorCategory::Heavy => (16, Some(0)),
                    ArmorCategory::Shield => (2, None),
                    ArmorCategory::Clothing => (0, None),
                };
            }
        }
    })
    .response
    .on_hover_text("Clothing gives no armor class (outfits, helmets, lights). Wearing armor without its proficiency (light-armor, medium-armor, heavy-armor, shields) gives disadvantage on STR and DEX rolls and attacks, and no spells can be cast.");
    ui.end_row();
    match a.category {
        Some(ArmorCategory::Light | ArmorCategory::Medium | ArmorCategory::Heavy) => {
            ui.label("Armor class");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut a.base_ac).range(0..=30)).on_hover_text("The armor class it gives, before DEX");
                let dex = |m: Option<i32>| match m {
                    None => "+ DEX".to_string(),
                    Some(0) => "no DEX".to_string(),
                    Some(m) => format!("+ DEX (max {m})"),
                };
                egui::ComboBox::from_id_salt("armor_dex").selected_text(dex(a.dex_max)).show_ui(ui, |ui| {
                    for m in [None, Some(2), Some(0)] {
                        ui.selectable_value(&mut a.dex_max, m, dex(m));
                    }
                });
            });
            ui.end_row();
            ui.label("STR needed");
            ui.add(egui::DragValue::new(&mut a.str_min).range(0..=30)).on_hover_text("Below this Strength the wearer is 10 ft slower (heavy armor; 0 = none)");
            ui.end_row();
            ui.label("Stealth");
            ui.checkbox(&mut a.stealth_disadvantage, "Disadvantage").on_hover_text("Stealth checks (and sneaking) with disadvantage while worn");
            ui.end_row();
        }
        Some(ArmorCategory::Shield) => {
            ui.label("Armor class");
            ui.add(egui::DragValue::new(&mut a.base_ac).range(0..=10).prefix("+ ")).on_hover_text("What the shield adds to the armor class");
            ui.end_row();
        }
        Some(ArmorCategory::Clothing) => {}
        None => {
            ui.label("Defense");
            ui.add(egui::DragValue::new(&mut a.defense).range(-99..=99)).on_hover_text("An item of 0.30 and older: its category comes from this when the project is opened");
            ui.end_row();
        }
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
            .on_hover_text("Base trade price in the shop's currency; merchants use it when their stock has no own price and the item has no cost in coins (0 = worthless, can't be sold)");
    });
    srd_item_ui(ui, d);
    magic_item_ui(ui, d, cx);
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
        (None, Some(a)) if a.slot == EquipSlot::Back => 7,
        (None, Some(a)) if a.slot.worn() => 8,
        (None, Some(_)) => 3,
        _ => 0,
    };
    let old = kind;
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut kind, 0, "None");
        ui.radio_value(&mut kind, 1, "Weapon");
        ui.radio_value(&mut kind, 2, "Helmet");
        ui.radio_value(&mut kind, 3, "Body armor");
        ui.radio_value(&mut kind, 4, "Left hand").on_hover_text("A shield (its armor class) or a light (lantern, torch: category Clothing, its light below). Not usable together with a two-handed weapon.");
        ui.radio_value(&mut kind, 5, "Clothing").on_hover_text("An outfit (speed); the model's art shows what a character wears.");
        ui.radio_value(&mut kind, 6, "Tool").on_hover_text("Goes into the Tool slot (a fishing rod, later pickaxes, hoes...): held in the right hand while it is used. Activities ask for it, e.g. a fishing spot's 'Needs item'.");
        ui.radio_value(&mut kind, 7, "Backpack").on_hover_text("Goes into the Back slot and shows on the character's back (its 3D model, below); gives the player more inventory slots while worn.");
        ui.radio_value(&mut kind, 8, "Worn item").on_hover_text("A ring, an amulet, a cloak, a belt, boots, gloves or bracers: worn in its own slot of the character window (not drawn on the model). Magic items work there.");
    });
    if kind != old {
        d.tool = if kind == 6 { if d.tool.is_empty() { "tool".into() } else { d.tool.clone() } } else { String::new() };
        d.weapon = (kind == 1).then(|| Weapon::melee(1, 0.4));
        d.armor = match kind {
            2..=5 | 7 | 8 => {
                let slot = match kind {
                    2 => EquipSlot::Head,
                    3 => EquipSlot::Body,
                    5 => EquipSlot::Clothing,
                    7 => EquipSlot::Back,
                    8 => EquipSlot::Ring,
                    _ => EquipSlot::Offhand,
                };
                // New body armor is light armor (leather: 11 + DEX), a new left-hand piece a shield (+2).
                let (category, base_ac) = match slot {
                    EquipSlot::Body => (ArmorCategory::Light, 11),
                    EquipSlot::Offhand => (ArmorCategory::Shield, 2),
                    _ => (ArmorCategory::Clothing, 0),
                };
                Some(d.armor.take().map(|a| Armor { slot, ..a }).unwrap_or(Armor { category: Some(category), base_ac, ..Armor::clothing(slot) }))
            }
            _ => None,
        };
        if kind != 0 && kind != 4 {
            d.max_stack = 1;
        }
        d.bag_slots = if kind == 7 { d.bag_slots.max(6) } else { 0 };
        // A backpack's model sits on the back.
        if let Some(m) = d.model.as_mut().filter(|_| kind == 7) {
            m.hold = rustpg2d_engine::defs::HoldStyle::Back;
        }
    }
    if let Some(a) = d.armor.as_mut().filter(|a| a.slot.worn()) {
        ui.horizontal(|ui| {
            ui.label("Worn on");
            egui::ComboBox::from_id_salt("item_worn_slot").selected_text(a.slot.label()).show_ui(ui, |ui| {
                for s in EquipSlot::WORN.into_iter().filter(|s| *s != EquipSlot::Ring2) {
                    ui.selectable_value(&mut a.slot, s, s.label());
                }
            });
            ui.weak("(a ring goes into either ring slot)");
        });
    }
    if d.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Back) {
        ui.horizontal(|ui| {
            ui.label("Extra inventory slots");
            ui.add(egui::DragValue::new(&mut d.bag_slots).range(0..=96)).on_hover_text("The player's inventory has this many more slots while the backpack is worn");
            ui.weak("(taking it off packs the items of those slots into free ones, or refuses when they don't fit)");
        });
    }
    if d.weapon.is_some() {
        weapon_ui(ui, "item_weapon", &mut d.weapon, cx, false);
    }
    if let Some(a) = &mut d.armor {
        egui::Grid::new("armor").num_columns(2).show(ui, |ui| {
            armor_srd_rows(ui, a);
            ui.label("Speed change");
            let mut pct = a.speed * 100.0;
            if ui.add(egui::DragValue::new(&mut pct).range(-90.0..=100.0).suffix(" %")).changed() {
                a.speed = pct / 100.0;
            }
            ui.end_row();
            ui.label("Resistances").on_hover_text("Damage types its wearer takes half damage from");
            ui.horizontal_wrapped(|ui| damage_type_toggles(ui, &mut a.resistances));
            ui.end_row();
        });
        ui.weak("Worn items count in play (armor class, speed); a character's model shows what it wears.");
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
        ui.weak("(scrolls, spell books, by the SRD's rule: a wizard copies the spell into the spellbook, another class with it on its list casts it once, anyone else can't — tick 'Consumed' for one-use scrolls)");
    });
    ui.separator();
    ui.heading("Furniture");
    ui.horizontal(|ui| {
        ui.label("Places prop");
        widgets::suggest(ui, "item_furniture", &mut d.furniture, &cx.refs.props, 140.0);
        ui.weak("(using it in a home of the player's places the prop in front; picking the prop up gives the item back)");
    });
    ui.separator();
    ui.heading("Light");
    widgets::light_ui(ui, "item_light", &mut d.light, "Gives light (while equipped, e.g. in the left hand, or lying on the ground)");
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "item_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
    art_buttons(ui, cx, "icon", (1, 1));
    let holdable = d.weapon.is_some() || !d.tool.is_empty() || d.armor.as_ref().is_some_and(|a| matches!(a.slot, EquipSlot::Offhand | EquipSlot::Back));
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
    if d.weapon.is_some() || d.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Offhand) {
        ui.separator();
        item_anims_ui(ui, d, cx);
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
    ui.horizontal(|ui| {
        ui.checkbox(&mut d.difficult, "Difficult terrain").on_hover_text("Undergrowth, rubble: crossing its tiles costs double movement in fights (as a decoration too).");
        ui.checkbox(&mut d.obscures, "Obscures").on_hover_text(
            "Bushes, tall grass, reeds: its tiles are lightly obscured in fights (a fighter there is seen dimly, and can hide there). As a decoration too.",
        );
    });
    ui.checkbox(&mut d.flat, "Lies flat on the ground").on_hover_text(
        "A rug, a mat, a puddle: drawn under everyone (like a Flat decoration) instead of sorted with characters by its feet; as furniture it may lie under tables and chairs.",
    );
    ui.checkbox(&mut d.door, "Door (E opens and closes it)").on_hover_text(
        "Needs a state named \"open\" (passable); its first state is the closed door. A lock keeps it shut until its key or a lockpick opens it; with the key, closing it locks it again. Villagers open unlocked doors on their way (locked ones when they carry the key) and close them behind them.",
    );
    ui.checkbox(&mut d.climb, "Ladder (the player climbs it)").on_hover_text("Placed as a decoration, the player climbs where it stands (slower, facing up); characters keep off it.");
    ui.horizontal(|ui| {
        ui.label("Snap to").on_hover_text("Placed as a decoration it sits on a grid of this many pixels (walls, fences); 0 = anywhere.");
        egui::ComboBox::from_id_salt("prop_snap").selected_text(if d.snap == 0 { "anywhere".to_string() } else { format!("{} px", d.snap) }).show_ui(ui, |ui| {
            for s in [0u16, 8, 16, 32] {
                ui.selectable_value(&mut d.snap, s, if s == 0 { "anywhere".to_string() } else { format!("{s} px") });
            }
        });
    });
    // An animated picture: frames of its sheet, shown in turn (its first state).
    ui.horizontal(|ui| {
        let mut on = d.anim.is_some();
        if ui.checkbox(&mut on, "Animation").on_hover_text("The default state cycles through frames of its sprite sheet (a torch, a flag, a water wheel), also as a decoration.").changed() {
            d.anim = on.then(|| rustpg2d_engine::defs::FrameAnim { frames: vec![d.sprite.frame, d.sprite.frame + 1], fps: 4.0 });
        }
        if let Some(a) = &mut d.anim {
            ui.label("frames");
            let mut text = a.frames.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(120.0)).on_hover_text("Frame numbers of the sheet, separated by commas").changed() {
                a.frames = text.split(',').filter_map(|t| t.trim().parse().ok()).collect();
            }
            ui.add(egui::DragValue::new(&mut a.fps).range(0.5..=30.0).speed(0.1).suffix(" fps"));
        }
    });
    wall_pieces_ui(ui, d, cx);
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

/// A wall set: the prop for each combination of neighbours (Decorations tool, Line).
fn wall_pieces_ui(ui: &mut egui::Ui, d: &mut PropDef, cx: &mut Cx) {
    use rustpg2d_engine::decor::{EAST, NORTH, SOUTH, WEST};
    let mut on = !d.pieces.is_empty();
    if ui.checkbox(&mut on, "Wall set (pieces join their neighbours)").on_hover_text("A line drawn with the Decorations tool places this prop and swaps in the piece each spot needs by its neighbours: corners, ends, crossings. Empty pieces use this prop.").changed() {
        d.pieces = if on { vec![String::new(); 16] } else { Vec::new() };
        if on && d.snap == 0 {
            d.snap = 32;
        }
    }
    if d.pieces.is_empty() {
        return;
    }
    d.pieces.resize(16, String::new());
    egui::CollapsingHeader::new("Wall pieces").id_salt("wall_pieces").show(ui, |ui| {
        let name = |m: usize| {
            let parts: Vec<&str> = [(NORTH, "north"), (EAST, "east"), (SOUTH, "south"), (WEST, "west")].iter().filter(|(b, _)| m & b != 0).map(|(_, n)| *n).collect();
            if parts.is_empty() { "alone".to_string() } else { parts.join(" + ") }
        };
        egui::Grid::new("pieces").num_columns(2).show(ui, |ui| {
            for m in 0..16 {
                ui.label(name(m)).on_hover_text("The neighbours of the spot that belong to the set");
                widgets::suggest(ui, &format!("piece{m}"), &mut d.pieces[m], &cx.refs.props, 180.0);
                ui.end_row();
            }
        });
    });
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
        Pose::Sit => "Interacting sits down (when the prop's own triggers did nothing), facing the chosen way (one prop per direction, e.g. chairs around a table); HP comes back faster while sitting. Moving, attacking or E stands up.",
        Pose::Lie => "Interacting asks \"Sleep until …?\": the night passes (HP and spell slots restored, effects worn off, enemies respawned, scheduled characters moved on), the bed's and the map's 'Woke up' triggers run, the game autosaves. Not with enemies nearby.",
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

/// A party member's class, subclass and level (Database › Classes): as a companion it fights
/// with the class's numbers and levels with the hero.
fn class_row(ui: &mut egui::Ui, d: &mut CharacterDef, db: &Database) {
    ui.horizontal(|ui| {
        ui.label("Class");
        let id = d.class.as_ref().map(|c| c.class.clone()).unwrap_or_default();
        let label = if id.is_empty() { "(none)".to_string() } else { db.class(&id).map_or(format!("{id} (missing)"), |c| c.name.clone()) };
        let mut pick = id.clone();
        egui::ComboBox::from_id_salt("chr_class").selected_text(label).show_ui(ui, |ui| {
            ui.selectable_value(&mut pick, String::new(), "(none)");
            for c in &db.classes {
                ui.selectable_value(&mut pick, c.id.clone(), format!("{}  ({})", c.name, c.id));
            }
        })
        .response
        .on_hover_text("Its class as a party member (Database \u{E13A} Classes): when it joins the player it fights with the class's hit points, armor class and attacks at its level, levels with the hero (the XP is split) and makes its choices in the level-up window. None = its own numbers (Combat, its stat block).");
        if pick != id {
            d.class = (!pick.is_empty()).then(|| rustpg2d_engine::classes::HeroClass { class: pick, ..Default::default() });
            d.level = d.level.max(1);
        }
        let Some(hc) = d.class.as_mut() else { return };
        if let Some(c) = db.class(&hc.class) {
            if !c.subclasses.is_empty() {
                let sub = c.subclass(&hc.subclass).map_or("(chosen in play)".to_string(), |s| s.name.clone());
                egui::ComboBox::from_id_salt("chr_subclass").selected_text(sub).show_ui(ui, |ui| {
                    ui.selectable_value(&mut hc.subclass, String::new(), "(chosen in play)");
                    for s in &c.subclasses {
                        ui.selectable_value(&mut hc.subclass, s.id.clone(), s.name.clone());
                    }
                })
                .response
                .on_hover_text(format!("{} (from level {}); empty = chosen in the level-up window", if c.subclass_title.is_empty() { "Subclass" } else { c.subclass_title.as_str() }, c.subclass_level));
            }
        }
        ui.label("level");
        let mut level = d.level.max(1);
        if ui.add(egui::DragValue::new(&mut level).range(1..=20)).on_hover_text("Its level when it joins; then it levels with the hero").changed() {
            d.level = level;
        }
    });
}

fn character_form(ui: &mut egui::Ui, d: &mut CharacterDef, cx: &mut Cx) {
    ui.heading("Character");
    rename_row(ui, RefKind::Character, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description").on_hover_text("What the bestiary (journal, J) says about it once the player has defeated one. Placeholders like {player} work.");
    text_box(ui, &mut d.description, 3, "Shown in the bestiary once the player has defeated one");
    // The quests it gives (Database › Quests: Giver), as links.
    // (Given by the definition, or by a character placed from it.)
    let placed = |who: &str| cx.refs.placed_characters.iter().any(|(n, def)| n == who && *def == d.id);
    let gives: Vec<(String, String)> = cx.db.quests.iter().filter(|q| q.giver == d.id || placed(&q.giver)).map(|q| (q.id.clone(), q.title.clone())).collect();
    if !gives.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label("Gives quests").on_hover_text("Quests whose giver is this character (Database › Quests): click one to open it");
            for (id, title) in gives {
                if ui.link(if title.is_empty() { id.clone() } else { title }).clicked() {
                    request_jump(RefKind::Quest, &id);
                }
            }
        });
    }
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
    if !cx.db.classes.is_empty() || d.class.is_some() {
        class_row(ui, d, cx.db);
    }
    {
        let race_faction = cx.db.race(&d.race).map(|r| r.faction.clone()).filter(|f| !f.is_empty()).map(|f| format!("its race's: {f}")).unwrap_or_default();
        faction_combo(ui, "chr_faction", &mut d.faction, &cx.refs.factions, &race_faction, "Who it sides with (Database \u{E13A} Factions): characters of hostile factions fight each other, and it attacks the player when the player's standing with its faction is -20 or below. Empty = its race's faction. A placed character can have its own (the map's inspector).");
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
        ui.label("Size").on_hover_text(
            "How big it is drawn, in percent of its 3D model's height (times its race's size): the figure, what it holds and its hitbox. Placed characters of this definition and, for the player character, the player get it.",
        );
        let mut pct = if d.appearance.size == 0 { 100 } else { d.appearance.size };
        if ui.add(egui::DragValue::new(&mut pct).range(25..=400).suffix(" %")).changed() {
            d.appearance.size = if pct == 100 { 0 } else { pct };
        }
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
                    .on_hover_text("Shooters (ranged weapon), when they go for a character of a hostile faction outside fights: they back off when it comes closer than this. In fights shooters keep within their range and away from melee.");
                ui.weak("(shooters)");
            });
            ui.end_row();
            ui.label("Group");
            ui.checkbox(group, "Fights as a group").on_hover_text(
                "Outside fights: they spread out instead of stacking up, and the badly hurt run to their allies, catch their breath and come back.",
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
        ui.add(egui::DragValue::new(&mut d.max_hp).range(0..=999)).on_hover_text("0 = can't be damaged (and doesn't fight). With a stat block its hit points are the stat block's.");
        ui.end_row();
        ui.label("Damage");
        ui.add(egui::DragValue::new(&mut d.damage).range(0..=999)).on_hover_text("Without attacks in its stat block, its attack's dice come from this (1 = 1d4, 2 = 1d6, 3 = 1d8, 4 = 1d10, 5+ = 1d12), or from its weapon");
        ui.end_row();
        ui.label("Defense");
        ui.add(egui::DragValue::new(&mut d.defense).range(-99..=99)).on_hover_text("Without a stat block its armor class is 10 + this. Outside fights it is subtracted from damage (scripts, traps).");
        ui.end_row();
        ui.label("Hostile");
        ui.checkbox(&mut d.hostile, "attacks the player").on_hover_text(
            "When it notices the player or a companion, a turn-based fight starts; it attacks with its stat block's attacks (else its weapon or Damage). With a faction it leaves players alone who are friends of that faction (standing 20 or more); without 'Hostile' it attacks when the player's standing is -20 or below.",
        );
        ui.end_row();
        ui.label("As companion");
        ui.checkbox(&mut d.gear_locked, "can't be given gear")
            .on_hover_text("A companion's window lets the player equip it and hand it items; tick this for animals and creatures that can't wear anything (they still carry items and use potions).");
        ui.end_row();
        ui.label("Experience");
        ui.add(egui::DragValue::new(&mut d.xp).range(0..=1_000_000).suffix(" XP"))
            .on_hover_text("What the player gains for defeating this character (Project › Experience and levels must be on). 'Defeat all enemies' gives none.");
        ui.end_row();
    });
    ui.weak("For the player character, a Max HP above 0 turns combat on (the hero's own HP follow the SRD rules: the hit die, CON and the level), and Damage is the bare-handed attack's (1 = 1d4 + STR). Defeated characters fire 'Defeated' triggers; 'Fight starts' / 'Fight ends' triggers run when it fights.");
    widgets::optional_effect_ui(
        ui,
        "chr_touch_fx",
        "On touch:",
        "A condition its melee hits give in fights (hostile characters), e.g. a poisonous slime. Without one, a melee weapon it holds gives that weapon's effect.",
        &mut d.touch_effect,
    );
    ui.label("Ranged attack (without attacks in its stat block it shoots with this in fights):");
    weapon_ui(ui, "chr_weapon", &mut d.weapon, cx, true);
    ui.separator();
    ui.heading("Stat block (SRD 5.1)");
    ui.weak("What it is by the rules: abilities, armor class, hit points, attacks, challenge. Turn-based fights use it (its hit points replace Max HP when that is above 0); scripts read it (npc_stat). A placed character can have its own (the map's inspector).");
    stat_block_ui(ui, "chr_stats", &mut d.stats, Some(&mut d.xp));
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
                d.features.push(ColorFeature { name: name.into(), shades: Vec::new(), colors, default: None });
                sel = d.features.len() - 1;
            }
        }
        if ui.button(ui::t(ui::ph::PLUS, "Other")).clicked() {
            d.features.push(ColorFeature { name: format!("part_{}", d.features.len() + 1), shades: Vec::new(), colors: Vec::new(), default: None });
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
        ui.horizontal(|ui| {
            ui.label("Default colour").on_hover_text(
                "The colour this part takes when a character (or the player in the creator's first choice, As drawn) chose none. Without one the art's own colours show: set one when they don't suit it.",
            );
            match &mut f.default {
                Some(c) => {
                    ui.color_edit_button_srgb(c);
                    for o in f.colors.clone() {
                        if crate::builder::swatch(ui, o, *c == o).on_hover_text("Use this offered colour").clicked() {
                            *c = o;
                        }
                    }
                    if ui.small_button(ui::t(ui::ph::X, "As drawn")).on_hover_text("No default: the art's own colours").clicked() {
                        f.default = None;
                    }
                }
                None => {
                    ui.weak("as drawn");
                    if ui.small_button(ui::t(ui::ph::PLUS, "Set")).clicked() {
                        f.default = Some(f.colors.first().copied().or_else(|| f.shades.get(f.shades.len() / 2).copied()).unwrap_or([200, 160, 120]));
                    }
                }
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
        let missing: Vec<&String> = d.materials.iter().filter(|m| cx.db.material(m).is_none()).chain(d.decor_defs.iter().filter(|p| cx.db.prop(p).is_none())).collect();
        let ground = if d.materials.is_empty() { "no ground".to_string() } else { format!("ground: {}", d.materials.join(", ")) };
        let props = match d.entities.len() {
            0 => String::new(),
            1 => " · 1 prop".to_string(),
            n => format!(" · {n} props"),
        };
        ui.weak(format!("{} × {} tiles · {ground} · {} decorations{props}", d.w, d.h, d.decor.len()));
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
        ui.label("Caption").on_hover_text("What it sounds like, shown while it plays to players who turned on Captions (Options › Accessibility): \"Door creaks\", \"Dog barks\"; for music the track's name. Translated like dialogs. Empty: no caption (footsteps, menu blips).");
        ui.add(egui::TextEdit::singleline(&mut d.caption).hint_text("(none)").desired_width(220.0));
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

fn font_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::fonts::FontDef, cx: &mut Cx) {
    ui.heading("Font");
    rename_row(ui, RefKind::Font, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.weak("A TTF or OTF font the game can draw its texts with: choose which texts use it under Project › Fonts. Fonts other than the built-in one show every script they have glyphs for (Cyrillic, Greek, Japanese…).");
    egui::Grid::new("font").num_columns(2).show(ui, |ui| {
        ui.label("File").on_hover_text("The font file (TTF / OTF), relative to the project; files from elsewhere are copied into fonts/.");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.file).hint_text("fonts/my_font.ttf").desired_width(220.0));
            if ui.button("Browse…").clicked() {
                *cx.browse = Some(Purpose::FontFile { id: d.id.clone(), license: false });
            }
        });
        ui.end_row();
        ui.label("Pixel size").on_hover_text("For a pixel font: the size its pixels are drawn for (its grid, e.g. 16). Texts are then drawn at whole multiples of it and stay crisp. 0 = a smooth font, drawn at any size.");
        ui.add(egui::DragValue::new(&mut d.pixel).range(0..=64).suffix(" px"));
        ui.end_row();
        ui.label("Scale").on_hover_text("How big it is drawn against the built-in font (1 = the same size asked for). Raise it for a font that looks small, lower it for one that looks big; pixel fonts then snap to their grid.");
        ui.add(egui::DragValue::new(&mut d.scale).range(0.25..=4.0).speed(0.01).fixed_decimals(2));
        ui.end_row();
        ui.label("Baseline").on_hover_text("Moves its letters down (+) or up (-) by this part of the text size, for fonts that sit higher or lower than the built-in one.");
        ui.add(egui::DragValue::new(&mut d.offset_y).range(-0.5..=0.5).speed(0.005).fixed_decimals(3));
        ui.end_row();
        ui.label("Licence").on_hover_text("The font's licence text (a file in fonts/). Exported games carry it with the font, as most font licences ask.");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.license).hint_text("fonts/OFL.txt").desired_width(220.0));
            if ui.button("Browse…").clicked() {
                *cx.browse = Some(Purpose::FontFile { id: d.id.clone(), license: true });
            }
        });
        ui.end_row();
        ui.label("Credit");
        ui.add(egui::TextEdit::singleline(&mut d.credit).hint_text("Font by …, licence").desired_width(260.0));
        ui.end_row();
    });
    ui.separator();
    font_preview(ui, cx.root, d);
}

thread_local! {
    /// Font files already handed to egui for previews (by their egui font name).
    static PREVIEW_FONTS: std::cell::RefCell<std::collections::BTreeMap<String, Option<usize>>> = std::cell::RefCell::default();
}

/// A sample of `d` set in its own face (egui's font), with how many characters the file has.
fn font_preview(ui: &mut egui::Ui, root: &std::path::Path, d: &rustpg2d_engine::fonts::FontDef) {
    if d.file.is_empty() {
        ui.weak("Choose a font file to see a sample.");
        return;
    }
    let path = root.join(&d.file);
    let name = format!("r2d-font:{}:{}", d.file, rustpg2d_engine::sys::modified_secs(&path));
    let glyphs = PREVIEW_FONTS.with(|p| {
        *p.borrow_mut().entry(name.clone()).or_insert_with(|| {
            let bytes = std::fs::read(&path).ok()?;
            let n = rustpg2d_engine::fonts::Coverage::of(&bytes)?.len();
            ui.ctx().add_font(egui::epaint::text::FontInsert::new(
                &name,
                egui::FontData::from_owned(bytes),
                vec![egui::epaint::text::InsertFontFamily { family: egui::FontFamily::Name(name.as_str().into()), priority: egui::epaint::text::FontPriority::Highest }],
            ));
            Some(n)
        })
    });
    let Some(n) = glyphs else {
        ui.colored_label(Color32::LIGHT_RED, format!("{} can't be read as a TTF / OTF font.", d.file));
        return;
    };
    ui.weak(format!("{n} characters. Sample (as the editor draws it; the game snaps pixel fonts to their grid):"));
    // (A font added to egui is there from the next frame on.)
    let family = egui::FontFamily::Name(name.as_str().into());
    if !ui.fonts(|f| f.families().contains(&family)) {
        ui.weak("…");
        ui.ctx().request_repaint();
        return;
    }
    let size = if d.pixel > 0 { d.pixel as f32 * 2.0 } else { 24.0 * d.scale };
    for line in ["The quick brown fox jumps over the lazy dog 0123456789", "Grüße · Привет · Γειά σου · こんにちは · 你好 · 안녕"] {
        ui.label(egui::RichText::new(line).family(family.clone()).size(size));
    }
}

/// Database › UI skins: a skin's pictures and colours, with the game's own drawing as a preview.
/// A background: its proficiencies, language choices, starting items and feature.
/// A class: hit die, proficiencies, skills to choose, starting equipment, the subclass level,
/// and the features by level with their texts (the SRD's come from the starter library).
fn class_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::classes::ClassDef, cx: &mut Cx) {
    ui.heading("Class");
    rename_row(ui, RefKind::Class, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    text_box(ui, &mut d.description, 2, "");
    ui.weak("The character creator offers every class; party members get one in their character form. The SRD's twelve come from the starter library. Features act by their effects (the import's), the rest is text on the sheet.");
    egui::Grid::new("class").num_columns(2).show(ui, |ui| {
        ui.label("Spellcasting").on_hover_text("How the class casts spells: its ability and how it has its spells. Its slots, cantrips and spells known are the level table's numbers (spellcasting.spell_slots_level_1 …)");
        ui.horizontal(|ui| {
            use rustpg2d_engine::classes::{CasterKind, Spellcasting};
            let mut on = d.spellcasting.is_some();
            if ui.checkbox(&mut on, "").changed() {
                d.spellcasting = on.then_some(Spellcasting { ability: rustpg2d_engine::srd::Ability::Int, kind: CasterKind::Known, half: false, ritual: false });
            }
            if let Some(sc) = &mut d.spellcasting {
                egui::ComboBox::from_id_salt("class_sc_ab").selected_text(sc.ability.short()).show_ui(ui, |ui| {
                    for a in rustpg2d_engine::srd::Ability::ALL {
                        ui.selectable_value(&mut sc.ability, a, a.short());
                    }
                });
                for (k, l, tip) in [
                    (CasterKind::Known, "Known", "Learns its spells at level-ups (bard, ranger, sorcerer)"),
                    (CasterKind::Prepared, "Prepared", "Prepares from its whole list: modifier + level (cleric, druid, paladin)"),
                    (CasterKind::Spellbook, "Spellbook", "Learns spells into a spellbook, copies scrolls, prepares from it (wizard)"),
                    (CasterKind::Pact, "Pact", "Knows its spells; its slots come back after a short rest (warlock)"),
                ] {
                    ui.selectable_value(&mut sc.kind, k, l).on_hover_text(tip);
                }
                ui.checkbox(&mut sc.half, "half").on_hover_text("Prepares half its level's worth (paladin)");
                ui.checkbox(&mut sc.ritual, "rituals").on_hover_text("Casts its ritual spells without a slot (10 minutes)");
            }
        });
        ui.end_row();
        ui.label("Hit die");
        egui::ComboBox::from_id_salt("class_die").selected_text(format!("d{}", d.hit_die)).show_ui(ui, |ui| {
            for die in [6, 8, 10, 12] {
                ui.selectable_value(&mut d.hit_die, die, format!("d{die}"));
            }
        });
        ui.end_row();
        ui.label("Saving throws");
        ui.horizontal_wrapped(|ui| {
            for a in rustpg2d_engine::srd::Ability::ALL {
                let mut on = d.saves.iter().any(|s| s == a.id());
                if ui.toggle_value(&mut on, a.short()).changed() {
                    if on {
                        d.saves.push(a.id().to_string());
                    } else {
                        d.saves.retain(|s| s != a.id());
                    }
                }
            }
        });
        ui.end_row();
        ui.label("Proficiencies").on_hover_text("Armor, weapons and tools (light-armor, simple-weapons, thieves-tools), comma separated");
        let mut text = d.proficiencies.join(", ");
        if ui.add(egui::TextEdit::singleline(&mut text).desired_width(360.0)).changed() {
            d.proficiencies = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
        ui.end_row();
        ui.label("Skills to choose");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut d.skill_choices).range(0..=6));
            ui.label("of");
            let mut text = d.skill_options.join(", ");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(300.0)).changed() {
                d.skill_options = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
        });
        ui.end_row();
        ui.label("Subclass at level");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut d.subclass_level).range(1..=20));
            ui.add(egui::TextEdit::singleline(&mut d.subclass_title).desired_width(160.0).hint_text("Martial Archetype"));
            let names: Vec<&str> = d.subclasses.iter().map(|s| s.name.as_str()).collect();
            ui.weak(names.join(", "));
        });
        ui.end_row();
    });
    ui.label("Starting equipment").on_hover_text("What a new hero of this class starts with (the creator)");
    ui.vertical(|ui| {
        let mut remove = None;
        for (k, s) in d.equipment.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.count).range(1..=999));
                widgets::suggest(ui, &format!("cl_item{k}"), &mut s.item, &cx.refs.items, 140.0);
                if ui.small_button("x").clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            d.equipment.remove(k);
        }
        if ui.small_button("+ Item").clicked() {
            d.equipment.push(rustpg2d_engine::inventory::ItemStack::new(cx.refs.items.first().map_or("", |s| s.as_str()), 1));
        }
    });
    ui.separator();
    egui::CollapsingHeader::new(format!("Features ({})", d.features.len())).id_salt("class_features").show(ui, |ui| {
        for (k, lv) in d.levels.iter().enumerate() {
            if lv.features.is_empty() {
                continue;
            }
            let names: Vec<String> = lv.features.iter().map(|f| d.features.iter().find(|x| x.id == *f).map_or(f.clone(), |x| if x.effects.is_empty() { x.name.clone() } else { format!("{} *", x.name) })).collect();
            ui.label(format!("Level {}: {}", k + 1, names.join(", ")));
        }
        ui.weak("* = acts in play (its effects); the others are their SRD text on the sheet.");
        for f in &mut d.features {
            ui.collapsing(format!("{} ({}, level {})", f.name, f.id, f.level), |ui| {
                ui.add(egui::TextEdit::multiline(&mut f.text).desired_rows(3).desired_width(f32::INFINITY));
                if !f.effects.is_empty() {
                    ui.weak(format!("Effects: {:?}", f.effects));
                }
            });
        }
    });
}

fn background_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::backgrounds::BackgroundDef, cx: &mut Cx) {
    ui.heading("Background");
    rename_row(ui, RefKind::Background, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    text_box(ui, &mut d.description, 2, "");
    ui.weak("The character creator offers every background: what the hero did before the adventure (SRD 5.1). The SRD's Acolyte comes from the starter library.");
    egui::Grid::new("background").num_columns(2).show(ui, |ui| {
        ui.label("Proficiencies").on_hover_text("Skills (insight), tools (thieves-tools) the hero gets");
        ui.vertical(|ui| {
            let mut remove = None;
            for (k, p) in d.proficiencies.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    widgets::suggest(ui, &format!("bg_prof{k}"), p, &cx.refs.skills, 160.0);
                    ui.weak(rustpg2d_engine::srd::abilities::proficiency_name(&rustpg2d_engine::srd::abilities::proficiency_id(p)));
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                d.proficiencies.remove(k);
            }
            if ui.small_button("+ Proficiency").clicked() {
                d.proficiencies.push("insight".into());
            }
        });
        ui.end_row();
        ui.label("Languages to choose");
        ui.add(egui::DragValue::new(&mut d.language_choices).range(0..=4)).on_hover_text("How many languages of their choice the hero learns (the creator asks)");
        ui.end_row();
        ui.label("Starting items");
        ui.vertical(|ui| {
            let mut remove = None;
            for (k, s) in d.equipment.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut s.count).range(1..=999));
                    widgets::suggest(ui, &format!("bg_item{k}"), &mut s.item, &cx.refs.items, 140.0);
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                d.equipment.remove(k);
            }
            if ui.small_button("+ Item").clicked() {
                d.equipment.push(rustpg2d_engine::inventory::ItemStack::new(cx.refs.items.first().map_or("", |s| s.as_str()), 1));
            }
        });
        ui.end_row();
        ui.label("Feature");
        ui.add(egui::TextEdit::singleline(&mut d.feature).desired_width(260.0).hint_text("Shelter of the Faithful"));
        ui.end_row();
    });
    ui.label("What the feature does");
    text_box(ui, &mut d.feature_text, 3, "");
}

fn common_event_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::common_events::CommonEventDef, cx: &mut Cx) {
    use rustpg2d_engine::common_events::CommonTrigger;
    ui.heading("Common event");
    rename_row(ui, RefKind::CommonEvent, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.weak("A list of actions used in many places: the action Call common event (or call_common(\"id\") in a script) runs it as if its actions stood there. With a flag it also runs by itself while the flag is set: parallel = again and again beside play (a clock, a rain dance), autorun = holding the player until its actions clear the flag (a scene that starts when a flag is set).");
    ui.horizontal(|ui| {
        ui.label("Runs");
        let flag = d.trigger.flag().to_string();
        egui::ComboBox::from_id_salt("ce_trigger").selected_text(d.trigger.label()).show_ui(ui, |ui| {
            for t in [CommonTrigger::Call, CommonTrigger::Parallel { flag: flag.clone() }, CommonTrigger::Autorun { flag: flag.clone() }] {
                let on = std::mem::discriminant(&t) == std::mem::discriminant(&d.trigger);
                let label = t.label();
                if ui.selectable_label(on, label).clicked() && !on {
                    d.trigger = t;
                }
            }
        });
        if let CommonTrigger::Parallel { flag } | CommonTrigger::Autorun { flag } = &mut d.trigger {
            ui.label("flag");
            widgets::suggest(ui, "ce_flag", flag, &cx.refs.all_flags, 140.0);
        }
    });
    ui.add_space(4.0);
    ui.label(ui::t(ph::LIGHTNING, "Actions"));
    widgets::actions_ui(ui, &format!("ce/{}", d.id), &mut d.actions, cx.refs);
}

fn skin_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::skin::SkinDef, cx: &mut Cx) {
    use rustpg2d_engine::skin::SkinColor;
    ui.heading("UI skin");
    rename_row(ui, RefKind::Skin, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.weak("How the game's windows, menus, dialog box, HUD and toasts look: pick it under Project › Look. Every part is optional: a part left empty keeps the built-in look. Pictures are frames of sprite sheets cut into nine parts: the corners keep their size, the edges and the middle stretch (or repeat). High contrast (Options › Accessibility) always shows the built-in look.");
    cx.skin_preview.ui(ui, d, cx.assets, cx.fonts, &cx.db.fonts, cx.root);
    ui.add_space(4.0);
    egui::CollapsingHeader::new(ui::t(ph::IMAGE, "Pictures")).id_salt("skin_pics").default_open(true).show(ui, |ui| {
        let parts: [(&str, &str, &mut Option<rustpg2d_engine::skin::NineSlice>); 11] = [
            ("Window frame", "Every window and menu panel.", &mut d.frame),
            ("Dialog box", "The dialog and choice box (empty: the window frame).", &mut d.dialog),
            ("Name box", "A box for the speaker's name on the dialog box's top edge (empty: the name stays inside the box).", &mut d.name_box),
            ("HUD panel", "The panel with the hero's portrait and bars (empty: the window frame, or the built-in gold frame).", &mut d.hud),
            ("Button", "Buttons, tabs and close buttons.", &mut d.button),
            ("Button under the pointer", "A button under the pointer and the open tab (empty: the button picture).", &mut d.button_hover),
            ("Cursor", "The selection marker of menus, lists and inventory slots.", &mut d.cursor),
            ("Toast", "The short messages at the top of the screen.", &mut d.toast),
            ("Around the HP bar", "A frame around the hit point bar.", &mut d.hp.frame),
            ("Around the mana bar", "Drawn around the mana bar.", &mut d.mana.frame),
            ("Around the experience bar", "Drawn around the experience bar.", &mut d.xp.frame),
        ];
        for (k, (label, tip, slot)) in parts.into_iter().enumerate() {
            ui.horizontal(|ui| {
                let mut on = slot.is_some();
                if ui.checkbox(&mut on, label).on_hover_text(tip).changed() {
                    *slot = on.then(|| rustpg2d_engine::skin::NineSlice::new(""));
                }
            });
            if let Some(n) = slot {
                ui.indent(("skin_pic", k), |ui| {
                    let mut sprite = SpriteRef { sheet: n.sheet.clone(), frame: n.frame, ..Default::default() };
                    if widgets::sprite_field(ui, &format!("skin_pic_{k}"), &mut sprite, cx.tex, cx.assets, cx.db, false) {
                        n.sheet = sprite.sheet;
                        n.frame = sprite.frame;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Corner").on_hover_text("The corners' size in the picture's pixels: they keep their size, the parts between them stretch.");
                        ui.add(egui::DragValue::new(&mut n.corner).range(0..=64).suffix(" px"));
                        ui.label("Pixel size").on_hover_text("Screen pixels per picture pixel: whole numbers keep pixel art crisp (2 = as the game's 16 px art is drawn).");
                        ui.add(egui::DragValue::new(&mut n.scale).range(1..=8).suffix("x"));
                        ui.checkbox(&mut n.tile, "Repeat").on_hover_text("Repeat the edges and the middle instead of stretching them (patterns, planks).");
                    });
                });
            }
        }
    });
    let color = |ui: &mut egui::Ui, label: &str, tip: &str, c: &mut Option<SkinColor>, builtin: [u8; 4]| {
        ui.horizontal(|ui| {
            let mut on = c.is_some();
            if ui.checkbox(&mut on, label).on_hover_text(tip).changed() {
                *c = on.then_some(SkinColor(builtin));
            }
            match c {
                Some(c) => {
                    ui.color_edit_button_srgba_unmultiplied(&mut c.0);
                }
                None => {
                    let [r, g, b, a] = builtin;
                    let (rect, _) = ui.allocate_exact_size(vec2(36.0, 16.0), Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(r, g, b, a));
                    ui.weak("built in");
                }
            }
        });
    };
    egui::CollapsingHeader::new(ui::t(ph::PALETTE, "Colours")).id_salt("skin_colors").default_open(true).show(ui, |ui| {
        ui.weak("Windows without a picture:");
        color(ui, "Background", "A panel's fill.", &mut d.background, [15, 15, 31, 235]);
        color(ui, "Border", "A panel's border, tab and slot outlines.", &mut d.border, [217, 204, 153, 255]);
        color(ui, "Button", "A button's fill.", &mut d.button_fill, [10, 10, 20, 217]);
        color(ui, "Button under the pointer", "A button under the pointer, the open tab.", &mut d.button_hover_fill, [41, 38, 56, 242]);
        color(ui, "Cursor", "The selection marker's fill.", &mut d.cursor_fill, [255, 255, 255, 31]);
        color(ui, "Toast", "A toast's fill.", &mut d.toast_fill, [13, 13, 26, 204]);
        color(ui, "Toast text", "A toast's text.", &mut d.toast_text, [255, 255, 255, 255]);
        ui.weak("Texts:");
        let t = &mut d.text;
        color(ui, "Normal", "Most text, dialog lines.", &mut t.normal, [255, 255, 255, 255]);
        color(ui, "Dim", "Hints, descriptions, choices not selected.", &mut t.dim, [199, 199, 199, 255]);
        color(ui, "Highlight", "The selected row, the open tab.", &mut t.highlight, [255, 203, 0, 255]);
        color(ui, "Disabled", "Entries that can't be chosen, help lines.", &mut t.disabled, [130, 130, 130, 255]);
        color(ui, "Title", "Window titles, the speaker's name.", &mut t.title, [255, 203, 0, 255]);
        color(ui, "Positive", "Good news: enough money, a quest done.", &mut t.positive, [153, 242, 153, 255]);
        color(ui, "Negative", "Bad news: too expensive, missing.", &mut t.negative, [255, 140, 128, 255]);
        ui.weak("HUD:");
        color(ui, "Hit points", "The HP bar's fill.", &mut d.hp.fill, [209, 36, 46, 255]);
        color(ui, "Hit point bar back", "", &mut d.hp.back, [0, 0, 0, 179]);
        color(ui, "Mana", "The mana bar's fill.", &mut d.mana.fill, [77, 115, 255, 255]);
        color(ui, "Mana bar back", "", &mut d.mana.back, [0, 0, 0, 179]);
        color(ui, "Experience", "The experience bar's fill.", &mut d.xp.fill, [217, 173, 51, 255]);
        color(ui, "Experience bar back", "", &mut d.xp.back, [0, 0, 0, 179]);
    });
    egui::CollapsingHeader::new(ui::t(ph::CHAT_TEXT, "Colour names for dialogs")).id_salt("skin_names").show(ui, |ui| {
        ui.weak("Names dialog texts can use in {c:name}…{/c} with this skin: new names, or other shades of the built-in ones (gold, red, …).");
        let mut remove = None;
        for (k, (name, c)) in d.colors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(120.0));
                ui.color_edit_button_srgba_unmultiplied(&mut c.0);
                if ui.small_button(ph::TRASH).on_hover_text("Remove").clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            d.colors.remove(k);
        }
        if ui.button(ui::t(ph::PLUS, "Add a colour name")).clicked() {
            d.colors.push((format!("colour{}", d.colors.len() + 1), SkinColor::rgb(250, 140, 60)));
        }
    });
}

/// The entries of a menu: up / down / remove per entry, custom entries' label and function, and
/// an Add menu with the entries that fit (`title`: the title screen, else the pause menu).
fn entries_ui(ui: &mut egui::Ui, salt: &str, entries: &mut Vec<rustpg2d_engine::menus::MenuEntry>, defaults: &[rustpg2d_engine::menus::MenuEntry], title: bool, functions: &[String]) {
    use rustpg2d_engine::menus::MenuEntry;
    if entries.is_empty() {
        ui.horizontal(|ui| {
            ui.weak(format!("Built in: {}.", defaults.iter().map(|e| e.label()).collect::<Vec<_>>().join(", ")));
            if ui.button(ui::t(ph::PENCIL_SIMPLE, "Change…")).on_hover_text("Start from the built-in entries and change them").clicked() {
                *entries = defaults.to_vec();
            }
        });
        return;
    }
    let (mut up, mut down, mut remove) = (None, None, None);
    let n = entries.len();
    for (k, e) in entries.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            if ui.add_enabled(k > 0, egui::Button::new(ph::ARROW_UP).small()).on_hover_text("Move up").clicked() {
                up = Some(k);
            }
            if ui.add_enabled(k + 1 < n, egui::Button::new(ph::ARROW_DOWN).small()).on_hover_text("Move down").clicked() {
                down = Some(k);
            }
            if ui.small_button(ph::TRASH).on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
            match e {
                MenuEntry::Custom { label, function, detail } => {
                    ui.add(egui::TextEdit::singleline(label).hint_text("label").desired_width(140.0)).on_hover_text("What the entry says (translated like dialogs)");
                    ui.label("runs");
                    widgets::suggest(ui, &format!("{salt}_fn_{k}"), function, functions, 130.0);
                    ui.add(egui::TextEdit::singleline(detail).hint_text("(line under it)").desired_width(160.0));
                }
                other => {
                    ui.label(other.label());
                }
            }
        });
    }
    if let Some(k) = up {
        entries.swap(k, k - 1);
    }
    if let Some(k) = down {
        entries.swap(k, k + 1);
    }
    if let Some(k) = remove {
        entries.remove(k);
    }
    ui.horizontal(|ui| {
        ui.menu_button(ui::t(ph::PLUS, "Add entry"), |ui| {
            let offered: &[MenuEntry] = if title { &MenuEntry::TITLE } else { &MenuEntry::PAUSE };
            let missing: Vec<MenuEntry> = offered.iter().filter(|e| !entries.contains(e)).cloned().collect();
            for e in missing {
                if ui.button(e.label()).clicked() {
                    entries.push(e);
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("Custom: runs a script function").clicked() {
                entries.push(MenuEntry::Custom { label: "New entry".into(), function: String::new(), detail: String::new() });
                ui.close_menu();
            }
        });
        if ui.button(ui::t(ph::ARROW_COUNTER_CLOCKWISE, "Built-in entries")).on_hover_text("Back to the built-in entries").clicked() {
            entries.clear();
        }
    });
}

/// Project › Title screen, Pause menu and Credits.
fn menus_form(ed: &mut Editor, ui: &mut egui::Ui) {
    use rustpg2d_engine::menus::{MenuAlign, PauseMenu, TitleBackground, TitleLogo, TitleScreen};
    let refs = ed.refs.clone();
    let mut play = false;
    {
        let tex = &ed.tex;
        let c = &mut ed.project.config;
        let name = c.name.clone();
        let start_map = c.start_map.clone();
        let t = &mut c.title;
        ui.heading("Title screen");
        ui.weak("What players see first: a map (the start map, or another one at a place and an hour of your choice) or a picture behind the menu, a logo, music, and the entries in the order you want. Custom entries run a script function (on the title screen, instant things such as variables and sounds; what waits runs once the game starts).");
        title_preview(ui, t, &name, &start_map, tex);
        egui::Grid::new("title_screen").num_columns(2).show(ui, |ui| {
            ui.label("Background");
            ui.horizontal(|ui| {
                let is_map = matches!(t.background, TitleBackground::Map { .. });
                if ui.selectable_label(is_map, "A map").clicked() && !is_map {
                    t.background = TitleBackground::default();
                }
                if ui.selectable_label(!is_map, "A picture").clicked() && is_map {
                    t.background = TitleBackground::Picture { sheet: String::new() };
                }
            });
            ui.end_row();
            match &mut t.background {
                TitleBackground::Map { map, at, hour, drift } => {
                    ui.label("Map").on_hover_text("Empty: the start map");
                    widgets::suggest(ui, "title_map", map, &refs.maps, 160.0);
                    ui.end_row();
                    ui.label("Camera on tile").on_hover_text("Where the camera looks (off: the map's start spawn)");
                    ui.horizontal(|ui| {
                        let mut on = at.is_some();
                        if ui.checkbox(&mut on, "").changed() {
                            *at = on.then_some((10.0, 10.0));
                        }
                        if let Some((x, y)) = at {
                            ui.add(egui::DragValue::new(x).range(0.0..=1024.0).prefix("x "));
                            ui.add(egui::DragValue::new(y).range(0.0..=1024.0).prefix("y "));
                        }
                    });
                    ui.end_row();
                    ui.label("Hour").on_hover_text("The time of day the title screen shows (off: the hour new games start at)");
                    ui.horizontal(|ui| {
                        let mut on = hour.is_some();
                        if ui.checkbox(&mut on, "").changed() {
                            *hour = on.then_some(19.5);
                        }
                        if let Some(h) = hour {
                            ui.add(egui::DragValue::new(h).range(0.0..=23.99).speed(0.1).custom_formatter(|v, _| format!("{:02}:{:02}", v as u32, ((v.fract()) * 60.0).round() as u32)));
                        }
                        ui.checkbox(drift, "Drift").on_hover_text("The camera drifts slowly over the map");
                    });
                    ui.end_row();
                }
                TitleBackground::Picture { sheet } => {
                    ui.label("Picture").on_hover_text("A sprite sheet drawn whole, covering the window");
                    widgets::suggest(ui, "title_picture", sheet, &refs.sheets, 160.0);
                    ui.end_row();
                }
            }
            ui.label("Logo").on_hover_text("A picture drawn instead of the game's name");
            ui.horizontal(|ui| {
                let mut on = t.logo.is_some();
                if ui.checkbox(&mut on, "").changed() {
                    t.logo = on.then(|| TitleLogo::new(""));
                }
                if let Some(l) = &mut t.logo {
                    widgets::suggest(ui, "title_logo", &mut l.sheet, &refs.sheets, 140.0);
                    ui.add(egui::DragValue::new(&mut l.x).range(0.0..=1.0).speed(0.01).prefix("x ")).on_hover_text("Its middle, as a part of the window's width");
                    ui.add(egui::DragValue::new(&mut l.y).range(0.0..=1.0).speed(0.01).prefix("y ")).on_hover_text("Its middle, as a part of the window's height");
                    ui.add(egui::DragValue::new(&mut l.scale).range(0.25..=8.0).speed(0.05).suffix("x"));
                } else {
                    ui.weak("(the game's name)");
                }
            });
            ui.end_row();
            ui.label("Music").on_hover_text("Empty: the background map's music");
            widgets::suggest(ui, "title_music", &mut t.music, &refs.sounds, 160.0);
            ui.end_row();
            ui.label("Menu");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("title_align").selected_text(t.align.label()).show_ui(ui, |ui| {
                    for a in MenuAlign::ALL {
                        ui.selectable_value(&mut t.align, a, a.label());
                    }
                });
                ui.checkbox(&mut t.panel, "On a panel").on_hover_text("Off: the entries float over the background");
            });
            ui.end_row();
            ui.label("Darken").on_hover_text("How much the background is darkened behind the menu (off: 55 % over a map, 20 % over a picture)");
            ui.horizontal(|ui| {
                let mut on = t.dim.is_some();
                if ui.checkbox(&mut on, "").changed() {
                    t.dim = on.then_some(0.3);
                }
                if let Some(d) = &mut t.dim {
                    let mut pc = *d * 100.0;
                    if ui.add(egui::Slider::new(&mut pc, 0.0..=90.0).suffix(" %")).changed() {
                        *d = (pc / 100.0 * 100.0).round() / 100.0;
                    }
                }
            });
            ui.end_row();
        });
        ui.label("Entries");
        entries_ui(ui, "title", &mut t.entries, &TitleScreen::DEFAULT, true, &refs.functions);
        if ui.button(ui::t(ph::PLAY, "Play title screen")).on_hover_text("Play-tests the game from its title screen").clicked() {
            play = true;
        }
        ui.separator();
        ui.heading("Pause menu");
        let m = &mut c.menu;
        ui.horizontal(|ui| {
            ui.label("Place");
            egui::ComboBox::from_id_salt("pause_align").selected_text(m.align.label()).show_ui(ui, |ui| {
                for a in MenuAlign::ALL {
                    ui.selectable_value(&mut m.align, a, a.label());
                }
            });
        });
        entries_ui(ui, "pause", &mut m.entries, &PauseMenu::DEFAULT, false, &refs.functions);
        ui.separator();
        ui.heading("Credits");
        ui.weak("The text of the Credits page (add a Credits entry to a menu to show it). Translated like dialogs.");
        ui.add(egui::TextEdit::multiline(&mut c.credits).desired_rows(6).desired_width(f32::INFINITY).hint_text("Made by …"));
        ui.checkbox(&mut c.srd, "Uses SRD 5.1 content").on_hover_text(
            "The game uses material of the System Reference Document 5.1 (CC BY 4.0): SRD races, backgrounds, stat blocks. The Credits page then ends with the SRD's attribution, and srd_attribution() returns it for scripted credits. (Exports always carry SRD-5.1-ATTRIBUTION.txt: the game runner's rules include SRD text.)",
        );
    }
    if play {
        ed.play_title();
    }
}

/// A small sketch of the title screen: the background, the logo at its place, the menu at its
/// side with the entries in order.
fn title_preview(ui: &mut egui::Ui, t: &rustpg2d_engine::menus::TitleScreen, name: &str, start_map: &str, tex: &widgets::Textures) {
    use egui::{pos2, Rect};
    use rustpg2d_engine::menus::{MenuAlign, TitleBackground};
    let (w, h) = (384.0, 216.0);
    let k = w / 1280.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_rgb(28, 40, 26));
    let small = egui::FontId::proportional(11.0);
    match &t.background {
        TitleBackground::Picture { sheet } => match tex.sheets.get(sheet) {
            Some(img) => {
                let s = img.size_vec2();
                let f = (w / s.x).max(h / s.y);
                let r = Rect::from_center_size(rect.center(), s * f);
                p.image(img.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                p.text(rect.center_top() + vec2(0.0, 12.0), egui::Align2::CENTER_TOP, "(choose a picture)", small.clone(), Color32::GRAY);
            }
        },
        TitleBackground::Map { map, at, hour, drift } => {
            let map = if map.is_empty() { start_map } else { map.as_str() };
            let at = at.map_or("its start spawn".to_string(), |(x, y)| format!("tile {x}, {y}"));
            let hour = hour.map_or(String::new(), |h| format!(", {:02}:{:02}", h as u32, (h.fract() * 60.0).round() as u32));
            p.text(rect.left_bottom() + vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("map {map} at {at}{hour}{}", if *drift { ", drifting" } else { "" }), small.clone(), Color32::from_gray(200));
        }
    }
    p.rect_filled(rect, 0.0, Color32::from_black_alpha(if matches!(t.background, TitleBackground::Picture { .. }) { 50 } else { 120 }));
    // The menu (520 px wide in a 1280 x 720 window).
    let entries = t.entries();
    let mh = (entries.len() as f32 * 40.0 + 76.0) * k;
    let mw = 520.0 * k;
    let edge = (1280.0 * 0.06) * k;
    let mx = match t.align {
        MenuAlign::Left => rect.left() + edge,
        MenuAlign::Center => rect.center().x - mw / 2.0,
        MenuAlign::Right => rect.right() - edge - mw,
    };
    let menu = Rect::from_min_size(pos2(mx, rect.center().y - mh / 2.0 + 40.0 * k), vec2(mw, mh));
    if t.panel {
        p.rect_filled(menu, 2.0, Color32::from_rgba_unmultiplied(15, 15, 31, 235));
        p.rect_stroke(menu, 2.0, egui::Stroke::new(1.0_f32, Color32::from_rgb(217, 204, 153)), egui::StrokeKind::Inside);
    }
    for (i, e) in entries.iter().enumerate() {
        let y = menu.top() + (26.0 + i as f32 * 40.0) * k;
        p.text(pos2(menu.left() + 8.0, y), egui::Align2::LEFT_TOP, e.label(), small.clone(), if i == 0 { Color32::GOLD } else { Color32::WHITE });
    }
    match t.logo.as_ref().and_then(|l| tex.sheets.get(&l.sheet).map(|img| (l, img))) {
        Some((l, img)) => {
            let s = img.size_vec2() * l.scale * k;
            let c = pos2(rect.left() + l.x * w, rect.top() + l.y * h);
            p.image(img.id(), Rect::from_center_size(c, s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            p.text(pos2(rect.center().x, (menu.top() - 60.0 * k).max(rect.top() + 80.0 * k)), egui::Align2::CENTER_BOTTOM, name, egui::FontId::proportional(18.0), Color32::GOLD);
        }
    }
}

/// Project › Look: the UI skin the game uses.
fn look_form(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.heading("Look");
    ui.weak("The UI skin of the game's windows, menus, dialog box and HUD (define skins under Database › UI skins, or add one from the library). Built in: the dark blue look.");
    let ids: Vec<(String, String)> = ed.project.db.skins.iter().map(|d| (d.id.clone(), if d.name.is_empty() { d.id.clone() } else { d.name.clone() })).collect();
    let c = &mut ed.project.config;
    ui.horizontal(|ui| {
        ui.label("Skin");
        let shown = if c.skin.is_empty() { "Built in".to_string() } else { ids.iter().find(|(id, _)| *id == c.skin).map_or(format!("{} (missing)", c.skin), |(_, n)| n.clone()) };
        egui::ComboBox::from_id_salt("project_skin").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut c.skin, String::new(), "Built in");
            for (id, name) in &ids {
                ui.selectable_value(&mut c.skin, id.clone(), name);
            }
        });
    });
}

/// Project › Fonts: which font each kind of text uses, and the fallbacks.
fn fonts_form(ed: &mut Editor, ui: &mut egui::Ui) {
    use rustpg2d_engine::fonts::Role;
    ui.heading("Fonts");
    ui.weak("The fonts the game draws its texts with (define them under Database › Fonts). Without one, the game uses its built-in pixel font, which has only Latin letters (with accents and umlauts). A character a font lacks comes from the first fallback that has it: add a font for Japanese, Chinese or Korean there.");
    let ids: Vec<String> = ed.project.db.fonts.iter().map(|d| d.id.clone()).collect();
    let f = &mut ed.project.config.fonts;
    egui::Grid::new("fonts").num_columns(2).show(ui, |ui| {
        for (role, label, help) in [
            (Role::Ui, "Windows and menus", "Menus, windows, tooltips: every text without a font of its own below. Empty: the built-in font."),
            (Role::Dialog, "Dialogs", "Dialog and choice boxes, speech bubbles, captions. Empty: the windows' font."),
            (Role::Hud, "HUD", "Bars, the quickbar, toasts, the clock, damage numbers. Empty: the windows' font."),
            (Role::Title, "Title", "The game's name on the title screen. Empty: the windows' font."),
        ] {
            ui.label(label).on_hover_text(help);
            let v = f.get_mut(role);
            let shown = if v.is_empty() { if role == Role::Ui { "(built-in)".to_string() } else { "(as windows)".to_string() } } else { v.clone() };
            egui::ComboBox::from_id_salt(("font_role", role.name())).selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(v, String::new(), if role == Role::Ui { "(built-in)" } else { "(as windows)" });
                for id in &ids {
                    ui.selectable_value(v, id.clone(), id);
                }
            });
            ui.end_row();
        }
        let mut remove = None;
        for (k, id) in f.fallback.iter_mut().enumerate() {
            ui.label(if k == 0 { "Fallbacks" } else { "" }).on_hover_text("Tried in order for characters the role's font has no glyph for; the built-in font comes last.");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("font_fallback", k)).selected_text(id.clone()).show_ui(ui, |ui| {
                    for i in &ids {
                        ui.selectable_value(id, i.clone(), i);
                    }
                });
                if ui.small_button(ph::TRASH).on_hover_text("Remove this fallback").clicked() {
                    remove = Some(k);
                }
            });
            ui.end_row();
        }
        if let Some(k) = remove {
            f.fallback.remove(k);
        }
        ui.label(if f.fallback.is_empty() { "Fallbacks" } else { "" });
        if ui.add_enabled(!ids.is_empty(), egui::Button::new(ui::t(ph::PLUS, "Fallback font"))).clicked() {
            f.fallback.push(ids.first().cloned().unwrap_or_default());
        }
        ui.end_row();
    });
}

fn scatter_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::decor::ScatterDef, cx: &mut Cx) {
    use rustpg2d_engine::decor::DecorLayer;
    ui.heading("Scatter set");
    rename_row(ui, RefKind::Scatter, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.weak("The Decorations tool's scatter brush places these props at random: a forest, flowers in a meadow, rocks on a slope.");
    egui::Grid::new("scatter").num_columns(2).show(ui, |ui| {
        ui.label("Density").on_hover_text("Decorations per tile under the brush (0.05: a sparse wood, 0.5: flowers).");
        ui.add(egui::Slider::new(&mut d.density, 0.01..=2.0).logarithmic(true));
        ui.end_row();
        ui.label("Spacing").on_hover_text("The least distance between two of them (and from what is there already).");
        ui.add(egui::Slider::new(&mut d.spacing, 0.0..=128.0).suffix(" px"));
        ui.end_row();
        ui.label("Layer").on_hover_text("Flat: on the ground · Sorted: with characters by their feet · Above: over everyone");
        ui.horizontal(|ui| {
            for l in DecorLayer::ALL {
                ui.selectable_value(&mut d.layer, l, l.label());
            }
        });
        ui.end_row();
        ui.label("");
        ui.checkbox(&mut d.flip, "Mirror half of them");
        ui.end_row();
    });
    ui.separator();
    ui.label("Props (how often each comes)");
    let mut remove = None;
    for (k, (prop, weight)) in d.props.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            widgets::suggest(ui, &format!("scatter_prop{k}"), prop, &cx.refs.props, 180.0);
            ui.add(egui::DragValue::new(weight).range(1..=100).prefix("x "));
            if ui.small_button(ui::t(ph::TRASH, "")).on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        d.props.remove(k);
    }
    if ui.button(ui::t(ph::PLUS, "Prop")).clicked() {
        d.props.push((String::new(), 1));
    }
}

fn material_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::ground::MaterialDef, cx: &mut Cx) {
    use rustpg2d_engine::ground::{Edge, Rim, Walk};
    ui.heading("Material");
    rename_row(ui, RefKind::Material, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    // The fields, and beside them the preview (below them in a narrow window).
    ui.horizontal_wrapped(|ui| {
        ui.vertical(|ui| {
            egui::Grid::new("material").num_columns(2).show(ui, |ui| {
                ui.label("Texture").on_hover_text("A seamless picture (PNG, up to 256 x 256 px, sizes in steps of 8) repeated over the ground painted with this material.");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut d.texture).hint_text("materials/grass.png").desired_width(220.0));
                    if ui.button("Browse…").on_hover_text("Pick a PNG: files from elsewhere are copied into materials/").clicked() {
                        *cx.browse = Some(Purpose::MaterialTexture(d.id.clone()));
                    }
                    if d.texture.is_empty() && ui.button(ui::t(ph::PAINT_BRUSH, "Draw a new one")).on_hover_text("A new texture, materials/<id>.png, drawn below").clicked() {
                        d.texture = format!("materials/{}.png", d.id);
                    }
                });
                ui.end_row();
                ui.label("Edge").on_hover_text("Crisp: a hard edge in steps of the art's pixels, made ragged by noise (pixel art). Soft: a fade into the neighbour. Straight: exactly along the 8 px cells, with square corners (floors, carpets, plazas); its rim is a border.");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut d.edge, Edge::Crisp, "Crisp");
                    ui.selectable_value(&mut d.edge, Edge::Soft, "Soft");
                    ui.selectable_value(&mut d.edge, Edge::Straight, "Straight");
                });
                ui.end_row();
                let shaped = d.edge != Edge::Straight;
                ui.label("Raggedness").on_hover_text("How far noise moves the edge, in pixels (0 = smooth curves along the painted cells).");
                ui.add_enabled(shaped, egui::Slider::new(&mut d.edge_noise, 0.0..=4.0).suffix(" px"));
                ui.end_row();
                ui.label("Edge pixel").on_hover_text("The step size of a crisp edge: the size of the texture's pixels in world pixels (1, 2 or 4).");
                ui.add_enabled(shaped, egui::Slider::new(&mut d.edge_pixel, 1..=4).suffix(" px"));
                ui.end_row();
                ui.label("Priority").on_hover_text("Where two materials meet, the one with the higher priority is drawn over the other (its edge shape wins). Water over sand over grass, for example.");
                ui.add(egui::Slider::new(&mut d.priority, 0..=255));
                ui.end_row();
                ui.label("Rim").on_hover_text("A fringe drawn just outside the material's edge: a sandy band round a pond, a dark outline round a hole.");
                ui.horizontal(|ui| {
                    let mut on = d.rim.is_some();
                    if ui.checkbox(&mut on, "").changed() {
                        d.rim = on.then_some(Rim { color: [200, 180, 120], width: 3 });
                    }
                    if let Some(r) = &mut d.rim {
                        ui.color_edit_button_srgb(&mut r.color);
                        ui.add(egui::Slider::new(&mut r.width, 1..=8).suffix(" px"));
                    }
                });
                ui.end_row();
                ui.label("Walking").on_hover_text("Walkable; Solid blocks walking, shots and sight (a cliff); Low blocks walking only, shots fly over it (water).");
                egui::ComboBox::from_id_salt("material_walk").selected_text(d.walk.label()).show_ui(ui, |ui| {
                    for w in Walk::ALL {
                        ui.selectable_value(&mut d.walk, w, w.label());
                    }
                });
                ui.end_row();
                ui.label("In fights");
                ui.checkbox(&mut d.difficult, "Difficult terrain").on_hover_text("A step onto it costs double movement in turn-based fights (mud, deep snow, shallow water, rubble)");
                ui.end_row();
                ui.label("Footsteps").on_hover_text("Sound played for each step on it (empty: none).");
                widgets::suggest(ui, "material_step", &mut d.step, &cx.refs.sounds, 200.0);
                ui.end_row();
                ui.label("Flow").on_hover_text("The texture drifts this many pixels per second (flowing water; 0 = still).");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut d.scroll[0]).speed(0.5).prefix("x "));
                    ui.add(egui::DragValue::new(&mut d.scroll[1]).speed(0.5).prefix("y "));
                });
                ui.end_row();
                ui.label("Animation").on_hover_text("Animation frames: the texture holds this many frames side by side (each one seamless), shown one after the other. Together they fit in 256 x 256 px. Changing the number adds copies of the last frame to the texture below (or removes frames at the end): draw the differences, then Save.");
                ui.horizontal(|ui| {
                    let before = d.frames;
                    ui.add(egui::DragValue::new(&mut d.frames).range(1..=32).suffix(if before == 1 { " frame" } else { " frames" }));
                    if before == 1 && d.frames > 1 && d.fps == 0.0 {
                        d.fps = 4.0;
                    }
                    ui.add_enabled(d.frames > 1, egui::DragValue::new(&mut d.fps).range(0.0..=25.0).speed(0.1).fixed_decimals(1).suffix(" per second"));
                });
                ui.end_row();
            });
        });
        ui.add_space(16.0);
        ui.vertical(|ui| cx.mat_preview.ui(ui, d, cx.db, cx.assets, cx.drawing));
    });
    ui.weak("Paint it on maps with the Ground tool. Draw the texture below: its copies show where the edges meet, and drawing wraps across them. Save draws it on the maps.");
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
    ui.separator();
    fonts_form(ed, ui);
    ui.separator();
    look_form(ed, ui);
    ui.separator();
    menus_form(ed, ui);
    let c = &mut ed.project.config;
    ui.separator();
    ui.heading("Rules (SRD 5.1)");
    rules_form(ui, &mut c.rules, &ed.refs.skills);
    ui.separator();
    ui.heading("Magic");
    {
        let s = &mut c.skills;
        ui.checkbox(&mut s.magic, "Spells tab for every hero").on_hover_text("The Spells tab (L) shows for heroes of every class (else only for casters and heroes who know spells). Spells are the SRD's (Database › Spells): casters cast them with spell slots by class level.");
    }
    ui.separator();
    ui.heading("Experience and levels");
    {
        let p = &mut c.progression;
        ui.checkbox(&mut p.enabled, "Experience and levels (defeated characters and completed quests give XP)")
            .on_hover_text("Set the XP per character (Characters › Combat) and per quest (Quests › Experience reward). The HUD shows the level and an XP bar.");
        ui.add_enabled_ui(p.enabled, |ui| {
            egui::Grid::new("progression_cfg").num_columns(2).show(ui, |ui| {
                ui.label("Highest level");
                ui.add(egui::DragValue::new(&mut p.max_level).range(1..=20));
                ui.end_row();
                ui.label("Hit points per level").on_hover_text("The hero's hit die (a d10 until classes): its average (6) or a roll, plus the CON modifier");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut p.hit_points, rustpg2d_engine::progression::HitPoints::Average, "Average");
                    ui.selectable_value(&mut p.hit_points, rustpg2d_engine::progression::HitPoints::Rolled, "Rolled");
                });
                ui.end_row();
            });
            ui.weak("Levels follow the SRD 5.1's experience table:");
            let steps: Vec<String> = (2..=p.max_level.clamp(1, 8)).map(|l| format!("level {l}: {} XP", p.xp_for_level(l))).collect();
            if !steps.is_empty() {
                ui.weak(format!("{}{}", steps.join(", "), if p.max_level > 8 { format!(" … level 20: {} XP", p.xp_for_level(20)) } else { String::new() }));
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
        ui.label("Battle music").on_hover_text("Played while the player fights (enemies hunting them or a companion, the player striking), faded in and back to the map's music a few seconds after the fight; empty = the map's music goes on. A map can name another track or none (map properties).");
        widgets::suggest(ui, "battle_music", &mut c.battle_music, &ed.refs.sounds, 140.0);
        ui.end_row();
    });
    ui.separator();
    hooks_form(ui, &mut c.hooks, &ed.refs.functions);
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
        ui.horizontal(|ui| {
            ui.label("Ability scores").on_hover_text("How the player sets the scores on the creator's second page (none: the scores of Rules)");
            for m in rustpg2d_engine::srd::abilities::AbilityMethod::ALL {
                let mut on = cc.ability_methods.contains(&m);
                if ui.checkbox(&mut on, m.label()).changed() {
                    if on {
                        cc.ability_methods.push(m);
                        cc.ability_methods.sort_by_key(|x| rustpg2d_engine::srd::abilities::AbilityMethod::ALL.iter().position(|y| y == x));
                    } else {
                        cc.ability_methods.retain(|x| *x != m);
                    }
                }
            }
        });
        ui.weak("The creator's second page: the scores, the race's choices (Database \u{E13A} Races) and a background (Database \u{E13A} Backgrounds).");
    }
    ui.weak("Dialogs address the player with {player} (name) and {sir|madam} (male|female); conditions have 'Player sex'; scripts have player_name(), player_sex(), is_female(), is_male(), text(\"...\").");
    ui.separator();
    match &ed.project_file {
        Some(pf) => {
            ui.label(format!("Project file: {}", pf.path.display()));
            ui.weak(format!("Working copy: {}", pf.work.display())).on_hover_text("The editor works on these files and writes the project file again whenever they change");
        }
        None => {
            ui.label(format!("Project folder: {}", ed.project.root.display()));
        }
    }
}

/// *Script hooks* in Database › Project: which script function the engine calls where
/// (`rustpg2d_engine::hooks`), in priority order.
fn hooks_form(ui: &mut egui::Ui, list: &mut Vec<rustpg2d_engine::hooks::HookDef>, functions: &[String]) {
    use rustpg2d_engine::hooks::{HookDef, HOOKS};
    ui.heading("Script hooks");
    ui.weak("Script functions that change what the engine does: each gets a map (#{target, amount, …}) and returns it changed, or nothing to keep it. Several on one hook run by priority, lower first. Hover a hook for its map.");
    let mut remove = None;
    if !list.is_empty() {
        egui::Grid::new("hooks").num_columns(4).striped(true).show(ui, |ui| {
            ui.weak("Hook");
            ui.weak("Function");
            ui.weak("Priority");
            ui.end_row();
            for (k, h) in list.iter_mut().enumerate() {
                let help = HOOKS.iter().find(|x| x.0 == h.hook).map_or_else(|| "Not a hook the engine calls".to_string(), |x| format!("#{{{}}}: {}", x.1, x.2));
                egui::ComboBox::from_id_salt(("hook_name", k))
                    .selected_text(if rustpg2d_engine::hooks::known(&h.hook) { h.hook.clone() } else { format!("{} (unknown)", h.hook) })
                    .show_ui(ui, |ui| {
                        for (name, fields, what) in HOOKS {
                            ui.selectable_value(&mut h.hook, name.to_string(), *name).on_hover_text(format!("#{{{fields}}}: {what}"));
                        }
                    })
                    .response
                    .on_hover_text(help);
                widgets::suggest(ui, &format!("hook_fn{k}"), &mut h.function, functions, 160.0);
                ui.add(egui::DragValue::new(&mut h.priority).range(-1000..=1000)).on_hover_text("Lower runs first");
                if crate::ui::icon_button(ui, true, ph::TRASH, "Remove hook", "", "The engine no longer calls the function here").clicked() {
                    remove = Some(k);
                }
                ui.end_row();
            }
        });
    }
    if let Some(k) = remove {
        list.remove(k);
    }
    if ui.button(crate::ui::t(ph::PLUS, "Add hook")).on_hover_text("Call a script function at one of the engine's hooks").clicked() {
        list.push(HookDef { hook: "damage".into(), function: String::new(), priority: 0 });
    }
}

/// An SRD stat block: a template (the SRD's NPCs: choosing one fills the block and, with `xp`, sets
/// the character's experience to the template's), *By hand*, the fields and *Remove*.
pub fn stat_block_ui(ui: &mut egui::Ui, salt: &str, stats: &mut Option<rustpg2d_engine::srd::StatBlock>, xp: Option<&mut u32>) {
    use rustpg2d_engine::srd::{abilities, npc_templates, Ability, StatBlock};
    let mut pick: Option<&StatBlock> = None;
    ui.horizontal(|ui| {
        ui.label("Template");
        let current = match stats.as_ref() {
            None => "(no stat block)".to_string(),
            Some(s) if s.template.is_empty() => "(by hand)".to_string(),
            Some(s) => format!("{} (CR {})", s.name, s.cr),
        };
        egui::ComboBox::from_id_salt((salt, "template"))
            .selected_text(current)
            .height(420.0)
            .show_ui(ui, |ui| {
                for t in npc_templates() {
                    let on = stats.as_ref().is_some_and(|s| s.template == t.template);
                    if ui.selectable_label(on, format!("{}  (CR {}, {} XP)", t.name, t.cr, t.xp)).clicked() {
                        pick = Some(t);
                    }
                }
            })
            .response
            .on_hover_text("The SRD 5.1's NPC stat blocks (commoner, guard, bandit, veteran…): choosing one fills in the whole block and sets the character's Experience to the template's XP. Change any field afterwards.");
        if stats.is_none() && ui.button(crate::ui::t(ph::PLUS, "By hand")).on_hover_text("An empty stat block to fill in").clicked() {
            *stats = Some(StatBlock::default());
        }
        if stats.is_some() && ui.button(crate::ui::t(ph::TRASH, "Remove stat block")).clicked() {
            *stats = None;
        }
    });
    if let Some(t) = pick {
        *stats = Some(t.clone());
        if let Some(x) = xp {
            *x = t.xp;
        }
    }
    let Some(s) = stats else { return };
    ui.horizontal_wrapped(|ui| {
        for a in Ability::ALL {
            let mut v = s.abilities.score(a);
            ui.label(a.short());
            if ui.add(egui::DragValue::new(&mut v).range(1..=30)).on_hover_text(a.name()).changed() {
                s.abilities.set(a, v);
            }
            ui.weak(format!("({})", abilities::signed(s.abilities.modifier(a) as i64)));
        }
    });
    egui::Grid::new((salt, "fields")).num_columns(2).show(ui, |ui| {
        ui.label("Armor class");
        ui.add(egui::DragValue::new(&mut s.ac).range(0..=30));
        ui.end_row();
        ui.label("Hit points");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut s.hp).range(1..=999));
            ui.label("hit dice");
            ui.add(egui::TextEdit::singleline(&mut s.hit_dice).desired_width(80.0)).on_hover_text("As the SRD writes them: \"2d8+2\" (the hit points are their average)");
        });
        ui.end_row();
        ui.label("Size and type");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt((salt, "size")).selected_text(s.size.name()).show_ui(ui, |ui| {
                for z in rustpg2d_engine::srd::Size::ALL {
                    ui.selectable_value(&mut s.size, z, z.name());
                }
            })
            .response
            .on_hover_text("Scales the character's look: Tiny 50 %, Small 75 %, Large 150 %, Huge 200 %, Gargantuan 300 % (in fights it takes one tile for now)");
            ui.add(egui::TextEdit::singleline(&mut s.creature_type).desired_width(160.0).hint_text("beast, undead…"));
        });
        ui.end_row();
        ui.label("Speed");
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::DragValue::new(&mut s.speed_ft).range(0..=120).suffix(" ft"));
            ui.label("fly");
            ui.add(egui::DragValue::new(&mut s.speeds.fly).range(0..=200).suffix(" ft")).on_hover_text("Fliers move at their best speed in fights");
            ui.label("swim");
            ui.add(egui::DragValue::new(&mut s.speeds.swim).range(0..=200).suffix(" ft"));
            ui.label("climb");
            ui.add(egui::DragValue::new(&mut s.speeds.climb).range(0..=200).suffix(" ft"));
            ui.label("darkvision");
            ui.add(egui::DragValue::new(&mut s.darkvision_ft).range(0..=300).suffix(" ft"));
        });
        ui.end_row();
        ui.label("Senses");
        ui.add(egui::TextEdit::singleline(&mut s.senses).desired_width(320.0).hint_text("blindsight 30 ft., passive Perception 10"));
        ui.end_row();
        ui.label("Languages");
        ui.add(egui::TextEdit::singleline(&mut s.languages).desired_width(320.0));
        ui.end_row();
        ui.label("Alignment");
        ui.add(egui::TextEdit::singleline(&mut s.alignment).desired_width(200.0).hint_text("any alignment")).on_hover_text("As the SRD writes it: lawful good, neutral, chaotic evil, any alignment…");
        ui.end_row();
        ui.label("Challenge");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut s.cr).desired_width(40.0)).on_hover_text("Challenge rating: 0, 1/8, 1/4, 1/2, 1, 2…");
            ui.add(egui::DragValue::new(&mut s.xp).range(0..=100_000).suffix(" XP")).on_hover_text("What the SRD says it is worth (the character's Experience under Combat is what defeating it gives)");
        });
        ui.end_row();
    });
    ui.label("Resistant to").on_hover_text("Damage types it takes half damage from");
    ui.horizontal_wrapped(|ui| damage_type_toggles(ui, &mut s.resistances));
    ui.label("Immune to").on_hover_text("Damage types that don't hurt it");
    ui.horizontal_wrapped(|ui| damage_type_toggles(ui, &mut s.immunities));
    ui.label("Vulnerable to").on_hover_text("Damage types it takes double damage from");
    ui.horizontal_wrapped(|ui| damage_type_toggles(ui, &mut s.vulnerabilities));
    ui.label("Can't get").on_hover_text("Conditions it is immune to");
    ui.horizontal_wrapped(|ui| {
        for c in rustpg2d_engine::srd::Condition::ALL {
            let mut on = s.condition_immunities.iter().any(|i| i == c.id());
            if ui.toggle_value(&mut on, c.name()).on_hover_text(c.rules_text()).changed() {
                if on {
                    s.condition_immunities.push(c.id().to_string());
                } else {
                    s.condition_immunities.retain(|i| i != c.id());
                }
            }
        }
    });
    ui.label("Proficiencies").on_hover_text("Saving throws and skills it adds its proficiency bonus to");
    ui.horizontal_wrapped(|ui| {
        let mut toggle = |ui: &mut egui::Ui, id: String, label: String| {
            let mut on = s.proficiencies.contains(&id);
            if ui.toggle_value(&mut on, label).changed() {
                if on {
                    s.proficiencies.push(id);
                } else {
                    s.proficiencies.retain(|p| *p != id);
                }
            }
        };
        for a in Ability::ALL {
            toggle(ui, format!("save:{}", a.id()), format!("{} save", a.short()));
        }
        for (id, name, _) in abilities::SKILLS {
            toggle(ui, id.to_string(), name.to_string());
        }
    });
    stat_attacks_ui(ui, salt, s);
    stat_spellcasting_ui(ui, salt, s);
    stat_features_ui(ui, salt, s);
}

/// A stat block's *Spellcasting* (the fight AI casts from it): caster level, ability, save DC,
/// spell attack bonus, slots per level as text (`4 3` = four 1st-level, three 2nd-level) and the
/// spells (by id; innate ones without slots, at will or a number of times a day).
fn stat_spellcasting_ui(ui: &mut egui::Ui, salt: &str, s: &mut rustpg2d_engine::srd::StatBlock) {
    use rustpg2d_engine::srd::{Ability, StatSpell, StatSpellcasting};
    let n = s.spellcasting.as_ref().map_or(0, |c| c.spells.len());
    egui::CollapsingHeader::new(format!("Spellcasting ({n})")).id_salt((salt, "spellcasting")).default_open(n > 0).show(ui, |ui| {
        let mut on = s.spellcasting.is_some();
        if ui.checkbox(&mut on, "Casts spells").on_hover_text("Its spells in fights: the AI casts them like a character's (the project needs the spells)").changed() {
            s.spellcasting = on.then(StatSpellcasting::default);
        }
        let Some(c) = &mut s.spellcasting else { return };
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::DragValue::new(&mut c.level).range(0..=20).prefix("level ")).on_hover_text("Its spellcaster level: a cantrip's damage grows with it");
            egui::ComboBox::from_id_salt((salt, "spell_ability")).width(60.0).selected_text(c.ability.short()).show_ui(ui, |ui| {
                for ab in Ability::ALL {
                    ui.selectable_value(&mut c.ability, ab, ab.short());
                }
            });
            ui.add(egui::DragValue::new(&mut c.dc).range(1..=30).prefix("DC ")).on_hover_text("The spell save DC");
            ui.add(egui::DragValue::new(&mut c.attack).range(-5..=20).prefix("attack +")).on_hover_text("The spell attack bonus");
            ui.label("slots");
            let mut text = c.slots.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(90.0).hint_text("4 3")).on_hover_text("Spell slots per level, from the 1st: \"4 3\" = four 1st-level and three 2nd-level slots").changed() {
                c.slots = text.split_whitespace().filter_map(|w| w.parse().ok()).take(9).collect();
            }
        });
        let mut gone = None;
        egui::Grid::new((salt, "stat_spells")).num_columns(5).show(ui, |ui| {
            if !c.spells.is_empty() {
                for h in ["Spell", "Level", "Innate", "Per day", ""] {
                    ui.weak(h);
                }
                ui.end_row();
            }
            for (k, sp) in c.spells.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(&mut sp.spell).desired_width(120.0).hint_text("sacred-flame")).on_hover_text("The spell's id in Database \u{E13A} Spells");
                ui.add(egui::DragValue::new(&mut sp.level).range(0..=9)).on_hover_text("Its level (0 = a cantrip, at will)");
                ui.checkbox(&mut sp.innate, "").on_hover_text("Innate: cast without a slot");
                if sp.innate {
                    ui.add(egui::DragValue::new(&mut sp.per_day).range(0..=9)).on_hover_text("Times a day (0 = at will)");
                } else {
                    ui.label("");
                }
                if ui.small_button(ph::TRASH).on_hover_text("Remove the spell").clicked() {
                    gone = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some(k) = gone {
            c.spells.remove(k);
        }
        if ui.button(crate::ui::t(ph::PLUS, "Add spell")).clicked() {
            c.spells.push(StatSpell { spell: "fire-bolt".into(), ..Default::default() });
        }
    });
}

/// A stat block's traits and special actions in the SRD's words (they act with 0.32; the
/// bestiary shows them).
fn stat_features_ui(ui: &mut egui::Ui, salt: &str, s: &mut rustpg2d_engine::srd::StatBlock) {
    use rustpg2d_engine::srd::{FeatureKind, StatFeature};
    let title = format!("Traits and special actions ({})", s.features.len());
    egui::CollapsingHeader::new(title).id_salt((salt, "features")).show(ui, |ui| {
        ui.weak("Texts only for now: the special abilities that act come with a later release. Attacks are above.");
        let mut remove = None;
        for (k, f) in s.features.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt((salt, "fk", k)).selected_text(f.kind.name()).show_ui(ui, |ui| {
                    for kind in [FeatureKind::Trait, FeatureKind::Action, FeatureKind::Reaction, FeatureKind::Legendary] {
                        ui.selectable_value(&mut f.kind, kind, kind.name());
                    }
                });
                ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(200.0).hint_text("Pack Tactics"));
                if ui.small_button(ph::TRASH).on_hover_text("Remove").clicked() {
                    remove = Some(k);
                }
            });
            ui.add(egui::TextEdit::multiline(&mut f.text).desired_rows(2).desired_width(f32::INFINITY));
        }
        if let Some(k) = remove {
            s.features.remove(k);
        }
        if ui.button(crate::ui::t(ph::PLUS, "Add trait")).clicked() {
            s.features.push(StatFeature { name: "Trait".into(), ..Default::default() });
        }
    });
}

/// A stat block's attacks in fights (name, attack bonus, damage dice and type, melee reach or a
/// range) and how many it makes with its Attack action (Multiattack).
/// An attack's extra damage parts and riders (a dragon's bite's fire, a wolf's knock prone).
fn stat_hit_ui(ui: &mut egui::Ui, salt: &str, a: &mut rustpg2d_engine::srd::StatAttack) {
    use rustpg2d_engine::srd::{Ability, DamagePart, HitRider};
    let mut gone = None;
    for (k, x) in a.extra.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label("plus");
            ui.add(egui::TextEdit::singleline(&mut x.dice).desired_width(64.0).hint_text("2d6"));
            widgets::damage_type_combo(ui, &format!("{salt}/x{k}"), &mut x.damage_type, "untyped");
            if ui.small_button(ph::TRASH).on_hover_text("Remove this damage").clicked() {
                gone = Some(k);
            }
        });
    }
    if let Some(k) = gone {
        a.extra.remove(k);
    }
    let mut gone = None;
    for (k, r) in a.on_hit.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let save = r.save.map_or("no save", |s| s.short());
            egui::ComboBox::from_id_salt((salt, "save", k)).width(70.0).selected_text(save).show_ui(ui, |ui| {
                ui.selectable_value(&mut r.save, None, "no save");
                for ab in Ability::ALL {
                    ui.selectable_value(&mut r.save, Some(ab), ab.short());
                }
            });
            if r.save.is_some() {
                ui.label("DC");
                ui.add(egui::DragValue::new(&mut r.dc).range(1..=30));
            }
            ui.label("or");
            widgets::condition_combo(ui, &format!("{salt}/c{k}"), &mut r.condition, "no condition");
            if r.condition.is_some() {
                ui.add(egui::DragValue::new(&mut r.rounds).range(0..=600).suffix(" rounds")).on_hover_text("How long it lasts (0 = until it ends: escaped, removed)");
                ui.add(egui::DragValue::new(&mut r.escape_dc).range(0..=30).prefix("escape DC ")).on_hover_text("A grapple's or restraint's escape DC (0 = none): the target's Escape action rolls Athletics or Acrobatics against it");
            }
            ui.label("damage");
            ui.add(egui::TextEdit::singleline(&mut r.dice).desired_width(54.0).hint_text("2d8")).on_hover_text("Damage on a failed save (empty = none)");
            widgets::damage_type_combo(ui, &format!("{salt}/d{k}"), &mut r.damage_type, "untyped");
            ui.checkbox(&mut r.half, "half on a save");
            if ui.small_button(ph::TRASH).on_hover_text("Remove this effect").clicked() {
                gone = Some(k);
            }
        });
    }
    if let Some(k) = gone {
        a.on_hit.remove(k);
    }
    ui.horizontal(|ui| {
        if ui.small_button(crate::ui::t(ph::PLUS, "Damage")).on_hover_text("More damage of another type on a hit (plus 2d6 fire)").clicked() {
            a.extra.push(DamagePart { dice: "1d6".into(), damage_type: None });
        }
        if ui.small_button(crate::ui::t(ph::PLUS, "Effect")).on_hover_text("A saving throw against a condition or more damage on a hit (a DC 11 STR save or prone)").clicked() {
            a.on_hit.push(HitRider { save: Some(Ability::Str), dc: 11, condition: Some(rustpg2d_engine::srd::Condition::Prone), ..Default::default() });
        }
    });
}

fn stat_attacks_ui(ui: &mut egui::Ui, salt: &str, s: &mut rustpg2d_engine::srd::StatBlock) {
    ui.horizontal(|ui| {
        ui.label("Attacks").on_hover_text("What it attacks with in fights. Without any it attacks with its weapon or its old damage (+2).");
        ui.label("per turn");
        ui.add(egui::DragValue::new(&mut s.multiattack).range(0..=6)).on_hover_text("Multiattack: how many attacks its Attack action makes (0 or 1 = one)");
    });
    let mut remove = None;
    egui::Grid::new((salt, "attacks")).num_columns(7).show(ui, |ui| {
        if !s.attacks.is_empty() {
            for h in ["Name", "Bonus", "Damage", "Type", "Reach", "Range", ""] {
                ui.weak(h);
            }
            ui.end_row();
        }
        for (k, a) in s.attacks.iter_mut().enumerate() {
            ui.add(egui::TextEdit::singleline(&mut a.name).desired_width(90.0).hint_text("Scimitar"));
            ui.add(egui::DragValue::new(&mut a.bonus).range(-5..=20).prefix("+")).on_hover_text("The attack roll's bonus (ability modifier + proficiency)");
            let bad = !a.dice.trim().is_empty() && rustpg2d_engine::srd::Expr::parse(&a.dice).is_err();
            let r = ui.add(egui::TextEdit::singleline(&mut a.dice).desired_width(64.0).hint_text("1d6+2"));
            if bad {
                r.on_hover_text("Not a dice expression (like 1d6+2)");
            }
            widgets::damage_type_combo(ui, &format!("{salt}/at{k}"), &mut a.damage_type, "untyped");
            ui.add(egui::DragValue::new(&mut a.reach_ft).range(0..=30).suffix(" ft").speed(5.0)).on_hover_text("Melee reach (5 ft = next to it; 0 = no melee)");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut a.range_ft).range(0..=600).suffix(" ft").speed(5.0)).on_hover_text("Normal range of a ranged attack (0 = none)");
                ui.label("/");
                ui.add(egui::DragValue::new(&mut a.long_ft).range(0..=1200).suffix(" ft").speed(5.0)).on_hover_text("Long range: farther shots have disadvantage");
            });
            if ui.small_button(ph::TRASH).on_hover_text("Remove the attack").clicked() {
                remove = Some(k);
            }
            ui.end_row();
        }
    });
    if let Some(k) = remove {
        s.attacks.remove(k);
    }
    // What a hit does besides its damage: more damage of other types, a save against a condition
    // or more damage, a grapple's escape DC.
    for (k, a) in s.attacks.iter_mut().enumerate() {
        let n = a.extra.len() + a.on_hit.len();
        egui::CollapsingHeader::new(format!("{}: on a hit ({n})", if a.name.is_empty() { "Attack" } else { &a.name }))
            .id_salt((salt, "onhit", k))
            .default_open(n > 0)
            .show(ui, |ui| stat_hit_ui(ui, &format!("{salt}/hit{k}"), a));
    }
    if ui.button(crate::ui::t(ph::PLUS, "Add attack")).clicked() {
        s.attacks.push(rustpg2d_engine::srd::StatAttack { name: "Attack".into(), bonus: 2, dice: "1d6".into(), reach_ft: 5, ..Default::default() });
    }
}

/// The 13 damage types as toggles, for a list of type ids (resistances…).
pub fn damage_type_toggles(ui: &mut egui::Ui, list: &mut Vec<String>) {
    for t in rustpg2d_engine::srd::DamageType::ALL {
        let mut on = list.iter().any(|r| r == t.id());
        if ui.toggle_value(&mut on, t.name()).changed() {
            if on {
                list.push(t.id().to_string());
            } else {
                list.retain(|r| r != t.id());
            }
        }
    }
    // The SRD's longer entries ("bludgeoning, piercing, and slashing from nonmagical attacks").
    let other: Vec<String> = list.iter().filter(|r| rustpg2d_engine::srd::DamageType::parse(r).is_none()).cloned().collect();
    for o in other {
        if ui.small_button(format!("{o}  {}", ph::X)).on_hover_text("Remove").clicked() {
            list.retain(|r| *r != o);
        }
    }
}

/// A *Faction* field: the project's factions; `inherited` = what "(none)" stands for ("its
/// race's: humans"); *New faction…* switches to the Factions tab.
pub fn faction_combo(ui: &mut egui::Ui, salt: &str, v: &mut String, factions: &[String], inherited: &str, help: &str) {
    ui.horizontal(|ui| {
        ui.label("Faction");
        let none = if inherited.is_empty() { "(none)".to_string() } else { format!("({inherited})") };
        let label = if v.is_empty() { none.clone() } else if factions.contains(v) { v.clone() } else { format!("{v} (missing)") };
        egui::ComboBox::from_id_salt(salt).selected_text(label).show_ui(ui, |ui| {
            ui.selectable_value(v, String::new(), none);
            for f in factions {
                ui.selectable_value(v, f.clone(), f);
            }
            ui.separator();
            if ui.button(crate::ui::t(ph::PLUS, "New faction…")).on_hover_text("Opens Database \u{E13A} Factions to add one").clicked() {
                OPEN_FACTIONS.with(|o| o.set(true));
            }
        })
        .response
        .on_hover_text(help);
    });
}

thread_local! {
    /// *New faction…* in a Faction field: the Database switches to the Factions tab.
    static OPEN_FACTIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a *New faction…* entry was clicked since the last call.
pub fn take_open_factions() -> bool {
    OPEN_FACTIONS.with(|o| o.replace(false))
}

/// Database › Factions: the factions, their attitudes and who belongs to each (races, character
/// definitions and placed characters), membership changed from the faction's side too.
fn factions_ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let placed = ed.placed_factions();
    let mut jump = None;
    let mut area = egui::ScrollArea::vertical().id_salt("factions_scroll").auto_shrink(false);
    if let Some(y) = ed.db.form_scroll.take() {
        area = area.vertical_scroll_offset(y);
    }
    let (config, db) = (&mut ed.project.config, &mut ed.project.db);
    area.show(ui, |ui| {
        factions_form(ui, &mut config.factions);
        if config.factions.is_empty() {
            return;
        }
        ui.separator();
        ui.heading("Members");
        ui.weak("Who belongs to each faction: a character belongs to its own Faction (Database \u{E13A} Characters), else its race's; a placed character can have one of its own (the map's inspector). In play the 'Set faction' action and set_faction() move characters and the player into another faction.");
        let ids: Vec<(String, String)> = config.factions.list.iter().map(|f| (f.id.clone(), f.name.clone())).collect();
        for (id, name) in &ids {
            let races: Vec<usize> = (0..db.races.len()).filter(|&k| db.races[k].faction == *id).collect();
            let own: Vec<usize> = (0..db.characters.len()).filter(|&k| db.characters[k].faction == *id).collect();
            let by_race: Vec<String> = db.characters.iter().filter(|c| c.faction.is_empty() && db.races.iter().any(|r| r.id == c.race && r.faction == *id)).map(|c| c.id.clone()).collect();
            let here: Vec<&(String, u32, String, String)> = placed.iter().filter(|p| p.3 == *id).collect();
            let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
            let title = format!("{name} ({id}): {}, {}, {} placed", n(races.len(), "race", "races"), n(own.len() + by_race.len(), "character", "characters"), here.len());
            egui::CollapsingHeader::new(title).id_salt(("faction_members", id)).default_open(true).show(ui, |ui| {
                // Races.
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Races:");
                    let mut out = None;
                    for &k in &races {
                        if ui.button(crate::ui::t(ph::X, &db.races[k].name)).on_hover_text(format!("Take the race '{}' out of {name}", db.races[k].id)).clicked() {
                            out = Some(k);
                        }
                    }
                    if let Some(k) = out {
                        db.races[k].faction.clear();
                    }
                    let mut add = None;
                    egui::ComboBox::from_id_salt(("faction_add_race", id)).selected_text("Add a race…").show_ui(ui, |ui| {
                        for (k, r) in db.races.iter().enumerate().filter(|(_, r)| r.faction != *id) {
                            let from = if r.faction.is_empty() { String::new() } else { format!(" (now {})", r.faction) };
                            if ui.selectable_label(false, format!("{}{from}", r.name)).clicked() {
                                add = Some(k);
                            }
                        }
                    });
                    if let Some(k) = add {
                        db.races[k].faction = id.clone();
                    }
                });
                // Character definitions: their own faction, or their race's.
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Characters:");
                    let mut out = None;
                    for &k in &own {
                        if ui.button(crate::ui::t(ph::X, &db.characters[k].name)).on_hover_text(format!("Take the character '{}' out of {name} (it then belongs to its race's faction)", db.characters[k].id)).clicked() {
                            out = Some(k);
                        }
                    }
                    for c in &by_race {
                        ui.weak(format!("{c} (by race)")).on_hover_text("Belongs to it through its race (Database \u{E13A} Races)");
                    }
                    if let Some(k) = out {
                        db.characters[k].faction.clear();
                    }
                    let mut add = None;
                    egui::ComboBox::from_id_salt(("faction_add_char", id)).selected_text("Add a character…").show_ui(ui, |ui| {
                        for (k, c) in db.characters.iter().enumerate().filter(|(_, c)| c.faction != *id) {
                            let from = if c.faction.is_empty() { String::new() } else { format!(" (now {})", c.faction) };
                            if ui.selectable_label(false, format!("{} [{}]{from}", c.name, c.id)).clicked() {
                                add = Some(k);
                            }
                        }
                    });
                    if let Some(k) = add {
                        db.characters[k].faction = id.clone();
                    }
                });
                // Placed characters with this faction of their own (set in the map's inspector).
                if !here.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong("Placed:");
                        for (map, uid, label, _) in &here {
                            if ui.link(format!("{label} on {map}")).on_hover_text("Show it on the map (its Faction is in the inspector)").clicked() {
                                jump = Some(crate::search::Target::Entity(map.clone(), *uid));
                            }
                        }
                    });
                }
            });
        }
    });
    if let Some(t) = jump {
        ed.jump_to(&t);
    }
}

/// The faction list and attitude table of Database › Factions: the list (ids are fixed once added: triggers and scripts
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
                    self.watch_note(&f);
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
                    Tab::Props => self.project.db.props.get(i).map(|d| d.state_look(self.db.state_sel.min(d.states.len())).0
                            .clone()),
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

/// Project › Rules: the new hero's ability scores and proficiencies (until the character creator
/// sets them), and whether checks show a line on screen.
pub(crate) fn rules_form(ui: &mut egui::Ui, r: &mut rustpg2d_engine::srd::RulesSettings, suggestions: &[String]) {
    use rustpg2d_engine::srd::abilities::{self, Ability};
    ui.weak("Every roll in the game follows the SRD 5.1: ability checks and saving throws roll a d20 plus the ability modifier, plus the proficiency bonus (+2 at level 1) when the hero is proficient. The Skills tab (K) shows them to the player.");
    ui.label("Ability scores of a new hero");
    egui::Grid::new("rules_abilities").num_columns(6).show(ui, |ui| {
        for a in Ability::ALL {
            ui.label(a.short()).on_hover_text(a.name());
        }
        ui.end_row();
        for a in Ability::ALL {
            let mut v = r.abilities.score(a);
            if ui.add(egui::DragValue::new(&mut v).range(1..=30)).on_hover_text(format!("{} {} (modifier {})", a.name(), v, abilities::signed(abilities::modifier(v) as i64))).changed() {
                r.abilities.set(a, v);
            }
        }
        ui.end_row();
    });
    ui.horizontal(|ui| {
        if ui.small_button("Standard array").on_hover_text("15 14 13 12 10 8").clicked() {
            r.abilities = rustpg2d_engine::srd::Abilities::STANDARD;
        }
        if ui.small_button("All 10").clicked() {
            r.abilities = rustpg2d_engine::srd::Abilities::default();
        }
    });
    ui.label("Proficiencies of a new hero").on_hover_text("Skills (perception), tools (thieves-tools) and saving throws (save:dex). The player's race adds its own.");
    let mut remove = None;
    for (k, p) in r.proficiencies.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            widgets::suggest(ui, &format!("rules_prof{k}"), p, suggestions, 180.0);
            let id = abilities::proficiency_id(p);
            if rustpg2d_engine::srd::Spec::parse(&id, false).is_some() || id.starts_with("save:") {
                ui.weak(abilities::proficiency_name(&id));
            } else if !p.trim().is_empty() {
                ui.weak("(not an SRD skill, tool or save)");
            }
            if ui.small_button(ph::X).on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        r.proficiencies.remove(k);
    }
    if ui.small_button(ui::t(ph::PLUS, "Add proficiency")).clicked() {
        r.proficiencies.push(String::new());
    }
    ui.checkbox(&mut r.show_checks, "Show check results on screen (\"Perception: 14 vs DC 12 - success\")").on_hover_text("Every roll is kept in the journal's Rolls page either way.");
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
    {
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
        ui.add(egui::DragValue::new(&mut d.max_hp).range(-50..=100)).on_hover_text("Added to the player's max HP once (negative: less). The SRD's races add none: their hit points come from the hit die and CON");
        ui.end_row();
        ui.label("HP per level");
        ui.add(egui::DragValue::new(&mut d.hp_per_level).range(-5..=10)).on_hover_text("Added to the player's max HP at every level (the hill dwarf's Dwarven Toughness: +1)");
        ui.end_row();
        ui.label("Speed");
        ui.add(egui::DragValue::new(&mut d.speed).range(-50.0..=100.0).suffix(" %")).on_hover_text("Faster (or slower, negative) than other players");
        ui.end_row();
        ui.label("Defense");
        ui.add(egui::DragValue::new(&mut d.defense).range(-20..=50)).on_hover_text("Added to the player's defense");
        ui.end_row();
        ui.label("Proficiencies").on_hover_text("Skills, tools and saving throws a hero of this race is proficient in (added to the project's starting ones)");
        ui.vertical(|ui| {
            let mut remove = None;
            for (k, s) in d.skills.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    widgets::suggest(ui, &format!("race_skill{k}"), &mut s.skill, &cx.refs.skills, 160.0);
                    s.level = s.level.max(1);
                    ui.weak(rustpg2d_engine::srd::abilities::proficiency_name(&rustpg2d_engine::srd::abilities::proficiency_id(&s.skill)));
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                d.skills.remove(k);
            }
            if ui.small_button("+ Proficiency").on_hover_text("A skill, tool or saving throw the hero is proficient in").clicked() {
                d.skills.push(rustpg2d_engine::races::RaceSkill { skill: "perception".into(), level: 1 });
            }
        });
        ui.end_row();
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Size");
        ui.add(egui::Slider::new(&mut d.size, 0.5..=2.0).step_by(0.05)).on_hover_text("Characters with this race's bodies are drawn this much larger (and their hitbox grows with them): trolls 1.5");
    });
    ui.separator();
    race_srd_form(ui, d, cx);
}

/// The race's SRD 5.1 data: subrace parent, ability increases, speed, size, darkvision,
/// languages, resistances, colours, traits and choices.
fn race_srd_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::races::RaceDef, cx: &mut Cx) {
    use rustpg2d_engine::races::{ChoiceKind, RaceChoice, RaceTrait};
    use rustpg2d_engine::srd::abilities::{self, Ability};
    ui.heading("SRD 5.1");
    ui.weak("The race's rules (the starter library's come from the SRD 5.1 by the srd_import tool). A subrace names its race: the creator offers the subrace, with both races' data.");
    let races: Vec<String> = cx.db.races.iter().filter(|r| r.id != d.id).map(|r| r.id.clone()).collect();
    let languages: Vec<String> = rustpg2d_engine::backgrounds::LANGUAGES.iter().map(|(id, _)| id.to_string()).collect();
    egui::Grid::new("race_srd").num_columns(2).show(ui, |ui| {
        ui.label("Subrace of").on_hover_text("Its race: a hill dwarf is a dwarf (its increases add up; Player race conditions for the race match it)");
        widgets::suggest(ui, "race_parent", &mut d.parent, &races, 140.0);
        ui.end_row();
        ui.label("Ability increases");
        ui.horizontal(|ui| {
            for a in Ability::ALL {
                let mut v = d.ability_bonuses.get(a.id()).copied().unwrap_or(0);
                ui.label(a.short());
                if ui.add(egui::DragValue::new(&mut v).range(-4..=4)).changed() {
                    if v == 0 {
                        d.ability_bonuses.remove(a.id());
                    } else {
                        d.ability_bonuses.insert(a.id().to_string(), v);
                    }
                }
            }
        });
        ui.end_row();
        ui.label("Speed");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut d.walk_ft).range(0..=60).suffix(" ft")).on_hover_text("Walking speed (0 = 30 ft); the hero moves at speed / 30");
            ui.label("Size");
            egui::ComboBox::from_id_salt("race_size_cat").selected_text(if d.size_category.is_empty() { "Medium" } else { d.size_category.as_str() }).show_ui(ui, |ui| {
                for c in ["Small", "Medium"] {
                    if ui.selectable_label(d.size_category == c, c).clicked() {
                        d.size_category = c.to_string();
                    }
                }
            });
            ui.label("Darkvision");
            ui.add(egui::DragValue::new(&mut d.darkvision_ft).range(0..=120).suffix(" ft")).on_hover_text("A hero with darkvision sees dark places as dim ones (0 = none)");
        });
        ui.end_row();
        for (label, list, choices, tip) in [("Languages", &mut d.languages, &languages, "common, dwarvish, elvish…"), ("Resistances", &mut d.resistances, &Vec::new(), "Damage types: fire, poison, cold…")] {
            ui.label(label).on_hover_text(tip);
            let mut text = list.join(", ");
            ui.horizontal(|ui| {
                if ui.add(egui::TextEdit::singleline(&mut text).desired_width(260.0)).changed() {
                    *list = text.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
                }
                if !choices.is_empty() {
                    ui.weak(format!("({})", choices.len()));
                }
            });
            ui.end_row();
        }
        ui.label("Colours").on_hover_text("The race's colours in the creator: a colour feature of its models and a colour (a tiefling's red skin)");
        ui.vertical(|ui| {
            let mut remove = None;
            for (k, (name, rgb)) in d.colors.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(name.as_str());
                    ui.color_edit_button_srgb(rgb);
                    if ui.small_button("x").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                if let Some(key) = d.colors.keys().nth(k).cloned() {
                    d.colors.remove(&key);
                }
            }
            if ui.small_button("+ Colour").on_hover_text("For the colour feature \"skin\" (rename by removing and adding)").clicked() {
                d.colors.entry("skin".into()).or_insert([180, 120, 90]);
            }
        });
        ui.end_row();
    });
    ui.label(format!("Traits ({})", d.traits.len())).on_hover_text("Its rules in words, shown in the creator and on the character sheet");
    let mut remove = None;
    for (k, t) in d.traits.iter_mut().enumerate() {
        egui::CollapsingHeader::new(if t.name.is_empty() { t.id.clone() } else { t.name.clone() }).id_salt(format!("race_trait{k}")).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(200.0));
                ui.label("Id");
                ui.add(egui::TextEdit::singleline(&mut t.id).desired_width(140.0));
                if ui.small_button("Remove").clicked() {
                    remove = Some(k);
                }
            });
            text_box(ui, &mut t.description, 3, "");
        });
    }
    if let Some(k) = remove {
        d.traits.remove(k);
    }
    if ui.small_button("+ Trait").clicked() {
        d.traits.push(RaceTrait { id: format!("trait-{}", d.traits.len() + 1), name: "New trait".into(), description: String::new() });
    }
    ui.label(format!("Choices in the creator ({})", d.choices.len())).on_hover_text("What a hero of the race picks: +1 to abilities, skills, a tool, a language, an ancestry, a cantrip");
    // The colour features of the race's models (a choice can colour them).
    let mut features: Vec<String> = d.bodies_male.iter().chain(&d.bodies_female).filter_map(|m| cx.db.model(m)).flat_map(|m| m.features.iter().map(|f| f.name.clone())).collect();
    features.sort();
    features.dedup();
    let mut remove = None;
    for (k, c) in d.choices.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(format!("race_choice_kind{k}")).width(90.0).selected_text(c.kind.label()).show_ui(ui, |ui| {
                for kind in ChoiceKind::ALL {
                    ui.selectable_value(&mut c.kind, kind, kind.label());
                }
            });
            ui.add(egui::TextEdit::singleline(&mut c.label).desired_width(150.0)).on_hover_text("What the creator calls it");
            ui.label("pick");
            ui.add(egui::DragValue::new(&mut c.choose).range(1..=6));
            let mut text = c.options.join(", ");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(220.0).hint_text("options: str, dex / stealth / dwarvish")).changed() {
                c.options = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
            if ui.small_button("x").clicked() {
                remove = Some(k);
            }
        });
        if c.kind == ChoiceKind::Ability {
            let bad: Vec<&String> = c.options.iter().filter(|o| Ability::parse(o).is_none()).collect();
            if !bad.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(230, 120, 90), format!("not abilities: {}", bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
            }
        }
        if matches!(c.kind, ChoiceKind::Skill | ChoiceKind::Tool) {
            let bad: Vec<&String> = c.options.iter().filter(|o| !abilities::is_known_proficiency(o)).collect();
            if !bad.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(230, 120, 90), format!("not SRD proficiencies: {}", bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
            }
        }
        // Colours per option: the creator then offers the choice on its first page, with the
        // preview, and the features it colours get no row of their own.
        if !features.is_empty() && c.choose == 1 && !c.options.is_empty() {
            let mut on = !c.colors.is_empty();
            if ui
                .checkbox(&mut on, "Colours the model")
                .on_hover_text("Each option gives the race's models colours (a dragonborn's ancestry: red scales, deep red clothes). The creator offers the choice on its first page with the preview, instead of rows for the colour features it sets.")
                .changed()
            {
                c.colors = if on { c.options.iter().map(|o| (o.clone(), features.iter().map(|f| (f.clone(), [128, 128, 128])).collect())).collect() } else { Default::default() };
            }
            if on {
                egui::Grid::new(format!("race_choice_colors{k}")).num_columns(features.len() + 1).show(ui, |ui| {
                    ui.label("");
                    for f in &features {
                        ui.weak(f);
                    }
                    ui.end_row();
                    for o in c.options.clone() {
                        ui.label(c.option_name(&o));
                        let row = c.colors.entry(o.clone()).or_default();
                        for f in &features {
                            let mut on = row.contains_key(f);
                            let mut col = row.get(f).copied().unwrap_or([128, 128, 128]);
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut on, "").on_hover_text("This option colours the feature").changed() && !on {
                                    row.remove(f);
                                }
                                if on && (ui.color_edit_button_srgb(&mut col).changed() || !row.contains_key(f)) {
                                    row.insert(f.clone(), col);
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
            }
        }
    }
    if let Some(k) = remove {
        d.choices.remove(k);
    }
    if ui.small_button("+ Choice").clicked() {
        let n = d.choices.len() + 1;
        d.choices.push(RaceChoice { id: format!("{}-choice-{n}", d.id), label: "Choice".into(), kind: ChoiceKind::Skill, choose: 1, options: vec!["perception".into(), "stealth".into()], ..Default::default() });
    }
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
        ui.label("Tool or skill").on_hover_text("The proficiency the recipe is about: a tool (smiths-tools, alchemists-supplies, cooks-utensils) or an SRD skill. Its check decides the better results.");
        ui.horizontal(|ui| {
            widgets::suggest(ui, "recipe_skill", &mut d.skill, &cx.refs.skills, 160.0);
            if !d.skill.is_empty() {
                let mut needed = d.level > 0;
                if ui.checkbox(&mut needed, "proficiency needed").on_hover_text("Only a hero proficient in it can make the recipe").changed() {
                    d.level = needed as u32;
                }
                let check = rustpg2d_engine::srd::Spec::parse(&d.check_what(), false).map_or(d.check_what(), |s| s.label);
                ui.weak(format!("check: {check}"));
            } else {
                ui.weak("(none: the better results roll an Intelligence check)");
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
            ui.weak("Making it rolls the tool or skill check; the best result whose DC the roll meets is made (a failed roll still makes the plain item).");
            let mut remove = None;
            for (k, q) in d.quality.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label("DC");
                    ui.add(egui::DragValue::new(&mut q.dc).range(1..=30));
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
            if ui.small_button("+ Better result").on_hover_text("A crafting roll of this DC or more makes another item (e.g. a Fine Iron Sword)").clicked() {
                let dc = d.quality.iter().map(|q| q.dc + 5).max().unwrap_or(15).min(30);
                d.quality.push(rustpg2d_engine::crafting::QualityStep { dc, result: d.result.clone() });
            }
        });
        ui.end_row();
    });
    ui.weak("Players make it in the crafting window: give a prop (a workbench, an anvil, an alchemy table) an Interact trigger with the 'Open crafting' action for this station. Scripts: open_crafting(station[, title]), can_craft(recipe).");
}

fn spell_form(ui: &mut egui::Ui, d: &mut rustpg2d_engine::skills::SpellDef, cx: &mut Cx) {
    use rustpg2d_engine::skills::{Area, Shape, SpellCast, SpellRange};
    ui.heading("Spell");
    rename_row(ui, RefKind::Spell, &d.id, cx.rename);
    name_row(ui, &mut d.name);
    ui.label("Description");
    text_box(ui, &mut d.description, 3, "");
    egui::Grid::new("spell_srd").num_columns(2).show(ui, |ui| {
        ui.label("Level").on_hover_text("The SRD spell level: 0 = a cantrip (scales with the caster's level), 1-9 = cast with a slot of that level or higher");
        ui.add(egui::DragValue::new(&mut d.level).range(0..=9));
        ui.end_row();
        ui.label("School");
        ui.add(egui::TextEdit::singleline(&mut d.school).desired_width(140.0));
        ui.end_row();
        ui.label("Casting time");
        ui.horizontal(|ui| {
            let mut k = match d.casting {
                SpellCast::Action => 0,
                SpellCast::Bonus => 1,
                SpellCast::Reaction => 2,
                SpellCast::Minutes(_) => 3,
            };
            let before = k;
            for (i, l) in ["Action", "Bonus action", "Reaction", "Minutes"].iter().enumerate() {
                ui.selectable_value(&mut k, i, *l);
            }
            if k != before {
                d.casting = [SpellCast::Action, SpellCast::Bonus, SpellCast::Reaction, SpellCast::Minutes(1)][k];
            }
            if let SpellCast::Minutes(m) = &mut d.casting {
                ui.add(egui::DragValue::new(m).range(1..=1440).suffix(" min"));
            }
        });
        ui.end_row();
        ui.label("Range");
        ui.horizontal(|ui| {
            let mut k = match d.range {
                SpellRange::Caster => 0,
                SpellRange::Touch => 1,
                SpellRange::Feet(_) => 2,
                SpellRange::Sight => 3,
            };
            let before = k;
            for (i, l) in ["Self", "Touch", "Feet", "Sight"].iter().enumerate() {
                ui.selectable_value(&mut k, i, *l);
            }
            if k != before {
                d.range = [SpellRange::Caster, SpellRange::Touch, SpellRange::Feet(30), SpellRange::Sight][k];
            }
            if let SpellRange::Feet(f) = &mut d.range {
                ui.add(egui::DragValue::new(f).range(5..=1000).speed(5).suffix(" ft"));
            }
        });
        ui.end_row();
        ui.label("Area").on_hover_text("Everyone on the area's tiles is a target: spheres and cylinders around the aimed tile (around the caster for a Self spell), cubes on it, cones and lines from the caster");
        ui.horizontal(|ui| {
            let mut on = d.area.is_some();
            if ui.checkbox(&mut on, "").changed() {
                d.area = on.then_some(Area { shape: Shape::Sphere, feet: 20 });
            }
            if let Some(a) = &mut d.area {
                egui::ComboBox::from_id_salt("spell_shape").selected_text(format!("{:?}", a.shape)).show_ui(ui, |ui| {
                    for sh in [Shape::Sphere, Shape::Cylinder, Shape::Cube, Shape::Cone, Shape::Line] {
                        ui.selectable_value(&mut a.shape, sh, format!("{sh:?}"));
                    }
                });
                ui.add(egui::DragValue::new(&mut a.feet).range(5..=500).speed(5).suffix(" ft"));
            }
        });
        ui.end_row();
        ui.label("Components");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.components).desired_width(70.0).hint_text("V, S, M"));
            ui.add(egui::TextEdit::singleline(&mut d.material).desired_width(220.0).hint_text("material"));
        });
        ui.end_row();
        ui.label("Duration");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.duration).desired_width(140.0).hint_text("Up to 1 minute"));
            ui.add(egui::DragValue::new(&mut d.duration_rounds).range(0..=100_000).suffix(" rounds")).on_hover_text("How long its conditions and boons last, in rounds of 6 seconds (10 = a minute; 0 = instantaneous)");
            ui.checkbox(&mut d.concentration, "Concentration");
            ui.checkbox(&mut d.ritual, "Ritual");
        });
        ui.end_row();
        ui.label("Classes").on_hover_text("The classes whose spell list has it (comma separated): they learn, prepare and cast it; scrolls of it work for them");
        let mut list = d.classes.join(", ");
        if ui.add(egui::TextEdit::singleline(&mut list).desired_width(300.0).hint_text("wizard, sorcerer")).changed() {
            d.classes = list.split(',').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
        }
        ui.end_row();
        ui.label("Story only").on_hover_text("Its effect is the story's: only events and scripts cast it (Cast spell, cast_spell); it isn't offered at level-ups");
        ui.checkbox(&mut d.story_only, "");
        ui.end_row();
    });
    ui.separator();
    ui.heading("What it does");
    ui.weak("The parts act on each target in order: an attack or a save first (damage halves or misses, conditions and boons need a failed save or a hit), then damage, healing, conditions, boons. Dice by level: \"1:1d10,5:2d10\" (slot level; a cantrip's: the caster's level); MOD = the spellcasting modifier.");
    spell_parts_ui(ui, &mut d.parts, cx);
    ui.separator();
    ui.heading("Look");
    egui::Grid::new("spell_look").num_columns(2).show(ui, |ui| {
        ui.label("Projectile");
        ui.horizontal(|ui| {
            let mut on = d.visual.projectile.is_some();
            if ui.checkbox(&mut on, "").changed() {
                d.visual.projectile = on.then(SpriteRef::default);
            }
        });
        ui.end_row();
        ui.label("Impact effect").on_hover_text("An effect sheet played where it lands (all its frames)");
        ui.horizontal(|ui| {
            widgets::suggest(ui, "spell_impact", &mut d.visual.impact, &cx.refs.sheets, 140.0);
            ui.add(egui::DragValue::new(&mut d.visual.impact_scale).range(0.0..=8.0).speed(0.05).prefix("x"));
        });
        ui.end_row();
        ui.label("Sound");
        widgets::suggest(ui, "spell_sound", &mut d.visual.sound, &cx.refs.sounds, 140.0);
        ui.end_row();
    });
    if let Some(p) = &mut d.visual.projectile {
        ui.label("Projectile sprite (drawn pointing right, rotated in flight):");
        widgets::sprite_picker(ui, "spell_proj", p, cx.tex, cx.assets);
    }
    ui.separator();
    ui.heading("Icon");
    widgets::sprite_field(ui, "spell_icon", &mut d.icon, cx.tex, cx.assets, cx.db, false);
    ui.weak("Casters learn it at level-ups or prepare it (Spells tab, L); wizards copy it from scrolls (items with 'Teaches spell'). In fights it is cast from the action bar's Cast entry; the 'Cast spell' action and cast_spell() cast it for the story.");
}

/// "1:1d10,5:2d10" for dice by level.
fn by_level_text(v: &[(u32, String)]) -> String {
    v.iter().map(|(l, d)| format!("{l}:{d}")).collect::<Vec<_>>().join(",")
}

fn parse_by_level(t: &str) -> Vec<(u32, String)> {
    t.split(',').filter_map(|e| {
        let (l, d) = e.split_once(':')?;
        Some((l.trim().parse().ok()?, d.trim().to_string()))
    }).collect()
}

/// The list of a spell's parts: a row each (its kind and fields, remove), and *Add*.
fn spell_parts_ui(ui: &mut egui::Ui, parts: &mut Vec<rustpg2d_engine::skills::SpellPart>, cx: &mut Cx) {
    use rustpg2d_engine::skills::SpellPart as P;
    use rustpg2d_engine::srd::conditions::{Buff as B, Condition};
    use rustpg2d_engine::srd::Ability;
    let mut remove = None;
    for (k, p) in parts.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button(crate::ui::ph::TRASH).on_hover_text("Remove this part").clicked() {
                remove = Some(k);
            }
            let by = |ui: &mut egui::Ui, v: &mut Vec<(u32, String)>| {
                let mut t = by_level_text(v);
                if ui.add(egui::TextEdit::singleline(&mut t).desired_width(220.0).hint_text("1:1d8 + MOD")).changed() {
                    *v = parse_by_level(&t);
                }
            };
            match p {
                P::Attack { ranged } => {
                    ui.strong("Spell attack");
                    ui.checkbox(ranged, "ranged");
                }
                P::Save { ability, half } => {
                    ui.strong("Saving throw");
                    egui::ComboBox::from_id_salt(("sp_save", k)).selected_text(ability.short()).show_ui(ui, |ui| {
                        for a in Ability::ALL {
                            ui.selectable_value(ability, a, a.short());
                        }
                    });
                    ui.checkbox(half, "half damage on a success");
                }
                P::Damage { by_level, damage_type } => {
                    ui.strong("Damage");
                    by(ui, by_level);
                    widgets::damage_type_combo(ui, &format!("sp_dt{k}"), damage_type, "untyped");
                }
                P::Heal { by_level } => {
                    ui.strong("Healing");
                    by(ui, by_level);
                }
                P::TempHp { by_level } => {
                    ui.strong("Temporary HP");
                    by(ui, by_level);
                }
                P::Condition { condition, on_fail } => {
                    ui.strong("Condition");
                    egui::ComboBox::from_id_salt(("sp_cond", k)).selected_text(condition.name()).show_ui(ui, |ui| {
                        for c in Condition::ALL.into_iter().filter(|c| *c != Condition::Any) {
                            ui.selectable_value(condition, c, c.name());
                        }
                    });
                    ui.checkbox(on_fail, "only on a failed save or a hit");
                }
                P::Targets { count, per_slot } => {
                    ui.strong("Targets");
                    ui.add(egui::DragValue::new(count).range(1..=20));
                    ui.label("+ per slot level above:");
                    ui.add(egui::DragValue::new(per_slot).range(0..=10));
                }
                P::Buff { buff } => {
                    ui.strong("Boon / bane");
                    let names = ["Attack die", "Save die", "AC +", "AC base", "Speed", "Haste", "Resistance", "Ability score"];
                    let mut kind = match buff {
                        B::AttackDie { .. } => 0,
                        B::SaveDie { .. } => 1,
                        B::Ac { .. } => 2,
                        B::AcBase { .. } => 3,
                        B::Speed { .. } => 4,
                        B::Haste => 5,
                        B::Resistance { .. } => 6,
                        B::AbilityScore { .. } => 7,
                    };
                    let before = kind;
                    egui::ComboBox::from_id_salt(("sp_buff", k)).selected_text(names[kind]).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut kind, i, *n);
                        }
                    });
                    if kind != before {
                        *buff = match kind {
                            0 => B::AttackDie { dice: "1d4".into(), minus: false },
                            1 => B::SaveDie { dice: "1d4".into(), minus: false },
                            2 => B::Ac { bonus: 2 },
                            3 => B::AcBase { base: 13 },
                            4 => B::Speed { feet: 10 },
                            5 => B::Haste,
                            6 => B::Resistance { types: vec!["fire".into()] },
                            _ => B::AbilityScore { ability: Ability::Str, score: 21 },
                        };
                    }
                    match buff {
                        B::AttackDie { dice, minus } | B::SaveDie { dice, minus } => {
                            ui.add(egui::TextEdit::singleline(dice).desired_width(50.0));
                            ui.checkbox(minus, "taken off (a bane)");
                        }
                        B::Ac { bonus } => {
                            ui.add(egui::DragValue::new(bonus).range(-10..=10));
                        }
                        B::AcBase { base } => {
                            ui.add(egui::DragValue::new(base).range(10..=20)).on_hover_text("+ DEX, without body armor");
                        }
                        B::Speed { feet } => {
                            ui.add(egui::DragValue::new(feet).range(-60..=60).speed(5).suffix(" ft"));
                        }
                        B::Haste => {}
                        B::Resistance { types } => {
                            let mut t = types.join(", ");
                            if ui.add(egui::TextEdit::singleline(&mut t).desired_width(160.0)).changed() {
                                *types = t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                            }
                        }
                        B::AbilityScore { ability, score } => {
                            egui::ComboBox::from_id_salt(("sp_buff_ab", k)).selected_text(ability.short()).show_ui(ui, |ui| {
                                for a in Ability::ALL {
                                    ui.selectable_value(ability, a, a.short());
                                }
                            });
                            ui.add(egui::DragValue::new(score).range(1..=30)).on_hover_text("The score becomes at least this");
                        }
                    }
                }
                P::Light { radius } => {
                    ui.strong("Light");
                    ui.add(egui::DragValue::new(radius).range(0.5..=30.0).speed(0.1).suffix(" tiles"));
                }
                P::Unlock => {
                    ui.strong("Opens the nearest lock");
                }
                P::Stabilize => {
                    ui.strong("Stabilizes").on_hover_text("A creature at 0 hit points stops making death saving throws (Spare the Dying)");
                }
                P::Script { function } => {
                    ui.strong("Script");
                    widgets::suggest(ui, &format!("sp_fn{k}"), function, &cx.refs.functions, 160.0);
                    ui.weak("function(caster, x, y, slot)");
                }
            }
        });
    }
    if let Some(k) = remove {
        parts.remove(k);
    }
    ui.horizontal(|ui| {
        ui.label("Add:");
        let templates: [(&str, P); 12] = [
            ("Attack", P::Attack { ranged: true }),
            ("Save", P::Save { ability: Ability::Dex, half: true }),
            ("Damage", P::Damage { by_level: vec![(1, "1d8".into())], damage_type: None }),
            ("Healing", P::Heal { by_level: vec![(1, "1d8 + MOD".into())] }),
            ("Temp HP", P::TempHp { by_level: vec![(1, "1d4 + 4".into())] }),
            ("Condition", P::Condition { condition: Condition::Prone, on_fail: true }),
            ("Targets", P::Targets { count: 3, per_slot: 1 }),
            ("Boon", P::Buff { buff: B::AttackDie { dice: "1d4".into(), minus: false } }),
            ("Light", P::Light { radius: 4.0 }),
            ("Unlock", P::Unlock),
            ("Stabilize", P::Stabilize),
            ("Script", P::Script { function: String::new() }),
        ];
        for (name, t) in templates {
            if ui.small_button(name).clicked() {
                parts.push(t);
            }
        }
    });
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
        ui.label("Shared clips");
        ui.vertical(|ui| {
            let mut drop = None;
            for (k, f) in d.shared_clips.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.monospace(f);
                    if ui.small_button(ui::t(ph::TRASH, "")).on_hover_text("Stop using this file's clips").clicked() {
                        drop = Some(k);
                    }
                });
            }
            if let Some(k) = drop {
                d.shared_clips.remove(k);
            }
            // Copy clips from another rigged model with the same bone names.
            let did = egui::Id::new(("share_from", &d.id));
            let mut from: String = ui.data_mut(|m| m.get_temp(did).unwrap_or_default());
            let sources: Vec<(String, String)> = cx.db.models.iter().filter(|m| m.id != d.id && m.rig.is_none() && m.clip_paths().next().is_some() && cx.assets.models.has(&m.id)).map(|m| (m.id.clone(), m.name.clone())).collect();
            ui.horizontal(|ui| {
                let shown = sources.iter().find(|m| m.0 == from).map_or("(a model)".to_string(), |m| m.1.clone());
                egui::ComboBox::from_id_salt(("share_from_combo", &d.id)).selected_text(shown).height(300.0).show_ui(ui, |ui| {
                    for (id, name) in &sources {
                        ui.selectable_value(&mut from, id.clone(), name);
                    }
                });
                if ui.add_enabled(!from.is_empty() && d.rig.is_none(), egui::Button::new(ui::t(ph::COPY, "Copy clips")))
                    .on_hover_text("Plays the other model's animations from its clip files, fitted to this model's bones (same bone names, e.g. two Meshy rigs); animations this model has keep their clips")
                    .clicked()
                {
                    SHARE_REQUEST.with(|r| *r.borrow_mut() = Some((d.id.clone(), from.clone())));
                }
            });
            ui.data_mut(|m| m.insert_temp(did, from));
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
    ui.weak("The clip each animation plays (12 frames a second unless set). Characters without a clip for an animation play idle; walking, idle, attacks and being hit are used by everyone, sitting and sleeping on furniture, the rest by activities. The weapon animations (attack_1h…) are played holding items that name them (Database › Items › Animations), else the plain ones; Hit is where in the clip the blow lands or the guard is up.");
    egui::Grid::new("model_clips").num_columns(4).show(ui, |ui| {
        for anim in rustpg2d_engine::model::all_anims() {
            if anim == rustpg2d_engine::defs::STANDARD_ANIMS[0] {
                ui.strong("Weapons");
                ui.end_row();
            }
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
                    d.clips.push(ClipMap { anim: anim.into(), clip: pick, fps: 12.0, hit: None });
                }
            }
            match d.clips.iter_mut().find(|c| c.anim == anim) {
                Some(c) => {
                    ui.add(egui::DragValue::new(&mut c.fps).range(1.0..=30.0).suffix(" fps"));
                    if rustpg2d_engine::model::has_hit(anim) {
                        let mut hit = c.hit_point();
                        if ui.add(egui::DragValue::new(&mut hit).range(0.05..=0.95).speed(0.01).prefix("hit ")).on_hover_text("Where in the clip (0 = start, 1 = end) the blow lands, or the guard is up").changed() {
                            c.hit = Some(hit);
                        }
                    } else {
                        ui.label("");
                    }
                }
                None => {
                    ui.label("");
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
    let t = crate::ui::now();
    let facings = [Facing::Down, Facing::Left, Facing::Up, Facing::Right];
    let facing = if turning { facings[(t / 1.5) as usize % 4] } else { facings[facing_k % 4] };
    let all: Vec<&str> = rustpg2d_engine::model::all_anims().collect();
    let anim = all[anim_k % all.len()];
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
                for (k, a) in all.iter().enumerate() {
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
    /// *Copy clips* asked for: (target model, source model) (handled after the form: it reads files).
    static SHARE_REQUEST: std::cell::RefCell<Option<(String, String)>> = const { std::cell::RefCell::new(None) };
}

/// The models whose clips the model form asked to share (once).
pub fn take_share_request() -> Option<(String, String)> {
    SHARE_REQUEST.with(|r| r.borrow_mut().take())
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

/// Sets the item form's preview (`itempreview` script command): `anim` idle / walk / attack / block,
/// playing or still, held by `model` ("" = the first playable model).
pub fn set_hold_preview(anim: &str, play: bool, model: &str) {
    let anim = ["idle", "walk", "attack", "block"].into_iter().find(|a| *a == anim).unwrap_or("idle");
    HOLD_PREVIEW.with(|p| *p.borrow_mut() = (anim, play, model.to_string()));
}

/// The animations characters play with the item in hand (`ItemDef.anims`): a name per motion
/// (empty = estimated from the weapon), and how many 3D models have a clip for it.
fn item_anims_ui(ui: &mut egui::Ui, d: &mut ItemDef, cx: &mut Cx) {
    ui.heading("Animations");
    ui.weak("What characters play holding it: names that 3D models map to clips (Database › 3D models › Animations). Empty = estimated from the weapon; a model without the named clip plays its plain one.");
    let est = ItemAnims::estimate(d);
    let rigged: Vec<String> = cx.db.models.iter().filter(|m| cx.assets.models.has(&m.id) && m.rig.is_none()).map(|m| m.id.clone()).collect();
    let names: Vec<String> = rustpg2d_engine::model::all_anims().map(str::to_string).collect();
    egui::Grid::new("item_anims").num_columns(3).show(ui, |ui| {
        let rows: [(&str, &str, &mut String, &str); 4] = [
            ("Attacking with it", "attack", &mut d.anims.attack, &est.attack),
            ("Standing with it", "idle", &mut d.anims.idle, &est.idle),
            ("Walking with it", "walk", &mut d.anims.walk, &est.walk),
            ("Blocking with it", "block", &mut d.anims.block, &est.block),
        ];
        for (label, base, field, guess) in rows {
            ui.label(label);
            let options: Vec<String> = names.iter().filter(|n| n.starts_with(base)).cloned().collect();
            widgets::suggest(ui, &format!("item_anim_{base}"), field, &options, 130.0);
            let name = if field.is_empty() { guess.to_string() } else { field.clone() };
            if name.is_empty() {
                ui.weak(format!("(plain {base})"));
            } else {
                let have: Vec<&String> = rigged.iter().filter(|m| cx.assets.models.has_anim(m, &name)).collect();
                let text = format!("{}{} models of {} have it", if field.is_empty() { format!("{name} (estimated): ") } else { String::new() }, have.len(), rigged.len());
                let tip = if have.is_empty() { "None: they play their plain animation".to_string() } else { have.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") };
                ui.weak(text).on_hover_text(tip);
            }
            ui.end_row();
        }
    });
    if ui.add_enabled(!d.anims.is_empty(), egui::Button::new(ui::t(ph::MAGIC_WAND, "Estimate animations"))).on_hover_text("Back to the names estimated from the weapon (all fields empty)").clicked() {
        d.anims = ItemAnims::default();
    }
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
        HoldStyle::Back => "On the upper back, its top up and its front facing away from the body: backpacks.",
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
            for a in ["idle", "walk", "attack", "block"] {
                ui.selectable_value(&mut anim, a, match a {
                    "idle" => "Standing",
                    "walk" => "Walking",
                    "block" => "Blocking",
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
        // (Battle-ready: the item in the hand and its stance, not sheathed at the hip.)
        let gear = match d.armor.as_ref().map(|a| a.slot) {
            Some(EquipSlot::Offhand) => Gear { offhand: Some(d.id.clone()), ready: true, ..Default::default() },
            Some(EquipSlot::Back) => Gear { back: Some(d.id.clone()), ready: true, ..Default::default() },
            _ => Gear { weapon: Some(d.id.clone()), ready: true, ..Default::default() },
        };
        // The clip its animations name for this model (`Models::anim_for`), at its blow.
        let clip = cx.assets.models.anim_for(cx.db, &look.model, &gear, anim);
        let hit = cx.assets.models.clip_map(&look.model, &clip).map_or(rustpg2d_engine::defs::DEFAULT_HIT, |c| c.hit_point());
        let once = matches!(anim, "attack" | "block") && clip != "idle";
        let clock = ui.input(|i| i.time) as f32;
        let time = match (play, once) {
            (true, true) => (clock * 0.9).fract(),
            (true, false) => clock,
            (false, true) => hit,
            (false, false) if anim == "walk" => 0.3,
            _ => 0.0,
        };
        if play {
            ui.ctx().request_repaint();
        }
        ui.horizontal(|ui| {
            for facing in [Facing::Down, Facing::Left, Facing::Right, Facing::Up] {
                let (r, _) = ui.allocate_exact_size(egui::vec2(170.0, 200.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 3.0, egui::Color32::from_rgb(58, 74, 52));
                let req = PoseReq { look: &look, gear: &gear, anim: &clip, time, once, facing, turn: 0, bust: false };
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
