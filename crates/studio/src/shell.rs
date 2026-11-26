//! The window: a menu bar made from the command list, panels and documents as tabs that can be
//! arranged (egui_dock), the status bar, the command palette and *Go to anything*.

use std::path::PathBuf;

use eframe::egui;
use editor_core::commands::COMMANDS;
use editor_core::search::Target;
use editor_core::{Gpu, Workspace};
use egui_dock::{DockArea, DockState, NodeIndex};
use rustpg2d_engine::refactor::RefKind;

use crate::play::PlayView;

/// What a tab shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Tab {
    /// The Project browser.
    Browser,
    Overview,
    /// The game, running.
    Play,
    Problems,
    History,
    /// A page of the game's settings (`schema::PAGES`), by its title.
    Settings(&'static str),
    /// Everything of a kind, as a grid of pictures or a list.
    Kind(RefKind),
    /// A definition.
    Def(RefKind, String),
    /// A map.
    Map(String),
}

impl Tab {
    fn title(&self, ws: &Workspace) -> String {
        match self {
            Tab::Browser => "Project".into(),
            Tab::Overview => "Overview".into(),
            Tab::Play => "Play".into(),
            Tab::Problems => {
                let (e, w) = ws.problems.counts();
                if e + w > 0 {
                    format!("Problems ({})", e + w)
                } else {
                    "Problems".into()
                }
            }
            Tab::History => "History".into(),
            Tab::Settings(page) => page.to_string(),
            Tab::Kind(k) => plural(k.label()),
            Tab::Def(k, id) => format!("{}: {id}", k.label()),
            Tab::Map(m) => {
                let dirty = ws.open_maps().iter().any(|d| d.map.name == *m && d.is_dirty());
                format!("Map: {m}{}", if dirty { " *" } else { "" })
            }
        }
    }

    /// `overview`, `kind:item`, `def:item:potion`, `map:village`: how scripts and `status` name a tab.
    pub fn name(&self) -> String {
        match self {
            Tab::Browser => "project".into(),
            Tab::Overview => "overview".into(),
            Tab::Play => "play".into(),
            Tab::Problems => "problems".into(),
            Tab::History => "history".into(),
            Tab::Settings(page) => format!("settings:{}", editor_core::schema::page_slug(page)),
            Tab::Kind(k) => format!("kind:{}", k.name()),
            Tab::Def(k, id) => format!("def:{}:{id}", k.name()),
            Tab::Map(m) => format!("map:{m}"),
        }
    }
}

/// The groups of the Project browser: the same place for the same kind of thing, always.
pub const GROUPS: &[(&str, &[RefKind])] = &[
    ("World", &[RefKind::Map, RefKind::Structure, RefKind::Material, RefKind::Scatter]),
    ("Objects", &[RefKind::Prop]),
    ("Characters", &[RefKind::Character, RefKind::Race, RefKind::Class, RefKind::Background]),
    ("Items", &[RefKind::Item, RefKind::Recipe]),
    ("Rules", &[RefKind::Spell, RefKind::Disease]),
    ("Story", &[RefKind::Quest, RefKind::Cutscene, RefKind::CommonEvent, RefKind::Achievement, RefKind::VisualScript]),
    ("Art & sound", &[RefKind::Model, RefKind::Sheet, RefKind::Animation, RefKind::Sound, RefKind::Font, RefKind::Skin]),
];

/// The categories of definitions (`editor_core::defs::categories`), by kind and id.
type Cats = std::collections::HashMap<RefKind, std::collections::HashMap<String, (u32, String)>>;

/// What is known to be wrong with assets, and which nothing uses.
#[derive(Default)]
pub struct Marks {
    /// By definition: the texts of its problems, errors first (after *Check the project*).
    problems: std::collections::HashMap<(RefKind, String), Vec<(bool, String)>>,
    /// Definitions nothing uses (after *Unused* was asked for; `None` = not looked yet).
    unused: Option<std::collections::HashSet<(RefKind, String)>>,
}

impl Marks {
    fn problems_of(&self, kind: RefKind, id: &str) -> &[(bool, String)] {
        self.problems.get(&(kind, id.to_string())).map_or(&[], Vec::as_slice)
    }

    fn is_unused(&self, kind: RefKind, id: &str) -> bool {
        self.unused.as_ref().is_some_and(|u| u.contains(&(kind, id.to_string())))
    }

    /// The colour of an asset's mark: red for an error, yellow for a warning, grey when unused.
    fn color(&self, ui: &egui::Ui, kind: RefKind, id: &str) -> Option<egui::Color32> {
        match self.problems_of(kind, id).first() {
            Some((true, _)) => Some(ui.visuals().error_fg_color),
            Some((false, _)) => Some(ui.visuals().warn_fg_color),
            None => self.is_unused(kind, id).then(|| ui.visuals().weak_text_color()),
        }
    }
}

/// The categories among `rows` in their order, with how many rows each has.
fn categories_of(cats: Option<&std::collections::HashMap<String, (u32, String)>>, rows: &[(String, String)]) -> Vec<(u32, String, usize)> {
    let mut found: Vec<(u32, String, usize)> = Vec::new();
    for (id, _) in rows {
        if let Some((rank, label)) = cats.and_then(|c| c.get(id)) {
            match found.iter_mut().find(|(_, l, _)| l == label) {
                Some(entry) => entry.2 += 1,
                None => found.push((*rank, label.clone(), 1)),
            }
        }
    }
    found.sort();
    found
}

/// "Items", "Classes": several of a kind.
pub fn plural(label: &str) -> String {
    if label.ends_with('s') {
        format!("{label}es")
    } else {
        format!("{label}s")
    }
}

/// A button that opens a small list to take a name from, or type a new one into (a tag, a
/// folder). `none`: an entry that takes the name away (gives ""). Returns what was chosen.
fn name_menu(ui: &mut egui::Ui, button: &str, hint: &str, known: &[String], none: Option<&str>) -> Option<String> {
    ui.menu_button(button, |ui| {
        let id = ui.scope_id().with("name");
        let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
        let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text(hint).desired_width(190.0));
        // (Enter gives the field's focus up: looked at before the field takes it again.)
        let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !entered && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        let mut pick = None;
        if entered && !text.trim().is_empty() {
            pick = Some(text.trim().to_string());
        }
        if let Some(none) = none {
            if ui.button(none).clicked() {
                pick = Some(String::new());
            }
        }
        let typed = text.trim().to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for name in known.iter().filter(|k| typed.is_empty() || k.to_lowercase().contains(&typed)) {
                if ui.button(name.as_str()).clicked() {
                    pick = Some(name.clone());
                }
            }
        });
        if pick.is_some() {
            text.clear();
            ui.close();
        }
        ui.data_mut(|d| d.insert_temp(id, text));
        pick
    })
    .inner
    .flatten()
}

/// The palette over the window: commands (Ctrl+Shift+P) or *Go to anything* (Ctrl+P).
#[derive(Default)]
pub struct Palette {
    pub open: Option<PaletteKind>,
    pub text: String,
    sel: usize,
    focus: bool,
    /// The answer of the command run last from the palette, shown until it closes.
    pub answer: Option<Result<String, String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKind {
    Commands,
    Search,
}

/// What a tab's UI asked for (done after the tabs are drawn).
enum Ask {
    Open(Tab),
    Run(String),
    /// The command palette with this typed (a command waiting for its words).
    Palette(String),
    /// The *New…* gallery, with this kind picked.
    New(Option<RefKind>),
    /// Look for what nothing uses.
    FindUnused,
}

pub struct Shell {
    pub ws: Workspace,
    pub dock: DockState<Tab>,
    pub play: Option<PlayView>,
    pub play_error: Option<String>,
    pub start: rustpg2d_engine::runtime::StartOptions,
    pub palette: Palette,
    /// The browser's lists: (kind, id, name), made again when the project changed.
    lists: Vec<(RefKind, Vec<(String, String)>)>,
    /// What each definition is, where its kind has categories: id -> (rank, label).
    cats: Cats,
    browser_filter: String,
    /// Forms holding something that isn't a valid definition yet: what was typed and why it
    /// can't be taken (the definition itself stays as it was until it can).
    forms: std::collections::HashMap<(RefKind, String), (serde_json::Value, String)>,
    /// Forms also show the fields for advanced use.
    pub show_all: bool,
    /// The engine's textures as egui shows them (thumbnails, pictures in forms).
    thumbs: crate::thumbs::Thumbs,
    /// The *New…* gallery while it is open.
    pub new_asset: Option<crate::gallery::NewAsset>,
    marks: Marks,
    /// The open maps' views (camera, selection, tool), by map.
    pub maps: std::collections::HashMap<String, crate::mapview::MapView>,
    /// Answers of commands, newest last (the script prints them too).
    pub log: Vec<String>,
}

impl Shell {
    pub fn open(project: PathBuf, start: rustpg2d_engine::runtime::StartOptions) -> Shell {
        let mut ws = match Workspace::open_path(project.clone(), Gpu::Yes) {
            Ok(ws) => ws,
            Err(e) => {
                let mut ws = Workspace::empty();
                ws.set_status(format!("{}: {e:#}", project.display()));
                ws
            }
        };
        ws.refresh_refs();
        ws.history_frame(None);
        // Documents in the middle, the browser left, problems and history below.
        let mut dock = DockState::new(vec![Tab::Overview]);
        let surface = dock.main_surface_mut();
        let [center, _left] = surface.split_left(NodeIndex::root(), 0.2, vec![Tab::Browser]);
        surface.split_below(center, 0.72, vec![Tab::Problems, Tab::History]);
        Shell { ws, dock, play: None, play_error: None, start, palette: Palette::default(), lists: Vec::new(), cats: Default::default(), browser_filter: String::new(), forms: Default::default(), show_all: false, thumbs: Default::default(), new_asset: None, marks: Marks::default(), maps: Default::default(), log: Vec::new() }
    }

    /// Shows `tab`: brings it to the front when it is open, else opens it beside the documents.
    pub fn show_tab(&mut self, tab: Tab) {
        if let Tab::Map(m) = &tab {
            self.ws.open_map(m);
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            return;
        }
        // Next to another document when there is one, else in the first leaf.
        let doc = self.dock.find_tab_from(|t| matches!(t, Tab::Overview | Tab::Settings(_) | Tab::Kind(_) | Tab::Def(..) | Tab::Map(_) | Tab::Play));
        match doc {
            Some(path) => {
                self.dock.set_focused_node_and_surface(path.node_path());
                self.dock.push_to_focused_leaf(tab);
            }
            None => self.dock.push_to_first_leaf(tab),
        }
    }

    /// The tab by its script name (`overview`, `play`, `def:item:potion`, `map:village`).
    pub fn tab_named(&self, name: &str) -> Option<Tab> {
        let mut parts = name.splitn(3, ':');
        Some(match (parts.next()?, parts.next(), parts.next()) {
            ("project", None, _) => Tab::Browser,
            ("overview", None, _) => Tab::Overview,
            ("play", None, _) => Tab::Play,
            ("problems", None, _) => Tab::Problems,
            ("history", None, _) => Tab::History,
            ("map", Some(m), None) if self.ws.maps.iter().any(|x| x == m) => Tab::Map(m.to_string()),
            ("settings", Some(page), None) => Tab::Settings(editor_core::schema::PAGES.iter().find(|(t, ..)| editor_core::schema::page_slug(t) == page)?.0),
            ("kind", Some(kind), None) => Tab::Kind(RefKind::ALL.iter().copied().find(|k| k.name() == kind && k.is_def())?),
            ("def", Some(kind), Some(id)) => {
                let k = RefKind::ALL.iter().copied().find(|k| k.name() == kind)?;
                self.ws.project.db.has(k, id).then(|| Tab::Def(k, id.to_string()))?
            }
            _ => return None,
        })
    }

    /// Where a search hit or a usage leads.
    pub fn go_to(&mut self, target: &Target) {
        match target {
            Target::Map(m) | Target::MapTriggers(m) | Target::Entity(m, _) | Target::Zone(m, _) => {
                if let Target::Entity(_, uid) = target {
                    self.maps.entry(m.clone()).or_default().select(editor_core::workspace::Selection::Entity(*uid));
                }
                if let Target::Zone(_, uid) = target {
                    self.maps.entry(m.clone()).or_default().select(editor_core::workspace::Selection::Zone(*uid));
                }
                self.show_tab(Tab::Map(m.clone()));
            }
            Target::Def(k, id) => self.show_tab(Tab::Def(*k, id.clone())),
            Target::Script(..) | Target::Project => self.show_tab(Tab::Overview),
        }
    }

    /// Runs a command line and keeps its answer.
    pub fn run(&mut self, line: &str) -> Result<String, String> {
        let r = self.ws.run_line(line);
        self.log.push(match &r {
            Ok(m) => format!("{line}: {}", m.lines().next().unwrap_or_default()),
            Err(e) => format!("{line}: failed: {e}"),
        });
        // What a command opened or made gets its tab.
        if r.is_ok() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["map.open", ..] => {
                    if let Some(m) = self.ws.doc.as_ref().map(|d| d.map.name.clone()) {
                        self.show_tab(Tab::Map(m));
                    }
                }
                ["defs.new", kind, id] | ["defs.duplicate", kind, _, id] => {
                    if let Some(t) = self.tab_named(&format!("def:{kind}:{id}")) {
                        self.show_tab(t);
                    }
                }
                _ => {}
            }
        }
        r
    }

    /// Changes one field of a definition as its form does: `pointer` is the field's place in the
    /// definition's data (`/name`, `/weapon/damage`, `/stages/0`), `text` its new value as data
    /// (`12`, `"a text"`, `true`). The script's way to fill in forms.
    pub fn set_field(&mut self, kind: RefKind, id: &str, pointer: &str, text: &str) -> Result<String, String> {
        let mut value = self.ws.def_value(kind, id).ok_or_else(|| format!("there is no {} \"{id}\"", kind.name()))?;
        let new: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("{text} isn't data: {e}"))?;
        match value.pointer_mut(pointer) {
            Some(place) => *place = new,
            // A field the definition leaves out while it holds its default: put into its group.
            None => {
                if !editor_core::schema::of_kind(kind).is_some_and(|n| n.at(pointer).is_some()) {
                    return Err(format!("{} definitions have no field {pointer}", kind.label()));
                }
                let (parent, field) = pointer.rsplit_once('/').ok_or_else(|| format!("{pointer} isn't a field's place (/name, /weapon/damage)"))?;
                let group = value.pointer_mut(parent).and_then(|p| p.as_object_mut()).ok_or_else(|| format!("{} \"{id}\" has no {parent} to put {field} into", kind.name()))?;
                group.insert(field.to_string(), new);
            }
        }
        self.ws.set_def_value(kind, id, value)?;
        self.ws.history_frame(None);
        Ok(format!("{} {id}{pointer} = {text}", kind.name()))
    }

    /// Changes one of the game's settings as its page does: `pointer` is its place in the
    /// project's settings (`/day_minutes`, `/rules/show_checks`), `text` its new value as data.
    pub fn set_setting(&mut self, pointer: &str, text: &str) -> Result<String, String> {
        let mut value = self.ws.config_value();
        let new: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("{text} isn't data: {e}"))?;
        if editor_core::schema::project().at(pointer).is_none() {
            return Err(format!("the settings have no {pointer}"));
        }
        match value.pointer_mut(pointer) {
            Some(place) => *place = new,
            None => {
                let (parent, field) = pointer.rsplit_once('/').ok_or_else(|| format!("{pointer} isn't a setting's place (/day_minutes)"))?;
                let group = value.pointer_mut(parent).and_then(|p| p.as_object_mut()).ok_or_else(|| format!("the settings have no {parent} to put {field} into"))?;
                group.insert(field.to_string(), new);
            }
        }
        self.ws.set_config_value(value)?;
        self.ws.history_frame(None);
        Ok(format!("settings{pointer} = {text}"))
    }

    /// `tabs=… focus=… history=… problems=… palette=…` for the script's `status`.
    /// The view of the map being worked on, with the map's name.
    pub fn map_view(&mut self) -> Option<(&mut crate::mapview::MapView, String)> {
        let name = self.ws.doc.as_ref()?.map.name.clone();
        Some((self.maps.entry(name.clone()).or_default(), name))
    }

    /// Something done with the selection of the map being worked on (`delete`, `duplicate`,
    /// `turn`), as its keys and buttons do it.
    pub fn map_act(&mut self, what: &str) {
        let Some(name) = self.ws.doc.as_ref().map(|d| d.map.name.clone()) else { return };
        let view = self.maps.entry(name.clone()).or_default();
        let mut parts = crate::mapview::Parts { ws: &mut self.ws, rs: None, lists: &self.lists, thumbs: &mut self.thumbs, show_all: self.show_all, asks: Vec::new() };
        view.act(&mut parts, &name, what);
    }

    /// The map view's line for `status` (empty without an open map).
    pub fn map_status(&self) -> String {
        self.ws.doc.as_ref().and_then(|d| self.maps.get(&d.map.name).map(|v| v.status(&self.ws, &d.map.name))).unwrap_or_default()
    }

    pub fn status(&mut self) -> String {
        let tabs: Vec<String> = self.dock.iter_all_tabs().map(|(_, t)| t.name()).collect();
        let focus = self.dock.find_active_focused().map(|(_, t)| t.name()).unwrap_or_else(|| "-".into());
        format!(
            "project={} tabs=[{}] focus={focus} map={} history={} problems={} dirty={} palette={}",
            if self.ws.project_open { self.ws.project.config.name.replace(' ', "_") } else { "-".into() },
            tabs.join(","),
            self.ws.doc.as_ref().map_or("-".into(), |d| d.map.name.clone()),
            self.ws.timeline.status(),
            self.ws.problems.status(),
            self.ws.is_dirty(),
            match self.palette.open {
                Some(PaletteKind::Commands) => format!("commands:{}", self.palette.text),
                Some(PaletteKind::Search) => format!("search:{}", self.palette.text),
                None => "closed".into(),
            }
        )
    }

    fn lists(&mut self) {
        if !self.ws.stale && !self.lists.is_empty() {
            return;
        }
        self.lists = GROUPS.iter().flat_map(|(_, kinds)| kinds.iter()).filter(|k| **k != RefKind::Map).map(|k| (*k, editor_core::defs::list(&self.ws.project.db, *k))).collect();
        // (What was unused may be used now.)
        if self.marks.unused.is_some() {
            self.find_unused();
        }
        self.cats = self.lists.iter().map(|(k, _)| (*k, editor_core::defs::categories(&self.ws.project.db, *k).into_iter().map(|(id, rank, label)| (id, (rank, label))).collect())).filter(|(_, c): &(RefKind, std::collections::HashMap<String, (u32, String)>)| !c.is_empty()).collect();
        self.ws.stale = false;
    }

    /// Looks for the definitions nothing uses (`engine::unused`).
    fn find_unused(&mut self) {
        match rustpg2d_engine::unused::find_in(&self.ws.project) {
            Ok(u) => self.marks.unused = Some(u.deletable().defs.into_iter().map(|d| (d.kind, d.id)).collect()),
            Err(e) => {
                self.marks.unused = None;
                self.ws.set_status(format!("Looking for unused assets failed: {e:#}"));
            }
        }
    }

    /// One frame of the window.
    pub fn ui(&mut self, ui: &mut egui::Ui, rs: Option<&eframe::egui_wgpu::RenderState>, fps: f32) {
        let ctx = ui.ctx().clone();
        self.ws.history_frame(ctx.memory(|m| m.focused()).map(|id| id.value()));
        self.lists();
        self.shortcuts(&ctx);
        if let Some(rs) = rs {
            self.thumbs.begin_frame(&self.ws, rs);
        }
        // What the last project check found, by definition (errors first).
        self.marks.problems.clear();
        for p in &self.ws.problems.list {
            if let Target::Def(kind, id) = &p.target {
                let entry = self.marks.problems.entry((*kind, id.clone())).or_default();
                let error = p.severity == editor_core::problems::Severity::Error;
                entry.insert(if error { 0 } else { entry.len() }, (error, p.text.clone()));
            }
        }

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.ws.status.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{fps:.0} fps"));
                    ui.separator();
                    let (e, w) = self.ws.problems.counts();
                    ui.label(format!("{e} errors, {w} warnings"));
                    ui.separator();
                    ui.label(if self.ws.is_dirty() { "unsaved changes" } else { "saved" });
                });
            });
        });

        let mut asks = Vec::new();
        {
            let mut viewer = Viewer { ws: &mut self.ws, play: &mut self.play, play_error: &mut self.play_error, start: &self.start, lists: &self.lists, cats: &self.cats, filter: &mut self.browser_filter, forms: &mut self.forms, show_all: &mut self.show_all, thumbs: &mut self.thumbs, marks: &self.marks, maps: &mut self.maps, rs, asks: &mut asks };
            DockArea::new(&mut self.dock).show_inside(ui, &mut viewer);
        }
        for ask in asks {
            match ask {
                Ask::Open(tab) => self.show_tab(tab),
                Ask::Run(line) => {
                    let _ = self.run(&line);
                }
                Ask::Palette(text) => self.open_palette(PaletteKind::Commands, &text),
                Ask::New(kind) => self.new_asset = Some(crate::gallery::NewAsset::of(kind)),
                Ask::FindUnused => self.find_unused(),
            }
        }
        self.gallery_ui(&ctx, rs);
        self.palette_ui(&ctx);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) && self.ws.project_open {
            self.new_asset = Some(crate::gallery::NewAsset::of(None));
        }
        let (commands, search, save, undo, redo, escape) = ctx.input(|i| {
            let c = i.modifiers.command;
            (c && i.modifiers.shift && i.key_pressed(egui::Key::P), c && !i.modifiers.shift && i.key_pressed(egui::Key::P), c && i.key_pressed(egui::Key::S), c && !i.modifiers.shift && i.key_pressed(egui::Key::Z), c && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))), i.key_pressed(egui::Key::Escape))
        });
        if commands {
            self.open_palette(PaletteKind::Commands, "");
        } else if search {
            self.open_palette(PaletteKind::Search, "");
        }
        if escape && self.palette.open.is_some() {
            self.palette.open = None;
        }
        // (A text field with the keyboard keeps Ctrl+Z for its own typing.)
        if save {
            let _ = self.run("file.save_all");
        }
        if !ctx.text_edit_focused() {
            if undo {
                let _ = self.run("edit.undo");
            } else if redo {
                let _ = self.run("edit.redo");
            }
        }
    }

    /// The *New…* gallery: makes what it asks for and opens it.
    fn gallery_ui(&mut self, ctx: &egui::Context, rs: Option<&eframe::egui_wgpu::RenderState>) {
        let Some(state) = self.new_asset.as_mut() else { return };
        let mut pics = rs.map(|rs| crate::thumbs::Pics { thumbs: &mut self.thumbs, ws: &self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        match crate::gallery::show(ctx, state, GROUPS, &self.lists, pictures) {
            Some(crate::gallery::Outcome::Create { kind, name, from }) => match self.ws.make_def(kind, &name, from.as_deref()) {
                Ok(id) => {
                    self.new_asset = None;
                    self.log.push(format!("Made {} \"{id}\"", kind.name()));
                    self.lists.clear();
                    self.show_tab(Tab::Def(kind, id));
                }
                Err(e) => state.error = Some(e),
            },
            Some(crate::gallery::Outcome::Cancel) => self.new_asset = None,
            None => {}
        }
    }

    pub fn open_palette(&mut self, kind: PaletteKind, text: &str) {
        self.palette = Palette { open: Some(kind), text: text.to_string(), sel: 0, focus: true, answer: None };
    }

    /// The menu bar: a menu per group of the command list.
    fn menus(&mut self, ui: &mut egui::Ui) {
        let mut groups: Vec<&str> = Vec::new();
        for c in COMMANDS {
            let g = c.id.split('.').next().unwrap_or_default();
            if !groups.contains(&g) {
                groups.push(g);
            }
        }
        let mut run = None;
        for g in groups {
            let title = format!("{}{}", g[..1].to_uppercase(), &g[1..]);
            ui.menu_button(title, |ui| {
                for c in COMMANDS.iter().filter(|c| c.id.starts_with(g) && c.id[g.len()..].starts_with('.')) {
                    let label = if c.keys.is_empty() { c.title.to_string() } else { format!("{}   {}", c.title, c.keys) };
                    let label = if c.params.is_empty() { label } else { format!("{label}…") };
                    if ui.button(label).on_hover_text(c.help).clicked() {
                        run = Some(c);
                        ui.close();
                    }
                }
            });
        }
        if let Some(c) = run {
            if c.id == "find.search" {
                self.open_palette(PaletteKind::Search, "");
            } else if c.id == "defs.new" {
                self.new_asset = Some(crate::gallery::NewAsset::of(None));
            } else if c.params.is_empty() {
                let _ = self.run(c.id);
            } else {
                // It needs words: the palette, with the command typed.
                self.open_palette(PaletteKind::Commands, &format!("{} ", c.id));
            }
        }
        ui.separator();
        if ui.button("Play").on_hover_text("Runs the game here, in a tab").clicked() {
            self.show_tab(Tab::Play);
        }
    }

    /// The entries the palette shows for its text: (label, detail, what Enter does).
    pub fn palette_entries(&mut self) -> Vec<(String, String, PaletteHit)> {
        let text = self.palette.text.clone();
        match self.palette.open {
            Some(PaletteKind::Commands) => {
                let word = text.split_whitespace().next().unwrap_or_default().to_lowercase();
                let typed_args = text.contains(' ');
                COMMANDS
                    .iter()
                    .filter(|c| if typed_args { c.id == word } else { word.is_empty() || c.id.contains(&word) || c.title.to_lowercase().contains(&word) })
                    .map(|c| {
                        let params = if c.params.is_empty() { String::new() } else { format!(" <{}>", c.params.join("> <")) };
                        (format!("{}{params}", c.title), format!("{}  {}", c.id, c.help), PaletteHit::Command(c.id, !c.params.is_empty()))
                    })
                    .collect()
            }
            Some(PaletteKind::Search) => self.ws.search(&text).into_iter().map(|h| (h.label.clone(), format!("{} · {}", h.what.name(), h.detail), PaletteHit::Go(h.target))).collect(),
            None => Vec::new(),
        }
    }

    /// Enter on the palette's entry `k`. True when the palette is done.
    pub fn palette_pick(&mut self, k: usize) -> bool {
        let entries = self.palette_entries();
        let Some((_, _, hit)) = entries.into_iter().nth(k) else { return false };
        match hit {
            PaletteHit::Go(target) => {
                self.go_to(&target);
                true
            }
            PaletteHit::Command(id, needs_words) => {
                let typed = self.palette.text.trim().to_string();
                let has_words = typed.split_whitespace().count() > 1 && typed.starts_with(id);
                if needs_words && !has_words {
                    // The command's id goes into the field for its words to follow.
                    self.palette.text = format!("{id} ");
                    self.palette.focus = true;
                    return false;
                }
                let line = if has_words { typed } else { id.to_string() };
                let r = self.run(&line);
                let done = r.is_ok() && r.as_ref().is_ok_and(|m| !m.contains('\n'));
                self.palette.answer = Some(r);
                done
            }
        }
    }

    fn palette_ui(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.palette.open else { return };
        let entries = self.palette_entries();
        let (down, up, enter) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter)));
        if down {
            self.palette.sel = (self.palette.sel + 1).min(entries.len().saturating_sub(1));
        }
        if up {
            self.palette.sel = self.palette.sel.saturating_sub(1);
        }
        self.palette.sel = self.palette.sel.min(entries.len().saturating_sub(1));
        let mut pick = enter.then_some(self.palette.sel);
        egui::Area::new(egui::Id::unique("palette")).anchor(egui::Align2::CENTER_TOP, [0.0, 70.0]).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(620.0);
                let hint = match kind {
                    PaletteKind::Commands => "Do anything: type a command",
                    PaletteKind::Search => "Go to anything: maps, definitions, functions, characters, zones, dialog lines",
                };
                let r = ui.add(egui::TextEdit::singleline(&mut self.palette.text).hint_text(hint).desired_width(f32::INFINITY));
                if std::mem::take(&mut self.palette.focus) {
                    r.request_focus();
                }
                if r.changed() {
                    self.palette.sel = 0;
                    self.palette.answer = None;
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for (k, (label, detail, _)) in entries.iter().enumerate().take(60) {
                        let text = egui::RichText::new(label).strong();
                        let row = ui.selectable_label(k == self.palette.sel, text).on_hover_text(detail);
                        ui.weak(detail);
                        if row.clicked() {
                            pick = Some(k);
                        }
                    }
                    if entries.is_empty() && !self.palette.text.trim().is_empty() {
                        ui.weak("Nothing found");
                    }
                });
                if let Some(a) = &self.palette.answer {
                    ui.separator();
                    match a {
                        Ok(m) => ui.label(m),
                        Err(e) => ui.colored_label(egui::Color32::LIGHT_RED, e),
                    };
                }
            });
        });
        if let Some(k) = pick {
            if self.palette_pick(k) {
                self.palette.open = None;
            }
        }
    }
}

/// What Enter does on a palette entry.
pub enum PaletteHit {
    /// A command, and whether it takes words.
    Command(&'static str, bool),
    Go(Target),
}

/// Draws the tabs.
struct Viewer<'a> {
    ws: &'a mut Workspace,
    play: &'a mut Option<PlayView>,
    play_error: &'a mut Option<String>,
    start: &'a rustpg2d_engine::runtime::StartOptions,
    lists: &'a [(RefKind, Vec<(String, String)>)],
    cats: &'a Cats,
    filter: &'a mut String,
    forms: &'a mut std::collections::HashMap<(RefKind, String), (serde_json::Value, String)>,
    show_all: &'a mut bool,
    thumbs: &'a mut crate::thumbs::Thumbs,
    marks: &'a Marks,
    maps: &'a mut std::collections::HashMap<String, crate::mapview::MapView>,
    rs: Option<&'a eframe::egui_wgpu::RenderState>,
    asks: &'a mut Vec<Ask>,
}

impl egui_dock::TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::unique(("tab", tab.name()))
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title(self.ws).into()
    }

    fn is_closeable(&self, tab: &Tab) -> bool {
        !matches!(tab, Tab::Browser)
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        // (The game and a map fill their tabs.)
        if matches!(tab, Tab::Play | Tab::Map(_)) {
            [false, false]
        } else {
            [false, true]
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Browser => self.browser(ui),
            Tab::Overview => self.overview(ui),
            Tab::Play => self.play_tab(ui),
            Tab::Problems => self.problems(ui),
            Tab::History => self.history(ui),
            Tab::Settings(page) => self.settings(ui, page),
            Tab::Kind(k) => self.kind(ui, *k),
            Tab::Def(k, id) => self.definition(ui, *k, id),
            Tab::Map(m) => self.map(ui, m),
        }
    }
}

impl Viewer<'_> {
    fn browser(&mut self, ui: &mut egui::Ui) {
        if !self.ws.project_open {
            ui.weak("No project is open.");
            return;
        }
        ui.add(egui::TextEdit::singleline(self.filter).hint_text("Filter").desired_width(f32::INFINITY));
        let f = self.filter.trim().to_lowercase();
        // (What is typed is looked for in ids and names, and in the creator's tags and folders.)
        let meta = self.ws.meta.clone();
        let fits = |kind: RefKind, id: &str, name: &str| {
            f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f) || meta.of(kind, id).is_some_and(|m| m.folder.to_lowercase().contains(&f) || m.tags.iter().any(|t| t.to_lowercase().contains(&f)))
        };
        egui::CollapsingHeader::new("Game").default_open(true).show(ui, |ui| {
            if f.is_empty() && ui.selectable_label(false, "Overview").clicked() {
                self.asks.push(Ask::Open(Tab::Overview));
            }
            // The settings' pages (what is typed also finds a page by a setting's name).
            for (page, about, fields) in editor_core::schema::PAGES {
                let fits = f.is_empty() || page.to_lowercase().contains(&f) || about.to_lowercase().contains(&f) || fields.split_whitespace().any(|k| editor_core::schema::label(&format!("ProjectConfig.{}", k.trim_start_matches('~')), k.trim_start_matches('~')).to_lowercase().contains(&f));
                if fits && ui.selectable_label(false, *page).on_hover_text(*about).clicked() {
                    self.asks.push(Ask::Open(Tab::Settings(page)));
                }
            }
        });
        // While something is typed, what fits is shown open and the rest is out of the way.
        let filtering = !f.is_empty();
        let name_of = |kind: RefKind, id: &str| -> Option<String> {
            if kind == RefKind::Map {
                self.ws.maps.iter().find(|m| *m == id).cloned()
            } else {
                self.lists.iter().find(|(k, _)| *k == kind).and_then(|(_, l)| l.iter().find(|(i, _)| i == id)).map(|(_, name)| name.clone())
            }
        };
        let favourites: Vec<(RefKind, String, String)> = meta
            .assets
            .iter()
            .filter(|(_, m)| m.favorite)
            .filter_map(|(key, _)| {
                let (kind, id) = key.split_once(':')?;
                let kind = RefKind::parse(kind)?;
                Some((kind, id.to_string(), name_of(kind, id)?))
            })
            .filter(|(kind, id, name)| fits(*kind, id, name))
            .collect();
        if !favourites.is_empty() {
            egui::CollapsingHeader::new(format!("Favourites ({})", favourites.len())).default_open(true).show(ui, |ui| {
                for (kind, id, name) in &favourites {
                    self.browser_row(ui, *kind, id, name);
                }
            });
        }
        for (group, kinds) in GROUPS {
            let found: Vec<(RefKind, Vec<(String, String)>)> = kinds
                .iter()
                .map(|kind| {
                    let rows = if *kind == RefKind::Map {
                        self.ws.maps.iter().filter(|m| fits(*kind, m, m)).map(|m| (m.clone(), m.clone())).collect()
                    } else {
                        self.lists.iter().find(|(k, _)| k == kind).map(|(_, l)| l.iter().filter(|(id, name)| fits(*kind, id, name)).cloned().collect()).unwrap_or_default()
                    };
                    (*kind, rows)
                })
                .filter(|(_, rows): &(RefKind, Vec<(String, String)>)| !filtering || !rows.is_empty())
                .collect();
            if filtering && found.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(*group).default_open(*group == "World").open(filtering.then_some(true)).show(ui, |ui| {
                for (kind, rows) in &found {
                    egui::CollapsingHeader::new(format!("{} ({})", plural(kind.label()), rows.len())).id_salt(("kind", kind.name())).open(filtering.then_some(rows.len() <= 40)).show(ui, |ui| {
                        if *kind != RefKind::Map {
                            ui.horizontal(|ui| {
                                if ui.small_button(format!("+ New {}", kind.label().to_lowercase())).clicked() {
                                    self.asks.push(Ask::New(Some(*kind)));
                                }
                                if ui.small_button("Show all").on_hover_text("All of them in a tab, as pictures or a list").clicked() {
                                    self.asks.push(Ask::Open(Tab::Kind(*kind)));
                                }
                            });
                        }
                        // Nothing typed: the creator's folders first, then the rest by what they
                        // are, where the kind has categories.
                        let folder_of = |id: &str| meta.of(*kind, id).map(|m| m.folder.as_str()).filter(|f| !f.is_empty());
                        let mut folders: Vec<&str> = if filtering { Vec::new() } else { rows.iter().filter_map(|(id, _)| folder_of(id)).collect() };
                        folders.sort_by_key(|f| f.to_lowercase());
                        folders.dedup();
                        for folder in &folders {
                            let inside: Vec<&(String, String)> = rows.iter().filter(|(id, _)| folder_of(id) == Some(*folder)).collect();
                            egui::CollapsingHeader::new(format!("{folder} ({})", inside.len())).id_salt(("folder", kind.name(), *folder)).default_open(true).show(ui, |ui| {
                                for (id, name) in inside {
                                    self.browser_row(ui, *kind, id, name);
                                }
                            });
                        }
                        let rest: Vec<(String, String)>;
                        let rows = if folders.is_empty() {
                            rows
                        } else {
                            rest = rows.iter().filter(|(id, _)| folder_of(id).is_none()).cloned().collect();
                            &rest
                        };
                        let cats = self.cats.get(kind);
                        let groups = if filtering { Vec::new() } else { categories_of(cats, rows) };
                        if groups.len() > 1 {
                            for (_, label, n) in &groups {
                                egui::CollapsingHeader::new(format!("{label} ({n})")).id_salt(("cat", kind.name(), label)).show(ui, |ui| {
                                    for (id, name) in rows.iter().filter(|(id, _)| cats.and_then(|c| c.get(id)).is_some_and(|(_, l)| l == label)) {
                                        self.browser_row(ui, *kind, id, name);
                                    }
                                });
                            }
                        } else {
                            for (id, name) in rows {
                                self.browser_row(ui, *kind, id, name);
                            }
                        }
                    });
                }
            });
        }
    }

    /// A definition (or map) in the browser: its picture and name, opened by a click.
    fn browser_row(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str, name: &str) {
        let label = if name == id { id.to_string() } else { format!("{name}  ({id})") };
        let clicked = ui
            .horizontal(|ui| {
                if let (true, Some(rs)) = (crate::thumbs::has_pictures(kind), self.rs) {
                    crate::thumbs::boxed(ui, 18.0, || self.thumbs.of(self.ws, rs, kind, id));
                }
                let clicked = ui.selectable_label(false, label).clicked();
                if let Some((error, text)) = self.marks.problems_of(kind, id).first() {
                    let color = if *error { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color };
                    ui.colored_label(color, "!").on_hover_text(text);
                }
                clicked
            })
            .inner;
        if clicked {
            self.asks.push(Ask::Open(if kind == RefKind::Map { Tab::Map(id.to_string()) } else { Tab::Def(kind, id.to_string()) }));
        }
    }

    fn overview(&mut self, ui: &mut egui::Ui) {
        if !self.ws.project_open {
            ui.heading("R(ust)PG2D Studio");
            ui.label(&self.ws.status.0);
            return;
        }
        let cfg = &self.ws.project.config;
        ui.heading(&cfg.name);
        ui.weak(self.ws.project_location().display().to_string());
        ui.add_space(8.0);
        ui.strong("To a playable game");
        let db = &self.ws.project.db;
        let (errors, _) = self.ws.problems.counts();
        let steps: [(&str, bool); 5] = [
            ("A map to start on", self.ws.maps.contains(&cfg.start_map)),
            ("A hero", !cfg.player.is_empty() && db.character(&cfg.player).is_some()),
            ("Somebody to talk to", db.characters.len() > 1),
            ("A quest", !db.quests.is_empty()),
            ("No errors in the project check", self.ws.problems.status() != "-" && errors == 0),
        ];
        for (text, done) in steps {
            ui.horizontal(|ui| {
                ui.label(if done { "[x]" } else { "[ ]" });
                ui.label(text);
            });
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Play").clicked() {
                self.asks.push(Ask::Open(Tab::Play));
            }
            if ui.button("Check the project").clicked() {
                self.asks.push(Ask::Run("check.problems".into()));
                self.asks.push(Ask::Open(Tab::Problems));
            }
        });
        ui.add_space(8.0);
        ui.strong("In this project");
        ui.label(format!("{} maps", self.ws.maps.len()));
        for (kind, list) in self.lists {
            if !list.is_empty() {
                ui.label(format!("{} {}", list.len(), plural(kind.label()).to_lowercase()));
            }
        }
    }

    fn play_tab(&mut self, ui: &mut egui::Ui) {
        if self.play.is_none() && self.play_error.is_none() && self.ws.project_open {
            match PlayView::open(&self.ws.project.root, self.start.clone()) {
                Ok(p) => *self.play = Some(p),
                Err(e) => *self.play_error = Some(format!("{e:#}")),
            }
        }
        match (self.play.as_mut(), self.rs) {
            (Some(p), Some(rs)) => p.show(ui, rs),
            _ => {
                ui.colored_label(egui::Color32::LIGHT_RED, self.play_error.clone().unwrap_or_else(|| "The game can't run here.".into()));
            }
        }
    }

    fn problems(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Check the project").clicked() {
                self.asks.push(Ask::Run("check.problems".into()));
            }
            let (e, w) = self.ws.problems.counts();
            ui.label(if self.ws.problems.status() == "-" { "Not checked yet".to_string() } else { format!("{e} error(s), {w} warning(s)") });
        });
        ui.separator();
        for p in &self.ws.problems.list {
            ui.horizontal_wrapped(|ui| {
                let color = if p.severity == editor_core::problems::Severity::Error { egui::Color32::from_rgb(235, 110, 100) } else { egui::Color32::from_rgb(230, 190, 90) };
                ui.colored_label(color, p.severity.name());
                if ui.link(&p.place).on_hover_text("Go there").clicked() {
                    match &p.target {
                        Target::Def(k, id) => self.asks.push(Ask::Open(Tab::Def(*k, id.clone()))),
                        Target::Map(m) | Target::MapTriggers(m) | Target::Entity(m, _) | Target::Zone(m, _) => self.asks.push(Ask::Open(Tab::Map(m.clone()))),
                        _ => self.asks.push(Ask::Open(Tab::Overview)),
                    }
                }
                ui.label(&p.text);
            });
        }
    }

    fn history(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.add_enabled(self.ws.can_undo(), egui::Button::new("Undo")).clicked() {
                self.asks.push(Ask::Run("edit.undo".into()));
            }
            if ui.add_enabled(self.ws.can_redo(), egui::Button::new("Redo")).clicked() {
                self.asks.push(Ask::Run("edit.redo".into()));
            }
        });
        ui.separator();
        for c in self.ws.timeline.undone.iter() {
            ui.weak(format!("(undone) {} · {}", c.label, c.at.describe()));
        }
        for c in self.ws.timeline.done.iter().rev() {
            ui.label(format!("{} · {}", c.label, c.at.describe()));
        }
        if self.ws.timeline.done.is_empty() && self.ws.timeline.undone.is_empty() {
            ui.weak("Nothing was changed yet. Every change shows here, newest first, and can be undone.");
        }
    }

    fn definition(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str) {
        let key = (kind, id.to_string());
        let Some(saved) = self.ws.def_value(kind, id) else {
            self.forms.remove(&key);
            ui.weak(format!("{} \"{id}\" is gone (renamed or deleted).", kind.label()));
            return;
        };
        ui.horizontal(|ui| {
            if let (true, Some(rs)) = (crate::thumbs::has_pictures(kind), self.rs) {
                crate::thumbs::boxed(ui, 56.0, || self.thumbs.of(self.ws, rs, kind, id));
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(saved.get("name").or(saved.get("title")).and_then(|n| n.as_str()).filter(|n| !n.is_empty()).unwrap_or(id));
                    ui.weak(format!("{} · {id}", kind.label()));
                });
                self.definition_buttons(ui, kind, id);
                self.meta_row(ui, kind, id);
            });
        });
        self.notes(ui, kind, id);
        for (error, text) in self.marks.problems_of(kind, id).iter().take(6) {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(if *error { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color }, if *error { "Error" } else { "Warning" });
                ui.label(text);
            });
        }
        if self.marks.is_unused(kind, id) {
            ui.weak("Nothing uses it (no other definition, map, setting or script names it).");
        }
        if self.ws.usages.of.as_ref().is_some_and(|(k, i)| *k == kind && i == id) {
            egui::CollapsingHeader::new("Used by").default_open(true).show(ui, |ui| {
                for u in &self.ws.usages.list {
                    if ui.link(format!("{}: {}", u.target.describe(), u.label)).clicked() {
                        match &u.target {
                            Target::Def(k, i) => self.asks.push(Ask::Open(Tab::Def(*k, i.clone()))),
                            Target::Map(m) | Target::MapTriggers(m) | Target::Entity(m, _) | Target::Zone(m, _) => self.asks.push(Ask::Open(Tab::Map(m.clone()))),
                            _ => {}
                        }
                    }
                }
            });
        }
        ui.separator();
        // The form shows what was typed last, also while that isn't a valid definition yet.
        let mut value = self.forms.get(&key).map_or_else(|| saved.clone(), |(v, _)| v.clone());
        let mut pics = self.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *self.thumbs, ws: &*self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: *self.show_all, compact: false, pictures, refs: Some(&self.ws.refs), asks: Vec::new() };
        let changed = match editor_core::schema::of_kind(kind) {
            Some(node) => crate::form::definition_ui(ui, &mut cx, node, &mut value),
            None => crate::form::value_ui(ui, kind.label(), &mut value),
        };
        for ask in cx.asks {
            match ask {
                crate::form::Ask::Open(RefKind::Map, map) => self.asks.push(Ask::Open(Tab::Map(map))),
                crate::form::Ask::Open(k, i) => self.asks.push(Ask::Open(Tab::Def(k, i))),
                crate::form::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
            }
        }
        if changed {
            match self.ws.set_def_value(kind, id, value.clone()) {
                Ok(()) => {
                    self.forms.remove(&key);
                }
                Err(e) => {
                    self.forms.insert(key.clone(), (value, e));
                }
            }
        }
        if let Some((_, why)) = self.forms.get(&key) {
            ui.separator();
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Not taken yet: {why}"));
            if ui.button("Back to how it was").clicked() {
                self.forms.remove(&key);
            }
        }
        let notes = self.ws.def_notes(kind);
        if !notes.is_empty() {
            ui.separator();
            egui::CollapsingHeader::new("About these fields").show(ui, |ui| {
                ui.label(notes);
            });
        }
    }

    /// The creator's own order for an asset: favourite, folder, tags.
    fn meta_row(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str) {
        let m = self.ws.meta.of(kind, id).cloned().unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            let mut favourite = m.favorite;
            if ui.toggle_value(&mut favourite, "Favourite").on_hover_text("Favourites are listed first in the Project browser").changed() {
                self.ws.meta_edit(kind, id, |m| m.favorite = favourite);
            }
            ui.separator();
            ui.weak("Folder");
            let folder = if m.folder.is_empty() { "none".to_string() } else { m.folder.clone() };
            if let Some(new) = name_menu(ui, &folder, "A folder: Village/People", &self.ws.meta.folders(), Some("No folder")).filter(|f| *f != m.folder) {
                self.ws.meta_edit(kind, id, |m| m.folder = new);
            }
            ui.separator();
            ui.weak("Tags");
            for tag in &m.tags {
                if ui.small_button(format!("{tag}  x")).on_hover_text("Take this tag away").clicked() {
                    self.ws.meta_edit(kind, id, |m| m.tags.retain(|t| t != tag));
                }
            }
            let others: Vec<String> = self.ws.meta.tags().into_iter().filter(|t| !m.tags.iter().any(|x| x.eq_ignore_ascii_case(t))).collect();
            if let Some(tag) = name_menu(ui, "+ Tag", "A tag of your own", &others, None) {
                self.ws.meta_edit(kind, id, |m| m.tags.push(tag));
            }
        });
    }

    /// The creator's notes on an asset (kept when the field is left).
    fn notes(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str) {
        let noted = self.ws.meta.of(kind, id).map(|m| m.notes.clone()).unwrap_or_default();
        egui::CollapsingHeader::new(if noted.is_empty() { "Notes".to_string() } else { format!("Notes: {}", noted.lines().next().unwrap_or("").chars().take(60).collect::<String>()) }).id_salt("notes").show(ui, |ui| {
            let bid = ui.scope_id().with("text");
            let mut text: String = ui.data_mut(|d| d.get_temp(bid)).unwrap_or_else(|| noted.clone());
            let r = ui.add(egui::TextEdit::multiline(&mut text).hint_text("Notes for yourself: what it is for, what is still to do. The game doesn't see them.").desired_rows(3).desired_width(520.0));
            if r.lost_focus() {
                self.ws.meta_edit(kind, id, |m| m.notes = text.trim_end().to_string());
                ui.data_mut(|d| d.remove::<String>(bid));
            } else if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(bid, text));
            }
        });
    }

    /// A page of the game's settings: the form of its fields of the project's settings.
    fn settings(&mut self, ui: &mut egui::Ui, page: &str) {
        let Some((title, about, fields)) = editor_core::schema::PAGES.iter().find(|(t, ..)| *t == page) else { return };
        ui.horizontal(|ui| {
            ui.heading(*title);
            ui.weak(*about);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(self.show_all, "Show all fields");
            });
        });
        ui.separator();
        let mut value = self.ws.config_value();
        let mut pics = self.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *self.thumbs, ws: &*self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: *self.show_all, compact: false, pictures, refs: Some(&self.ws.refs), asks: Vec::new() };
        let changed = crate::form::page_ui(ui, &mut cx, editor_core::schema::project(), &mut value, fields);
        for ask in cx.asks {
            match ask {
                crate::form::Ask::Open(RefKind::Map, map) => self.asks.push(Ask::Open(Tab::Map(map))),
                crate::form::Ask::Open(k, i) => self.asks.push(Ask::Open(Tab::Def(k, i))),
                crate::form::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
            }
        }
        if changed {
            if let Err(e) = self.ws.set_config_value(value) {
                self.ws.set_status(e);
            }
        }
    }

    /// What can be done with a definition as a whole.
    fn definition_buttons(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str) {
        ui.horizontal(|ui| {
            if ui.button("Rename…").on_hover_text("Changes the id, and every place that names it").clicked() {
                self.asks.push(Ask::Palette(format!("edit.rename {} {id} ", kind.name())));
            }
            if ui.button("Duplicate…").clicked() {
                self.asks.push(Ask::Palette(format!("defs.duplicate {} {id} ", kind.name())));
            }
            if ui.button("Delete").on_hover_text("Refused while something still uses it").clicked() {
                self.asks.push(Ask::Run(format!("defs.delete {} {id}", kind.name())));
            }
            if ui.button("Find usages").clicked() {
                self.asks.push(Ask::Run(format!("find.usages {} {id}", kind.name())));
            }
            if self.ws.usages.of.as_ref().is_some_and(|(k, i)| *k == kind && i == id) {
                ui.label(format!("{} usage(s)", self.ws.usages.list.len()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(self.show_all, "Show all fields").on_hover_text("Also the fields that are rarely needed: values from before the SRD rules, data other tools edit");
            });
        });
    }

    /// Everything of a kind: cards with pictures (or a plain list), filtered by what is typed.
    fn kind(&mut self, ui: &mut egui::Ui, kind: RefKind) {
        let all: &[(String, String)] = self.lists.iter().find(|(k, _)| *k == kind).map_or(&[], |(_, l)| l.as_slice());
        let (fid, lid) = (ui.scope_id().with("filter"), egui::Id::unique(("kind as list", kind.name())));
        let mut filter: String = ui.data_mut(|d| d.get_temp(fid).unwrap_or_default());
        let pictures = crate::thumbs::has_pictures(kind) && self.rs.is_some();
        // 0 = cards with pictures, 1 = a list, 2 = a table with a column per field.
        let mut view: u8 = ui.data_mut(|d| d.get_persisted(lid).unwrap_or(if pictures { 0 } else { 1 }));
        if view == 0 && !pictures {
            view = 1;
        }
        ui.horizontal(|ui| {
            ui.heading(plural(kind.label()));
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter").desired_width(220.0));
            if pictures {
                ui.selectable_value(&mut view, 0, "Pictures");
            }
            ui.selectable_value(&mut view, 1, "List");
            ui.selectable_value(&mut view, 2, "Table").on_hover_text("A column per field: compare, sort and change many at once");
            if ui.button(format!("+ New {}", kind.label().to_lowercase())).clicked() {
                self.asks.push(Ask::New(Some(kind)));
            }
        });
        // Only the ones something is wrong with, or the ones nothing uses.
        let with_problems = all.iter().filter(|(id, _)| !self.marks.problems_of(kind, id).is_empty()).count();
        let wid = ui.scope_id().with("which");
        let mut which: u8 = ui.data_mut(|d| d.get_temp(wid).unwrap_or(0));
        ui.horizontal_wrapped(|ui| {
            ui.weak("Show");
            ui.selectable_value(&mut which, 0, "all");
            let checked = self.ws.problems.built;
            let label = if checked { format!("with problems ({with_problems})") } else { "with problems…".to_string() };
            if ui.selectable_value(&mut which, 1, label).on_hover_text("What the project check found wrong").clicked() && !checked {
                self.asks.push(Ask::Run("check.problems".into()));
            }
            // (The kinds with files of their own: what the clean-up of unused files looks at.)
            if matches!(kind, RefKind::Sheet | RefKind::Model | RefKind::Sound | RefKind::Material | RefKind::Font) {
                let label = match &self.marks.unused {
                    Some(_) => format!("unused ({})", all.iter().filter(|(id, _)| self.marks.is_unused(kind, id)).count()),
                    None => "unused…".to_string(),
                };
                if ui.selectable_value(&mut which, 2, label).on_hover_text("What no other definition, map, setting or script names").clicked() && self.marks.unused.is_none() {
                    self.asks.push(Ask::FindUnused);
                }
            } else if which == 2 {
                which = 0;
            }
        });
        let marks = self.marks;
        let shown = |id: &str| match which {
            1 => !marks.problems_of(kind, id).is_empty(),
            2 => marks.is_unused(kind, id),
            _ => true,
        };
        // What they are: one category at a time, or all.
        let cats = self.cats.get(&kind);
        let groups = categories_of(cats, all);
        let cid = ui.scope_id().with("category");
        let mut category: String = ui.data_mut(|d| d.get_temp(cid).unwrap_or_default());
        if groups.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(category.is_empty(), format!("All ({})", all.len())).clicked() {
                    category.clear();
                }
                for (_, label, n) in &groups {
                    if ui.selectable_label(category == *label, format!("{label} ({n})")).clicked() {
                        category = label.clone();
                    }
                }
            });
        }
        // The creator's tags among them: one at a time, or none.
        let meta = self.ws.meta.clone();
        let mut tags: Vec<(String, usize)> = Vec::new();
        for tag in meta.tags() {
            let n = all.iter().filter(|(id, _)| meta.has_tag(kind, id, &tag)).count();
            if n > 0 {
                tags.push((tag, n));
            }
        }
        let tid = ui.scope_id().with("tag");
        let mut tag: String = ui.data_mut(|d| d.get_temp(tid).unwrap_or_default());
        if !tags.iter().any(|(t, _)| *t == tag) {
            tag.clear();
        }
        if !tags.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Tags");
                for (t, n) in &tags {
                    if ui.selectable_label(tag == *t, format!("{t} ({n})")).clicked() {
                        tag = if tag == *t { String::new() } else { t.clone() };
                    }
                }
            });
        }
        let f = filter.trim().to_lowercase();
        let in_category = |id: &str| category.is_empty() || cats.and_then(|c| c.get(id)).is_some_and(|(_, l)| *l == category);
        let tagged = |id: &str| tag.is_empty() || meta.has_tag(kind, id, &tag);
        let rows: Vec<&(String, String)> = all.iter().filter(|(id, name)| shown(id) && in_category(id) && tagged(id) && (f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f))).collect();
        // Several chosen at once (Ctrl+click a card, a row's box): tagged, moved, deleted together.
        let sid = ui.scope_id().with("chosen");
        let mut chosen: std::collections::BTreeSet<String> = ui.data_mut(|d| d.get_temp(sid).unwrap_or_default());
        chosen.retain(|id| all.iter().any(|(i, _)| i == id));
        if chosen.is_empty() {
            // (The row is always there: choosing the first doesn't push everything down.)
            ui.horizontal(|ui| {
                ui.set_min_height(ui.spacing().interact_size.y);
                ui.weak("Choose several (Ctrl+click a card, a row's box) to tag, move, change or delete them together.");
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} chosen", chosen.len()));
                let what = format!("{} {}", chosen.len(), plural(kind.label()).to_lowercase());
                if let Some(t) = name_menu(ui, "Tag…", "A tag of your own", &meta.tags(), None) {
                    self.ws.begin(&format!("Tag {what}"));
                    for id in &chosen {
                        self.ws.meta_edit(kind, id, |m| m.tags.push(t.clone()));
                    }
                }
                if let Some(t) = name_menu(ui, "Untag…", "", &tags.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>(), None) {
                    self.ws.begin(&format!("Untag {what}"));
                    for id in &chosen {
                        self.ws.meta_edit(kind, id, |m| m.tags.retain(|x| !x.eq_ignore_ascii_case(&t)));
                    }
                }
                if let Some(folder) = name_menu(ui, "Move to folder…", "A folder: Village/People", &meta.folders(), Some("No folder")) {
                    self.ws.begin(&format!("Move {what}"));
                    for id in &chosen {
                        self.ws.meta_edit(kind, id, |m| m.folder = folder.clone());
                    }
                }
                if ui.button("Favourite").clicked() {
                    self.ws.begin(&format!("Favourite {what}"));
                    for id in &chosen {
                        self.ws.meta_edit(kind, id, |m| m.favorite = true);
                    }
                }
                if ui.button("Delete").on_hover_text("Each is refused while something still uses it").clicked() {
                    for id in &chosen {
                        self.asks.push(Ask::Run(format!("defs.delete {} {id}", kind.name())));
                    }
                    chosen.clear();
                }
                if ui.button("Choose all shown").clicked() {
                    chosen.extend(rows.iter().map(|(id, _)| id.clone()));
                }
                if ui.button("Choose none").clicked() {
                    chosen.clear();
                }
            });
        }
        ui.weak(if rows.len() == all.len() { format!("{}", all.len()) } else { format!("{} of {}", rows.len(), all.len()) });
        ui.separator();
        let mut open = None;
        if view == 2 {
            let ids: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
            open = self.kind_table(ui, kind, &ids, &mut chosen);
        } else if view == 1 {
            for (id, name) in &rows {
                ui.horizontal(|ui| {
                    let mut on = chosen.contains(id);
                    if ui.checkbox(&mut on, "").on_hover_text("Choose it, to tag, move or delete several at once").changed() {
                        if on {
                            chosen.insert(id.clone());
                        } else {
                            chosen.remove(id);
                        }
                    }
                    if let (true, Some(rs)) = (pictures, self.rs) {
                        crate::thumbs::boxed(ui, 22.0, || self.thumbs.of(self.ws, rs, kind, id));
                    }
                    if ui.selectable_label(false, name.as_str()).clicked() {
                        open = Some(id.clone());
                    }
                    if name != id {
                        ui.weak(id.as_str());
                    }
                    if let Some((error, text)) = marks.problems_of(kind, id).first() {
                        ui.colored_label(if *error { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color }, text);
                    } else if marks.is_unused(kind, id) {
                        ui.weak("unused");
                    }
                    if let Some(m) = meta.of(kind, id) {
                        if !m.folder.is_empty() {
                            ui.weak(format!("in {}", m.folder));
                        }
                        for t in &m.tags {
                            ui.weak(format!("#{t}"));
                        }
                    }
                });
            }
        } else if let Some(rs) = self.rs {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (id, name) in &rows {
                    let mark = marks.color(ui, kind, id);
                    let r = crate::thumbs::card(ui, name, id, chosen.contains(id), mark, || self.thumbs.of(self.ws, rs, kind, id));
                    // Ctrl+click chooses (several at once); a plain click opens.
                    if r.clicked() && ui.input(|i| i.modifiers.command) {
                        if !chosen.remove(id) {
                            chosen.insert(id.clone());
                        }
                    } else if r.clicked() {
                        open = Some(id.clone());
                    }
                }
            });
        }
        if let Some(id) = open {
            self.asks.push(Ask::Open(Tab::Def(kind, id)));
        }
        ui.data_mut(|d| {
            d.insert_temp(fid, filter);
            d.insert_temp(cid, category);
            d.insert_temp(wid, which);
            d.insert_temp(tid, tag);
            d.insert_temp(sid, chosen);
            d.insert_persisted(lid, view);
        });
    }

    /// A kind as a table: a row per definition, a column per field that fits into a cell (the
    /// *Columns* menu picks which), cells changed in place. A click on a column's head sorts by
    /// it. A change in a chosen row is made in every chosen row. Returns the id to open.
    fn kind_table(&mut self, ui: &mut egui::Ui, kind: RefKind, ids: &[String], chosen: &mut std::collections::BTreeSet<String>) -> Option<String> {
        use editor_core::schema::{self, Shape};
        let node = schema::of_kind(kind)?;
        let Shape::Struct(fields) = node.shape() else { return None };
        let cells: Vec<&schema::Field> = fields.iter().filter(|f| f.key != "id" && crate::form::fits_cell(f)).collect();
        // The columns: as chosen before, else the first section's fields.
        let col_id = egui::Id::unique(("columns", kind.name()));
        let first: Vec<String> = schema::sections(node.ty).first().map(|(_, list)| list.split_whitespace().filter(|k| !k.starts_with('~')).map(str::to_string).collect()).unwrap_or_default();
        let mut columns: Vec<String> = ui.data_mut(|d| d.get_persisted(col_id)).unwrap_or_else(|| cells.iter().filter(|f| first.is_empty() || first.iter().any(|k| k == f.key)).take(7).map(|f| f.key.to_string()).collect());
        let sort_id = ui.scope_id().with("sort");
        let mut sort: (String, bool) = ui.data_mut(|d| d.get_temp(sort_id).unwrap_or_default());
        ui.horizontal(|ui| {
            ui.menu_button("Columns", |ui| {
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for f in &cells {
                        let mut on = columns.iter().any(|c| c == f.key);
                        if ui.checkbox(&mut on, f.label()).changed() {
                            if on {
                                columns.push(f.key.to_string());
                            } else {
                                columns.retain(|c| c != f.key);
                            }
                        }
                    }
                });
            });
            if !chosen.is_empty() {
                ui.weak(format!("A change in a chosen row is made in all {} chosen.", chosen.len()));
            }
        });
        // (In the definition's order, whatever order they were ticked in.)
        let shown: Vec<&schema::Field> = cells.iter().copied().filter(|f| columns.iter().any(|c| c == f.key)).collect();
        // Sorted by a column: the order is worked out again when something changed.
        let mut order: Vec<String> = ids.to_vec();
        if !sort.0.is_empty() {
            let stamp = format!("{}|{}|{}|{}|{}", sort.0, sort.1, self.ws.timeline.status(), ids.len(), ids.first().map_or("", String::as_str));
            let cache_id = ui.scope_id().with("order");
            let cached: Option<(String, Vec<String>)> = ui.data_mut(|d| d.get_temp(cache_id));
            order = match cached {
                Some((s, o)) if s == stamp => o,
                _ => {
                    let blank = shown.iter().find(|f| f.key == sort.0).map(|f| f.node.blank()).unwrap_or(serde_json::Value::Null);
                    let mut keyed: Vec<(serde_json::Value, String)> = ids.iter().map(|id| (self.ws.def_value(kind, id).and_then(|v| v.get(&sort.0).cloned()).unwrap_or_else(|| blank.clone()), id.clone())).collect();
                    keyed.sort_by(|(a, _), (b, _)| {
                        let by_number = a.as_f64().zip(b.as_f64()).map(|(x, y)| x.total_cmp(&y));
                        let o = by_number.unwrap_or_else(|| a.as_str().map(str::to_lowercase).unwrap_or_else(|| a.to_string()).cmp(&b.as_str().map(str::to_lowercase).unwrap_or_else(|| b.to_string())));
                        if sort.1 {
                            o.reverse()
                        } else {
                            o
                        }
                    });
                    let o: Vec<String> = keyed.into_iter().map(|(_, id)| id).collect();
                    ui.data_mut(|d| d.insert_temp(cache_id, (stamp, o.clone())));
                    o
                }
            };
        }
        let row_height = ui.spacing().interact_size.y + 2.0;
        let name_width = 250.0;
        let mut open = None;
        let mut edits: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut asks = Vec::new();
        let pictures = crate::thumbs::has_pictures(kind);
        let names: std::collections::HashMap<&str, &str> = self.lists.iter().find(|(k, _)| *k == kind).map(|(_, l)| l.iter().map(|(id, name)| (id.as_str(), name.as_str())).collect()).unwrap_or_default();
        egui::ScrollArea::horizontal().id_salt("table").show(ui, |ui| {
            // The heads: a click sorts by the column, another turns the order round.
            let mut heads: Vec<egui::Id> = Vec::new();
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(name_width + 26.0, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(name_width + 26.0);
                    if ui.selectable_label(sort.0.is_empty(), kind.label()).on_hover_text("In the order of the project's file").clicked() {
                        sort = Default::default();
                    }
                });
                for f in &shown {
                    ui.allocate_ui_with_layout(egui::vec2(crate::form::cell_width(f), row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(crate::form::cell_width(f));
                        let mark = if sort.0 == f.key { if sort.1 { "  v" } else { "  ^" } } else { "" };
                        let r = ui.selectable_label(sort.0 == f.key, format!("{}{mark}", f.label()));
                        heads.push(r.id);
                        let help = f.node.help();
                        if r.clicked() {
                            sort = if sort.0 == f.key { (sort.0.clone(), !sort.1) } else { (f.key.to_string(), false) };
                        }
                        if !help.is_empty() {
                            r.on_hover_text(help);
                        }
                    });
                }
            });
            ui.separator();
            let height = (ui.available_height() - 4.0).max(row_height * 6.0);
            egui::ScrollArea::vertical().id_salt("rows").max_height(height).auto_shrink([false, false]).show_rows(ui, row_height, order.len(), |ui, range| {
                let mut pics = self.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *self.thumbs, ws: &*self.ws, rs });
                let pictures_for = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
                let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: true, compact: true, pictures: pictures_for, refs: Some(&self.ws.refs), asks: Vec::new() };
                for id in &order[range] {
                    let Some(mut value) = self.ws.def_value(kind, id) else { continue };
                    ui.push_id(id.as_str(), |ui| {
                        ui.horizontal(|ui| {
                            let mut on = chosen.contains(id);
                            if ui.checkbox(&mut on, "").changed() {
                                if on {
                                    chosen.insert(id.clone());
                                } else {
                                    chosen.remove(id);
                                }
                            }
                            ui.allocate_ui_with_layout(egui::vec2(name_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(name_width);
                                if pictures {
                                    crate::thumbs::boxed(ui, row_height - 2.0, || cx.pictures.as_mut().and_then(|p| p.of(kind, id)));
                                }
                                let name = names.get(id.as_str()).copied().unwrap_or(id.as_str());
                                if ui.add(egui::Label::new(name).truncate().sense(egui::Sense::click())).on_hover_text(format!("Open {name}  ({id})")).clicked() {
                                    open = Some(id.clone());
                                }
                            });
                            let Some(map) = value.as_object_mut() else { return };
                            for (f, head) in shown.iter().zip(&heads) {
                                ui.allocate_ui_with_layout(egui::vec2(crate::form::cell_width(f), row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.set_min_width(crate::form::cell_width(f));
                                    ui.push_id(f.key, |ui| {
                                        if crate::form::cell(ui, &mut cx, f, map, *head) {
                                            edits.push((id.clone(), f.key.to_string(), map.get(f.key).cloned().unwrap_or(serde_json::Value::Null)));
                                        }
                                    });
                                });
                            }
                        });
                    });
                }
                asks.append(&mut cx.asks);
            });
        });
        // A change: in its row, and in every chosen row when the row is one of them.
        for (id, key, new) in edits {
            let targets: Vec<String> = if chosen.contains(&id) { chosen.iter().cloned().collect() } else { vec![id] };
            if targets.len() > 1 {
                self.ws.begin(&format!("Change {} of {} {}", key.replace('_', " "), targets.len(), plural(kind.label()).to_lowercase()));
            }
            for target in targets {
                let Some(mut value) = self.ws.def_value(kind, &target) else { continue };
                value[key.as_str()] = new.clone();
                if let Err(e) = self.ws.set_def_value(kind, &target, value) {
                    self.ws.set_status(format!("{target}: {e}"));
                }
            }
        }
        for ask in asks {
            match ask {
                crate::form::Ask::Open(RefKind::Map, map) => self.asks.push(Ask::Open(Tab::Map(map))),
                crate::form::Ask::Open(k, i) => self.asks.push(Ask::Open(Tab::Def(k, i))),
                crate::form::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
            }
        }
        ui.data_mut(|d| {
            d.insert_persisted(col_id, columns);
            d.insert_temp(sort_id, sort);
        });
        open
    }

    fn map(&mut self, ui: &mut egui::Ui, name: &str) {
        let view = self.maps.entry(name.to_string()).or_default();
        let mut parts = crate::mapview::Parts { ws: &mut *self.ws, rs: self.rs, lists: self.lists, thumbs: &mut *self.thumbs, show_all: *self.show_all, asks: Vec::new() };
        view.show(ui, &mut parts, name);
        for ask in parts.asks {
            match ask {
                crate::mapview::Ask::OpenDef(k, id) => self.asks.push(Ask::Open(Tab::Def(k, id))),
                crate::mapview::Ask::OpenMap(m) => self.asks.push(Ask::Open(Tab::Map(m))),
                crate::mapview::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
            }
        }
    }
}
