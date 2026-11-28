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
    /// Every map and the ways between them.
    World,
    /// A map's events as a graph, by the map's name.
    Graph(String),
    /// A script file of the project, by its name (`main.rhai`).
    Script(String),
    /// Every flag and variable with where it is set and read.
    Names,
    /// The game exported to play without the editor.
    Publish,
    /// The asset library: what it holds, added to the project.
    Library,
    /// Generate art: what Meshy is asked for, the jobs, their results.
    Art,
    /// The Creator's Manual.
    Help,
    /// AI agents: the agent server, the set-up for an agent's program, what the agents did.
    Agents,
}

impl Tab {
    fn title(&self, ws: &Workspace) -> String {
        match self {
            Tab::Browser => "Project".into(),
            Tab::Overview => "Overview".into(),
            Tab::World => "World".into(),
            Tab::Graph(m) => format!("Events: {m}"),
            Tab::Script(file) => file.clone(),
            Tab::Names => "Flags and variables".into(),
            Tab::Publish => "Publishing".into(),
            Tab::Library => "Library".into(),
            Tab::Art => "Generate art".into(),
            Tab::Help => "Help".into(),
            Tab::Agents => "AI agents".into(),
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
            Tab::World => "world".into(),
            Tab::Graph(m) => format!("graph:{m}"),
            Tab::Script(file) => format!("script:{file}"),
            Tab::Names => "names".into(),
            Tab::Publish => "publish".into(),
            Tab::Library => "library".into(),
            Tab::Art => "art".into(),
            Tab::Help => "help".into(),
            Tab::Agents => "agents".into(),
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

/// How the Problems tab mends something that is named but doesn't exist.
enum Fix {
    /// An empty one under that id.
    Create,
    /// The asset library's one of that id.
    Library,
    /// Another one that exists, named instead everywhere.
    Another(String),
}

/// What a tab's UI asked for (done after the tabs are drawn).
enum Ask {
    Open(Tab),
    Run(String),
    /// The command palette with this typed (a command waiting for its words).
    Palette(String),
    /// The *New…* gallery, with this kind picked.
    New(Option<RefKind>),
    /// Generate art for this thing of the project.
    Art(RefKind, String),
    /// *Share…*: these assets as a pack.
    Share(Vec<rustpg2d_engine::library::AssetRef>),
    /// The manual at this chapter.
    Help(&'static str),
    /// *What's new*: the changelog's part for this version, and the news counted as seen.
    News,
    /// The news were put away unread.
    NewsSeen,
    /// Play-tests start in this language from now on; true: one starts now, from the title.
    PlayIn(Option<String>, bool),
    /// Look for what nothing uses.
    FindUnused,
    /// Start the game in the Play tab: from the title screen, on the start map, on a map at a
    /// tile, or again as it was started.
    Play(PlayFrom),
    /// The game in its own window, started as the Play tab's was.
    PlayWindow,
    /// The first tab above the second, both in view.
    Above(Tab, Tab),
    /// Open this project in the window; make a new one there and open it; back to none.
    OpenProject(PathBuf),
    /// A new project there: what the game is called, and whether it starts with the basics.
    NewProject(PathBuf, String, bool),
    CloseProject,
    /// Where a search hit, a usage or a card of the events graph leads.
    GoTo(Target),
    /// *Test setup…*.
    TestSetup,
    /// *Try it*: the game on this map with the hero on the tile and the event raised.
    Try(String, (i32, i32), Box<(rustpg2d_engine::events::TriggerOn, rustpg2d_engine::runtime::EventOwner, String)>),
}

/// Where a game in the Play tab starts.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayFrom {
    Title,
    Start,
    /// A map and the tile the hero stands on; true: with the project's test setup (a saved
    /// game loaded there: the map's *Map enter* events don't run).
    Tile(String, (i32, i32), bool),
    /// A cut-scene on its map, from this second of it.
    Cutscene(String, f32),
    Again,
}

/// The library asset *New…* shows a picture of: it and what it needs as a small workspace of
/// its own, with the pictures drawn from that.
pub struct LibLook {
    kind: RefKind,
    id: String,
    ws: Box<Workspace>,
    thumbs: crate::thumbs::Thumbs,
}

/// The file of the manual's chapter `k`.
fn manual_file(k: usize) -> &'static str {
    editor_core::manual::chapters().get(k).map_or("README.md", |c| c.file)
}

/// Pixel editors are kept by sheet id; a ground material's under this before its id (no id
/// has a colon).
const MATERIAL_KEY: &str = "material:";

fn material_key(id: &str) -> String {
    format!("{MATERIAL_KEY}{id}")
}

/// What is typed on the home screen.
#[derive(Default)]
pub struct Home {
    new_name: String,
    /// Where the new project goes (filled in from its name until it is typed).
    new_at: String,
    new_at_typed: bool,
    /// The new project doesn't get the basics from the asset library.
    new_empty: bool,
    open_path: String,
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
    /// The *Share…* window while it is open.
    pub share: Option<crate::packspage::ShareWindow>,
    /// *Clean up unused files…* while it is open.
    pub cleanup: Option<crate::cleanupwin::CleanupWindow>,
    lib_look: Option<LibLook>,
    marks: Marks,
    /// The open maps' views (camera, selection, tool), by map.
    pub maps: std::collections::HashMap<String, crate::mapview::MapView>,
    /// The World view.
    pub world: crate::worldview::WorldView,
    /// The events graphs open in tabs, by map.
    pub graphs: std::collections::HashMap<String, crate::graphview::GraphView>,
    /// A transition zone waiting for its other side to be drawn on another map: (map, zone).
    pub link: Option<(String, String)>,
    /// What was copied on a map, pasted on any.
    pub clip: Option<editor_core::mapedit::Clip>,
    /// The scripts open in tabs, by file name.
    pub scripts: std::collections::HashMap<String, crate::scriptedit::ScriptTab>,
    /// The Flags and variables page.
    pub names: crate::namespage::NamesPage,
    /// The Publishing page.
    pub publish: crate::publish::PublishPage,
    /// The Library tab.
    pub library: crate::librarypage::LibraryPage,
    pub art: crate::artpage::ArtPage,
    pub help: crate::helpview::HelpView,
    pub tour: crate::tourpanel::TourPanel,
    pub agents: crate::agentspage::AgentsPage,
    /// Translations, on the Languages page.
    pub langs: crate::langpage::LangPage,
    /// Plugins, on the Scripts & plugins page.
    pub plugins: crate::pluginspage::PluginsPage,
    /// The language play-tests start in (`None`: as the player's settings say).
    pub play_lang: Option<String>,
    /// The editor is newer than the one whose news were looked at: the Overview says so.
    pub news: bool,
    /// The home screen's fields.
    pub home: Home,
    /// The timelines of the cut-scenes open in tabs, by id.
    pub timelines: std::collections::HashMap<String, crate::timeline::Timeline>,
    /// The canvases of the visual scripts open in tabs, by id.
    pub nodes: std::collections::HashMap<String, crate::nodesview::NodesView>,
    /// The workshops of the props open in tabs, by id.
    pub stages: std::collections::HashMap<String, crate::propshop::PropStage>,
    /// The workshops of the characters open in tabs, by id.
    pub figures: std::collections::HashMap<String, crate::charshop::CharStage>,
    pub model_stages: std::collections::HashMap<String, crate::modelshop::ModelStage>,
    /// The workshops of the items open in tabs, by id.
    pub item_stages: std::collections::HashMap<String, crate::itemshop::ItemStage>,
    /// The animations open in tabs, by id: played on their stage.
    pub anim_stages: std::collections::HashMap<String, crate::animshop::AnimStage>,
    /// The pixel editors of sprite sheets, by sheet (a picture changed there waits to be saved).
    pub pixels: std::collections::HashMap<String, crate::pixeledit::PixelEdit>,
    /// The sounds and sprite sheets open in tabs, by id: listened to, cut into frames.
    pub sounds: std::collections::HashMap<String, crate::artshop::SoundStage>,
    pub sheets: std::collections::HashMap<String, crate::artshop::SheetStage>,
    /// Where a cut-scene has everyone at its playhead (its map, the moment, the frame it was
    /// looked at in): the map's tab shows it while the timeline is in view.
    pub scrub: Option<(String, editor_core::cutscene::Preview, u64)>,
    /// The event the next game in the Play tab raises once it runs (*Try it*).
    pub try_next: Option<(rustpg2d_engine::events::TriggerOn, rustpg2d_engine::runtime::EventOwner, String)>,
    /// Answers of commands, newest last (the script prints them too).
    pub log: Vec<String>,
    /// What the creator asked for that leaves the project, waiting for the answer to *There
    /// are changes that aren't saved*.
    pub leaving: Option<Leaving>,
    /// The editor may quit now (asked and answered): the window's close goes through.
    pub quit_now: bool,
    /// The clean-up was offered for this leaving already.
    cleanup_passed: bool,
    /// *Test setup…* while it is open.
    pub setup: Option<crate::setupwin::SetupWindow>,
}

/// What leaves the open project: the editor quits, the project is closed, another one is opened
/// or made (where, its name, with the basics).
#[derive(Clone, Debug, PartialEq)]
pub enum Leaving {
    Quit,
    Close,
    Open(PathBuf),
    New(PathBuf, String, bool),
}

impl Leaving {
    /// For scripts: `quit`, `close`, `open`, `new`.
    pub fn word(&self) -> &'static str {
        match self {
            Leaving::Quit => "quit",
            Leaving::Close => "close",
            Leaving::Open(_) => "open",
            Leaving::New(..) => "new",
        }
    }

    /// What is done once answered, in words: "quit", "open game.r2dproject".
    fn doing(&self) -> String {
        let name = |p: &std::path::Path| p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
        match self {
            Leaving::Quit => "quit".into(),
            Leaving::Close => "close this project".into(),
            Leaving::Open(p) => format!("open {}", name(p)),
            Leaving::New(p, ..) => format!("make the project {}", name(p)),
        }
    }
}

/// What is selected on a map's tab, as the owner of events the graph can be focused on.
fn selected_owner(maps: &std::collections::HashMap<String, crate::mapview::MapView>, map: &str) -> Option<editor_core::graph::Owner> {
    use editor_core::workspace::Selection;
    match maps.get(map).and_then(|v| v.sel.first()) {
        Some(Selection::Entity(uid)) => Some(editor_core::graph::Owner::Entity(*uid)),
        Some(Selection::Zone(uid)) => Some(editor_core::graph::Owner::Zone(*uid)),
        _ => None,
    }
}

impl Shell {
    pub fn open(project: Option<PathBuf>, start: rustpg2d_engine::runtime::StartOptions) -> Shell {
        // (Started without a project: the home screen.)
        let ws = match project {
            None => Workspace::empty(),
            Some(project) => match Workspace::open_path(project.clone(), Gpu::Yes) {
                Ok(ws) => ws,
                Err(e) => {
                    let mut ws = Workspace::empty();
                    ws.set_status(format!("{}: {e:#}", project.display()));
                    ws
                }
            },
        };
        let mut shell = Shell::around(ws, start);
        // (The window's Help tab opens where it was left.)
        shell.help = crate::helpview::HelpView::restored();
        shell
    }

    /// The window's parts around a workspace (a project just opened, or none).
    fn around(mut ws: Workspace, start: rustpg2d_engine::runtime::StartOptions) -> Shell {
        ws.refresh_refs();
        ws.history_frame(None);
        // (The agent server's port and whether it starts with the editor are the editor's settings.)
        let settings = editor_core::files::load_settings();
        (ws.mcp.port, ws.mcp.autostart) = (settings.mcp_port, settings.mcp_autostart);
        // Documents in the middle, the browser left, problems and history below.
        let mut dock = DockState::new(vec![Tab::Overview]);
        let surface = dock.main_surface_mut();
        let [center, _left] = surface.split_left(NodeIndex::root(), 0.2, vec![Tab::Browser]);
        surface.split_below(center, 0.72, vec![Tab::Problems, Tab::History]);
        Shell { ws, dock, play: None, play_error: None, start, palette: Palette::default(), lists: Vec::new(), cats: Default::default(), browser_filter: String::new(), forms: Default::default(), show_all: false, thumbs: Default::default(), new_asset: None, share: None, cleanup: None, lib_look: None, marks: Marks::default(), maps: Default::default(), world: Default::default(), graphs: Default::default(), link: None, clip: None, scripts: Default::default(), names: Default::default(), publish: Default::default(), library: Default::default(), art: Default::default(), help: Default::default(), tour: Default::default(), agents: Default::default(), langs: Default::default(), plugins: Default::default(), play_lang: None, news: false, timelines: Default::default(), nodes: Default::default(), stages: Default::default(), figures: Default::default(), model_stages: Default::default(), item_stages: Default::default(), anim_stages: Default::default(), pixels: Default::default(), sounds: Default::default(), sheets: Default::default(), scrub: None, try_next: None, home: Home::default(), log: Vec::new(), leaving: None, quit_now: false, cleanup_passed: false, setup: None }
    }

    /// The window around another workspace (another project, or none). What belongs to the
    /// editor and not to a project stays: its answers, the agent server (agents that are
    /// connected go on in the new project), the Help tab, the news.
    fn around_another(&mut self, mut ws: Workspace) {
        ws.mcp = std::mem::take(&mut self.ws.mcp);
        let (log, help, agents, news) = (std::mem::take(&mut self.log), std::mem::take(&mut self.help), std::mem::take(&mut self.agents), self.news);
        let (port, autostart) = (ws.mcp.port, ws.mcp.autostart);
        *self = Shell::around(ws, rustpg2d_engine::runtime::StartOptions::default());
        // (A server that listens keeps its port, whatever the settings say meanwhile.)
        (self.ws.mcp.port, self.ws.mcp.autostart) = (port, autostart);
        (self.log, self.help, self.agents, self.news) = (log, help, agents, news);
    }

    /// Whether anything waits to be saved: definitions, maps, scripts.
    pub fn unsaved(&self) -> bool {
        self.ws.is_dirty() || self.scripts.values().any(|t| t.dirty()) || self.pixels.values().any(|p| p.dirty()) || self.langs.dirty()
    }

    /// Opens another project in this window (a project file, or a folder). Refused while
    /// something isn't saved; the project that was open has its file written first.
    pub fn switch_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        if self.unsaved() {
            return Err("there are unsaved changes: save first (Ctrl+S)".into());
        }
        self.open_now(path)
    }

    /// Opens another project, whatever wasn't saved in this one (it is dropped).
    fn open_now(&mut self, path: &std::path::Path) -> Result<(), String> {
        let ws = Workspace::open_path(path.to_path_buf(), Gpu::Yes).map_err(|e| format!("{}: {e:#}", path.display()))?;
        self.ws.finish_project_file();
        self.around_another(ws);
        self.ws.set_status(format!("Opened {}", path.display()));
        Ok(())
    }

    /// Makes a new project at `path` (a project file; a folder where folders are allowed) and
    /// opens it. `name`: what the game is called ("": like its file). `basics`: it starts with
    /// what a game needs first from the asset library (ground, a hero, a few props and items);
    /// else empty.
    pub fn new_project(&mut self, path: &std::path::Path, name: &str, basics: bool) -> Result<(), String> {
        if self.unsaved() {
            return Err("there are unsaved changes: save first (Ctrl+S)".into());
        }
        self.new_now(path, name, basics)
    }

    /// `new_project`, whatever wasn't saved in the project open now (it is dropped).
    fn new_now(&mut self, path: &std::path::Path, name: &str, basics: bool) -> Result<(), String> {
        let made = if editor_core::project_file::folders_allowed() {
            editor_core::files::create_project(path).map(|()| path.to_path_buf())
        } else {
            editor_core::files::create_project_file(path)
        };
        let made = made.map_err(|e| format!("{e:#}"))?;
        // (Filled before it is opened: what comes from the library is no step to undo.)
        let mut said = String::new();
        if basics || !name.trim().is_empty() {
            let mut new = Workspace::open_path(made.clone(), Gpu::No).map_err(|e| format!("{}: {e:#}", made.display()))?;
            if basics {
                said = new.start_with_basics(name)?;
            } else {
                new.project.config.name = name.trim().to_string();
                new.defs_dirty = true;
                new.save_all();
            }
            new.close_project();
        }
        self.open_now(&made)?;
        if !said.is_empty() {
            self.ws.set_status(format!("Made {}: {said}", self.ws.project.config.name));
        }
        Ok(())
    }

    /// Back to no project (the home screen).
    pub fn close_project(&mut self) -> Result<(), String> {
        if self.unsaved() {
            return Err("there are unsaved changes: save first (Ctrl+S)".into());
        }
        self.close_now();
        Ok(())
    }

    /// Back to no project, whatever wasn't saved (it is dropped: nothing is written but the
    /// project file of what was saved before).
    fn close_now(&mut self) {
        self.ws.close_project();
        self.around_another(Workspace::empty());
    }

    /// The creator asked to leave the project (to quit, close it, open or make another): done at
    /// once when everything is saved, else *There are changes that aren't saved* asks first.
    pub fn leave(&mut self, what: Leaving) {
        if self.unsaved() {
            self.leaving = Some(what);
        } else {
            self.leave_now(what);
        }
    }

    /// The answer to *There are changes that aren't saved*: `Some(true)` = save, then go on;
    /// `Some(false)` = go on, what isn't saved is dropped; `None` = stay.
    pub fn leave_answer(&mut self, save: Option<bool>) -> Result<(), String> {
        let Some(what) = self.leaving.take() else { return Err("nothing is asked".into()) };
        match save {
            None => {
                self.ws.set_status("Stayed: nothing was saved, nothing was dropped");
                return Ok(());
            }
            Some(true) => {
                let _ = self.run("file.save_all");
                // (Something that couldn't be written: the creator stays and reads why.)
                if self.unsaved() {
                    return Err(format!("not everything could be saved, so this didn't {}: {}", what.doing(), self.ws.status.0));
                }
            }
            Some(false) => {}
        }
        self.leave_now(what);
        Ok(())
    }

    /// Does what leaves the project: the clean-up of unused files is offered first (once a
    /// leaving) when the project has some it didn't keep.
    fn leave_now(&mut self, what: Leaving) {
        if !std::mem::take(&mut self.cleanup_passed) {
            if let Some(c) = self.ws.cleanup_offer() {
                self.cleanup = Some(crate::cleanupwin::CleanupWindow::leaving(c, what));
                return;
            }
        }
        let failed = match what {
            Leaving::Quit => {
                self.quit_now = true;
                Ok(())
            }
            Leaving::Close => {
                self.close_now();
                Ok(())
            }
            Leaving::Open(path) => self.open_now(&path).map_err(|e| format!("Not opened: {e}")),
            Leaving::New(path, name, basics) => self.new_now(&path, &name, basics).map_err(|e| format!("No new project: {e}")),
        };
        if let Err(e) = failed {
            self.ws.set_status(e);
        }
    }

    /// The clean-up window was answered or closed: what was deleted is gone from the lists,
    /// and leaving goes on (unless deleting failed).
    fn cleanup_closed(&mut self, w: crate::cleanupwin::CleanupWindow) {
        self.lists.clear();
        self.marks.unused = None;
        if let Some(then) = w.then.filter(|_| !w.failed) {
            self.cleanup_passed = true;
            self.leave_now(then);
        }
    }

    /// *Clean up unused files…*, from the menu or offered on leaving.
    fn cleanup_ui(&mut self, ctx: &egui::Context) {
        if let Some(mut w) = self.cleanup.take() {
            if w.show(ctx, &mut self.ws) {
                self.cleanup = Some(w);
            } else {
                // (What was deleted is gone from the lists and the pictures; leaving goes on.)
                self.cleanup_closed(w);
            }
        }
    }

    /// Whether something is being asked before the project is left (unsaved changes, the clean-up).
    pub fn leaving_asks(&self) -> bool {
        self.leaving.is_some() || self.cleanup.as_ref().is_some_and(|w| w.then.is_some())
    }

    /// `leave`: what is asked (`-|quit|close|open|new`); `leave save|discard|stay` answers.
    pub fn leave_word(&mut self, args: &[String]) -> String {
        let answer = match args.first().map(String::as_str) {
            None => return self.leaving.as_ref().map_or("-", Leaving::word).to_string(),
            Some("save") => Some(true),
            Some("discard") => Some(false),
            Some("stay") => None,
            Some(other) => return format!("leave {other}: save, discard, stay"),
        };
        match self.leave_answer(answer) {
            Ok(()) => format!("{} said={}", self.leaving.as_ref().map_or("-", Leaving::word), self.ws.status.0),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// *There are changes that aren't saved*, while leaving waits for its answer.
    fn leaving_ui(&mut self, ctx: &egui::Context) {
        let Some(what) = &self.leaving else { return };
        let doing = what.doing();
        let mut answer = None;
        // (Nothing else is clicked meanwhile; Escape or a click beside it is *Stay*.)
        let modal = egui::Modal::new(egui::Id::unique("unsaved changes")).show(ctx, |ui| {
            ui.heading("Unsaved changes");
            ui.label("There are changes that aren't saved.");
            ui.label(format!("Save them before you {doing}?"));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Save and go on").on_hover_text(format!("Saves everything, then does what you asked: {doing}")).clicked() {
                    answer = Some(Some(true));
                }
                if ui.button("Don't save").on_hover_text(format!("What isn't saved is lost: {doing}")).clicked() {
                    answer = Some(Some(false));
                }
                if ui.button("Stay").on_hover_text("Back to work: nothing is saved, nothing is lost").clicked() {
                    answer = Some(None);
                }
            });
        });
        if answer.is_none() && modal.should_close() {
            answer = Some(None);
        }
        if let Some(save) = answer {
            if let Err(e) = self.leave_answer(save) {
                self.ws.set_status(e);
            }
        }
    }

    /// Shows `tab`: brings it to the front when it is open, else opens it beside the documents.
    pub fn show_tab(&mut self, tab: Tab) {
        if let Tab::Map(m) = &tab {
            self.ws.open_map(m);
        }
        // (A structure is worked on as a map of its own: that is then the map in front.)
        if let Tab::Def(RefKind::Structure, id) = &tab {
            if let Ok(name) = self.ws.open_structure(id) {
                self.ws.open_map(&name);
            }
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            return;
        }
        // Next to another document when there is one, else in the first leaf.
        let doc = self.dock.find_tab_from(|t| matches!(t, Tab::Overview | Tab::World | Tab::Graph(_) | Tab::Settings(_) | Tab::Kind(_) | Tab::Def(..) | Tab::Map(_) | Tab::Script(_) | Tab::Names | Tab::Publish | Tab::Library | Tab::Art | Tab::Help | Tab::Agents | Tab::Play));
        match doc {
            Some(path) => {
                self.dock.set_focused_node_and_surface(path.node_path());
                self.dock.push_to_focused_leaf(tab);
            }
            None => self.dock.push_to_first_leaf(tab),
        }
    }

    /// Shows `tab` above `beside` (both in view: a map over the timeline that scrubs it). A
    /// tab that is in view elsewhere already is only brought forward.
    pub fn show_above(&mut self, tab: Tab, beside: &Tab) {
        if let Tab::Map(m) = &tab {
            self.ws.open_map(m);
            // A cut-scene's map looks at where its moment is: the camera's frame, else its
            // first actor.
            if let (Tab::Def(RefKind::Cutscene, id), ts) = (beside, self.ws.ts()) {
                let time = self.timelines.get(id).map_or(0.0, |t| t.time);
                if let Some(p) = self.ws.cutscene_preview(id, time) {
                    if let Some(at) = p.camera.map(|c| c.0).or_else(|| p.actors.first().map(|a| a.1)) {
                        let view = self.maps.entry(m.clone()).or_default();
                        view.look_at(gfx::math::vec2((at.x + 0.5) * ts, (at.y + 0.5) * ts), 0.8);
                    }
                }
            }
        }
        let (there, anchor) = (self.dock.find_tab(&tab), self.dock.find_tab(beside));
        match (there, anchor) {
            (Some(t), Some(a)) if (t.surface, t.node) != (a.surface, a.node) => {
                let _ = self.dock.set_active_tab(t);
            }
            (there, Some(a)) if a.surface.is_main() => {
                if let Some(t) = there {
                    self.dock.remove_tab(t);
                }
                // (Found again: taking a tab out may have moved the leaf.)
                if let Some(a) = self.dock.find_tab(beside) {
                    self.dock.main_surface_mut().split_above(a.node, 0.55, vec![tab]);
                }
            }
            _ => self.show_tab(tab),
        }
    }

    /// Writes the scripts changed in their tabs and the pictures painted in their sheets' (with
    /// everything else that is saved; the game reads the files). Returns how many files.
    pub fn save_scripts(&mut self) -> usize {
        let mut saved = 0;
        for tab in self.scripts.values_mut().filter(|t| t.dirty()) {
            match tab.save() {
                Ok(()) => saved += 1,
                Err(e) => self.ws.set_status(e),
            }
        }
        if saved > 0 {
            self.ws.refresh_refs();
            self.ws.stale = true;
            self.ws.problems.stale = true;
        }
        // A language's table that was typed into is a file too.
        if self.langs.save(&mut self.ws) {
            saved += 1;
        }
        // Pictures painted in their sheets' tabs are files of their own too.
        let painted: Vec<String> = self.pixels.iter().filter(|(_, p)| p.dirty()).map(|(id, _)| id.clone()).collect();
        for key in painted {
            // (A material's editor is kept under a key of its own: its id is what follows.)
            let id = key.strip_prefix(MATERIAL_KEY).unwrap_or(&key).to_string();
            match self.pixels.get_mut(&key).map(|p| p.save(&mut self.ws, &id)) {
                Some(Err(e)) => self.ws.set_status(e),
                Some(Ok(())) => saved += 1,
                None => {}
            }
        }
        saved
    }

    /// The timeline of the cut-scene tab in front: `time SECS`, `cue TRACK CUE` (picks one),
    /// `add TRACK ACTION…` (a cue at the playhead, the action by its name), `play`. Returns
    /// its `status` line ("" when no cut-scene's tab is in front).
    /// `graph …` with an events graph's tab in front (`GraphView::word`); prints its lines.
    pub fn graph_word(&mut self, args: &[String]) -> Vec<String> {
        let Some(Tab::Graph(map)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return vec!["failed: no events graph is in front (map.graph)".into()] };
        // (`graph focus entity 12`: selected on the map, then shown alone.)
        let arg = |k: usize| args.get(k).map(String::as_str).unwrap_or("");
        if arg(0) == "focus" {
            let sel = match (arg(1), arg(2).parse::<u32>()) {
                ("entity", Ok(uid)) => Some(editor_core::workspace::Selection::Entity(uid)),
                ("zone", Ok(uid)) => Some(editor_core::workspace::Selection::Zone(uid)),
                _ => None,
            };
            if let Some(sel) = sel {
                self.maps.entry(map.clone()).or_default().select(sel);
            }
        }
        let selected = selected_owner(&self.maps, &map);
        let (lines, go) = self.graphs.entry(map.clone()).or_default().word(&self.ws, &map, selected, args);
        if let Some(t) = go {
            self.go_to(&t);
        }
        lines
    }

    pub fn timeline_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Cutscene, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let Some(pos) = self.ws.project.db.cutscenes.iter().position(|d| d.id == id) else { return String::new() };
        let tl = self.timelines.entry(id.clone()).or_default();
        let arg = |k: usize| args.get(k).map(String::as_str).unwrap_or("");
        match arg(0) {
            "time" => tl.time = arg(1).parse().unwrap_or(0.0),
            "cue" => tl.sel = Some((arg(1).parse().unwrap_or(0), arg(2).parse().unwrap_or(0))),
            "add" => {
                let mut def = self.ws.project.db.cutscenes[pos].clone();
                if tl.add_cue(&mut def, arg(1).parse().unwrap_or(0), &args[2.min(args.len())..].join(" ")) {
                    self.ws.begin(&format!("Change the cut-scene {id}"));
                    self.ws.project.db.cutscenes[pos] = def;
                } else {
                    let offers: Vec<&str> = def.tracks.get(arg(1).parse().unwrap_or(0)).map_or(Vec::new(), |t| editor_core::cutscene::offered(t.kind, &t.target).iter().map(|a| a.label()).collect());
                    return format!("{id}: that track offers no such action (it offers: {})", offers.join(", "));
                }
            }
            "map" => {
                if let Some(map) = self.ws.cutscene_map(&id) {
                    self.show_above(Tab::Map(map), &Tab::Def(RefKind::Cutscene, id.clone()));
                    // (The cut-scene's tab keeps the keyboard and the script's words.)
                    if let Some(path) = self.dock.find_tab(&Tab::Def(RefKind::Cutscene, id.clone())) {
                        let _ = self.dock.set_active_tab(path);
                        self.dock.set_focused_node_and_surface(path.node_path());
                    }
                }
                return format!("{id}: its map above");
            }
            "play" => {
                let at = tl.time;
                self.play_from(PlayFrom::Cutscene(id.clone(), at));
                return format!("{id} playing from {at:.1} s");
            }
            _ => {}
        }
        let tl = &self.timelines[&id];
        let preview = self.ws.cutscene_preview(&id, tl.time);
        let actors: Vec<String> = preview.as_ref().map_or(Vec::new(), |p| p.actors.iter().map(|(n, v, w)| format!("{n}@{:.1},{:.1}{}", v.x, v.y, if *w { "~" } else { "" })).collect());
        let cam = preview.and_then(|p| p.camera).map_or("-".into(), |(c, z)| format!("{:.1},{:.1}x{z:.1}", c.x, c.y));
        format!("{id}{} [{}] cam={cam}", tl.status(&self.ws.project.db.cutscenes[pos]), actors.join(" "))
    }

    /// The canvas of the visual script's tab in front: the words of `NodesView::word`; alone,
    /// its `status` line ("" when no visual script's tab is in front).
    pub fn nodes_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::VisualScript, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let view = self.nodes.entry(id.clone()).or_default();
        if args.is_empty() {
            view.status(&self.ws, &id)
        } else {
            view.word(&mut self.ws, &id, args)
        }
    }

    /// The workshop of the prop's tab in front: `state NAME`, `turn`, `dark on|off`; returns
    /// its `status` line ("" when no prop's tab is in front).
    pub fn prop_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Prop, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.stages.entry(id.clone()).or_default();
        stage.word(&mut self.ws, &id, args);
        stage.status(&self.ws, &id)
    }

    /// The workshop of the item's tab in front: the words of `ItemStage::word`; returns its
    /// `status` line ("" when no item's tab is in front).
    pub fn item_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Item, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.item_stages.entry(id.clone()).or_default();
        match stage.word(&mut self.ws, &id, args) {
            Ok(()) => stage.status(&self.ws, &id),
            Err(e) => format!("{id}: {e}"),
        }
    }

    /// `testsetup …` (`setupwin::word`; `open` / `close` the window): the setup's summary.
    pub fn testsetup_word(&mut self, args: &[String]) -> String {
        match args.first().map(String::as_str) {
            Some("open") => self.setup = Some(crate::setupwin::SetupWindow::open(&self.ws)),
            Some("close") => self.setup = None,
            _ => {
                if let Err(e) = crate::setupwin::word(&mut self.ws, args) {
                    return format!("failed: {e}");
                }
            }
        }
        // (An open window shows what the word changed.)
        if let Some(w) = &mut self.setup {
            w.setup = self.ws.test_setup();
        }
        let s = self.ws.test_setup();
        format!("{}{}", if s == Default::default() { "none".to_string() } else { s.summary() }, if self.setup.is_some() { " window=open" } else { "" })
    }

    /// The animation's tab in front: the words of `AnimStage::word`; returns its `status` line
    /// ("" when no animation's tab is in front).
    pub fn anim_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Animation, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.anim_stages.entry(id.clone()).or_default();
        match stage.word(&self.ws, &id, args) {
            Ok(()) => stage.status(&self.ws, &id),
            Err(e) => format!("{id}: {e}"),
        }
    }

    /// The workshop of the character's tab in front: the words of `CharStage::word`; returns
    /// its `status` line ("" when no character's tab is in front).
    pub fn char_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Character, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.figures.entry(id.clone()).or_default();
        match stage.word(&mut self.ws, &id, args) {
            Ok(()) => stage.status(&self.ws, &id),
            Err(e) => format!("{id}: {e}"),
        }
    }

    /// The Generate art page: the words of `ArtPage::word`; returns its `status` lines.
    pub fn art_word(&mut self, args: &[String]) -> String {
        match self.art.word(&mut self.ws, args) {
            Ok(()) => self.art.status(&self.ws),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// Where the manual says something about the tab in front: a chapter's file, and words one
    /// of which the heading of the section has.
    pub fn help_topic(&mut self) -> (&'static str, Vec<String>) {
        let words = |w: &[&str]| w.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let of_kind = |kind: RefKind| -> (&'static str, Vec<String>) {
            let label = plural(kind.label()).to_lowercase();
            match kind {
                RefKind::Character => ("06-characters.md", Vec::new()),
                RefKind::Race | RefKind::Class | RefKind::Background | RefKind::Spell | RefKind::Disease => ("19-srd-rules.md", vec![label]),
                RefKind::Cutscene => ("07-events.md", words(&["cut-scene", "cutscene"])),
                RefKind::CommonEvent => ("07-events.md", words(&["common event"])),
                RefKind::VisualScript => ("20-visual-scripts.md", Vec::new()),
                RefKind::Structure => ("03-maps.md", words(&["structures"])),
                RefKind::Model => ("10-art-and-sound.md", words(&["3d model", "model"])),
                RefKind::Sheet | RefKind::Sound | RefKind::Animation => ("10-art-and-sound.md", vec![label]),
                RefKind::Map => ("03-maps.md", Vec::new()),
                _ => ("05-database.md", vec![label, kind.label().to_lowercase()]),
            }
        };
        match self.dock.find_active_focused().map(|(_, t)| t.clone()) {
            Some(Tab::Map(m)) => match self.maps.get(&m).map(|v| v.tool) {
                Some(crate::mapview::Tool::Terrain) => ("03-maps.md", words(&["ground tool", "painted ground"])),
                Some(crate::mapview::Tool::Build) => ("03-maps.md", words(&["building walls", "walls"])),
                Some(crate::mapview::Tool::Collision) => ("03-maps.md", words(&["collision"])),
                Some(crate::mapview::Tool::Zones) => ("04-objects-and-zones.md", words(&["zones"])),
                Some(crate::mapview::Tool::Place) => ("04-objects-and-zones.md", words(&["placing"])),
                _ => ("03-maps.md", Vec::new()),
            },
            Some(Tab::Def(kind, _)) | Some(Tab::Kind(kind)) => of_kind(kind),
            Some(Tab::World) => ("04-objects-and-zones.md", words(&["transitions"])),
            Some(Tab::Graph(_)) => ("07-events.md", words(&["graph"])),
            Some(Tab::Script(_)) => ("08-scripting.md", Vec::new()),
            Some(Tab::Names) => ("07-events.md", words(&["variables", "flags"])),
            Some(Tab::Publish) => ("14-publishing.md", Vec::new()),
            Some(Tab::Library) => ("11-asset-library.md", Vec::new()),
            Some(Tab::Art) => ("10-art-and-sound.md", words(&["generate art", "meshy"])),
            Some(Tab::Play) => ("02-the-editor.md", words(&["play-test", "play"])),
            Some(Tab::Problems) => ("16-troubleshooting.md", Vec::new()),
            Some(Tab::History) => ("02-the-editor.md", words(&["undo", "saving"])),
            Some(Tab::Settings(page)) => ("05-database.md", vec![page.to_lowercase(), "project".into()]),
            Some(Tab::Help) => (manual_file(self.help.chapter), Vec::new()),
            Some(Tab::Agents) => ("13-ai-agents.md", Vec::new()),
            _ => ("01-getting-started.md", Vec::new()),
        }
    }

    /// A field of a form was asked about: the Help tab with what it is for and where the
    /// manual names it.
    pub fn open_field_help(&mut self, label: &str, help: &str) {
        let (file, _) = self.help_topic();
        self.help.open_field(label, help, file);
        self.show_tab(Tab::Help);
    }

    /// F1: the Help tab at the manual's part for what is in front.
    pub fn open_help(&mut self) {
        let (file, words) = self.help_topic();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let (chapter, anchor) = editor_core::manual::topic(file, &words);
        self.help.open(chapter, &anchor);
        self.show_tab(Tab::Help);
    }

    /// The Help tab: the words of `HelpView::word`, and `topic` (as F1); returns its `status` line.
    pub fn help_word(&mut self, args: &[String]) -> String {
        if args.first().map(String::as_str) == Some("topic") {
            self.open_help();
            return self.help.status();
        }
        // (*What's new* as its button opens it: in front, and the news count as seen.)
        if args.first().map(String::as_str) == Some("news") && args.len() == 1 {
            self.open_news();
            return self.help.status();
        }
        match self.help.word(args) {
            Ok(()) => self.help.status(),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// *What's new*: the Help tab at the changelog's part for this version; the news count as seen.
    pub fn open_news(&mut self) {
        self.help.open_news(0);
        self.show_tab(Tab::Help);
        self.news_seen();
    }

    /// The news of this version were looked at, or put away: not offered again.
    pub fn news_seen(&mut self) {
        self.news = false;
        editor_core::files::save_settings(&editor_core::files::EditorSettings { seen_version: editor_core::manual::VERSION.to_string(), ..editor_core::files::load_settings() });
    }

    /// Looked at when the editor starts: whether it is newer than the one that ran here last
    /// (then the Overview offers *What's new*). An editor that starts for the first time has
    /// no news; the agent server starts when the settings say so.
    pub fn started(&mut self) {
        let settings = editor_core::files::load_settings();
        if !editor_core::files::settings_exist() {
            editor_core::files::save_settings(&editor_core::files::EditorSettings { seen_version: editor_core::manual::VERSION.to_string(), ..settings });
        } else {
            self.news = settings.seen_version != editor_core::manual::VERSION;
        }
        self.ws.mcp_autostart();
    }

    /// Play-tests start in `code` from now on (`None`: as the player's settings say); `now`:
    /// one starts, from the title screen.
    pub fn play_in(&mut self, code: Option<String>, now: bool) {
        self.ws.set_status(match &code {
            Some(c) => format!("Play-tests start in {c}"),
            None => "Play-tests start in the language the player's settings say".to_string(),
        });
        self.play_lang = code;
        if now {
            self.play_from(PlayFrom::Title);
        }
    }

    /// Translations on the Languages page: the words of `LangPage::word`; returns its `status` line.
    pub fn lang_word(&mut self, args: &[String]) -> String {
        match self.langs.word(&mut self.ws, args) {
            Ok(want) => {
                if let Some(crate::langpage::Want::PlayIn(code, now)) = want {
                    self.play_in(code, now);
                }
                format!("{} play={}", self.langs.status(&self.ws), self.play_lang.as_deref().unwrap_or("-"))
            }
            Err(e) => format!("failed: {e}"),
        }
    }

    /// *Clean up unused files…*: `open`, `pick KEY on|off`, `delete`, `keep`, `cancel`; returns
    /// `closed` or `TICKED/ALL` with a line an entry.
    pub fn cleanup_word(&mut self, args: &[String]) -> String {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "open" => {
                if let Err(e) = self.run("assets.cleanup") {
                    return format!("failed: {e}");
                }
            }
            "pick" => {
                if let Some(w) = self.cleanup.as_mut().filter(|w| w.cleanup.picks.contains_key(arg(1))) {
                    w.cleanup.picks.insert(arg(1).to_string(), arg(2) == "on");
                } else {
                    return format!("failed: nothing unused is called {}", arg(1));
                }
            }
            "delete" | "keep" => {
                let Some(mut w) = self.cleanup.take() else { return "failed: the clean-up isn't open".into() };
                match self.ws.cleanup_apply(&w.cleanup, arg(0) == "delete") {
                    Ok(said) => self.ws.set_status(said),
                    Err(e) => {
                        w.failed = true;
                        self.ws.set_status(format!("Cleaning up failed: {e}"));
                    }
                }
                let failed = w.failed.then(|| self.ws.status.0.clone());
                self.cleanup_closed(w);
                if let Some(e) = failed {
                    return format!("failed: {e}");
                }
            }
            // (The window's close button: on leaving, that goes on too.)
            "cancel" => {
                if let Some(w) = self.cleanup.take() {
                    self.cleanup_closed(w);
                }
            }
            _ => {}
        }
        match &self.cleanup {
            Some(w) => std::iter::once(w.status()).chain(w.listing()).collect::<Vec<_>>().join("\n[cleanup] "),
            None => "closed".into(),
        }
    }

    /// Plugins on the Scripts & plugins page: the words of `PluginsPage::word`; returns its `status` line.
    pub fn plugins_word(&mut self, args: &[String]) -> String {
        match self.plugins.word(&mut self.ws, args) {
            Ok(()) => self.plugins.status(&self.ws),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// The AI agents page: the words of `AgentsPage::word`; returns its `status` line.
    pub fn agents_word(&mut self, ctx: &egui::Context, args: &[String]) -> String {
        match self.agents.word(ctx, &mut self.ws, args) {
            Ok(()) => self.agents.status(&self.ws),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// The tour that is followed: the words of `TourPanel::word`; returns its `status` line.
    pub fn tour_word(&mut self, ctx: &egui::Context, args: &[String]) -> String {
        match self.tour.word(&self.ws, args) {
            Ok(()) => self.tour.status(ctx, &self.ws),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// A file dropped on the window: a pack is looked at on the Library tab; a 3D model, a
    /// picture or a sound opens *New…* with it.
    pub fn dropped(&mut self, file: &std::path::Path) {
        if crate::packspage::PacksPage::takes(file) {
            self.library.on_packs = true;
            self.library.packs.open_file(&mut self.ws, file);
            self.show_tab(Tab::Library);
            return;
        }
        match crate::gallery::NewAsset::of_file(file) {
            Some(new) => self.new_asset = Some(new),
            None => self.ws.set_status(format!("{} is no 3D model (.glb), picture (.png), sound (.ogg, .wav), font (.ttf, .otf) or pack (.r2dpack)", file.display())),
        }
    }

    /// The packs of the Library tab: the words of `PacksPage::word`; returns its `status` line.
    pub fn packs_word(&mut self, args: &[String]) -> String {
        self.library.on_packs = true;
        match self.library.packs.word(&mut self.ws, args) {
            Ok(()) => self.library.packs.status(&mut self.ws),
            Err(e) => format!("failed: {e}"),
        }
    }

    /// The pixel editor of the sprite sheet's or ground material's tab in front: the words of
    /// `PixelEdit::word`; returns its `status` line ("" when neither's tab is in front).
    pub fn pixels_word(&mut self, args: &[String]) -> String {
        let (id, material) = match self.dock.find_active_focused().map(|(_, t)| t.clone()) {
            Some(Tab::Def(RefKind::Sheet, id)) => (id, false),
            Some(Tab::Def(RefKind::Material, id)) => (id, true),
            _ => return String::new(),
        };
        let edit = if material { self.pixels.entry(material_key(&id)).or_insert_with(crate::pixeledit::PixelEdit::of_material) } else { self.pixels.entry(id.clone()).or_default() };
        // (The frame a sheet is painted in is the one picked in its picture.)
        if let (false, Some("frame"), Some(k)) = (material, args.first().map(String::as_str), args.get(1).and_then(|k| k.parse().ok())) {
            self.sheets.entry(id.clone()).or_default().frame = Some(k);
        }
        match edit.word(&mut self.ws, &id, args) {
            Ok(()) => edit.status(&id),
            Err(e) => format!("{id}: {e}"),
        }
    }

    /// The workshop of the 3D model's tab in front: the words of `ModelStage::word`; returns
    /// its `status` line ("" when no model's tab is in front).
    pub fn model_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Model, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.model_stages.entry(id.clone()).or_default();
        match stage.word(&mut self.ws, &id, args) {
            Ok(()) => stage.status(&self.ws, &id),
            Err(e) => format!("{id}: {e}"),
        }
    }

    /// The sound's tab in front: `play`, `loop`, `stop`; returns its `status` line ("" when no
    /// sound's tab is in front).
    pub fn sound_word(&mut self, args: &[String], now: f64) -> String {
        let Some(Tab::Def(RefKind::Sound, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.sounds.entry(id.clone()).or_default();
        match args.first().map(String::as_str) {
            Some("play") => stage.play(&self.ws, &id, false, now),
            Some("loop") => stage.play(&self.ws, &id, true, now),
            Some("stop") => stage.stop(),
            _ => {}
        }
        stage.status(&self.ws, &id)
    }

    /// The sprite sheet's tab in front: `cut ACROSS DOWN`, `frame K`; returns its `status`
    /// line ("" when no sheet's tab is in front).
    pub fn sheet_word(&mut self, args: &[String]) -> String {
        let Some(Tab::Def(RefKind::Sheet, id)) = self.dock.find_active_focused().map(|(_, t)| t.clone()) else { return String::new() };
        let stage = self.sheets.entry(id.clone()).or_default();
        let num = |k: usize| args.get(k).and_then(|a| a.parse::<u32>().ok());
        match args.first().map(String::as_str) {
            Some("cut") => {
                stage.cut(&mut self.ws, &id, num(1).unwrap_or(1), num(2).unwrap_or(1));
            }
            Some("frame") => stage.frame = num(1),
            _ => {}
        }
        stage.status(&self.ws, &id)
    }

    /// `script=…` of the script tab in front ("" when none is).
    pub fn script_status(&mut self) -> String {
        match self.dock.find_active_focused().map(|(_, t)| t.clone()) {
            Some(Tab::Script(f)) => self.scripts.get(&f).map_or_else(String::new, |t| t.status()),
            _ => String::new(),
        }
    }

    /// The tab by its script name (`overview`, `play`, `def:item:potion`, `map:village`).
    pub fn tab_named(&self, name: &str) -> Option<Tab> {
        let mut parts = name.splitn(3, ':');
        Some(match (parts.next()?, parts.next(), parts.next()) {
            ("project", None, _) => Tab::Browser,
            ("overview", None, _) => Tab::Overview,
            ("world", None, _) => Tab::World,
            ("names", None, _) => Tab::Names,
            ("publish", None, _) => Tab::Publish,
            ("library", None, _) => Tab::Library,
            ("art", None, _) => Tab::Art,
            ("help", None, _) => Tab::Help,
            ("agents", None, _) => Tab::Agents,
            ("play", None, _) => Tab::Play,
            ("problems", None, _) => Tab::Problems,
            ("history", None, _) => Tab::History,
            ("map", Some(m), None) if self.ws.maps.iter().any(|x| x == m) => Tab::Map(m.to_string()),
            ("graph", Some(m), None) if self.ws.maps.iter().any(|x| x == m) => Tab::Graph(m.to_string()),
            ("script", Some(f), None) if self.ws.script_names().iter().any(|(n, _)| n == f) => Tab::Script(f.to_string()),
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
            Target::Script(path, at) => {
                let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.scripts.entry(file.clone()).or_insert_with(|| crate::scriptedit::ScriptTab::open(path.clone())).reveal(*at);
                self.show_tab(Tab::Script(file));
            }
            Target::Project => self.show_tab(Tab::Overview),
        }
    }

    /// Runs a command line and keeps its answer.
    pub fn run(&mut self, line: &str) -> Result<String, String> {
        // (In the window the clean-up is a list to tick, not a line of text.)
        if line.trim() == "assets.cleanup" {
            self.save_scripts();
            return match crate::cleanupwin::CleanupWindow::open(&mut self.ws) {
                Ok(w) => {
                    let said = format!("{} unused", w.listing().len());
                    self.cleanup = Some(w);
                    Ok(said)
                }
                Err(e) => Err(e),
            };
        }
        // (In the window closing the project asks about what isn't saved, offers the clean-up
        // and puts the window back to the start, as *Close this project* does.)
        if line.trim() == "file.close_project" {
            if !self.ws.project_open {
                return Err("no project is open".into());
            }
            self.leave(Leaving::Close);
            return Ok(if self.leaving_asks() { "Asked first: there is something to answer".into() } else { "Closed the project".into() });
        }
        // (In the window the events graph is a tab.)
        if line.trim() == "map.graph" {
            let Some(map) = self.ws.doc.as_ref().map(|d| d.map.name.clone()).filter(|m| !m.starts_with('@')) else { return Err("no map is open".into()) };
            self.show_tab(Tab::Graph(map.clone()));
            return Ok(format!("The events graph of {map}"));
        }
        let scripts = if line.trim() == "file.save_all" { self.save_scripts() } else { 0 };
        let r = self.ws.run_line(line);
        // (Scripts and painted pictures are saved with everything; said when they were all there was to save.)
        let r = match r {
            Ok(m) if scripts > 0 && m.starts_with("Nothing") => Ok(format!("Saved {scripts} file{} (scripts, pictures)", if scripts == 1 { "" } else { "s" })),
            other => other,
        };
        self.log.push(match &r {
            Ok(m) => format!("{line}: {}", m.lines().next().unwrap_or_default()),
            Err(e) => format!("{line}: failed: {e}"),
        });
        // What a command opened or made gets its tab.
        if r.is_ok() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["map.open", ..] | ["map.new", ..] => {
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
        let what = editor_core::schema::of_kind(kind).and_then(|n| n.label_at(pointer)).map(|l| format!("{l} changed"));
        let step = self.ws.def_step_name(kind, id, what.as_deref());
        self.ws.set_def_value(kind, id, value)?;
        self.ws.begin(&step);
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
        let what = editor_core::schema::project().label_at(pointer).map_or_else(|| "changed".to_string(), |l| format!("{l} changed"));
        self.ws.begin(&format!("Game settings: {what}"));
        self.ws.set_config_value(value)?;
        self.ws.history_frame(None);
        Ok(format!("settings{pointer} = {text}"))
    }

    /// `tabs=… focus=… history=… problems=… palette=…` for the script's `status`.
    /// Starts the game in the Play tab (everything is saved first: the game reads the
    /// project's files).
    pub fn play_from(&mut self, from: PlayFrom) {
        if !self.ws.project_open {
            return;
        }
        self.tour.played();
        if matches!(&from, PlayFrom::Tile(map, ..) if editor_core::structdoc::is_scratch(map)) {
            self.ws.set_status("A structure is no map to play on: place it on a map and play from there");
            return;
        }
        self.save_scripts();
        if self.ws.is_dirty() {
            self.ws.save_all();
        }
        let ts = self.ws.ts();
        // (In the language picked on the Languages page, when one is.)
        let base = rustpg2d_engine::runtime::StartOptions { dev: true, lang: self.play_lang.clone(), ..Default::default() };
        self.start = match from {
            PlayFrom::Again => self.start.clone(),
            PlayFrom::Title => rustpg2d_engine::runtime::StartOptions { title: true, ..base },
            PlayFrom::Start => base,
            PlayFrom::Cutscene(id, at) => rustpg2d_engine::runtime::StartOptions { map: self.ws.cutscene_map(&id), cutscene: Some(id), cutscene_at: at, ..base },
            PlayFrom::Tile(map, (x, y), setup) => {
                let mut start = rustpg2d_engine::runtime::StartOptions { map: Some(map.clone()), pos: Some(gfx::math::vec2((x as f32 + 0.5) * ts, (y as f32 + 0.5) * ts)), ..base };
                if setup {
                    match self.ws.write_test_save(&map, (x, y)) {
                        Ok((file, problems)) => {
                            start.load = Some(file);
                            if !problems.is_empty() {
                                self.log.push(format!("The test setup has problems: {}", problems.join(", ")));
                            }
                        }
                        Err(e) => {
                            self.ws.set_status(format!("Could not write the test save: {e}"));
                            return;
                        }
                    }
                }
                start
            }
        };
        self.play = None;
        self.play_error = None;
        self.show_tab(Tab::Play);
        let where_ = match (&self.start.map, self.start.title) {
            (Some(m), _) if self.start.cutscene.is_some() => format!("the cut-scene {} on {m} from {:.1} s", self.start.cutscene.as_deref().unwrap_or_default(), self.start.cutscene_at),
            (Some(m), _) if self.start.load.is_some() => format!("on {m} with the test setup ({})", self.ws.test_setup().summary()),
            (Some(m), _) => format!("on {m}"),
            (None, true) => "from the title screen".into(),
            (None, false) => "on the start map".into(),
        };
        self.ws.set_status(format!("Playing {where_}"));
    }

    /// On the map being worked on, where its view looks (the tile in the middle).
    pub fn play_here(&mut self) {
        let ts = self.ws.ts();
        let Some((view, name)) = self.map_view() else {
            return self.play_from(PlayFrom::Start);
        };
        let c = view.center();
        self.play_from(PlayFrom::Tile(name, ((c.x / ts).floor() as i32, (c.y / ts).floor() as i32), false));
    }

    /// The game in its own window (the real runner beside this program), started as the Play
    /// tab's was.
    pub fn play_window(&mut self) {
        if !self.ws.project_open {
            return;
        }
        self.save_scripts();
        if self.ws.is_dirty() {
            self.ws.save_all();
        }
        let runner = rustpg2d_engine::release::Platform::host().game_runner();
        let Some(exe) = std::env::current_exe().ok().map(|e| e.with_file_name(runner)).filter(|e| e.is_file()) else {
            self.ws.set_status(format!("{runner} (the game) was not found next to the editor; from source run `cargo build -p rustpg2d-game`"));
            return;
        };
        // (A runner of another engine might not read what this editor wrote.)
        if let Err(e) = editor_core::export::runner_check(&exe) {
            self.ws.set_status(e);
            return;
        }
        let mut cmd = std::process::Command::new(exe);
        cmd.arg(self.ws.project.root.canonicalize().unwrap_or_else(|_| self.ws.project.root.clone())).arg("--dev");
        if !self.start.title {
            cmd.arg("--no-title");
        }
        if let Some(file) = &self.start.load {
            cmd.arg("--load").arg(file);
        }
        if let Some(m) = &self.start.map {
            cmd.args(["--map", m]);
            if let Some(s) = &self.start.spawn {
                cmd.args(["--spawn", s]);
            }
            if let Some(p) = self.start.pos {
                cmd.args(["--pos", &format!("{},{}", p.x, p.y)]);
            }
        }
        match cmd.spawn() {
            Ok(_) => self.ws.set_status("The game runs in its own window"),
            Err(e) => self.ws.set_status(format!("The game could not be started: {e}")),
        }
    }

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
        let mut parts = crate::mapview::Parts { ws: &mut self.ws, rs: None, lists: &self.lists, thumbs: &mut self.thumbs, show_all: self.show_all, link: &mut self.link, clip: &mut self.clip, scrub: None, asks: Vec::new() };
        view.act(&mut parts, &name, what);
        // (What the script's word asks of the window: an event to try.)
        for ask in parts.asks {
            if let crate::mapview::Ask::Try(tile, on, whose, what) = ask {
                use editor_core::workspace::Selection;
                use rustpg2d_engine::runtime::EventOwner;
                let owner = match whose {
                    Selection::Entity(uid) => EventOwner::Entity(uid),
                    Selection::Zone(uid) => EventOwner::Zone(uid),
                    _ => EventOwner::Map,
                };
                self.try_next = Some((on, owner, what));
                self.play_from(PlayFrom::Tile(name.clone(), tile, true));
            }
        }
    }

    /// The World view's line for `status`.
    pub fn world_status(&self) -> String {
        self.world.status()
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
            self.unsaved(),
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
        // A file dropped on the window: *New…* with it (a 3D model or a picture as a prop, a sound).
        if self.ws.project_open {
            let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
            if let Some(file) = dropped.first() {
                self.dropped(file);
            }
        }
        // A sound stops when its tab is closed.
        let closed: Vec<String> = self.sounds.keys().filter(|id| self.dock.find_tab(&Tab::Def(RefKind::Sound, (*id).clone())).is_none()).cloned().collect();
        for id in closed {
            if let Some(mut s) = self.sounds.remove(&id) {
                s.stop();
            }
        }
        // A structure is open as a map for as long as its tab is.
        let gone: Vec<String> = self.ws.struct_docs.keys().filter(|id| self.dock.find_tab(&Tab::Def(RefKind::Structure, (*id).clone())).is_none()).cloned().collect();
        for id in gone {
            self.ws.close_structure(&id);
            self.maps.remove(&editor_core::structdoc::doc_name(&id));
        }
        crate::pick::frame();
        // (A file picked for a form's field is copied into the project that is open.)
        crate::form::set_root(&ctx, self.ws.project_open.then(|| self.ws.project.root.clone()));
        self.ws.art_tick();
        // (Both look once a second or less: what another program changed in the project's
        // files, and whether the project's file is to be written again.)
        self.ws.watch_project();
        self.ws.project_file_tick();
        // (A drag that asked for it last frame counts as typing on in one field: one step.)
        let held = ctx.data_mut(|d| d.remove_temp::<u64>(held_step_id()));
        self.ws.history_frame(held.or(ctx.memory(|m| m.focused()).map(|id| id.value())));
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
        // Files another program changed while the editor has unsaved changes of its own there.
        if !self.ws.watch.changed.is_empty() {
            let mut answer = None;
            egui::Panel::top("changed on disk").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let what: Vec<String> = self.ws.watch.changed.iter().map(|f| match f.as_str() {
                        "defs/" => "the definitions".to_string(),
                        "project.ron" => "the game's settings".to_string(),
                        f => f.strip_prefix("maps/").map(|m| format!("the map {}", m.trim_end_matches(".map"))).unwrap_or_else(|| f.to_string()),
                    }).collect();
                    ui.colored_label(ui.visuals().warn_fg_color, format!("Another program changed {} on disk while you have changes there that aren't saved.", what.join(", ")));
                    if ui.button("Take the files' version").on_hover_text("Reads them again: your unsaved changes there are dropped").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Keep mine").on_hover_text("Stays as it is here: saving writes it over the files").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(reload) = answer {
                self.ws.watch_answer(reload);
            }
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            // The right-hand part first: the message has what is left of the line, cut short
            // where it would run into it (the whole message on hover).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{fps:.0} fps"));
                ui.separator();
                let (e, w) = self.ws.problems.counts();
                let counts = format!("{e} errors, {w} warnings");
                if self.ws.problems.built && self.ws.problems.stale {
                    ui.weak(counts).on_hover_text("What the last check found: the project changed since (Problems, Check again)");
                } else {
                    ui.label(counts);
                }
                ui.separator();
                let r = ui.label(if self.unsaved() { "unsaved changes" } else { "saved" });
                crate::tourpanel::mark(ui, "status:saved", r.rect);
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(&self.ws.status.0).truncate());
                });
            });
        });

        let mut asks = Vec::new();
        {
            let mut viewer = Viewer { ws: &mut self.ws, play: &mut self.play, play_error: &mut self.play_error, start: &self.start, lists: &self.lists, cats: &self.cats, filter: &mut self.browser_filter, forms: &mut self.forms, show_all: &mut self.show_all, thumbs: &mut self.thumbs, marks: &self.marks, maps: &mut self.maps, world: &mut self.world, graphs: &mut self.graphs, link: &mut self.link, clip: &mut self.clip, scripts: &mut self.scripts, names: &mut self.names, publish: &mut self.publish, library: &mut self.library, art: &mut self.art, help: &mut self.help, tour: &mut self.tour, agents: &mut self.agents, langs: &mut self.langs, plugins: &mut self.plugins, play_lang: &self.play_lang, news: &mut self.news, home: &mut self.home, timelines: &mut self.timelines, nodes: &mut self.nodes, stages: &mut self.stages, figures: &mut self.figures, model_stages: &mut self.model_stages, item_stages: &mut self.item_stages, anim_stages: &mut self.anim_stages, pixels: &mut self.pixels, sounds: &mut self.sounds, sheets: &mut self.sheets, scrub: &mut self.scrub, try_next: &mut self.try_next, rs, asks: &mut asks };
            DockArea::new(&mut self.dock).show_inside(ui, &mut viewer);
        }
        for ask in asks {
            match ask {
                Ask::Open(tab) => self.show_tab(tab),
                Ask::GoTo(target) => self.go_to(&target),
                Ask::TestSetup => self.setup = Some(crate::setupwin::SetupWindow::open(&self.ws)),
                Ask::Run(line) => {
                    let _ = self.run(&line);
                }
                Ask::Palette(text) => self.open_palette(PaletteKind::Commands, &text),
                Ask::New(kind) => self.new_asset = Some(crate::gallery::NewAsset::of(kind)),
                Ask::Art(kind, id) => {
                    self.art.set_for(kind, &id, &self.ws);
                    self.show_tab(Tab::Art);
                }
                Ask::Share(assets) => self.share = Some(crate::packspage::ShareWindow::of(&self.ws, assets)),
                Ask::Help(file) => {
                    let (chapter, anchor) = editor_core::manual::topic(file, &[]);
                    self.help.open(chapter, &anchor);
                    self.show_tab(Tab::Help);
                }
                Ask::News => self.open_news(),
                Ask::NewsSeen => self.news_seen(),
                Ask::PlayIn(code, now) => self.play_in(code, now),
                Ask::FindUnused => self.find_unused(),
                Ask::Play(from) => self.play_from(from),
                Ask::PlayWindow => self.play_window(),
                Ask::Above(tab, beside) => self.show_above(tab, &beside),
                // (The window may be another project's then: what else was asked was the old one's.)
                Ask::OpenProject(path) => {
                    self.leave(Leaving::Open(path));
                    break;
                }
                Ask::NewProject(path, name, basics) => {
                    self.leave(Leaving::New(path, name, basics));
                    break;
                }
                Ask::CloseProject => {
                    self.leave(Leaving::Close);
                    break;
                }
                Ask::Try(map, tile, what) => {
                    // (With the test setup: a game in progress, so the map's intro doesn't play first.)
                    self.try_next = Some(*what);
                    self.play_from(PlayFrom::Tile(map, tile, true));
                }
            }
        }
        self.gallery_ui(&ctx, rs);
        // A tour that is followed: its steps, and a frame around the control to use.
        match self.tour.show(&ctx, &self.ws) {
            Some(crate::tourpanel::Want::Help(file, words)) => {
                let (chapter, anchor) = editor_core::manual::topic(file, words);
                self.help.open(chapter, &anchor);
                self.show_tab(Tab::Help);
            }
            // (The tab the step at hand is done in.)
            Some(crate::tourpanel::Want::Show(name)) => {
                if let Some(tab) = self.tab_named(&name) {
                    self.show_tab(tab);
                }
            }
            None => {}
        }
        self.cleanup_ui(&ctx);
        // *Test setup…*
        if let Some(mut w) = self.setup.take() {
            let mut asks = Vec::new();
            if w.show(&ctx, &mut self.ws, &self.lists, &mut asks) {
                self.setup = Some(w);
            }
            for ask in asks {
                match ask {
                    crate::form::Ask::Open(RefKind::Map, map) => self.show_tab(Tab::Map(map)),
                    crate::form::Ask::Open(k, i) => self.show_tab(Tab::Def(k, i)),
                    crate::form::Ask::New(k) => self.new_asset = Some(crate::gallery::NewAsset::of(Some(k))),
                }
            }
        }
        // Leaving the project with changes that aren't saved: asked first.
        self.leaving_ui(&ctx);
        // The *Share…* window.
        if let Some(mut w) = self.share.take() {
            if w.show(&ctx, &mut self.ws) {
                self.share = Some(w);
            }
        }
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
        // F1: the manual, at the chapter for what is in front; with the pointer on a field's
        // label, about that field. (A field's menu asks the same.)
        let asked = crate::form::asked_field(ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            match crate::form::pointed_field(ctx) {
                Some((label, help)) => self.open_field_help(&label, &help),
                None => self.open_help(),
            }
        } else if let Some((label, help)) = asked {
            self.open_field_help(&label, &help);
        }
        let (f5, f6) = ctx.input(|i| (i.key_pressed(egui::Key::F5), i.key_pressed(egui::Key::F6)));
        if f5 {
            self.play_from(PlayFrom::Title);
        } else if f6 {
            self.play_here();
        }
        // (A text field with the keyboard keeps Ctrl+Z for its own typing.)
        if save {
            let _ = self.run("file.save_all");
        }
        // (And the pointer over a picture being painted: there they are the picture's.)
        if !ctx.text_edit_focused() && !crate::pixeledit::has_pointer(ctx) {
            if undo {
                let _ = self.run("edit.undo");
            } else if redo {
                let _ = self.run("edit.redo");
            }
        }
    }

    /// The *New…* gallery: makes what it asks for and opens it.
    fn gallery_ui(&mut self, ctx: &egui::Context, rs: Option<&eframe::egui_wgpu::RenderState>) {
        let Some(state) = self.new_asset.as_mut() else {
            // (The library asset that was looked at isn't any more.)
            if let (Some(mut look), Some(rs)) = (self.lib_look.take(), rs) {
                look.thumbs.free(rs);
            }
            return;
        };
        // (What the library has of a kind is listed when that kind is picked.)
        self.ws.lib.ensure_loaded();
        // The library asset picked is looked at as a small project of its own: it and what it
        // needs, read when it is picked.
        let picked = state.kind.zip(state.from.clone()).filter(|(k, _)| state.library && crate::thumbs::has_pictures(*k));
        if self.lib_look.as_ref().map(|l| (l.kind, l.id.clone())) != picked {
            if let (Some(mut look), Some(rs)) = (self.lib_look.take(), rs) {
                look.thumbs.free(rs);
            }
            self.lib_look = picked.and_then(|(kind, id)| self.ws.library_look(kind, &id).map(|ws| LibLook { kind, id, ws: Box::new(ws), thumbs: Default::default() }));
        }
        let mut look = self.lib_look.as_mut();
        if let (Some(l), Some(rs)) = (look.as_mut(), rs) {
            l.thumbs.begin_frame(&l.ws, rs);
        }
        let mut library_picture = |kind: RefKind, id: &str| -> Option<crate::thumbs::Thumb> {
            let l = look.as_mut().filter(|l| l.kind == kind && l.id == id)?;
            l.thumbs.of(&l.ws, rs?, kind, id)
        };
        let mut pics = rs.map(|rs| crate::thumbs::Pics { thumbs: &mut self.thumbs, ws: &self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        let lib = &self.ws.lib;
        let library = |kind: RefKind| lib.lib.as_ref().map_or(Vec::new(), |l| editor_core::defs::list(&l.db, kind));
        // (The maps are a list like the others here: a new one's name must be free.)
        let mut lists = self.lists.clone();
        lists.push((RefKind::Map, self.ws.maps.iter().map(|m| (m.clone(), m.clone())).collect()));
        let outcome = crate::gallery::show(ctx, state, GROUPS, &lists, &library, pictures, &mut library_picture);
        if let Some(crate::gallery::Outcome::Map { name, width, height, ground }) = &outcome {
            match self.ws.create_map(name, *width, *height, Some(ground.as_str())) {
                Ok(said) => {
                    self.new_asset = None;
                    self.log.push(said.clone());
                    self.ws.set_status(said);
                    self.show_tab(Tab::Map(name.clone()));
                }
                Err(e) => {
                    if let Some(state) = self.new_asset.as_mut() {
                        state.error = Some(e);
                    }
                }
            }
            return;
        }
        let made = match outcome {
            Some(crate::gallery::Outcome::FromLibrary { kind, name, from }) => Some((kind, self.ws.make_from_library(kind, &from, &name))),
            Some(crate::gallery::Outcome::FromFile { kind, name, file }) => Some((kind, self.ws.make_from_file(kind, &file, &name))),
            Some(crate::gallery::Outcome::Create { kind, name, from }) => Some((kind, self.ws.make_def(kind, &name, from.as_deref()))),
            Some(crate::gallery::Outcome::Cancel) => {
                self.new_asset = None;
                None
            }
            Some(crate::gallery::Outcome::Map { .. }) | None => None,
        };
        if let Some((kind, made)) = made {
            match made {
                Ok(id) => {
                    self.new_asset = None;
                    self.log.push(format!("Made {} \"{id}\"", kind.name()));
                    self.lists.clear();
                    self.show_tab(Tab::Def(kind, id));
                }
                Err(e) => {
                    if let Some(state) = self.new_asset.as_mut() {
                        state.error = Some(e);
                    }
                }
            }
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
        let play_menu = ui.menu_button("Play", |ui| {
            if ui.button("From the title screen   F5").on_hover_text("The game as players get it, here in a tab").clicked() {
                self.play_from(PlayFrom::Title);
                ui.close();
            }
            if ui.button("On this map   F6").on_hover_text("On the map being worked on, where its view looks (right-click the map for an exact spot)").clicked() {
                self.play_here();
                ui.close();
            }
            if ui.button("On the start map").clicked() {
                self.play_from(PlayFrom::Start);
                ui.close();
            }
            ui.separator();
            if ui.button("In its own window").on_hover_text("The real game beside the editor, started as the Play tab's was").clicked() {
                self.play_window();
                ui.close();
            }
            if ui.button("Show the Play tab").clicked() {
                self.show_tab(Tab::Play);
                ui.close();
            }
        });
        crate::tourpanel::mark(ui, "menu:Play", play_menu.response.rect);
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
    world: &'a mut crate::worldview::WorldView,
    graphs: &'a mut std::collections::HashMap<String, crate::graphview::GraphView>,
    link: &'a mut Option<(String, String)>,
    clip: &'a mut Option<editor_core::mapedit::Clip>,
    scripts: &'a mut std::collections::HashMap<String, crate::scriptedit::ScriptTab>,
    names: &'a mut crate::namespage::NamesPage,
    publish: &'a mut crate::publish::PublishPage,
    library: &'a mut crate::librarypage::LibraryPage,
    art: &'a mut crate::artpage::ArtPage,
    help: &'a mut crate::helpview::HelpView,
    tour: &'a mut crate::tourpanel::TourPanel,
    agents: &'a mut crate::agentspage::AgentsPage,
    langs: &'a mut crate::langpage::LangPage,
    plugins: &'a mut crate::pluginspage::PluginsPage,
    play_lang: &'a Option<String>,
    news: &'a mut bool,
    home: &'a mut Home,
    timelines: &'a mut std::collections::HashMap<String, crate::timeline::Timeline>,
    nodes: &'a mut std::collections::HashMap<String, crate::nodesview::NodesView>,
    stages: &'a mut std::collections::HashMap<String, crate::propshop::PropStage>,
    figures: &'a mut std::collections::HashMap<String, crate::charshop::CharStage>,
    model_stages: &'a mut std::collections::HashMap<String, crate::modelshop::ModelStage>,
    item_stages: &'a mut std::collections::HashMap<String, crate::itemshop::ItemStage>,
    anim_stages: &'a mut std::collections::HashMap<String, crate::animshop::AnimStage>,
    pixels: &'a mut std::collections::HashMap<String, crate::pixeledit::PixelEdit>,
    sounds: &'a mut std::collections::HashMap<String, crate::artshop::SoundStage>,
    sheets: &'a mut std::collections::HashMap<String, crate::artshop::SheetStage>,
    scrub: &'a mut Option<(String, editor_core::cutscene::Preview, u64)>,
    try_next: &'a mut Option<(rustpg2d_engine::events::TriggerOn, rustpg2d_engine::runtime::EventOwner, String)>,
    rs: Option<&'a eframe::egui_wgpu::RenderState>,
    asks: &'a mut Vec<Ask>,
}

impl egui_dock::TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::unique(("tab", tab.name()))
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        match tab {
            Tab::Script(f) if self.scripts.get(f).is_some_and(|t| t.dirty()) => format!("{f} *").into(),
            _ => tab.title(self.ws).into(),
        }
    }

    fn is_closeable(&self, tab: &Tab) -> bool {
        !matches!(tab, Tab::Browser)
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        // (The game and a map fill their tabs.)
        if matches!(tab, Tab::Play | Tab::Map(_) | Tab::Script(_) | Tab::Def(RefKind::VisualScript | RefKind::Structure, _)) {
            [false, false]
        } else {
            [false, true]
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        // (A tab that is drawn is in view: a tour's step done in it needn't lead there.)
        crate::tourpanel::mark(ui, &format!("tab:{}", tab.name()), ui.max_rect());
        match tab {
            Tab::Browser => self.browser(ui),
            Tab::Overview => self.overview(ui),
            Tab::World => {
                if let Some(map) = self.world.show(ui, self.ws) {
                    self.asks.push(Ask::Open(Tab::Map(map)));
                }
            }
            Tab::Graph(m) => {
                let selected = selected_owner(self.maps, m);
                if let Some(t) = self.graphs.entry(m.clone()).or_default().show(ui, self.ws, m, selected) {
                    self.asks.push(Ask::GoTo(t));
                }
            }
            Tab::Play => self.play_tab(ui),
            Tab::Problems => self.problems(ui),
            Tab::History => self.history(ui),
            Tab::Settings(page) => self.settings(ui, page),
            Tab::Kind(k) => self.kind(ui, *k),
            Tab::Def(RefKind::VisualScript, id) => self.visual_script(ui, id),
            Tab::Def(RefKind::Structure, id) => self.structure(ui, id),
            Tab::Def(k, id) => self.definition(ui, *k, id),
            Tab::Map(m) => self.map(ui, m),
            Tab::Script(f) => self.script(ui, f),
            Tab::Library => {
                if let Some((kind, id)) = self.library.show(ui, self.ws) {
                    self.asks.push(Ask::Open(Tab::Def(kind, id)));
                }
            }
            Tab::Art => {
                if let Some((kind, id)) = self.art.show(ui, self.ws) {
                    self.asks.push(Ask::Open(Tab::Def(kind, id)));
                }
            }
            Tab::Help => self.help.show(ui),
            Tab::Agents => {
                if let Some(crate::agentspage::Want::Help) = self.agents.show(ui, self.ws) {
                    self.asks.push(Ask::Help("13-ai-agents.md"));
                }
            }
            Tab::Publish => {
                // (The game is exported from its files: what is in the script tabs is written first.)
                if self.scripts.values().any(|t| t.dirty()) {
                    ui.colored_label(ui.visuals().warn_fg_color, "Scripts have changes that aren't saved yet: Ctrl+S, or they aren't in the export.");
                }
                self.publish.show(ui, self.ws);
            }
            Tab::Names => {
                // (Scripts saved from their tabs change what the page shows.)
                let saved = self.scripts.values().filter(|t| !t.dirty()).count().to_string();
                if let Some(target) = self.names.show(ui, self.ws, &saved) {
                    self.go(&target);
                }
            }
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
            if (f.is_empty() || "library".contains(&f)) && ui.selectable_label(false, "Asset library").on_hover_text("Ready-made characters, props, items, sounds and more: tick what you want and add it to the project").clicked() {
                self.asks.push(Ask::Open(Tab::Library));
            }
            if (f.is_empty() || "generate art meshy".contains(&f)) && ui.selectable_label(false, "Generate art").on_hover_text("Icons, portraits, pictures and 3D models drawn by Meshy with your own account").clicked() {
                self.asks.push(Ask::Open(Tab::Art));
            }
            if (f.is_empty() || "ai agents mcp".contains(&f)) && ui.selectable_label(false, "AI agents").on_hover_text("An AI assistant works in the project with you: the agent server and the set-up for its program").clicked() {
                self.asks.push(Ask::Open(Tab::Agents));
            }
            if (f.is_empty() || "publishing export".contains(&f)) && ui.selectable_label(false, "Publishing").on_hover_text("The game exported to play without the editor: for Linux, Windows, macOS and the web").clicked() {
                self.asks.push(Ask::Open(Tab::Publish));
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
            let group_header = egui::CollapsingHeader::new(*group).default_open(*group == "World").open(filtering.then_some(true)).show(ui, |ui| {
                if *group == "World" && !filtering && ui.selectable_label(false, "World view").on_hover_text("Every map and the ways between them").clicked() {
                    self.asks.push(Ask::Open(Tab::World));
                }
                if *group == "Story" {
                    if !filtering && ui.selectable_label(false, "Flags and variables").on_hover_text("What the game remembers: where each is set and read, a note, rename").clicked() {
                        self.asks.push(Ask::Open(Tab::Names));
                    }
                    self.browser_scripts(ui, &f);
                }
                for (kind, rows) in &found {
                    let header = egui::CollapsingHeader::new(format!("{} ({})", plural(kind.label()), rows.len())).id_salt(("kind", kind.name())).open(filtering.then_some(rows.len() <= 40)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button(format!("+ New {}", kind.label().to_lowercase())).clicked() {
                                self.asks.push(Ask::New(Some(*kind)));
                            }
                            if *kind != RefKind::Map && ui.small_button("Show all").on_hover_text("All of them in a tab, as pictures or a list").clicked() {
                                self.asks.push(Ask::Open(Tab::Kind(*kind)));
                            }
                        });
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
                    crate::tourpanel::mark(ui, &format!("browser:{}", kind.name()), header.header_response.rect);
                }
            });
            // (A group that is folded away stands in for its kinds: a tour points at it.)
            for kind in kinds.iter() {
                let name = format!("browser:{}", kind.name());
                if !crate::tourpanel::marked_now(ui.ctx(), &name) {
                    crate::tourpanel::mark(ui, &name, group_header.header_response.rect);
                }
            }
        }
    }

    /// A definition (or map) in the browser: its picture and name, opened by a click.
    /// The project's script files, in the Story group.
    fn browser_scripts(&mut self, ui: &mut egui::Ui, filter: &str) {
        let files: Vec<String> = self.ws.script_names().into_iter().map(|(n, _)| n).filter(|n| filter.is_empty() || n.to_lowercase().contains(filter)).collect();
        if files.is_empty() && !filter.is_empty() {
            return;
        }
        egui::CollapsingHeader::new(format!("Scripts ({})", files.len())).id_salt("scripts").open((!filter.is_empty()).then_some(true)).show(ui, |ui| {
            for f in &files {
                let dirty = self.scripts.get(f).is_some_and(|t| t.dirty());
                if ui.selectable_label(false, if dirty { format!("{f} *") } else { f.clone() }).on_hover_text("A file of script functions (Rhai): what events run by name").clicked() {
                    self.asks.push(Ask::Open(Tab::Script(f.clone())));
                }
            }
            // A new file: named, with a first function to write in.
            let id = ui.scope_id().with("new script");
            let mut name: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut name).hint_text("new file").desired_width(110.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Name of the new script"));
                let file = format!("{}.rhai", name.trim().trim_end_matches(".rhai"));
                let ok = !name.trim().is_empty() && rustpg2d_engine::project::safe_rel(&file) && !file.contains('/') && !files.contains(&file);
                if ui.add_enabled(ok, egui::Button::new("+ New script")).clicked() {
                    let path = self.ws.project.scripts_dir().join(&file);
                    let written = std::fs::create_dir_all(self.ws.project.scripts_dir()).and_then(|_| std::fs::write(&path, format!("// {file}\n\nfn {}() {{\n}}\n", name.trim().trim_end_matches(".rhai").replace(|c: char| !c.is_alphanumeric(), "_"))));
                    match written {
                        Ok(()) => {
                            self.ws.refresh_refs();
                            self.asks.push(Ask::Open(Tab::Script(file)));
                            name.clear();
                        }
                        Err(e) => self.ws.set_status(format!("{file}: {e}")),
                    }
                }
            });
            ui.data_mut(|d| d.insert_temp(id, name));
        });
    }

    fn browser_row(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str, name: &str) {
        let label = if name == id { id.to_string() } else { format!("{name}  ({id})") };
        let clicked = ui
            .horizontal(|ui| {
                if let (true, Some(rs)) = (crate::thumbs::has_pictures(kind), self.rs) {
                    crate::thumbs::boxed(ui, 18.0, || self.thumbs.of(self.ws, rs, kind, id));
                }
                let r = ui.selectable_label(false, label);
                // What can stand on a map is dragged out of the list onto one.
                let r = if matches!(kind, RefKind::Character | RefKind::Prop | RefKind::Item | RefKind::Structure) {
                    let r = r.interact(egui::Sense::click_and_drag());
                    r.dnd_set_drag_payload((kind, id.to_string()));
                    r.on_hover_text("Click to open it; drag it onto a map to put it down there")
                } else {
                    r
                };
                let clicked = r.clicked();
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
            if !self.ws.status.0.is_empty() {
                ui.weak(&self.ws.status.0);
            }
            ui.add_space(8.0);
            self.news_banner(ui);
            self.home(ui);
            return;
        }
        self.news_banner(ui);
        let cfg = &self.ws.project.config;
        ui.heading(&cfg.name);
        ui.weak(self.ws.project_location().display().to_string());
        ui.add_space(8.0);
        ui.strong("Learn by doing");
        ui.weak("Three short tours through the editor: each shows its steps and points at what to click.");
        if let Some(k) = self.tour.offer(ui) {
            self.tour.start(self.ws, k);
        }
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
        // (What is still to do leads to where it is done.)
        let hero_page = editor_core::schema::PAGES.iter().map(|(title, ..)| *title).find(|t| *t == "Hero & party");
        for (k, (text, done)) in steps.into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(if done { "[x]" } else { "[ ]" });
                if done {
                    ui.label(text);
                    return;
                }
                let (how, tip) = match k {
                    0 => ("Game", "The Game page names the map the game starts on"),
                    1 => ("Hero & party", "The Hero & party page names the player's character: one of the project's characters (the asset library has heroes to start from)"),
                    2 => ("Asset library", "Ready-made characters to add, or New… makes one"),
                    3 => ("New quest", "A quest with its steps and rewards"),
                    _ => ("Check the project", "Looks for what players would run into"),
                };
                if ui.link(text).on_hover_text(tip).clicked() {
                    match k {
                        0 => self.asks.push(Ask::Open(Tab::Settings("Game"))),
                        1 => self.asks.extend(hero_page.map(|p| Ask::Open(Tab::Settings(p)))),
                        2 => self.asks.push(Ask::Open(Tab::Library)),
                        3 => self.asks.push(Ask::New(Some(RefKind::Quest))),
                        _ => {
                            self.asks.push(Ask::Run("check.problems".into()));
                            self.asks.push(Ask::Open(Tab::Problems));
                        }
                    }
                }
                ui.weak(how);
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
        // Other projects: another one in this window, or none.
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Another project").id_salt("home").default_open(false).show(ui, |ui| {
            if ui.button("Close this project").on_hover_text("Back to the start, with no project open").clicked() {
                self.asks.push(Ask::CloseProject);
            }
            ui.add_space(6.0);
            self.home(ui);
        });
    }

    /// After an update: this editor is newer than the one that ran here last.
    fn news_banner(&mut self, ui: &mut egui::Ui) {
        if !*self.news {
            return;
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("The editor was updated to {}.", editor_core::manual::VERSION));
                if ui.button("What's new").on_hover_text("What changed, in the Help tab (it stays there to read later)").clicked() {
                    self.asks.push(Ask::News);
                }
                if ui.small_button("Not now").on_hover_text("Puts this away. What's new stays at the end of the Help tab's chapters.").clicked() {
                    self.asks.push(Ask::NewsSeen);
                }
            });
        });
        ui.add_space(8.0);
    }

    fn play_tab(&mut self, ui: &mut egui::Ui) {
        if self.play.is_none() && self.play_error.is_none() && self.ws.project_open {
            match PlayView::open(&self.ws.project.root, self.start.clone()) {
                Ok(mut p) => {
                    if let Some((on, owner, what)) = self.try_next.take() {
                        p.raise_soon(on, owner, what);
                    }
                    *self.play = Some(p);
                }
                Err(e) => *self.play_error = Some(format!("{e:#}")),
            }
        }
        match (self.play.as_mut(), self.rs) {
            (Some(p), Some(rs)) => {
                let setup = self.ws.test_setup();
                let summary = if setup == Default::default() { String::new() } else { setup.summary() };
                for ask in p.show(ui, rs, &summary) {
                    use crate::play::PlayAsk;
                    self.asks.push(match ask {
                        PlayAsk::Again => Ask::Play(PlayFrom::Again),
                        PlayAsk::FromTitle => Ask::Play(PlayFrom::Title),
                        PlayAsk::FromStart => Ask::Play(PlayFrom::Start),
                        PlayAsk::OwnWindow => Ask::PlayWindow,
                        // What was clicked in the game: its place on its map when it is placed
                        // there, else (spawned in play) what it is.
                        PlayAsk::Open(p) => Ask::Open(Tab::Def(editor_core::mapedit::kind_ref(p.kind), p.def)),
                        PlayAsk::KeepSetup(setup) => {
                            self.ws.set_status(format!("The test setup: {}", setup.summary()));
                            self.ws.set_test_setup(*setup);
                            continue;
                        }
                        PlayAsk::ForgetSetup => {
                            self.ws.set_test_setup(Default::default());
                            self.ws.set_status("The test setup is forgotten");
                            continue;
                        }
                        PlayAsk::EditSetup => Ask::TestSetup,
                    });
                }
            }
            _ => {
                ui.colored_label(egui::Color32::LIGHT_RED, self.play_error.clone().unwrap_or_else(|| "The game can't run here.".into()));
                if ui.button("Try again").clicked() {
                    self.asks.push(Ask::Play(PlayFrom::Again));
                }
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
        // (Checking everything takes a moment: it is done when asked, and what changed since
        // the last check is said, its list dimmed.)
        let outdated = self.ws.problems.built && self.ws.problems.stale;
        if outdated {
            ui.horizontal(|ui| {
                ui.label("The project changed since this check.");
                if ui.button("Check again").clicked() {
                    self.asks.push(Ask::Run("check.problems".into()));
                }
            });
        }
        ui.separator();
        if outdated {
            ui.multiply_opacity(0.5);
        }
        // Nothing listed: why, and what to do.
        if self.ws.problems.list.is_empty() {
            if self.ws.problems.status() == "-" {
                ui.weak("The check looks through everything for what players would run into: a door that leads nowhere, a quest nobody gives, a script that doesn't compile. What it finds is listed here, each with the way to its place.");
            } else {
                ui.label("Nothing was found: the check has nothing to say against this game.");
                ui.weak("It looks for what is broken, not for what is missing in the story: play it too.");
                if ui.button("Play").clicked() {
                    self.asks.push(Ask::Play(PlayFrom::Title));
                }
            }
            return;
        }
        // (What the library has is only known once it is read: when something is missing.)
        if self.ws.problems.list.iter().any(|p| p.missing().is_some()) {
            self.ws.lib.ensure_loaded();
        }
        let (mut go, mut fix, mut choose) = (None, None, None);
        let list = &self.ws.problems.list;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // By where they are, each group with how many.
            let mut areas: Vec<editor_core::problems::Area> = Vec::new();
            for p in list {
                if !areas.contains(&p.area) {
                    areas.push(p.area);
                }
            }
            for area in areas {
                let of: Vec<&editor_core::problems::Problem> = list.iter().filter(|p| p.area == area).collect();
                egui::CollapsingHeader::new(format!("{} ({})", area.label(), of.len())).id_salt(("problems of", area.name())).default_open(true).show(ui, |ui| {
                    for p in of {
                        ui.horizontal_wrapped(|ui| {
                            let color = if p.severity == editor_core::problems::Severity::Error { egui::Color32::from_rgb(235, 110, 100) } else { egui::Color32::from_rgb(230, 190, 90) };
                            ui.colored_label(color, p.severity.name());
                            if ui.link(&p.place).on_hover_text("Go there").clicked() {
                                go = Some(p.target.clone());
                            }
                            ui.label(&p.text);
                            // A broken reference: what is missing can be made, or taken from the library.
                            if let Some((kind, id)) = p.missing().filter(|(k, _)| k.is_def()) {
                                let what = kind.label().to_lowercase();
                                if self.ws.lib.lib.as_ref().is_some_and(|l| l.db.has(kind, &id)) && ui.small_button("Take it from the library").on_hover_text(format!("The library has a {what} \"{id}\": it comes with what it needs")).clicked() {
                                    fix = Some((kind, id.clone(), Fix::Library));
                                }
                                if ui.small_button("Create it").on_hover_text(format!("Makes an empty {what} \"{id}\" and opens it to fill in")).clicked() {
                                    fix = Some((kind, id.clone(), Fix::Create));
                                }
                                // One that exists instead, everywhere the missing one is named.
                                let known: Vec<String> = self.lists.iter().find(|(k, _)| *k == kind).map_or(Vec::new(), |(_, l)| l.iter().map(|(i, _)| i.clone()).collect());
                                if !known.is_empty() {
                                    let picked = ui.push_id(("use another", &p.place, &id), |ui| name_menu(ui, "Use another…", &format!("Search the {}", plural(kind.label()).to_lowercase()), &known, None)).inner;
                                    if let Some(to) = picked {
                                        fix = Some((kind, id, Fix::Another(to)));
                                    }
                                }
                            }
                            // A setting to choose (the player character, the start map): from the
                            // project's own.
                            if let Some(c) = p.choice() {
                                let known: Vec<String> = match c.kind() {
                                    RefKind::Map => self.ws.maps.clone(),
                                    k => self.lists.iter().find(|(l, _)| *l == k).map_or(Vec::new(), |(_, l)| l.iter().map(|(i, _)| i.clone()).collect()),
                                };
                                if known.is_empty() {
                                    ui.weak(format!("(the project has no {} to choose yet)", plural(c.kind().label()).to_lowercase()));
                                } else if let Some(to) = ui.push_id(("choose one", &p.text), |ui| name_menu(ui, "Choose one…", &format!("Search the {}", plural(c.kind().label()).to_lowercase()), &known, None)).inner {
                                    choose = Some((c, to));
                                }
                            }
                        });
                    }
                });
            }
        });
        if let Some(target) = go {
            self.go(&target);
        }
        if let Some((c, to)) = choose {
            match self.ws.choose_setting(c, &to) {
                Ok(said) => self.ws.set_status(said),
                Err(e) => self.ws.set_status(format!("Not fixed: {e}")),
            }
        }
        if let Some((kind, id, how)) = fix {
            let (r, open) = match &how {
                Fix::Library => (self.ws.add_missing_from_library(kind, &id), Some(id.clone())),
                Fix::Create => (self.ws.create_missing(kind, &id), Some(id.clone())),
                // (Everything that named the missing one names this one: nothing new to open.)
                Fix::Another(to) => (self.ws.use_another(kind, &id, to), None),
            };
            match r {
                Ok(said) => {
                    self.ws.set_status(said);
                    self.asks.extend(open.map(|id| Ask::Open(Tab::Def(kind, id))));
                }
                Err(e) => self.ws.set_status(format!("Not fixed: {e}")),
            }
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
        if kind == RefKind::Cutscene {
            self.timeline(ui, id);
            ui.separator();
        }
        if kind == RefKind::Prop {
            self.stages.entry(id.to_string()).or_default().show(ui, self.ws, self.rs, self.thumbs, id);
            ui.separator();
        }
        if kind == RefKind::Item {
            self.item_stages.entry(id.to_string()).or_default().show(ui, self.ws, self.rs, self.thumbs, id);
            ui.separator();
        }
        if kind == RefKind::Animation {
            self.anim_stages.entry(id.to_string()).or_default().show(ui, self.ws, self.rs, self.thumbs, id);
            ui.separator();
        }
        if kind == RefKind::Character {
            self.figures.entry(id.to_string()).or_default().show(ui, self.ws, self.rs, self.thumbs, id);
            ui.separator();
        }
        if kind == RefKind::Model {
            self.model_stages.entry(id.to_string()).or_default().show(ui, self.ws, self.rs, self.thumbs, id);
            ui.separator();
        }
        if kind == RefKind::Material {
            self.pixels.entry(material_key(id)).or_insert_with(crate::pixeledit::PixelEdit::of_material).show(ui, self.ws, id, None);
            ui.separator();
        }
        if kind == RefKind::Sound {
            self.sounds.entry(id.to_string()).or_default().show(ui, self.ws, id);
            ui.separator();
        }
        if kind == RefKind::Sheet {
            let stage = self.sheets.entry(id.to_string()).or_default();
            stage.show(ui, self.ws, self.rs, self.thumbs, id);
            let frame = stage.frame;
            self.pixels.entry(id.to_string()).or_default().show(ui, self.ws, id, frame);
            ui.separator();
        }
        // The form shows what was typed last, also while that isn't a valid definition yet.
        let mut value = self.forms.get(&key).map_or_else(|| saved.clone(), |(v, _)| v.clone());
        let mut pics = self.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *self.thumbs, ws: &*self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: *self.show_all, compact: false, pictures, refs: Some(&self.ws.refs), asks: Vec::new(), said: Default::default() };
        let changed = match editor_core::schema::of_kind(kind) {
            Some(node) => crate::form::definition_ui(ui, &mut cx, node, &mut value),
            None => crate::form::value_ui(ui, kind.label(), &mut value),
        };
        let said = std::mem::take(&mut cx.said);
        for ask in cx.asks {
            match ask {
                crate::form::Ask::Open(RefKind::Map, map) => self.asks.push(Ask::Open(Tab::Map(map))),
                crate::form::Ask::Open(k, i) => self.asks.push(Ask::Open(Tab::Def(k, i))),
                crate::form::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
            }
        }
        if changed {
            match crate::form::take(self.ws, kind, id, value.clone(), &said) {
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
        // The Languages page: the translations before the settings.
        if page == "Languages" {
            if let Some(crate::langpage::Want::PlayIn(code, now)) = self.langs.show(ui, self.ws, self.play_lang) {
                self.asks.push(Ask::PlayIn(code, now));
            }
        }
        // Scripts & plugins: the plugins before the hooks.
        if page == "Scripts & plugins" {
            self.plugins.show(ui, self.ws, self.lists);
        }
        let mut value = self.ws.config_value();
        let mut pics = self.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *self.thumbs, ws: &*self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: *self.show_all, compact: false, pictures, refs: Some(&self.ws.refs), asks: Vec::new(), said: Default::default() };
        let changed = crate::form::page_ui(ui, &mut cx, editor_core::schema::project(), &mut value, fields);
        let step = format!("Game settings: {}", cx.said.words().unwrap_or("changed"));
        for ask in cx.asks {
            match ask {
                crate::form::Ask::Open(RefKind::Map, map) => self.asks.push(Ask::Open(Tab::Map(map))),
                crate::form::Ask::Open(k, i) => self.asks.push(Ask::Open(Tab::Def(k, i))),
                crate::form::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
            }
        }
        if changed {
            self.ws.begin(&step);
            if let Err(e) = self.ws.set_config_value(value) {
                self.ws.set_status(e);
            }
        }
    }

    /// What can be done with a definition as a whole.
    fn definition_buttons(&mut self, ui: &mut egui::Ui, kind: RefKind, id: &str) {
        // (Beside a canvas the form is narrow: the buttons wrap.)
        let wide = ui.available_width() > 620.0;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Rename…").on_hover_text("Changes the id, and every place that names it").clicked() {
                self.asks.push(Ask::Palette(format!("edit.rename {} {id} ", kind.name())));
            }
            if ui.button("Duplicate…").clicked() {
                self.asks.push(Ask::Palette(format!("defs.duplicate {} {id} ", kind.name())));
            }
            if ui.button("Delete").on_hover_text("Refused while something still uses it").clicked() {
                self.asks.push(Ask::Run(format!("defs.delete {} {id}", kind.name())));
            }
            if rustpg2d_engine::library::KINDS.contains(&kind) && ui.button("Share…").on_hover_text("Writes it as a pack file with what it needs (pictures, sounds, models, the script functions its events call), under your name and a licence: to give to others, or to offer from a web address").clicked() {
                self.asks.push(Ask::Share(vec![(kind, id.to_string())]));
            }
            if rustpg2d_engine::library::KINDS.contains(&kind) && ui.button("Send to library").on_hover_text("Puts it into the asset library with what it needs (pictures, sounds, models, the script functions its events call), for your other projects. One that is there already is added under a new id.").clicked() {
                match self.ws.project_to_library(&[(kind, id.to_string())], false, false) {
                    Ok(m) => self.ws.set_status(m),
                    Err(e) => self.ws.set_status(format!("Not sent: {e:#}")),
                }
            }
            if ui.button("Find usages").clicked() {
                self.asks.push(Ask::Run(format!("find.usages {} {id}", kind.name())));
            }
            let art = match kind {
                RefKind::Prop => Some("A 3D model of this prop, made by Meshy from its picture and a description"),
                RefKind::Item => Some("A 3D model of this item for characters' hands, made by Meshy from its icon"),
                RefKind::Character => Some("A dialog portrait of this character, drawn by Meshy"),
                RefKind::Model if self.ws.project.db.model(id).is_some_and(|m| m.part.is_none() && m.rig.is_none()) => Some("More animations for this model from Meshy's library"),
                _ => None,
            };
            if let Some(tip) = art {
                if ui.button("Generate art…").on_hover_text(format!("{tip}. Opens Generate art with it filled in: nothing is spent before you say so there.")).clicked() {
                    self.asks.push(Ask::Art(kind, id.to_string()));
                }
            }
            if self.ws.usages.of.as_ref().is_some_and(|(k, i)| *k == kind && i == id) {
                ui.label(format!("{} usage(s)", self.ws.usages.list.len()));
            }
            let hint = "Also the fields that are rarely needed: values from before the SRD rules, data other tools edit";
            if wide {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(self.show_all, "Show all fields").on_hover_text(hint);
                });
            } else {
                ui.checkbox(self.show_all, "Show all fields").on_hover_text(hint);
            }
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
                if rustpg2d_engine::library::KINDS.contains(&kind) && ui.button("Share…").on_hover_text("Writes those chosen as one pack file, each with what it needs").clicked() {
                    self.asks.push(Ask::Share(chosen.iter().map(|id| (kind, id.clone())).collect()));
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
        // Nothing of this kind yet: what belongs here, and the ways to the first one.
        if all.is_empty() {
            let (one, many) = (kind.label().to_lowercase(), plural(kind.label()).to_lowercase());
            ui.add_space(6.0);
            ui.label(format!("This game has no {many} yet."));
            ui.weak(editor_core::defs::about(kind));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(format!("New {one}…")).on_hover_text("Blank, or from a file where the kind takes one").clicked() {
                    self.asks.push(Ask::New(Some(kind)));
                }
                if rustpg2d_engine::library::KINDS.contains(&kind) && ui.button("Take some from the asset library").on_hover_text("Ready-made ones: tick what you want, they come with what they need").clicked() {
                    self.asks.push(Ask::Open(Tab::Library));
                }
            });
        } else if rows.is_empty() {
            ui.add_space(6.0);
            ui.weak(format!("None of the {} fits what is typed and picked above.", all.len()));
        }
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
                let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: true, compact: true, pictures: pictures_for, refs: Some(&self.ws.refs), asks: Vec::new(), said: Default::default() };
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

    /// The home screen: a new project, one opened by where it is, the ones worked on lately.
    /// With a project open it is the Overview's last section.
    fn home(&mut self, ui: &mut egui::Ui) {
        let h = &mut *self.home;
        ui.strong("A new game");
        ui.horizontal(|ui| {
            let l = ui.label("Its name");
            ui.add(egui::TextEdit::singleline(&mut h.new_name).hint_text("My game").desired_width(220.0)).labelled_by(l.id);
        });
        // Where it goes: beside the others, named after it, until another place is typed.
        if !h.new_at_typed {
            let base = rustpg2d_engine::paths::home().map(|d| d.join("rustpg2d")).unwrap_or_default();
            let slug = rustpg2d_engine::release::slug(if h.new_name.trim().is_empty() { "my-game" } else { h.new_name.trim() });
            let file = if editor_core::project_file::folders_allowed() { slug } else { format!("{slug}.{}", editor_core::project_file::EXT) };
            h.new_at = base.join(file).display().to_string();
        }
        ui.horizontal(|ui| {
            let l = ui.label("Kept in");
            if ui.add(egui::TextEdit::singleline(&mut h.new_at).desired_width(520.0)).labelled_by(l.id).changed() {
                h.new_at_typed = true;
            }
            // A project file is asked for by its name; a project folder by the folder it goes into.
            let folders = editor_core::project_file::folders_allowed();
            let at = PathBuf::from(h.new_at.trim());
            let offered = at.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let want = || if folders { crate::pick::Want::Folder } else { crate::pick::Want::Save("R(ust)PG2D project", editor_core::project_file::EXT, offered.clone()) };
            if let Some(picked) = crate::pick::browse(ui, crate::pick::Key::NewProject, "Browse…", if folders { "The folder the new game's folder goes into" } else { "Where the new game is kept" }, want, at.parent().map(PathBuf::from)) {
                let picked = if folders {
                    picked.join(&offered)
                } else if picked.extension().is_none() {
                    picked.with_extension(editor_core::project_file::EXT)
                } else {
                    picked
                };
                h.new_at = picked.display().to_string();
                h.new_at_typed = true;
            }
        });
        ui.horizontal(|ui| {
            let ok = !h.new_name.trim().is_empty() && !h.new_at.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new(egui::RichText::new("Make it").strong())).on_hover_text("A project with one map to start on and a script file. Everything else comes from the asset library or is made here.").on_disabled_hover_text("Give it a name first").clicked() {
                self.asks.push(Ask::NewProject(PathBuf::from(h.new_at.trim()), h.new_name.trim().to_string(), !h.new_empty));
            }
            let mut basics = !h.new_empty;
            ui.checkbox(&mut basics, "Start with the basics from the asset library").on_hover_text("Ground to paint with (grass, dirt, water, cobblestone…), forest and rock scatter sets, a wall and a fence, a hero who is the player's character, a few items and props, grass on the first map: Play works at once. Off: an empty project, everything of your own.");
            h.new_empty = !basics;
        });
        ui.add_space(10.0);
        ui.strong("Open a project");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut h.open_path).hint_text("a .r2dproject file, or a project's folder").desired_width(520.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Where the project is"));
            if ui.add_enabled(!h.open_path.trim().is_empty(), egui::Button::new("Open")).clicked() {
                self.asks.push(Ask::OpenProject(PathBuf::from(h.open_path.trim())));
            }
            // Picked in a dialog, it opens at once.
            let start = Some(PathBuf::from(h.open_path.trim())).filter(|p| !p.as_os_str().is_empty()).or_else(|| rustpg2d_engine::paths::home().map(|d| d.join("rustpg2d")));
            let file = crate::pick::browse(ui, crate::pick::Key::OpenProject, "Browse…", "Open a project file", || crate::pick::Want::File("R(ust)PG2D project", &[editor_core::project_file::EXT]), start.clone());
            let folder = crate::pick::browse(ui, crate::pick::Key::OpenFolder, "A folder…", "Open a project's folder (the one with project.ron in it)", || crate::pick::Want::Folder, start);
            if let Some(picked) = file.or(folder) {
                h.open_path = picked.display().to_string();
                self.asks.push(Ask::OpenProject(picked));
            }
        });
        let here = editor_core::project_file::canonical(&self.ws.project_location());
        let recent: Vec<PathBuf> = editor_core::files::recent_projects().into_iter().filter(|p| !self.ws.project_open || *p != here).collect();
        if !recent.is_empty() {
            ui.add_space(6.0);
            ui.weak("Worked on lately");
            for p in recent {
                let name = p.file_stem().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string());
                ui.horizontal(|ui| {
                    if ui.link(name).clicked() {
                        self.asks.push(Ask::OpenProject(p.clone()));
                    }
                    ui.weak(p.display().to_string());
                });
            }
        }
        if let Some(example) = editor_core::files::bundled_example() {
            ui.add_space(6.0);
            if ui.link("The example game").on_hover_text(format!("{}: a small finished game to look into and change (your own copy of it)", example.display())).clicked() {
                match editor_core::files::example_copy() {
                    Ok(copy) => self.asks.push(Ask::OpenProject(copy)),
                    Err(e) => self.ws.set_status(format!("{e:#}")),
                }
            }
        }
    }

    /// Opens the place a problem, a use or a search hit is at.
    fn go(&mut self, target: &Target) {
        match target {
            Target::Def(k, id) => self.asks.push(Ask::Open(Tab::Def(*k, id.clone()))),
            Target::Map(m) | Target::MapTriggers(m) => self.asks.push(Ask::Open(Tab::Map(m.clone()))),
            Target::Entity(m, uid) => {
                self.maps.entry(m.clone()).or_default().select(editor_core::workspace::Selection::Entity(*uid));
                self.asks.push(Ask::Open(Tab::Map(m.clone())));
            }
            Target::Zone(m, uid) => {
                self.maps.entry(m.clone()).or_default().select(editor_core::workspace::Selection::Zone(*uid));
                self.asks.push(Ask::Open(Tab::Map(m.clone())));
            }
            Target::Script(path, at) => {
                let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.scripts.entry(file.clone()).or_insert_with(|| crate::scriptedit::ScriptTab::open(path.clone())).reveal(*at);
                self.asks.push(Ask::Open(Tab::Script(file)));
            }
            Target::Project => self.asks.push(Ask::Open(Tab::Overview)),
        }
    }

    /// A cut-scene's timeline, above its form.
    fn timeline(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(pos) = self.ws.project.db.cutscenes.iter().position(|d| d.id == id) else { return };
        // (Changed on a copy: the form's helpers read the project while the timeline is shown.)
        let mut def = self.ws.project.db.cutscenes[pos].clone();
        let tl = self.timelines.entry(id.to_string()).or_default();
        let mut pics = self.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *self.thumbs, ws: &*self.ws, rs });
        let pictures = pics.as_mut().map(|p| p as &mut dyn crate::form::Pictures);
        let mut cx = crate::form::Cx { lists: self.lists, maps: &self.ws.maps, show_all: *self.show_all, compact: false, pictures, refs: Some(&self.ws.refs), asks: Vec::new(), said: Default::default() };
        let (changed, ask) = tl.show(ui, &mut cx, &mut def, &self.ws.refs.targets);
        let time = tl.time;
        if changed {
            self.ws.begin(&format!("Change the cut-scene {}", if def.name.is_empty() { id } else { &def.name }));
            self.ws.project.db.cutscenes[pos] = def;
            self.ws.stale = true;
        }
        // Where everyone is at the playhead, for the map's tab.
        if let (Some(map), Some(preview)) = (self.ws.cutscene_map(id), self.ws.cutscene_preview(id, time)) {
            *self.scrub = Some((map, preview, ui.ctx().cumulative_frame_nr()));
        }
        match ask {
            Some(crate::timeline::Ask::Play(at)) => self.asks.push(Ask::Play(PlayFrom::Cutscene(id.to_string(), at))),
            Some(crate::timeline::Ask::ShowMap) => {
                if let Some(map) = self.ws.cutscene_map(id) {
                    self.asks.push(Ask::Above(Tab::Map(map), Tab::Def(RefKind::Cutscene, id.to_string())));
                }
            }
            None => {}
        }
    }

    /// A structure's tab: the structure as a small map of its own, worked on with the map's
    /// tools; what is done there goes into the definition (`editor_core::structdoc`).
    fn structure(&mut self, ui: &mut egui::Ui, id: &str) {
        let name = match self.ws.open_structure(id) {
            Ok(name) => name,
            Err(_) => {
                ui.weak(format!("Structure \"{id}\" is gone (renamed or deleted)."));
                return;
            }
        };
        egui::Panel::top(egui::Id::unique(("structure head", id))).show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(rs) = self.rs {
                    crate::thumbs::boxed(ui, 44.0, || self.thumbs.of(self.ws, rs, RefKind::Structure, id));
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let d = self.ws.project.db.structure(id);
                        ui.heading(d.map(|d| d.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(id));
                        ui.weak(format!("Structure · {id}"));
                    });
                    self.definition_buttons(ui, RefKind::Structure, id);
                });
            });
            ui.add_space(2.0);
        });
        self.map(ui, &name);
    }

    /// A visual script's tab: what it is called and takes on the left, its nodes on the canvas.
    fn visual_script(&mut self, ui: &mut egui::Ui, id: &str) {
        egui::Panel::left(egui::Id::unique(("visual script form", id))).default_size(330.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.definition(ui, RefKind::VisualScript, id));
        });
        if !self.ws.project.db.has(RefKind::VisualScript, id) {
            self.nodes.remove(id);
            return;
        }
        match self.nodes.entry(id.to_string()).or_default().show(ui, self.ws, id) {
            Some(crate::nodesview::Ask::Open(other)) => self.asks.push(Ask::Open(Tab::Def(RefKind::VisualScript, other))),
            None => {}
        }
    }

    /// A script's tab.
    fn script(&mut self, ui: &mut egui::Ui, file: &str) {
        let Some(path) = self.ws.script_names().into_iter().find(|(n, _)| n == file).map(|(_, p)| p).or_else(|| self.scripts.get(file).map(|t| t.path.clone())) else {
            ui.weak(format!("There is no script {file} in this project."));
            return;
        };
        let tab = self.scripts.entry(file.to_string()).or_insert_with(|| crate::scriptedit::ScriptTab::open(path));
        if let Some(crate::scriptedit::Ask::Function(name)) = tab.show(ui, self.ws) {
            // Defined in another file: that file's tab, at the function.
            let found = self.ws.script_names().into_iter().find_map(|(n, p)| {
                let src = std::fs::read_to_string(&p).ok()?;
                let at = src.find(&format!("fn {name}"))?;
                Some((n, p, src[..at].chars().count()))
            });
            if let Some((n, p, at)) = found {
                self.scripts.entry(n.clone()).or_insert_with(|| crate::scriptedit::ScriptTab::open(p)).reveal(at);
                self.asks.push(Ask::Open(Tab::Script(n)));
            }
        }
    }

    fn map(&mut self, ui: &mut egui::Ui, name: &str) {
        let view = self.maps.entry(name.to_string()).or_default();
        // (A cut-scene's moment, while its timeline was in view this frame or the last.)
        let frame = ui.ctx().cumulative_frame_nr();
        let scrub = self.scrub.as_ref().filter(|(map, _, seen)| map == name && frame.saturating_sub(*seen) <= 2).map(|(_, p, _)| p);
        let mut parts = crate::mapview::Parts { ws: &mut *self.ws, rs: self.rs, lists: self.lists, thumbs: &mut *self.thumbs, show_all: *self.show_all, link: &mut *self.link, clip: &mut *self.clip, scrub, asks: Vec::new() };
        view.show(ui, &mut parts, name);
        for ask in parts.asks {
            match ask {
                crate::mapview::Ask::OpenDef(k, id) => self.asks.push(Ask::Open(Tab::Def(k, id))),
                crate::mapview::Ask::OpenMap(m) => self.asks.push(Ask::Open(Tab::Map(m))),
                crate::mapview::Ask::Graph => self.asks.push(Ask::Open(Tab::Graph(name.to_string()))),
                crate::mapview::Ask::TestSetup => self.asks.push(Ask::TestSetup),
                crate::mapview::Ask::New(k) => self.asks.push(Ask::New(Some(k))),
                crate::mapview::Ask::PlayHere(tile, setup) => self.asks.push(Ask::Play(PlayFrom::Tile(name.to_string(), tile, setup))),
                crate::mapview::Ask::Usages(k, id) => {
                    self.asks.push(Ask::Run(format!("find.usages {} {id}", k.name())));
                    self.asks.push(Ask::Open(Tab::Def(k, id)));
                }
                crate::mapview::Ask::Try(tile, on, whose, what) => {
                    use editor_core::workspace::Selection;
                    use rustpg2d_engine::runtime::EventOwner;
                    let owner = match whose {
                        Selection::Entity(uid) => EventOwner::Entity(uid),
                        Selection::Zone(uid) => EventOwner::Zone(uid),
                        _ => EventOwner::Map,
                    };
                    self.asks.push(Ask::Try(name.to_string(), tile, Box::new((on, owner, what))));
                }
            }
        }
    }
}

pub(crate) fn held_step_id() -> egui::Id {
    egui::Id::unique("a step of the history held over frames")
}

/// A drag that changes definitions frame after frame is one step of the history: call this in
/// every frame it changed something, with what is dragged as the key (as typing on in one
/// field is one step).
pub fn hold_step(ctx: &egui::Context, key: impl std::hash::Hash + std::fmt::Debug) {
    ctx.data_mut(|d| d.insert_temp(held_step_id(), egui::Id::unique(key).value()));
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    /// A window around a new project whose game's name was changed and not saved.
    fn changed(dir: &std::path::Path) -> Shell {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        editor_core::files::create_project(dir).unwrap();
        let mut shell = Shell::around(Workspace::new(dir.to_path_buf(), Gpu::No).unwrap(), Default::default());
        shell.ws.project.config.name = "Changed".into();
        shell.ws.defs_dirty = true;
        shell
    }

    fn name_on_disk(dir: &std::path::Path) -> String {
        rustpg2d_engine::project::Project::load(dir).unwrap().config.name
    }

    fn harness(shell: Shell) -> Harness<'static, Shell> {
        Harness::builder().with_size(egui::vec2(900.0, 700.0)).build_ui_state(
            |ui, s: &mut Shell| {
                s.leaving_ui(ui.ctx());
                s.cleanup_ui(ui.ctx());
            },
            shell,
        )
    }

    #[test]
    fn leaving_with_unsaved_changes_asks_and_each_answer_does_what_it_says() {
        let base = std::env::temp_dir().join(format!("rustpg2d-leaving-{}", std::process::id()));
        // (The editor's settings are the test's own.)
        editor_core::files::config_dir_here(Some(base.join("config")));
        let dir = base.join("game");
        let mut shell = changed(&dir);
        let before = name_on_disk(&dir);
        // Stay: nothing saved, nothing dropped.
        shell.leave(Leaving::Close);
        assert_eq!(shell.leave_word(&[]), "close");
        let mut h = harness(shell);
        h.run();
        h.get_by_label("There are changes that aren't saved.");
        h.get_by_label("Stay").click();
        h.run();
        assert!(h.state().leaving.is_none() && h.state().ws.project_open && h.state().unsaved());
        // Don't save: the project is closed and its files are as they were.
        h.state_mut().leave(Leaving::Close);
        h.run();
        h.get_by_label("Don't save").click();
        h.run();
        assert!(!h.state().ws.project_open && !h.state().unsaved());
        assert_eq!(name_on_disk(&dir), before);
        // Save and go on: written, then closed.
        let mut h = harness(changed(&dir));
        h.state_mut().leave(Leaving::Close);
        h.run();
        h.get_by_label("Save and go on").click();
        h.run();
        assert!(!h.state().ws.project_open);
        assert_eq!(name_on_disk(&dir), "Changed");
        // Quitting: answered, the window may close; nothing is asked when all is saved.
        let mut h = harness(changed(&dir));
        h.state_mut().leave(Leaving::Quit);
        h.run();
        h.get_by_label("Don't save").click();
        h.run();
        assert!(h.state().quit_now && h.state().ws.project_open);
        let mut shell = changed(&dir);
        shell.ws.defs_dirty = false;
        shell.leave(Leaving::Close);
        assert!(shell.leaving.is_none() && !shell.ws.project_open, "saved: closed at once");
        editor_core::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn the_problems_list_says_when_the_project_changed_since_its_check() {
        let base = std::env::temp_dir().join(format!("rustpg2d-problems-outdated-{}", std::process::id()));
        editor_core::files::config_dir_here(Some(base.join("config")));
        let dir = base.join("game");
        let mut shell = changed(&dir);
        shell.ws.project.config.start_map = "nowhere".into();
        shell.ws.history_frame(None);
        // (The whole window: egui_dock's splitters have no names of their own. The Problems tab
        // is shown below the documents.)
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).with_accessibility_check(false).build_ui_state(|ui, s: &mut Shell| s.ui(ui, None, 60.0), shell);
        h.run();
        h.state_mut().run("check.problems").unwrap();
        h.run_steps(2);
        let start_map = |h: &Harness<Shell>| h.state().ws.problems.list.iter().any(|p| p.choice() == Some(editor_core::problems::Choice::StartMap));
        assert!(start_map(&h), "{:?}", h.state().ws.problems.list.iter().map(|p| p.line()).collect::<Vec<_>>());
        assert!(h.query_by_label("The project changed since this check.").is_none());
        // Mended by hand (not with the list's buttons): the list says it may be out of date.
        h.state_mut().set_setting("/start_map", "\"start\"").unwrap();
        h.run();
        h.get_by_label("The project changed since this check.");
        assert!(start_map(&h), "listed until checked again");
        h.get_by_label("Check again").click();
        h.run_steps(2);
        assert!(!start_map(&h) && h.query_by_label("The project changed since this check.").is_none());
        // A fix from the list checks again by itself: nothing said.
        h.state_mut().set_setting("/start_map", "\"nowhere\"").unwrap();
        h.state_mut().run("check.problems").unwrap();
        h.run();
        h.state_mut().ws.choose_setting(editor_core::problems::Choice::StartMap, "start").unwrap();
        h.run_steps(2);
        assert!(!start_map(&h) && h.query_by_label("The project changed since this check.").is_none());
        // An undo is a change too.
        h.state_mut().run("edit.undo").unwrap();
        h.run();
        h.get_by_label("The project changed since this check.");
        editor_core::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn the_clean_up_is_offered_once_on_leaving_and_keep_all_is_not_asked_again() {
        let base = std::env::temp_dir().join(format!("rustpg2d-leaving-cleanup-{}", std::process::id()));
        editor_core::files::config_dir_here(Some(base.join("config")));
        let dir = base.join("game");
        let mut shell = changed(&dir);
        // A sound nothing plays, saved with the rest.
        let file = base.join("old_bell.ogg");
        std::fs::write(&file, b"OggS").unwrap();
        shell.ws.make_from_file(RefKind::Sound, &file, "Old bell").unwrap();
        shell.ws.history_frame(None);
        shell.ws.save_all();
        // Closing: the clean-up first, then the project is closed.
        shell.leave(Leaving::Close);
        assert_eq!(shell.cleanup_word(&[]), "1/1:leaving\n[cleanup] x sound:old_bell 1 KB sounds/old_bell.ogg");
        let mut h = harness(shell);
        h.run();
        h.get_by_label("Before you go: this project has files nothing uses.");
        h.get_by_label("Keep all").click();
        h.run();
        assert!(h.state().cleanup.is_none() && !h.state().ws.project_open, "kept, then closed");
        // Opened again and closed: not asked again.
        let mut shell = Shell::around(Workspace::new(dir.clone(), Gpu::No).unwrap(), Default::default());
        shell.leave(Leaving::Close);
        assert!(shell.cleanup.is_none() && !shell.ws.project_open);
        // Unsaved changes are asked about first, the clean-up after: a second sound, unsaved name.
        let mut shell = Shell::around(Workspace::new(dir.clone(), Gpu::No).unwrap(), Default::default());
        let file = base.join("old_horn.ogg");
        std::fs::write(&file, b"OggS").unwrap();
        shell.ws.make_from_file(RefKind::Sound, &file, "Old horn").unwrap();
        shell.ws.history_frame(None);
        shell.ws.save_all();
        shell.ws.project.config.name = "Dropped".into();
        shell.ws.defs_dirty = true;
        shell.leave(Leaving::Quit);
        assert_eq!(shell.leave_word(&[]), "quit");
        assert!(shell.leave_word(&["discard".into()]).starts_with("- "));
        assert!(shell.cleanup_word(&[]).starts_with("1/1:leaving"), "offered after the answer");
        assert!(!shell.quit_now, "not yet");
        // Deleted: then the editor may quit.
        assert_eq!(shell.cleanup_word(&["delete".into()]), "closed");
        assert!(shell.quit_now && !dir.join("sounds/old_horn.ogg").exists());
        assert_eq!(rustpg2d_engine::project::Project::load(&dir).unwrap().config.name, "Changed", "the dropped name wasn't written");
        editor_core::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(base);
    }
}
