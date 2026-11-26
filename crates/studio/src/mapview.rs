//! A map's tab: the map drawn by the engine's renderer into a picture of its own (`scene.rs`)
//! with the tools' overlays, its camera and its selection; beside it what the tool offers, the
//! Outliner (everything on the map) and the Inspector (the selected thing's form). One
//! [`MapView`] per open map; several can be shown side by side.
//!
//! Changes go through `editor-core` (`mapedit.rs`, `doc.rs`): the view only says what was
//! pressed where.

use eframe::egui;
use eframe::egui_wgpu::RenderState;
use editor_core::doc::MapDoc;
use editor_core::mapedit::{entity_kind, kind_ref, Brush, MapSnap};
use editor_core::schema::{self, Shape};
use editor_core::workspace::{Selection, ViewOpts};
use editor_core::Workspace;
use gfx::input::Input;
use gfx::prelude::{vec2, Color, Rect, Vec2};
use rustpg2d_engine::decor::{Decor, DecorLayer};
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::map::{CollisionOverride, Map, ZoneKind};
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::render::{self, ground::GroundRenderer, lighting::LightPass, weather::WeatherFx};

use crate::scene::{zone_color, Look, Scene};
use crate::thumbs::Thumbs;

/// What the pointer does on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Terrain,
    Place,
    Zones,
    Collision,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Select, Tool::Terrain, Tool::Place, Tool::Zones, Tool::Collision];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Terrain => "Terrain",
            Tool::Place => "Place",
            Tool::Zones => "Zones",
            Tool::Collision => "Collision",
        }
    }

    fn key(self) -> egui::Key {
        match self {
            Tool::Select => egui::Key::V,
            Tool::Terrain => egui::Key::P,
            Tool::Place => egui::Key::N,
            Tool::Zones => egui::Key::Z,
            Tool::Collision => egui::Key::C,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Tool::Select => "Click something to select it, drag it to move it (scenery and zones: once they are selected); drag on empty ground to select what is inside. Shift or Ctrl+click adds to the selection.",
            Tool::Terrain => "Paint the ground with the material picked on the left. Shift erases, the right button picks the material under the pointer, [ and ] size the brush.",
            Tool::Place => "Pick what to put down on the left, then click the map: characters, objects, items, scenery, whole structures, scatter sets. Shift places off the grid, R turns it first.",
            Tool::Zones => "Drag a rectangle to make a zone of the kind picked on the left; drag a zone to move it, its corners to size it.",
            Tool::Collision => "Paint where nobody can walk: left button blocks tiles, Shift gives them back to the ground's own rule, right button opens them.",
        }
    }

    pub fn named(name: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(name))
    }
}

/// How the Terrain tool paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundMode {
    Brush,
    Line,
    Rect,
    Fill,
}

impl GroundMode {
    const ALL: [GroundMode; 4] = [GroundMode::Brush, GroundMode::Line, GroundMode::Rect, GroundMode::Fill];

    fn label(self) -> &'static str {
        match self {
            GroundMode::Brush => "Brush",
            GroundMode::Line => "Line",
            GroundMode::Rect => "Rectangle",
            GroundMode::Fill => "Fill",
        }
    }

    fn help(self) -> &'static str {
        match self {
            GroundMode::Brush => "Click or drag a brush: round, or square for straight edges",
            GroundMode::Line => "Drag from one end to the other: a road or a stream as wide as the brush",
            GroundMode::Rect => "Drag from one corner to the other: a filled rectangle (a floor, a carpet, a field)",
            GroundMode::Fill => "Click: the connected area of the same material (or of no ground)",
        }
    }

    pub fn named(name: &str) -> Option<GroundMode> {
        GroundMode::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(name) || (name.eq_ignore_ascii_case("rect") && *m == GroundMode::Rect))
    }
}

/// What the Select tool picks.
#[derive(Clone, Copy, Debug)]
pub struct Picks {
    pub things: bool,
    pub scenery: bool,
    pub zones: bool,
}

enum Drag {
    Pan,
    /// The selection follows the pointer: each thing as it is now and where it was (entities
    /// and scenery: feet, zones: corner tile), where the press was against the first one's feet.
    Move { from: Vec2, grab: Vec2, origs: Vec<(Selection, Vec2)>, before: Box<MapSnap>, moved: bool },
    /// A box over what gets selected, from this pixel. `pick`: what a click without a drag
    /// selects (a zone or scenery under the press: they are picked by a click, so that a drag
    /// inside a big one still draws a box).
    Box { from: Vec2, pick: Option<Selection> },
    /// A new zone from this tile (`at`: the pixel); `pick` as for the box.
    NewZone { from: (i32, i32), at: Vec2, pick: Option<Selection> },
    /// A zone's corner follows the pointer; `fixed` is the opposite corner (tile lines).
    Resize { uid: u32, fixed: (i32, i32), before: Box<MapSnap> },
    /// Collision painted tile by tile.
    Paint { last: (i32, i32), value: u8 },
    /// The ground's brush: where it was last, and the cells changed so far.
    Ground { last: Vec2, value: u8, cells: Vec<(i32, i32, u8)> },
    /// A line or a rectangle of ground from here, painted when the button goes up.
    GroundShape { from: Vec2, value: u8 },
    /// A row of scenery from here (props with a grid of their own: walls, fences).
    Row { from: Vec2 },
    /// A scatter set brushed on: where the last dab was.
    Scatter { last: Vec2, before: Box<MapSnap> },
}

/// What a map tab asks of the window.
pub enum Ask {
    OpenDef(RefKind, String),
    OpenMap(String),
    New(RefKind),
    /// Play the game with the hero on this tile of this map; true: with the test setup.
    PlayHere((i32, i32), bool),
    /// Show where a definition is used.
    Usages(RefKind, String),
}

/// What the window lends a map tab for a frame.
pub struct Parts<'a> {
    pub ws: &'a mut Workspace,
    pub rs: Option<&'a RenderState>,
    pub lists: &'a [(RefKind, Vec<(String, String)>)],
    pub thumbs: &'a mut Thumbs,
    pub show_all: bool,
    /// A transition zone waiting for its other side to be drawn on another map: (map, zone).
    pub link: &'a mut Option<(String, String)>,
    pub asks: Vec<Ask>,
}

pub struct MapView {
    /// The picture the map is drawn into (made with the first drawing: none without a
    /// graphics adapter), and its size in pixels.
    screen: Option<gfx::Screen>,
    size: (u32, u32),
    input: Input,
    texture: Option<(egui::TextureId, (u32, u32))>,
    /// The world point in the middle and the zoom; `None` until the view was shown once.
    cam: Option<(Vec2, f32)>,
    pub sel: Vec<Selection>,
    pub tool: Tool,
    pub opts: ViewOpts,
    ground: GroundRenderer,
    light_pass: Option<LightPass>,
    weather_fx: WeatherFx,
    drag: Option<Drag>,
    /// The world point under the pointer while it is over the map.
    hover: Option<Vec2>,
    /// Where the view was shown last (window points): the script clicks by tile through it.
    rect: egui::Rect,
    ppp: f32,
    /// The Place tool's pick (a character, prop, item, structure or scatter set) and the
    /// list's filter and kind.
    pub place: Option<(RefKind, String)>,
    place_filter: String,
    place_kind: Option<RefKind>,
    /// Quarter turns of the scenery about to be placed, its layer (`None`: by the prop), and
    /// whether each is mirrored at random.
    place_turn: u8,
    place_layer: Option<DecorLayer>,
    place_flip: bool,
    /// The scatter brush's radius (world pixels), and what its dabs are told apart by.
    pub scatter_size: f32,
    seed: u64,
    /// The Terrain tool: the material (`None`: none picked yet, "" = erase), how it paints,
    /// the brush.
    pub material: Option<String>,
    pub ground_mode: GroundMode,
    pub brush: Brush,
    /// What the Select tool picks.
    pub picks: Picks,
    /// The layers shown: the painted ground, scenery, characters objects and items (what is
    /// hidden can't be picked either).
    pub layers: [bool; 3],
    /// The Zones tool's kind (a variant of `ZoneKind`).
    pub zone_kind: String,
    outliner_filter: String,
    /// The Inspector's form holding something that can't be taken yet, and why.
    pending: Option<(Selection, serde_json::Value, String)>,
    /// A thing to bring into view with the next frame.
    reveal: Option<Selection>,
    /// *Save as structure*: the name typed, and whether the ground comes along.
    structure_name: String,
    structure_ground: bool,
    /// The tile the right button was last pressed on (the menu's *Play from here*).
    menu_tile: (i32, i32),
    /// The things under the last press, when the front one was selected already: a click
    /// without a drag goes on to the next.
    cycle: Option<Vec<Selection>>,
    /// Shift is held (the tools' previews show what it does).
    shift: bool,
    /// Milliseconds the last frame's drawing took (CPU).
    pub draw_ms: f32,
}

impl Default for MapView {
    fn default() -> MapView {
        MapView {
            screen: None,
            size: (16, 16),
            input: Input::new(),
            texture: None,
            cam: None,
            sel: Vec::new(),
            tool: Tool::Select,
            opts: ViewOpts::default(),
            ground: GroundRenderer::new(),
            light_pass: None,
            weather_fx: WeatherFx::default(),
            drag: None,
            hover: None,
            rect: egui::Rect::NOTHING,
            ppp: 1.0,
            place: None,
            place_filter: String::new(),
            place_kind: None,
            place_turn: 0,
            place_layer: None,
            place_flip: false,
            scatter_size: 48.0,
            seed: 1,
            material: None,
            ground_mode: GroundMode::Brush,
            brush: Brush { size: 16.0, square: false, snap: false },
            picks: Picks { things: true, scenery: true, zones: true },
            layers: [true; 3],
            zone_kind: "Trigger".into(),
            outliner_filter: String::new(),
            pending: None,
            reveal: None,
            structure_name: String::new(),
            structure_ground: true,
            menu_tile: (0, 0),
            cycle: None,
            shift: false,
            draw_ms: 0.0,
        }
    }
}

fn c32(c: Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied((c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, (c.a * 255.0) as u8)
}

fn rect_of(a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1)
}

/// The tiles from `a` to `b` (a painted line).
fn line(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
    (0..=n).map(|k| (a.0 + ((b.0 - a.0) as f32 * k as f32 / n as f32).round() as i32, a.1 + ((b.1 - a.1) as f32 * k as f32 / n as f32).round() as i32)).collect()
}

/// The open map called `name` (the one being worked on, or one beside it).
fn doc_of<'a>(ws: &'a Workspace, name: &str) -> Option<&'a MapDoc> {
    ws.doc.as_ref().filter(|d| d.map.name == name).or_else(|| ws.parked.get(name))
}

/// What a scenery layer is called.
fn layer_label(l: DecorLayer) -> &'static str {
    match l {
        DecorLayer::Flat => "On the ground",
        DecorLayer::Sorted => "Standing",
        DecorLayer::Above => "Above everyone",
    }
}

/// The name of the variant an enum's data is of (`"Spawn"`, `{"Portal": {…}}`).
fn variant_name(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(o) => o.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

/// What a zone says about itself on the map.
fn zone_label(z: &rustpg2d_engine::map::Zone) -> String {
    match &z.kind {
        ZoneKind::Portal { target_map, target_spawn } => format!("{} > {target_map}:{target_spawn}", z.name),
        ZoneKind::Transition { target_map, .. } if target_map.is_empty() => format!("{} (not linked)", z.name),
        ZoneKind::Transition { target_map, target_zone } => format!("{} <> {target_map}:{target_zone}", z.name),
        ZoneKind::Spawner { character, count, .. } => format!("{} [{count}x {character}]", z.name),
        ZoneKind::Encounter { monsters, difficulty, .. } => {
            let list: Vec<String> = monsters.iter().map(|m| if m.count > 1 { format!("{}x {}", m.count, m.character) } else { m.character.clone() }).collect();
            format!("{} [{}: {}]", z.name, difficulty.label(), list.join(", "))
        }
        ZoneKind::Drop { height_ft, .. } => format!("{} [drop {height_ft} ft]", z.name),
        _ => z.name.clone(),
    }
}

impl MapView {
    /// `map=… tool=… sel=… zoom=…` for the script's `status`.
    pub fn status(&self, ws: &Workspace, name: &str) -> String {
        let sel: Vec<String> = self.sel.iter().map(|s| match s {
            Selection::Entity(u) => format!("e{u}"),
            Selection::Zone(u) => format!("z{u}"),
            Selection::Scenery(d) => format!("s{},{}", d.x, d.y),
            _ => "-".into(),
        }).collect();
        let (e, z, d) = doc_of(ws, name).map_or((0, 0, 0), |d| (d.map.entities.len(), d.map.zones.len(), d.map.decor.len()));
        let place = self.place.as_ref().map_or("-".to_string(), |(_, id)| id.clone());
        let ground = format!("{}:{}:{}{}{}", match self.material.as_deref() {
            None => "-",
            Some("") => "erase",
            Some(m) => m,
        }, self.ground_mode.label().to_lowercase(), self.brush.size * 2.0 / ws.ts(), if self.brush.square { ":square" } else { "" }, if self.brush.snap { ":snap" } else { "" });
        format!("map={name} tool={} sel=[{}] entities={e} zones={z} decor={d} zoom={:.2} place={place} zonekind={} ground={ground} canvas_ms={:.1}", self.tool.label().to_lowercase(), sel.join(","), self.cam.map_or(0.0, |c| c.1), self.zone_kind, self.draw_ms)
    }

    /// Selects a thing and brings it into view (a search hit, a problem's place).
    pub fn select(&mut self, sel: Selection) {
        self.sel = vec![sel];
        self.reveal = Some(sel);
    }

    /// The world point in the middle of the view.
    pub fn center(&self) -> Vec2 {
        self.cam.map_or(Vec2::ZERO, |c| c.0)
    }

    /// Looks at world point `center` at `zoom` (the script's `mapview`).
    pub fn look_at(&mut self, center: Vec2, zoom: f32) {
        self.cam = Some((center, zoom.clamp(0.02, 16.0)));
    }

    fn look(&self) -> Look {
        let (center, zoom) = self.cam.unwrap_or((Vec2::ZERO, 1.0));
        Look { center, zoom, screen: vec2(self.size.0 as f32, self.size.1 as f32), topdown: self.opts.topdown }
    }

    /// The window position (points) showing the middle of tile `(x, y)` (fractions allowed):
    /// where the script's `mapclick` clicks.
    pub fn window_pos(&self, ts: f32, x: f32, y: f32) -> egui::Pos2 {
        let s = self.look().to_screen(vec2((x + 0.5) * ts, (y + 0.5) * ts));
        self.rect.min + egui::vec2(s.x, s.y) / self.ppp
    }

    fn to_points(&self, px: Vec2) -> egui::Pos2 {
        self.rect.min + egui::vec2(px.x, px.y) / self.ppp
    }

    /// The whole tab.
    pub fn show(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        if doc_of(p.ws, name).is_none() {
            ui.weak(format!("Map \"{name}\" isn't open."));
            if p.ws.maps.iter().any(|m| m == name) && ui.button("Open it").clicked() {
                p.ws.open_map(name);
            }
            return;
        }
        // Things that went away (undone, deleted elsewhere) leave the selection.
        if let Some(doc) = doc_of(p.ws, name) {
            self.sel.retain(|s| match s {
                Selection::Entity(u) => doc.map.entity(*u).is_some(),
                Selection::Zone(u) => doc.map.zones.iter().any(|z| z.uid == *u),
                Selection::Scenery(d) => doc.map.decor.contains(d),
                _ => false,
            });
        }
        egui::Panel::top(egui::Id::unique(("map bar", name))).show(ui, |ui| self.bar(ui, p, name));
        egui::Panel::right(egui::Id::unique(("map side", name))).default_size(350.0).resizable(true).show(ui, |ui| self.side(ui, p, name));
        if self.tool != Tool::Select {
            egui::Panel::left(egui::Id::unique(("map tool", name))).default_size(210.0).resizable(true).show(ui, |ui| self.tool_panel(ui, p));
        }
        self.canvas(ui, p, name);
        if p.ws.doc.as_ref().is_some_and(|d| d.map.name == name) {
            p.ws.sel = self.sel.first().copied().unwrap_or_default();
        }
    }

    fn bar(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        ui.horizontal_wrapped(|ui| {
            for t in Tool::ALL {
                if ui.selectable_label(self.tool == t, t.label()).on_hover_text(format!("{} ({})", t.help(), t.key().name())).clicked() {
                    self.tool = t;
                }
            }
            if self.tool == Tool::Select {
                ui.separator();
                ui.menu_button("Picks", |ui| {
                    ui.checkbox(&mut self.picks.things, "Characters, objects and items");
                    ui.checkbox(&mut self.picks.scenery, "Scenery").on_hover_text("Trees, walls, buildings and everything else that only stands there");
                    ui.checkbox(&mut self.picks.zones, "Zones").on_hover_text("Where zone areas are shown");
                })
                .response
                .on_hover_text("What clicks and boxes select");
            }
            ui.separator();
            ui.menu_button("Layers", |ui| {
                ui.checkbox(&mut self.layers[0], "The painted ground");
                ui.checkbox(&mut self.layers[1], "Scenery");
                ui.checkbox(&mut self.layers[2], "Characters, objects and items");
            })
            .response
            .on_hover_text("What of the map is drawn; what is hidden can't be picked either");
            ui.weak("Show");
            ui.toggle_value(&mut self.opts.grid, "Grid").on_hover_text("The tiles' lines (G)");
            ui.toggle_value(&mut self.opts.zones, "Zone areas").on_hover_text("Zones and their names");
            ui.toggle_value(&mut self.opts.collision, "Blocked tiles").on_hover_text("Where nobody can walk: red = blocked, yellow = low (water), green = opened by hand");
            ui.toggle_value(&mut self.opts.lighting, "Light").on_hover_text("The map's darkness and lights at the hour beside it");
            if self.opts.lighting {
                ui.add(egui::Slider::new(&mut self.opts.hour, 0.0..=24.0).fixed_decimals(0).suffix(" h")).on_hover_text("The hour of the day shown");
            }
            ui.toggle_value(&mut self.opts.weather, "Weather").on_hover_text("The map's weather");
            ui.toggle_value(&mut self.opts.topdown, "From above").on_hover_text("Straight from above instead of the game's view");
            ui.separator();
            let zoom = self.cam.map_or(1.0, |c| c.1);
            if ui.button("-").on_hover_text("Zoom out").clicked() {
                self.cam = self.cam.map(|(c, z)| (c, (z / 1.25).max(0.02)));
            }
            ui.label(format!("{:.0}%", zoom * 100.0));
            if ui.button("+").on_hover_text("Zoom in").clicked() {
                self.cam = self.cam.map(|(c, z)| (c, (z * 1.25).min(16.0)));
            }
            if ui.button("Fit").on_hover_text("The whole map (Home)").clicked() {
                self.fit(p.ws, name);
            }
            if doc_of(p.ws, name).is_some_and(|d| d.is_dirty()) {
                ui.weak("changed");
            }
            if let Some((map, zone)) = p.link.clone() {
                ui.separator();
                ui.colored_label(ui.visuals().warn_fg_color, format!("{map}: {zone} waits for its other side")).on_hover_text("Open another map and draw a Transition zone there (Zones tool): the two become each other's other side.");
                if ui.small_button("Stop").clicked() {
                    *p.link = None;
                }
            }
        });
    }

    fn fit(&mut self, ws: &Workspace, name: &str) {
        let Some(doc) = doc_of(ws, name) else { return };
        let ts = ws.ts();
        let (w, h) = (doc.map.width as f32 * ts, doc.map.height as f32 * ts);
        let look = self.look();
        self.cam = Some((vec2(w / 2.0, h / 2.0), Look::fit(w, h, look.screen, look.topdown).clamp(0.02, 16.0)));
    }

    // ------------------------------------------------------------ the tool's panel

    fn tool_panel(&mut self, ui: &mut egui::Ui, p: &mut Parts) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(self.tool.label()).strong());
        ui.weak(self.tool.help());
        ui.separator();
        match self.tool {
            Tool::Select | Tool::Collision => {}
            Tool::Terrain => self.terrain_panel(ui, p),
            Tool::Place => self.place_panel(ui, p),
            Tool::Zones => {
                let Some(Shape::Choice(variants)) = schema::zone().at("/kind").map(|n| n.shape()) else { return };
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for v in &variants {
                        let r = ui.selectable_label(self.zone_kind == v.name, schema::variant_label(v.name));
                        let help = v.help();
                        let r = if help.is_empty() { r } else { r.on_hover_text(help) };
                        if r.clicked() {
                            self.zone_kind = v.name.to_string();
                        }
                    }
                });
            }
        }
    }

    fn terrain_panel(&mut self, ui: &mut egui::Ui, p: &mut Parts) {
        let ts = p.ws.ts();
        ui.horizontal_wrapped(|ui| {
            for m in GroundMode::ALL {
                if ui.selectable_label(self.ground_mode == m, m.label()).on_hover_text(m.help()).clicked() {
                    self.ground_mode = m;
                }
            }
        });
        if matches!(self.ground_mode, GroundMode::Brush | GroundMode::Line) {
            let mut tiles = self.brush.size * 2.0 / ts;
            let r = ui.add(egui::Slider::new(&mut tiles, 0.25..=8.0).step_by(0.25).suffix(" tiles")).on_hover_text("The brush's width; [ and ] change it");
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "Brush width"));
            if r.changed() {
                self.brush.size = tiles * ts / 2.0;
            }
        }
        ui.horizontal_wrapped(|ui| {
            if self.ground_mode == GroundMode::Brush {
                ui.selectable_value(&mut self.brush.square, false, "Round");
                ui.selectable_value(&mut self.brush.square, true, "Square");
            }
            if self.ground_mode != GroundMode::Fill {
                ui.checkbox(&mut self.brush.snap, "On tiles").on_hover_text("Squares, rectangles and line ends lie on whole tiles (else on the ground's cells, an eighth of a tile)");
            }
        });
        ui.separator();
        let mut pick = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if ui.add(egui::Button::selectable(self.material.as_deref() == Some(""), "Erase (no ground)").truncate()).on_hover_text("Takes the painted ground away (Shift does it with any material)").clicked() {
                pick = Some(String::new());
            }
            let Some((_, list)) = p.lists.iter().find(|(k, _)| *k == RefKind::Material) else { return };
            for (id, label) in list {
                let r = ui
                    .horizontal(|ui| {
                        if let Some(rs) = p.rs {
                            crate::thumbs::boxed(ui, 30.0, || p.thumbs.of(p.ws, rs, RefKind::Material, id));
                        }
                        ui.add(egui::Button::selectable(self.material.as_deref() == Some(id.as_str()), if label.is_empty() { id.as_str() } else { label.as_str() }).truncate()).on_hover_text(id)
                    })
                    .inner;
                if r.clicked() {
                    pick = Some(id.clone());
                }
                if r.double_clicked() {
                    p.asks.push(Ask::OpenDef(RefKind::Material, id.clone()));
                }
            }
        });
        if let Some(pick) = pick {
            self.material = Some(pick);
        }
    }

    fn place_panel(&mut self, ui: &mut egui::Ui, p: &mut Parts) {
        const KINDS: [(RefKind, &str); 5] = [(RefKind::Character, "Characters"), (RefKind::Prop, "Props"), (RefKind::Item, "Items"), (RefKind::Structure, "Structures"), (RefKind::Scatter, "Scatter sets")];
        ui.add(egui::TextEdit::singleline(&mut self.place_filter).hint_text("Search").desired_width(f32::INFINITY)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search what to place"));
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.place_kind.is_none(), "All").clicked() {
                self.place_kind = None;
            }
            for (kind, label) in KINDS {
                if ui.selectable_label(self.place_kind == Some(kind), label).clicked() {
                    self.place_kind = Some(kind);
                }
            }
        });
        if let Some((kind, id)) = self.place.clone() {
            ui.horizontal(|ui| {
                ui.weak(format!("Placing {id}"));
                if ui.small_button("Stop").on_hover_text("Esc, or right-click on the map").clicked() {
                    self.place = None;
                }
            });
            match kind {
                RefKind::Prop if !p.ws.prop_acts(&id) => {
                    let def = p.ws.project.db.prop(&id);
                    ui.weak("Scenery: it stands there. Select it and Make interactive to give it a name and events.");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Layer").on_hover_text("On the ground under everyone, standing among everyone by its feet, or above everyone (treetops, roofs)");
                        ui.selectable_value(&mut self.place_layer, None, "Its own");
                        for l in DecorLayer::ALL {
                            ui.selectable_value(&mut self.place_layer, Some(l), layer_label(l));
                        }
                    });
                    ui.checkbox(&mut self.place_flip, "Mirror at random").on_hover_text("Each one placed is mirrored left to right half of the time");
                    if def.is_some_and(|d| d.turns()) && ui.button(format!("Facing {} · Turn", rustpg2d_engine::decor::turn_facing(self.place_turn).label().to_lowercase())).on_hover_text("R").clicked() {
                        self.place_turn = (self.place_turn + 1) % 4;
                    }
                    if def.is_some_and(|d| d.snap > 0) {
                        ui.weak("Drag for a row of them.");
                    }
                }
                RefKind::Prop => {
                    ui.weak("An object: it does something (events, states, things in it), so it is placed with a name to give and events of its own.");
                }
                RefKind::Scatter => {
                    let ts = p.ws.ts();
                    let mut tiles = self.scatter_size * 2.0 / ts;
                    let r = ui.add(egui::Slider::new(&mut tiles, 0.5..=16.0).step_by(0.5).suffix(" tiles")).on_hover_text("The brush's width; [ and ] change it");
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "Brush width"));
                    if r.changed() {
                        self.scatter_size = tiles * ts / 2.0;
                    }
                }
                _ => {}
            }
        }
        ui.separator();
        let f = self.place_filter.trim().to_lowercase();
        let mut pick = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (kind, title) in KINDS {
                if self.place_kind.is_some_and(|k| k != kind) {
                    continue;
                }
                let Some((_, list)) = p.lists.iter().find(|(k, _)| *k == kind) else { continue };
                let rows: Vec<&(String, String)> = list.iter().filter(|(id, label)| f.is_empty() || id.to_lowercase().contains(&f) || label.to_lowercase().contains(&f)).collect();
                if rows.is_empty() {
                    continue;
                }
                if self.place_kind.is_none() {
                    ui.weak(title);
                }
                for (id, label) in rows {
                    let armed = self.place.as_ref().is_some_and(|(k, i)| *k == kind && i == id);
                    let r = ui
                        .horizontal(|ui| {
                            if let (Some(rs), true) = (p.rs, crate::thumbs::has_pictures(kind)) {
                                crate::thumbs::boxed(ui, 30.0, || p.thumbs.of(p.ws, rs, kind, id));
                            }
                            ui.add(egui::Button::selectable(armed, if label.is_empty() { id.as_str() } else { label.as_str() }).truncate()).on_hover_text(format!("{} · {id}", kind.label()))
                        })
                        .inner;
                    if r.clicked() {
                        pick = Some((kind, id.clone()));
                    }
                    if r.double_clicked() {
                        p.asks.push(Ask::OpenDef(kind, id.clone()));
                    }
                }
            }
        });
        if let Some(pick) = pick {
            self.place = Some(pick);
            self.place_turn = 0;
        }
    }

    // ------------------------------------------------------------ outliner and inspector

    fn side(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        let half = (ui.available_height() * 0.38).max(120.0);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("On this map").strong());
            ui.add(egui::TextEdit::singleline(&mut self.outliner_filter).hint_text("Search").desired_width(f32::INFINITY)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search this map"));
        });
        let f = self.outliner_filter.trim().to_lowercase();
        let mut click: Option<(Selection, bool)> = None;
        let mut pick_scenery: Option<u16> = None;
        if let Some(doc) = doc_of(p.ws, name) {
            let ws = &*p.ws;
            egui::ScrollArea::vertical().id_salt("outliner").max_height(half).auto_shrink([false, false]).show(ui, |ui| {
                let groups = [("Characters", EntityKind::Character), ("Objects", EntityKind::Prop), ("Items", EntityKind::Item)];
                for (title, kind) in groups {
                    let rows: Vec<(u32, String, &str)> = doc
                        .map
                        .entities
                        .iter()
                        .filter(|e| e.kind == kind)
                        .map(|e| (e.uid, if e.name.is_empty() { ws.def_label(e.kind, &e.def) } else { e.name.clone() }, e.def.as_str()))
                        .filter(|(_, label, def)| f.is_empty() || label.to_lowercase().contains(&f) || def.to_lowercase().contains(&f))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    egui::CollapsingHeader::new(format!("{title} ({})", rows.len())).id_salt(title).default_open(kind == EntityKind::Character).open((!f.is_empty()).then_some(true)).show(ui, |ui| {
                        for (uid, label, def) in rows {
                            let s = Selection::Entity(uid);
                            let r = ui.add(egui::Button::selectable(self.sel.contains(&s), label).truncate()).on_hover_text(format!("{def} · number {uid}"));
                            if r.clicked() {
                                click = Some((s, ui.input(|i| i.modifiers.command || i.modifiers.shift)));
                            }
                        }
                    });
                }
                // Scenery by what it is (there is too much of it to list each piece).
                let mut scenery: std::collections::BTreeMap<(String, u16), usize> = std::collections::BTreeMap::new();
                for d in &doc.map.decor {
                    if let Some(id) = doc.map.decor_defs.get(d.def as usize) {
                        let label = ws.def_label(EntityKind::Prop, id);
                        if f.is_empty() || label.to_lowercase().contains(&f) || id.to_lowercase().contains(&f) {
                            *scenery.entry((label, d.def)).or_default() += 1;
                        }
                    }
                }
                if !scenery.is_empty() {
                    egui::CollapsingHeader::new(format!("Scenery ({})", scenery.values().sum::<usize>())).id_salt("scenery").default_open(false).open((!f.is_empty()).then_some(true)).show(ui, |ui| {
                        for ((label, slot), n) in &scenery {
                            let chosen = self.sel.iter().any(|s| matches!(s, Selection::Scenery(d) if d.def == *slot));
                            if ui.add(egui::Button::selectable(chosen, format!("{label} x {n}")).truncate()).on_hover_text("Selects every one of them on this map").clicked() {
                                pick_scenery = Some(*slot);
                            }
                        }
                    });
                }
                let zones: Vec<&rustpg2d_engine::map::Zone> = doc.map.zones.iter().filter(|z| f.is_empty() || z.name.to_lowercase().contains(&f)).collect();
                if !zones.is_empty() {
                    egui::CollapsingHeader::new(format!("Zones ({})", zones.len())).id_salt("zones").default_open(true).open((!f.is_empty()).then_some(true)).show(ui, |ui| {
                        for z in zones {
                            let s = Selection::Zone(z.uid);
                            let kind = schema::variant_label(&variant_name(&serde_json::to_value(&z.kind).unwrap_or_default()));
                            let r = ui.add(egui::Button::selectable(self.sel.contains(&s), z.name.as_str()).truncate()).on_hover_text(format!("{kind} · {} x {} tiles at {}, {}", z.w, z.h, z.x, z.y));
                            if r.clicked() {
                                click = Some((s, ui.input(|i| i.modifiers.command || i.modifiers.shift)));
                            }
                        }
                    });
                }
            });
        }
        if let Some((s, add)) = click {
            if add {
                match self.sel.iter().position(|x| *x == s) {
                    Some(k) => drop(self.sel.remove(k)),
                    None => self.sel.push(s),
                }
            } else {
                self.select(s);
            }
        }
        if let (Some(slot), Some(doc)) = (pick_scenery, doc_of(p.ws, name)) {
            self.sel = doc.map.decor.iter().filter(|d| d.def == slot).map(|d| Selection::Scenery(*d)).collect();
        }
        ui.separator();
        // (Both ways: a wide row scrolls instead of widening the panel.)
        egui::ScrollArea::both().id_salt("inspector").auto_shrink([false, false]).show(ui, |ui| {
            crate::form::set_narrow(ui.ctx(), true);
            self.inspector(ui, p, name);
            crate::form::set_narrow(ui.ctx(), false);
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        let Some(doc) = doc_of(p.ws, name) else { return };
        let ts = p.ws.ts();
        match self.sel.as_slice() {
            [] => {
                let m = &doc.map;
                ui.label(egui::RichText::new(name).strong());
                ui.weak(format!("{} x {} tiles · {} characters, objects and items · {} pieces of scenery · {} zones", m.width, m.height, m.entities.len(), m.decor.len(), m.zones.len()));
                ui.add_space(6.0);
                ui.weak("Select something on the map or in the list above to see and change it here. Below: the map itself.");
                ui.separator();
                // The map's own settings (the map being worked on: a map beside it shows its facts).
                let current = p.ws.doc.as_ref().is_some_and(|d| d.map.name == name);
                let Some(saved) = current.then(|| p.ws.map_settings_value()).flatten() else { return };
                let mut value = self.pending.as_ref().filter(|(s, ..)| *s == Selection::None).map_or_else(|| saved.clone(), |(_, v, _)| v.clone());
                let mut pics = p.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *p.thumbs, ws: &*p.ws, rs });
                let pictures = pics.as_mut().map(|x| x as &mut dyn crate::form::Pictures);
                let mut cx = crate::form::Cx { lists: p.lists, maps: &p.ws.maps, show_all: p.show_all, compact: true, pictures, refs: Some(&p.ws.refs), asks: Vec::new() };
                let changed = crate::form::sections_ui(ui, &mut cx, schema::map_settings(), &mut value, schema::MAP_SECTIONS);
                for ask in cx.asks {
                    match ask {
                        crate::form::Ask::Open(RefKind::Map, map) => p.asks.push(Ask::OpenMap(map)),
                        crate::form::Ask::Open(k, i) => p.asks.push(Ask::OpenDef(k, i)),
                        crate::form::Ask::New(k) => p.asks.push(Ask::New(k)),
                    }
                }
                if changed {
                    self.pending = p.ws.set_map_settings_value(value.clone()).err().map(|e| (Selection::None, value, e));
                }
                if let Some((_, _, why)) = self.pending.as_ref().filter(|(s, ..)| *s == Selection::None) {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Not taken yet: {why}"));
                    if ui.button("Back to how it was").clicked() {
                        self.pending = None;
                    }
                }
            }
            [Selection::Scenery(d)] => {
                let d = *d;
                let Some(prop) = doc.map.decor_defs.get(d.def as usize).cloned() else { return };
                let def = p.ws.project.db.prop(&prop);
                let (turns, states): (bool, Vec<String>) = def.map_or((false, Vec::new()), |q| (q.turns(), q.states.iter().map(|s| s.name.clone()).collect()));
                ui.horizontal(|ui| {
                    if let Some(rs) = p.rs {
                        crate::thumbs::boxed(ui, 44.0, || p.thumbs.of(p.ws, rs, RefKind::Prop, &prop));
                    }
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(p.ws.def_label(EntityKind::Prop, &prop)).heading());
                        ui.weak(format!("Scenery at tile {}, {}", (d.x as f32 / ts).floor(), ((d.y as f32 - 0.5) / ts).floor()));
                    });
                });
                let mut act = None;
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Edit definition").on_hover_text(format!("Opens {prop}: what every one of them is")).clicked() {
                        p.asks.push(Ask::OpenDef(RefKind::Prop, prop.clone()));
                    }
                    if turns && ui.button("Turn").on_hover_text("A quarter further (R)").clicked() {
                        act = Some("turn");
                    }
                    if ui.button("Duplicate").on_hover_text("Ctrl+D").clicked() {
                        act = Some("duplicate");
                    }
                    if ui.button("Delete").on_hover_text("Del").clicked() {
                        act = Some("delete");
                    }
                });
                ui.add_space(4.0);
                ui.weak("Scenery only stands there: no name, no events.");
                if ui.button("Make interactive").on_hover_text("Turns it into an object of the map: with a name to give, events, states, things in it. The same prop on the same spot.").clicked() {
                    act = Some("interactive");
                }
                if let Some(act) = act {
                    self.act(p, name, act);
                    return;
                }
                ui.separator();
                let mut new = d;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Layer").on_hover_text("On the ground under everyone, standing among everyone by its feet, or above everyone (treetops, roofs)");
                    for l in DecorLayer::ALL {
                        ui.selectable_value(&mut new.layer, l, layer_label(l));
                    }
                });
                ui.checkbox(&mut new.flip, "Mirrored").on_hover_text("Left to right");
                if states.len() > 1 {
                    let mut k = d.state_index().min(states.len() - 1);
                    egui::ComboBox::from_label("Shown in state").selected_text(states[k].as_str()).show_index(ui, &mut k, states.len(), |i| states[i].clone());
                    let turn = d.turn();
                    new.state = k as u8;
                    new.set_turn(turn);
                }
                if new != d {
                    p.ws.open_map(name);
                    if p.ws.set_scenery(d, new) {
                        self.sel = vec![Selection::Scenery(new)];
                    }
                }
            }
            [one] => {
                let one = *one;
                let (title, sub, def, value) = match one {
                    Selection::Entity(uid) => {
                        let Some(e) = doc.map.entity(uid) else { return };
                        let rk = kind_ref(e.kind);
                        let title = if e.name.is_empty() { p.ws.def_label(e.kind, &e.def) } else { e.name.clone() };
                        let tile = ((e.x / ts).floor() as i32, ((e.y - 0.5) / ts).floor() as i32);
                        (title, format!("{} at tile {}, {}", rk.label(), tile.0, tile.1), Some((rk, e.def.clone())), p.ws.entity_value(uid))
                    }
                    Selection::Zone(uid) => {
                        let Some(z) = doc.map.zones.iter().find(|z| z.uid == uid) else { return };
                        (z.name.clone(), "Zone".to_string(), None, p.ws.zone_value(uid))
                    }
                    _ => return,
                };
                let current = p.ws.doc.as_ref().is_some_and(|d| d.map.name == name);
                // (Data of a map shown beside the one being worked on is read from it directly.)
                let value = if current { value } else { self.value_of(doc, one) };
                let Some(saved) = value else { return };
                ui.horizontal(|ui| {
                    if let (Some((rk, id)), Some(rs)) = (&def, p.rs) {
                        crate::thumbs::boxed(ui, 44.0, || p.thumbs.of(p.ws, rs, *rk, id));
                    }
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&title).heading());
                        ui.weak(sub);
                    });
                });
                let mut act = None;
                ui.horizontal_wrapped(|ui| {
                    if let Some((rk, id)) = &def {
                        if ui.button("Edit definition").on_hover_text(format!("Opens {id}: what every one of them is. What is set here is only this one's.")).clicked() {
                            p.asks.push(Ask::OpenDef(*rk, id.clone()));
                        }
                        if ui.button("Turn").on_hover_text("A quarter further (R)").clicked() {
                            act = Some("turn");
                        }
                        if let (RefKind::Prop, Selection::Entity(uid)) = (*rk, one) {
                            let why = p.ws.keeps_from_scenery(uid);
                            let r = ui.add_enabled(why.is_none(), egui::Button::new("Make scenery"));
                            let r = r.on_hover_text("Turns it into scenery: it only stands there, without a name or events").on_disabled_hover_text(format!("It can't be scenery: {}", why.unwrap_or_default()));
                            if r.clicked() {
                                act = Some("scenery");
                            }
                        }
                    }
                    if ui.button("Duplicate").on_hover_text("Ctrl+D").clicked() {
                        act = Some("duplicate");
                    }
                    if ui.button("Delete").on_hover_text("Del").clicked() {
                        act = Some("delete");
                    }
                    let transition = saved.pointer("/kind/Transition/target_map").is_some();
                    if transition && ui.button("Link…").on_hover_text("Draw a Transition zone on another map after this: the two become each other's other side").clicked() {
                        *p.link = Some((name.to_string(), title.clone()));
                        self.tool = Tool::Zones;
                        self.zone_kind = "Transition".into();
                    }
                });
                if let Some(act) = act {
                    self.act(p, name, act);
                    return;
                }
                ui.separator();
                // The form shows what was typed last, also while that can't be taken yet.
                let mut value = self.pending.as_ref().filter(|(s, ..)| *s == one).map_or_else(|| saved.clone(), |(_, v, _)| v.clone());
                let (node, sections) = match one {
                    Selection::Entity(_) => (schema::entity(), schema::entity_sections(saved.get("kind").and_then(|k| k.as_str()).unwrap_or(""))),
                    _ => (schema::zone(), schema::ZONE_SECTIONS),
                };
                let mut pics = p.rs.map(|rs| crate::thumbs::Pics { thumbs: &mut *p.thumbs, ws: &*p.ws, rs });
                let pictures = pics.as_mut().map(|x| x as &mut dyn crate::form::Pictures);
                let mut cx = crate::form::Cx { lists: p.lists, maps: &p.ws.maps, show_all: p.show_all, compact: true, pictures, refs: Some(&p.ws.refs), asks: Vec::new() };
                let changed = crate::form::sections_ui(ui, &mut cx, node, &mut value, sections);
                for ask in cx.asks {
                    match ask {
                        crate::form::Ask::Open(RefKind::Map, map) => p.asks.push(Ask::OpenMap(map)),
                        crate::form::Ask::Open(k, i) => p.asks.push(Ask::OpenDef(k, i)),
                        crate::form::Ask::New(k) => p.asks.push(Ask::New(k)),
                    }
                }
                if changed {
                    p.ws.open_map(name);
                    let r = match one {
                        Selection::Entity(uid) => p.ws.set_entity_value(uid, value.clone()),
                        Selection::Zone(uid) => p.ws.set_zone_value(uid, value.clone()),
                        _ => Ok(()),
                    };
                    self.pending = r.err().map(|e| (one, value, e));
                }
                if let Some((_, _, why)) = self.pending.as_ref().filter(|(s, ..)| *s == one) {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Not taken yet: {why}"));
                    if ui.button("Back to how it was").clicked() {
                        self.pending = None;
                    }
                }
            }
            many => {
                ui.label(egui::RichText::new(format!("{} things selected", many.len())).strong());
                ui.weak("Drag one of them to move them all.");
                let mut act = None;
                ui.horizontal(|ui| {
                    if ui.button("Duplicate").clicked() {
                        act = Some("duplicate");
                    }
                    if ui.button("Delete").clicked() {
                        act = Some("delete");
                    }
                });
                if let Some(act) = act {
                    self.act(p, name, act);
                    return;
                }
                // What is selected, with the ground under it, as a structure to place again.
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Save as structure").strong());
                let rect = p.ws.doc.as_ref().filter(|d| d.map.name == name).and_then(|_| p.ws.tiles_of(many));
                let Some(rect) = rect else { return };
                ui.weak(format!("{} x {} tiles with everything standing on them: placed again as one piece (Place, Structures).", rect.2, rect.3));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.structure_name).hint_text("its name").desired_width(150.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "The structure's name"));
                    ui.checkbox(&mut self.structure_ground, "With the ground").on_hover_text("Off for things that sit on any ground (a market stall, a camp)");
                });
                if ui.add_enabled(!self.structure_name.trim().is_empty(), egui::Button::new("Save as structure")).clicked() {
                    match p.ws.save_as_structure(rect, &self.structure_name, self.structure_ground) {
                        Ok(id) => {
                            p.ws.set_status(format!("Saved as the structure {id}"));
                            self.structure_name.clear();
                            p.asks.push(Ask::OpenDef(RefKind::Structure, id));
                        }
                        Err(e) => p.ws.set_status(e),
                    }
                }
            }
        }
    }

    fn value_of(&self, doc: &MapDoc, sel: Selection) -> Option<serde_json::Value> {
        match sel {
            Selection::Entity(uid) => doc.map.entity(uid).and_then(|e| serde_json::to_value(e).ok()),
            Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid).and_then(|z| serde_json::to_value(z).ok()),
            _ => None,
        }
    }

    /// Something done with the selection: `delete`, `duplicate`, `turn`.
    pub fn act(&mut self, p: &mut Parts, name: &str, what: &str) {
        p.ws.open_map(name);
        match what {
            "delete" => {
                let n = p.ws.delete_objects(&self.sel);
                if n > 0 {
                    p.ws.set_status(format!("Deleted {n}"));
                }
                self.sel.clear();
            }
            "duplicate" => {
                let made = p.ws.duplicate_objects(&self.sel);
                if !made.is_empty() {
                    self.sel = made;
                }
            }
            "turn" => {
                for k in 0..self.sel.len() {
                    let r = match self.sel[k] {
                        Selection::Entity(uid) => p.ws.turn_entity(uid),
                        Selection::Scenery(d) => p.ws.turn_scenery(d).map(|new| self.sel[k] = Selection::Scenery(new)),
                        _ => Ok(()),
                    };
                    if let Err(e) = r {
                        p.ws.set_status(e);
                    }
                }
            }
            "interactive" => {
                if let [Selection::Scenery(d)] = self.sel.as_slice() {
                    match p.ws.make_interactive(*d) {
                        Ok(uid) => self.sel = vec![Selection::Entity(uid)],
                        Err(e) => p.ws.set_status(e),
                    }
                }
            }
            "scenery" => {
                if let [Selection::Entity(uid)] = self.sel.as_slice() {
                    match p.ws.make_scenery(*uid) {
                        Ok(d) => self.sel = vec![Selection::Scenery(d)],
                        Err(e) => p.ws.set_status(e),
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ the map itself

    fn canvas(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.interact(rect, egui::Id::unique(("map view", name)), egui::Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, format!("Map {name}")));
        let ppp = ui.ctx().pixels_per_point();
        let size = ((rect.width() * ppp).round().max(16.0) as u32, (rect.height() * ppp).round().max(16.0) as u32);
        (self.size, self.rect, self.ppp) = (size, rect, ppp);
        let ts = p.ws.ts();
        if self.cam.is_none() {
            // The first look: the whole map when it is small, else its middle at a size where
            // things can be told apart.
            self.fit(p.ws, name);
            self.cam = self.cam.map(|(c, z)| (c, z.clamp(0.6, 2.0)));
        }
        if let Some(what) = self.reveal.take() {
            let at = doc_of(p.ws, name).and_then(|d| match what {
                Selection::Entity(uid) => d.map.entity(uid).map(|e| vec2(e.x, e.y)),
                Selection::Zone(uid) => d.map.zones.iter().find(|z| z.uid == uid).map(|z| z.pixel_rect(ts).center()),
                _ => None,
            });
            if let (Some(at), Some((c, z))) = (at, self.cam) {
                let s = self.look().to_screen(at);
                let inside = s.x > 40.0 && s.y > 40.0 && s.x < size.0 as f32 - 40.0 && s.y < size.1 as f32 - 40.0;
                self.cam = Some((if inside { c } else { at }, z));
            }
        }
        self.input_frame(ui, p, name, &resp);
        self.draw(p.ws, name);
        // egui shows the picture; a new size is a new texture.
        if let (Some(rs), Some(screen)) = (p.rs, &self.screen) {
            let view = screen.texture().view();
            let id = match self.texture {
                Some((id, at)) if at == size => id,
                Some((id, _)) => {
                    rs.renderer.write().update_egui_texture_from_wgpu_texture(&rs.device, view, eframe::wgpu::FilterMode::Linear, id);
                    self.texture = Some((id, size));
                    id
                }
                None => {
                    let id = rs.renderer.write().register_native_texture(&rs.device, view, eframe::wgpu::FilterMode::Linear);
                    self.texture = Some((id, size));
                    id
                }
            };
            ui.painter().image(id, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        self.labels(ui, p, name);
        self.context_menu(p, name, &resp);
    }

    fn scene<'a>(&self, ws: &'a Workspace) -> Scene<'a> {
        Scene { db: &ws.project.db, assets: &ws.assets, look: self.look() }
    }

    /// What is under pixel `at` of what the Select tool picks: characters, objects and items in
    /// front first, then scenery, then (where zone areas are shown) the zones around it, the
    /// smallest first.
    fn things_at(&self, ws: &Workspace, map: &Map, at: Vec2) -> Vec<Selection> {
        let scene = self.scene(ws);
        let mut out: Vec<Selection> = Vec::new();
        if self.picks.things && self.layers[2] {
            out.extend(scene.entities_at(map, at).into_iter().map(Selection::Entity));
        }
        if self.picks.scenery && self.layers[1] {
            out.extend(scene.scenery_at(map, at).into_iter().map(Selection::Scenery));
        }
        if self.opts.zones && self.picks.zones {
            out.extend(self.zones_at(ws, map, at));
        }
        out
    }

    fn zones_at(&self, ws: &Workspace, map: &Map, at: Vec2) -> Vec<Selection> {
        let p = self.look().to_world(at);
        let mut zones: Vec<_> = map.zones.iter().filter(|z| z.pixel_rect(ws.ts()).contains(p)).collect();
        zones.sort_by_key(|z| z.w * z.h);
        zones.iter().map(|z| Selection::Zone(z.uid)).collect()
    }

    /// The corners of the selected zone as handles: (pixel, the opposite corner in tile lines).
    fn handles(&self, ws: &Workspace, map: &Map) -> Vec<(u32, Vec2, (i32, i32))> {
        let [Selection::Zone(uid)] = self.sel.as_slice() else { return Vec::new() };
        if !matches!(self.tool, Tool::Select | Tool::Zones) {
            return Vec::new();
        }
        let Some(z) = map.zones.iter().find(|z| z.uid == *uid) else { return Vec::new() };
        let (ts, look) = (ws.ts(), self.look());
        let (x0, y0, x1, y1) = (z.x as i32, z.y as i32, (z.x + z.w) as i32, (z.y + z.h) as i32);
        [(x0, y0, x1, y1), (x1, y0, x0, y1), (x0, y1, x1, y0), (x1, y1, x0, y0)].into_iter().map(|(x, y, fx, fy)| (*uid, look.to_screen(vec2(x as f32 * ts, y as f32 * ts)), (fx, fy))).collect()
    }

    fn input_frame(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str, resp: &egui::Response) {
        let (rect, ppp, ts) = (self.rect, self.ppp, p.ws.ts());
        let over = resp.contains_pointer();
        let i = ui.input(|i| {
            let ptr = &i.pointer;
            (ptr.latest_pos(), ptr.primary_pressed(), ptr.primary_down(), ptr.primary_released(), ptr.secondary_pressed(), ptr.secondary_down(), ptr.button_pressed(egui::PointerButton::Middle), ptr.button_down(egui::PointerButton::Middle), i.modifiers, i.smooth_scroll_delta.y, i.key_down(egui::Key::Space), ptr.delta())
        });
        let (pos, pressed, down, released, rpressed, rdown, mpressed, mdown, mods, scroll, space, delta) = i;
        let at = pos.map(|q| vec2((q.x - rect.min.x) * ppp, (q.y - rect.min.y) * ppp));
        let look = self.look();
        let world = at.map(|a| look.to_world(a));
        self.hover = world.filter(|_| over || self.drag.is_some());
        let tile_of = |w: Vec2| ((w.x / ts).floor() as i32, (w.y / ts).floor() as i32);
        if (pressed || rpressed || mpressed) && over {
            resp.request_focus();
            // The map pressed on is the one being worked on.
            p.ws.open_map(name);
        }
        let focused = resp.has_focus();
        if focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: true }));
            self.keys(ui, p, name);
        }

        // The wheel zooms around the pointer.
        if let (true, Some(w), Some((c, z))) = (over && scroll != 0.0, world, self.cam) {
            let new = (z * 1.15f32.powf((scroll / 50.0).clamp(-3.0, 3.0))).clamp(0.02, 16.0);
            self.cam = Some((w + (c - w) * (z / new), new));
        }
        // The middle button, or Space with the left one, moves the view.
        if over && (mpressed || (pressed && space)) {
            self.drag = Some(Drag::Pan);
        }
        if matches!(self.drag, Some(Drag::Pan)) {
            if !(mdown || down) {
                self.drag = None;
            } else if let Some((c, z)) = self.cam {
                self.cam = Some((c - look.pan(vec2(delta.x, delta.y) * ppp), z));
            }
            return;
        }
        let (Some(at), Some(world)) = (at, world) else { return };
        let tile = tile_of(world);
        let add = mods.shift || mods.command;
        // A definition dragged out of the Project browser is put down where it is let go.
        if let Some(dropped) = resp.dnd_release_payload::<(RefKind, String)>() {
            p.ws.open_map(name);
            let (kind, id) = (dropped.0, dropped.1.as_str());
            let r = match kind {
                RefKind::Structure => p.ws.place_structure(id, tile).map(|made| self.sel = made.into_iter().map(Selection::Entity).collect()),
                RefKind::Prop if !p.ws.prop_acts(id) => p.ws.place_scenery(id, world, None, false, 0).map(|d| self.sel = vec![Selection::Scenery(d)]),
                _ => match entity_kind(kind) {
                    Some(ek) => {
                        let feet = p.ws.snap_feet(ek, id, world, mods.shift);
                        p.ws.place_entity(ek, id, feet).map(|uid| self.sel = vec![Selection::Entity(uid)])
                    }
                    None => Err(format!("a {} isn't placed on a map", kind.name())),
                },
            };
            if let Err(e) = r {
                p.ws.set_status(e);
            }
            return;
        }

        self.shift = mods.shift;
        if pressed && over {
            self.cycle = None;
            let Some(doc) = p.ws.doc.as_ref() else { return };
            // A corner of the selected zone.
            if let Some((uid, _, fixed)) = self.handles(p.ws, &doc.map).into_iter().find(|(_, h, _)| h.distance(at) <= 7.0 * ppp) {
                self.drag = p.ws.snap_map().map(|before| Drag::Resize { uid, fixed, before });
                return;
            }
            match self.tool {
                Tool::Select | Tool::Zones => {
                    let stack = if self.tool == Tool::Zones { self.zones_at(p.ws, &doc.map, at) } else { self.things_at(p.ws, &doc.map, at) };
                    // A zone or a piece of scenery that isn't selected is picked by a click; a
                    // drag from it draws (a box, a new zone) as on empty ground, so that a house
                    // or a big zone isn't moved by a drag meant for something else.
                    let lone_zone = stack.first().copied().filter(|s| matches!(s, Selection::Zone(_) | Selection::Scenery(_)) && !self.sel.contains(s));
                    match stack.first().copied().filter(|_| lone_zone.is_none() || add) {
                        Some(first) if add => match self.sel.iter().position(|x| *x == first) {
                            Some(k) => drop(self.sel.remove(k)),
                            None => self.sel.push(first),
                        },
                        Some(first) => {
                            // A press on one of the selected moves them all; else the front one
                            // is selected (and a click without a drag goes on to what is behind).
                            let hit = stack.iter().copied().find(|s| self.sel.contains(s));
                            if hit.is_none() {
                                self.sel = vec![first];
                            } else if self.sel.len() == 1 && stack.len() > 1 {
                                self.cycle = Some(stack.clone());
                            }
                            let lead = hit.unwrap_or(first);
                            let mut origs: Vec<(Selection, Vec2)> = self
                                .sel
                                .iter()
                                .filter_map(|s| match s {
                                    Selection::Entity(u) => doc.map.entity(*u).map(|e| (*s, vec2(e.x, e.y))),
                                    Selection::Zone(u) => doc.map.zones.iter().find(|z| z.uid == *u).map(|z| (*s, vec2(z.x as f32, z.y as f32))),
                                    Selection::Scenery(d) => Some((*s, vec2(d.x as f32, d.y as f32))),
                                    _ => None,
                                })
                                .collect();
                            let grab = match lead {
                                Selection::Entity(u) => doc.map.entity(u).map_or(Vec2::ZERO, |e| world - vec2(e.x, e.y)),
                                Selection::Scenery(d) => world - vec2(d.x as f32, d.y as f32),
                                _ => Vec2::ZERO,
                            };
                            // (The lead goes first: a single thing snaps to the grid by itself.)
                            origs.sort_by_key(|(s, _)| *s != lead);
                            self.drag = p.ws.snap_map().map(|before| Drag::Move { from: world, grab, origs, before, moved: false });
                        }
                        None if self.tool == Tool::Zones => self.drag = Some(Drag::NewZone { from: tile, at, pick: lone_zone }),
                        None => {
                            if !add {
                                self.sel.clear();
                            }
                            self.drag = Some(Drag::Box { from: at, pick: lone_zone });
                        }
                    }
                }
                Tool::Place => self.place_press(p, world, tile),
                Tool::Terrain => self.terrain_press(p, world),
                Tool::Collision => {
                    let value = if mods.shift { CollisionOverride::Auto } else { CollisionOverride::Solid } as u8;
                    if let Some(doc) = &mut p.ws.doc {
                        doc.begin_stroke();
                        doc.paint_collision(tile.0, tile.1, value);
                    }
                    self.drag = Some(Drag::Paint { last: tile, value });
                }
            }
        }
        // The Collision tool's right button opens tiles.
        if self.tool == Tool::Collision && self.drag.is_none() && rpressed && over {
            if let Some(doc) = &mut p.ws.doc {
                doc.begin_stroke();
                doc.paint_collision(tile.0, tile.1, CollisionOverride::Walkable as u8);
            }
            self.drag = Some(Drag::Paint { last: tile, value: CollisionOverride::Walkable as u8 });
        }
        // Right-click with the Place tool stops placing; with the Terrain tool it picks the
        // material under the pointer.
        if self.tool == Tool::Place && rpressed && over {
            self.place = None;
        }
        if self.tool == Tool::Terrain && rpressed && over {
            match p.ws.ground_at(world) {
                Some(m) => {
                    p.ws.set_status(format!("Material: {m}"));
                    self.material = Some(m);
                }
                None => p.ws.set_status("No painted ground here"),
            }
        }

        let held = down || (rdown && matches!(self.drag, Some(Drag::Paint { .. })));
        match self.drag.take() {
            Some(Drag::Move { from, grab, mut origs, before, mut moved }) if held => {
                moved |= (look.to_screen(world) - look.to_screen(from)).length() >= 4.0 * ppp;
                if moved {
                    let d = (tile_of(world).0 - tile_of(from).0, tile_of(world).1 - tile_of(from).1);
                    // One thing alone snaps as it does when it is placed.
                    let snapped = match origs.as_slice() {
                        [(Selection::Entity(u), _)] => p.ws.doc.as_ref().and_then(|doc| doc.map.entity(*u)).map(|e| p.ws.snap_feet(e.kind, &e.def, world - grab, mods.shift)),
                        [(Selection::Scenery(dec), _)] => {
                            let prop = p.ws.doc.as_ref().and_then(|doc| doc.map.decor_defs.get(dec.def as usize).cloned()).unwrap_or_default();
                            Some(if mods.shift { (world - grab).round() } else { p.ws.scenery_spot(&prop, world - grab + vec2(0.0, -0.5)) })
                        }
                        _ => None,
                    };
                    if let Some(doc) = &mut p.ws.doc {
                        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
                        for (s, orig) in origs.iter_mut() {
                            let to = snapped.unwrap_or(*orig + vec2(d.0 as f32, d.1 as f32) * ts);
                            let to = vec2(to.x.clamp(0.0, mw as f32 * ts), to.y.clamp(0.0, mh as f32 * ts));
                            match s {
                                Selection::Entity(u) => {
                                    if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == *u) {
                                        (e.x, e.y) = (to.x, to.y);
                                    }
                                }
                                Selection::Zone(u) => {
                                    if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == *u) {
                                        z.x = (orig.x as i32 + d.0).clamp(0, (mw - z.w as i32).max(0)) as u32;
                                        z.y = (orig.y as i32 + d.1).clamp(0, (mh - z.h as i32).max(0)) as u32;
                                    }
                                }
                                Selection::Scenery(cur) => {
                                    // (Scenery is told by what it is: the selection follows it.)
                                    let new = Decor { x: to.x as i32, y: to.y as i32, ..*cur };
                                    if let Some(x) = doc.map.decor.iter_mut().find(|x| **x == *cur) {
                                        *x = new;
                                        *cur = new;
                                    }
                                }
                                _ => {}
                            }
                        }
                        doc.overview.decor_stale = true;
                    }
                    self.sel = origs.iter().map(|o| o.0).collect();
                }
                self.drag = Some(Drag::Move { from, grab, origs, before, moved });
            }
            Some(Drag::Move { before, moved, origs, .. }) => {
                if moved {
                    let label = match origs.as_slice() {
                        [(one, _)] => format!("Move {}", p.ws.describe_object(*one)),
                        many => format!("Move {} things", many.len()),
                    };
                    p.ws.commit_map(*before, &label);
                } else if let Some(stack) = self.cycle.take() {
                    // A click on what was selected already: the next thing under the pointer.
                    let k = stack.iter().position(|s| self.sel.contains(s)).unwrap_or(0);
                    self.sel = vec![stack[(k + 1) % stack.len()]];
                }
            }
            Some(Drag::Resize { uid, fixed, before }) if held => {
                // (The corner under the pointer: the nearest tile line.)
                let corner = ((world.x / ts).round() as i32, (world.y / ts).round() as i32);
                if let Some(doc) = &mut p.ws.doc {
                    let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
                    let (x0, x1) = (corner.0.min(fixed.0).clamp(0, mw - 1), corner.0.max(fixed.0).clamp(1, mw));
                    let (y0, y1) = (corner.1.min(fixed.1).clamp(0, mh - 1), corner.1.max(fixed.1).clamp(1, mh));
                    if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) {
                        (z.x, z.y, z.w, z.h) = (x0 as u32, y0 as u32, (x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32);
                    }
                }
                self.drag = Some(Drag::Resize { uid, fixed, before });
            }
            Some(Drag::Resize { uid, before, .. }) => {
                let label = format!("Size zone {}", p.ws.describe_object(Selection::Zone(uid)));
                p.ws.commit_map(*before, &label);
            }
            Some(Drag::Box { from, pick }) if held => self.drag = Some(Drag::Box { from, pick }),
            Some(Drag::Box { from, pick }) => {
                if (at - from).length() < 4.0 * ppp {
                    if let Some(pick) = pick {
                        self.sel = vec![pick];
                    }
                } else {
                    let r = Rect::new(from.x.min(at.x), from.y.min(at.y), (from.x - at.x).abs(), (from.y - at.y).abs());
                    if let Some(doc) = p.ws.doc.as_ref() {
                        let mut found: Vec<Selection> = Vec::new();
                        if self.picks.things && self.layers[2] {
                            found.extend(doc.map.entities.iter().filter(|e| r.contains(look.to_screen(vec2(e.x, e.y)))).map(|e| Selection::Entity(e.uid)));
                        }
                        if self.picks.scenery && self.layers[1] {
                            found.extend(doc.map.decor.iter().filter(|d| r.contains(look.to_screen(vec2(d.x as f32, d.y as f32)))).map(|d| Selection::Scenery(*d)));
                        }
                        if self.opts.zones && self.picks.zones {
                            found.extend(doc.map.zones.iter().filter(|z| r.contains(look.to_screen(z.pixel_rect(ts).center()))).map(|z| Selection::Zone(z.uid)));
                        }
                        for s in found {
                            if !self.sel.contains(&s) {
                                self.sel.push(s);
                            }
                        }
                    }
                }
            }
            Some(Drag::NewZone { from, at: start, pick }) if held => self.drag = Some(Drag::NewZone { from, at: start, pick }),
            Some(Drag::NewZone { from, at: start, pick }) => match pick.filter(|_| (at - start).length() < 4.0 * ppp) {
                Some(pick) => self.sel = vec![pick],
                None => match self.zone_of_kind(p.ws).and_then(|kind| p.ws.add_zone(kind, rect_of(from, tile))) {
                    Ok(uid) => {
                        self.sel = vec![Selection::Zone(uid)];
                        self.transition_drawn(p, name, uid);
                    }
                    Err(e) => p.ws.set_status(e),
                },
            },
            Some(Drag::Paint { last, value }) if held => {
                if tile != last {
                    if let Some(doc) = &mut p.ws.doc {
                        for t in line(last, tile).into_iter().skip(1) {
                            doc.paint_collision(t.0, t.1, value);
                        }
                    }
                }
                self.drag = Some(Drag::Paint { last: tile, value });
            }
            Some(Drag::Paint { .. }) => {
                if let Some(doc) = &mut p.ws.doc {
                    doc.end_stroke();
                }
                p.ws.begin("Paint collision");
            }
            Some(Drag::Ground { last, value, mut cells }) if held => {
                if world.distance(last) >= 1.0 {
                    cells.extend(p.ws.ground_stroke(self.brush, last, world, value));
                    self.drag = Some(Drag::Ground { last: world, value, cells });
                } else {
                    self.drag = Some(Drag::Ground { last, value, cells });
                }
            }
            Some(Drag::Ground { value, cells, .. }) => {
                p.ws.ground_commit(cells, value);
            }
            Some(Drag::GroundShape { from, value }) if held => self.drag = Some(Drag::GroundShape { from, value }),
            Some(Drag::GroundShape { from, value }) => {
                let cells = if self.ground_mode == GroundMode::Rect { p.ws.ground_rect(self.brush, from, world, value) } else { p.ws.ground_line(self.brush, from, world, value) };
                let n = p.ws.ground_commit(cells, value);
                p.ws.set_status(format!("Painted {n} cells"));
            }
            Some(Drag::Row { from }) if held => self.drag = Some(Drag::Row { from }),
            Some(Drag::Row { from }) => {
                if let Some((RefKind::Prop, prop)) = self.place.clone() {
                    let r = if p.ws.scenery_spot(&prop, from) == p.ws.scenery_spot(&prop, world) {
                        p.ws.place_scenery(&prop, from, self.place_layer, false, self.place_turn).map(|d| self.sel = vec![Selection::Scenery(d)])
                    } else {
                        p.ws.place_row(&prop, from, world, self.place_layer).map(|n| p.ws.set_status(format!("Placed a row of {n}")))
                    };
                    if let Err(e) = r {
                        p.ws.set_status(e);
                    }
                }
            }
            Some(Drag::Scatter { last, before }) if held => {
                let mut last = last;
                if let (Some((RefKind::Scatter, set)), true) = (self.place.clone(), world.distance(last) >= self.scatter_size * 0.5) {
                    self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
                    p.ws.scatter(&set, world, self.scatter_size, self.seed);
                    if let Some(doc) = &mut p.ws.doc {
                        doc.overview.decor_stale = true;
                    }
                    last = world;
                }
                self.drag = Some(Drag::Scatter { last, before });
            }
            Some(Drag::Scatter { before, .. }) => {
                let name = self.place.as_ref().and_then(|(_, id)| p.ws.project.db.scatter_set(id)).map_or_else(String::new, |s| if s.name.is_empty() { s.id.clone() } else { s.name.clone() });
                p.ws.commit_map(*before, &format!("Scatter {name}"));
            }
            Some(Drag::Pan) | None => {}
        }
        let _ = released;

        // A double click opens what was clicked: a thing's definition, a portal's other side.
        if resp.double_clicked() && self.tool == Tool::Select {
            if let (Some(doc), Some(first)) = (p.ws.doc.as_ref(), self.sel.first()) {
                match first {
                    Selection::Entity(u) => {
                        if let Some(e) = doc.map.entity(*u) {
                            p.asks.push(Ask::OpenDef(kind_ref(e.kind), e.def.clone()));
                        }
                    }
                    Selection::Zone(u) => {
                        if let Some(ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. }) = doc.map.zones.iter().find(|z| z.uid == *u).map(|z| &z.kind) {
                            if !target_map.is_empty() {
                                p.asks.push(Ask::OpenMap(target_map.clone()));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The Place tool's press: what is picked is put down.
    fn place_press(&mut self, p: &mut Parts, world: Vec2, tile: (i32, i32)) {
        self.sel.clear();
        let Some((kind, id)) = self.place.clone() else {
            p.ws.set_status("Pick what to place in the list on the left first");
            return;
        };
        let r = match kind {
            RefKind::Structure => p.ws.place_structure(&id, tile).map(|made| self.sel = made.into_iter().map(Selection::Entity).collect()),
            RefKind::Scatter => {
                let Some(before) = p.ws.snap_map() else { return };
                self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
                p.ws.scatter(&id, world, self.scatter_size, self.seed);
                self.drag = Some(Drag::Scatter { last: world, before });
                Ok(())
            }
            RefKind::Prop if !p.ws.prop_acts(&id) => {
                if p.ws.project.db.prop(&id).is_some_and(|d| d.snap > 0) {
                    // (On its own grid: a drag makes a row, put down when the button goes up.)
                    self.drag = Some(Drag::Row { from: world });
                    Ok(())
                } else {
                    self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let flip = self.place_flip && (self.seed >> 33) & 1 == 1;
                    let at = if self.shift { world.round() } else { p.ws.scenery_spot(&id, world) };
                    p.ws.place_scenery(&id, at, self.place_layer, flip, self.place_turn).map(|d| self.sel = vec![Selection::Scenery(d)])
                }
            }
            _ => match entity_kind(kind) {
                Some(ek) => {
                    let feet = p.ws.snap_feet(ek, &id, world, self.shift);
                    p.ws.place_entity(ek, &id, feet).map(|uid| self.sel = vec![Selection::Entity(uid)])
                }
                None => Err(format!("a {} isn't placed on a map", kind.name())),
            },
        };
        if let Err(e) = r {
            p.ws.set_status(e);
        }
    }

    /// The Terrain tool's press.
    fn terrain_press(&mut self, p: &mut Parts, world: Vec2) {
        let Some(material) = self.material.clone() else {
            p.ws.set_status("Pick a material in the list on the left first");
            return;
        };
        let value = match p.ws.ground_value(if self.shift { "" } else { &material }) {
            Ok(v) => v,
            Err(e) => {
                p.ws.set_status(e);
                return;
            }
        };
        match self.ground_mode {
            GroundMode::Brush => {
                let cells = p.ws.ground_dab(self.brush, world, value);
                self.drag = Some(Drag::Ground { last: world, value, cells });
            }
            GroundMode::Line | GroundMode::Rect => self.drag = Some(Drag::GroundShape { from: world, value }),
            GroundMode::Fill => {
                let cells = p.ws.ground_fill(world, value);
                let n = p.ws.ground_commit(cells, value);
                p.ws.set_status(format!("Filled {n} cells"));
            }
        }
    }

    /// A transition zone was drawn: the first of a pair waits for its other side; the second,
    /// on another map, makes them each other's.
    fn transition_drawn(&mut self, p: &mut Parts, name: &str, uid: u32) {
        let Some(zone) = p.ws.doc.as_ref().and_then(|d| d.map.zones.iter().find(|z| z.uid == uid)).filter(|z| matches!(&z.kind, ZoneKind::Transition { target_map, .. } if target_map.is_empty())).map(|z| z.name.clone()) else { return };
        match p.link.clone() {
            Some((other, there)) if other != name => {
                // (The zone's own step gets its name before the link's two.)
                p.ws.history_frame(None);
                match p.ws.link_transitions((&other, &there), (name, &zone)) {
                    Ok(()) => {
                        *p.link = None;
                        p.ws.set_status(format!("{name} and {other} are linked: walking into one side arrives beside the other"));
                    }
                    Err(e) => p.ws.set_status(e),
                }
            }
            _ => {
                *p.link = Some((name.to_string(), zone));
                p.ws.set_status("Now draw the transition's other side on another map (Zones, Transition)");
            }
        }
    }

    /// The Zones tool's kind as a zone starts with it.
    fn zone_of_kind(&self, ws: &Workspace) -> Result<ZoneKind, String> {
        let Some(Shape::Choice(variants)) = schema::zone().at("/kind").map(|n| n.shape()) else { return Err("zones have no kinds".into()) };
        let v = variants.iter().find(|v| v.name == self.zone_kind).ok_or_else(|| format!("\"{}\" is not a kind of zone", self.zone_kind))?;
        let mut kind: ZoneKind = serde_json::from_value(v.blank()).map_err(|e| e.to_string())?;
        // (What a blank one can't know: someone to spawn, and more than none of them.)
        if let ZoneKind::Spawner { character, count, .. } = &mut kind {
            *character = ws.project.db.characters.first().map(|c| c.id.clone()).unwrap_or_default();
            *count = (*count).max(1);
        }
        Ok(kind)
    }

    fn keys(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        let k = ui.input(|i| {
            let plain = !i.modifiers.command && !i.modifiers.alt;
            let key = |k: egui::Key| plain && i.key_pressed(k);
            let tool = Tool::ALL.into_iter().find(|t| key(t.key()));
            let pan = vec2(i.key_down(egui::Key::ArrowRight) as i32 as f32 - i.key_down(egui::Key::ArrowLeft) as i32 as f32, i.key_down(egui::Key::ArrowDown) as i32 as f32 - i.key_down(egui::Key::ArrowUp) as i32 as f32);
            let size = key(egui::Key::CloseBracket) as i32 - key(egui::Key::OpenBracket) as i32;
            (tool, key(egui::Key::G), key(egui::Key::R), key(egui::Key::Delete) || key(egui::Key::Backspace), i.modifiers.command && i.key_pressed(egui::Key::D), key(egui::Key::Escape), key(egui::Key::Home), pan, i.stable_dt, size)
        });
        let (tool, grid, turn, delete, duplicate, escape, home, pan, dt, size) = k;
        if size != 0 {
            // [ and ]: the brush's size.
            let ts = p.ws.ts();
            match self.tool {
                Tool::Terrain => self.brush.size = (self.brush.size + size as f32 * ts / 8.0).clamp(ts / 8.0, ts * 4.0),
                Tool::Place => self.scatter_size = (self.scatter_size + size as f32 * ts / 4.0).clamp(ts / 4.0, ts * 8.0),
                _ => {}
            }
        }
        if let Some(t) = tool {
            self.tool = t;
        }
        if grid {
            self.opts.grid = !self.opts.grid;
        }
        if turn && self.tool == Tool::Place && self.place.is_some() {
            // (What is about to be placed turns, not what was placed last.)
            self.place_turn = (self.place_turn + 1) % 4;
        } else if turn {
            self.act(p, name, "turn");
        }
        if delete && !self.sel.is_empty() {
            self.act(p, name, "delete");
        }
        if duplicate && !self.sel.is_empty() {
            self.act(p, name, "duplicate");
        }
        if escape {
            // Esc: stop placing, else let go of the selection, else back to Select.
            if self.tool == Tool::Place && self.place.is_some() {
                self.place = None;
            } else if !self.sel.is_empty() {
                self.sel.clear();
            } else {
                self.tool = Tool::Select;
            }
        }
        if home {
            self.fit(p.ws, name);
        }
        if pan != Vec2::ZERO {
            let look = self.look();
            if let Some((c, z)) = self.cam {
                self.cam = Some((c + look.pan(pan * 600.0 * dt.min(0.05)), z));
            }
        }
    }

    fn context_menu(&mut self, p: &mut Parts, name: &str, resp: &egui::Response) {
        if self.tool != Tool::Select {
            return;
        }
        // The right button selects what is under it (unless that is selected already).
        if resp.secondary_clicked() {
            if let (Some(doc), Some(w)) = (doc_of(p.ws, name), self.hover) {
                let ts = p.ws.ts();
                self.menu_tile = ((w.x / ts).floor() as i32, (w.y / ts).floor() as i32);
                let at = self.look().to_screen(w);
                let stack = self.things_at(p.ws, &doc.map, at);
                if !stack.iter().any(|s| self.sel.contains(s)) {
                    self.sel = stack.first().map(|s| vec![*s]).unwrap_or_default();
                }
            }
        }
        let mut act = None;
        let def = match (self.sel.as_slice(), doc_of(p.ws, name)) {
            ([Selection::Entity(u)], Some(doc)) => doc.map.entity(*u).map(|e| (kind_ref(e.kind), e.def.clone())),
            ([Selection::Scenery(d)], Some(doc)) => doc.map.decor_defs.get(d.def as usize).map(|id| (RefKind::Prop, id.clone())),
            _ => None,
        };
        let scenery = matches!(self.sel.as_slice(), [Selection::Scenery(_)]);
        let any = !self.sel.is_empty();
        let tile = self.menu_tile;
        let setup = {
            let setup = p.ws.test_setup();
            if setup == Default::default() { "No test setup yet: a new hero, nothing done".to_string() } else { format!("The test setup: {}", setup.summary()) }
        };
        resp.context_menu(|ui| {
            if ui.button("Play from here").on_hover_text(format!("A new game with the hero on tile {}, {}: the map's Map enter events run", tile.0, tile.1)).clicked() {
                p.asks.push(Ask::PlayHere(tile, false));
                ui.close();
            }
            if ui.button("Play from here with the test setup").on_hover_text(format!("{}. A game in progress loaded here: the map's Map enter events don't run. The Play tab's State panel keeps a running game's state as the setup.", setup)).clicked() {
                p.asks.push(Ask::PlayHere(tile, true));
                ui.close();
            }
            if !any {
                return;
            }
            ui.separator();
            if let Some((rk, id)) = &def {
                if ui.button("Edit definition").clicked() {
                    p.asks.push(Ask::OpenDef(*rk, id.clone()));
                    ui.close();
                }
                if ui.button("Find usages").on_hover_text("Where this definition is placed and named, on every map").clicked() {
                    p.asks.push(Ask::Usages(*rk, id.clone()));
                    ui.close();
                }
                if ui.button("Turn").clicked() {
                    act = Some("turn");
                    ui.close();
                }
                if scenery && ui.button("Make interactive").clicked() {
                    act = Some("interactive");
                    ui.close();
                }
            }
            if ui.button("Duplicate").clicked() {
                act = Some("duplicate");
                ui.close();
            }
            if ui.button("Delete").clicked() {
                act = Some("delete");
                ui.close();
            }
        });
        if let Some(act) = act {
            self.act(p, name, act);
        }
    }

    // ------------------------------------------------------------ drawing

    fn draw(&mut self, ws: &mut Workspace, name: &str) {
        if ws.gpu == editor_core::Gpu::No {
            return;
        }
        let started = std::time::Instant::now();
        // (The map is taken out while it is drawn: its overview is refreshed on the way.)
        let current = ws.doc.as_ref().is_some_and(|d| d.map.name == name);
        let Some(mut doc) = (if current { ws.doc.take() } else { ws.parked.remove(name) }) else { return };
        if doc.overview.decor_stale {
            doc.overview.refresh_decor(&doc.map, &ws.assets, &ws.project.db);
        }
        {
            let ws = &*ws;
            let scene = self.scene(ws);
            let (w, h) = self.size;
            let mut screen = self.screen.take().unwrap_or_else(|| gfx::Screen::new(w, h, 1.0));
            screen.resize(w, h, 1.0);
            let mut input = std::mem::take(&mut self.input);
            gfx::with_screen(&mut screen, &mut input, || self.draw_map(&scene, ws, &mut doc));
            (self.screen, self.input) = (Some(screen), input);
        }
        if current {
            ws.doc = Some(doc);
        } else {
            ws.parked.insert(name.to_string(), doc);
        }
        self.draw_ms = started.elapsed().as_secs_f32() * 1000.0;
    }

    /// A picture of tiles of the map being worked on, as the game shows them: what an agent's
    /// `screenshot_map` asked for. (Names are not written into it: `get_map` has them.)
    pub fn shot(ws: &mut Workspace, spec: &editor_core::mcp::exec::ShotSpec) -> Result<rustpg2d_engine::imageio::Rgba, String> {
        let Some((x, y, w, h)) = spec.rect else { return Err("pictures of the whole window aren't taken yet: ask for a map with screenshot_map".into()) };
        if ws.gpu == editor_core::Gpu::No {
            return Err("this editor has no graphics adapter: read the map with get_map instead".into());
        }
        let Some(name) = ws.doc.as_ref().map(|d| d.map.name.clone()) else { return Err("no map is open".into()) };
        let ts = ws.ts();
        let opts = ViewOpts { grid: spec.grid.unwrap_or(false), collision: spec.collision.unwrap_or(false), zones: spec.zones.unwrap_or(false), ..ViewOpts::default() };
        let mut view = MapView { opts, ..Default::default() };
        // The picture's size for the tiles at the zoom asked for (room above for what stands
        // on the northern tiles), at most 2048 pixels a side.
        let (rw, rh) = (w as f32 * ts, h as f32 * ts);
        let full = |zoom: f32| vec2((rw + rh) * zoom / render::iso::UPRIGHT, (rw + rh) * 0.5 * zoom / render::iso::UPRIGHT + 3.0 * ts * zoom);
        let mut zoom = spec.zoom.unwrap_or(1.0).clamp(0.1, 4.0);
        let size = full(zoom);
        zoom *= (2048.0 / size.x.max(size.y)).min(1.0);
        let size = full(zoom);
        view.size = ((size.x.ceil() as u32).max(16), (size.y.ceil() as u32).max(16));
        // (The room above is taken from the middle: the tiles sit that much lower.)
        let center = vec2(x as f32 * ts + rw / 2.0, y as f32 * ts + rh / 2.0);
        view.cam = Some((center, zoom));
        let lift = view.look().pan(vec2(0.0, 1.5 * ts * zoom));
        view.cam = Some((center - lift, zoom));
        view.draw(ws, &name);
        let screen = view.screen.as_ref().ok_or("the map could not be drawn")?;
        let img = screen.texture().get_texture_data();
        Ok(rustpg2d_engine::imageio::Rgba { w: img.width as u32, h: img.height as u32, px: img.bytes })
    }

    fn draw_map(&mut self, scene: &Scene, ws: &Workspace, doc: &mut MapDoc) {
        use gfx::prelude::*;
        let (ts, look) = (scene.ts(), scene.look);
        let cam = look.ground();
        let view = look.visible();
        let px = 1.0 / look.zoom;
        let time = get_time();
        clear_background(Color::from_rgba(32, 34, 40, 255));
        set_camera(&cam);

        let weather = if self.opts.weather { doc.map.weather } else { rustpg2d_engine::map::Weather::default() };
        let base = if self.opts.lighting { render::lighting::ambient(doc.map.lighting, self.opts.hour) } else { None };
        // Lights only where it is dark (weather alone just tints the daylight), as in the game.
        let lighting = render::weather::tint_ambient(base, weather).map(|a| (a, if base.is_some() { scene.lights(&doc.map) } else { Vec::new() }));
        let brightness = lighting.as_ref().map_or(1.0, |(c, _)| (c.r + c.g + c.b) / 3.0);
        let (mw, mh) = (doc.map.width as f32 * ts, doc.map.height as f32 * ts);
        let bg = doc.map.background;
        draw_rectangle(0.0, 0.0, mw, mh, Color::from_rgba(bg[0], bg[1], bg[2], 255));
        if self.opts.parallax {
            render::parallax::draw(scene.assets, &doc.map.parallax, false, view, time as f32);
        }
        let overview = render::visible_tiles(view, ts) > render::OVERVIEW_THRESHOLD;
        if overview {
            doc.overview.draw(&doc.map, ts);
        }
        let map = &doc.map;
        if !overview && self.layers[0] {
            self.ground.draw(map, scene.db, scene.assets, view, WHITE);
            for z in map.zones.iter().filter(|z| matches!(z.kind, ZoneKind::FishingSpot { .. })) {
                render::fish::draw_shadows(z.pixel_rect(ts), z.uid, time as f32, ts);
            }
        }
        scene.draw_objects(map, view, overview, WHITE, matches!(self.tool, Tool::Terrain | Tool::Place), (self.layers[2], self.layers[1]));
        if self.opts.parallax {
            render::parallax::draw(scene.assets, &map.parallax, true, view, time as f32);
        }
        if let Some((ambient, lights)) = lighting {
            self.light_pass.get_or_insert_with(LightPass::new).draw(&cam, ambient, &lights, ts, time);
            set_camera(&cam);
        }
        if weather.active() && !overview {
            self.weather_fx.update(get_frame_time().min(0.05), weather, view, ts / 32.0);
            self.weather_fx.draw(weather, view, ts / 32.0, brightness);
        }

        // The tiles in view.
        let x0 = (view.x / ts).floor().max(0.0) as u32;
        let y0 = (view.y / ts).floor().max(0.0) as u32;
        let x1 = ((view.x + view.w) / ts).ceil().clamp(0.0, map.width as f32) as u32;
        let y1 = ((view.y + view.h) / ts).ceil().clamp(0.0, map.height as f32) as u32;
        let tile_px = ts * look.zoom;

        if (self.opts.collision || self.tool == Tool::Collision) && tile_px >= 3.0 {
            for y in y0..y1 {
                for x in x0..x1 {
                    let (fx, fy) = (x as f32 * ts, y as f32 * ts);
                    match map.collision_at(x, y) {
                        CollisionOverride::Solid => draw_rectangle(fx, fy, ts, ts, Color::new(1.0, 0.1, 0.1, 0.55)),
                        CollisionOverride::Walkable => draw_rectangle(fx, fy, ts, ts, Color::new(0.2, 1.0, 0.3, 0.3)),
                        CollisionOverride::Auto => {}
                    }
                }
            }
            // The painted ground cell by cell (red: solid, yellow: low such as water), and the
            // hitboxes of solid scenery.
            if !map.ground.is_empty() && tile_px >= 8.0 {
                use rustpg2d_engine::ground::{MaterialTable, Walk, CELL, CELLS};
                let table = MaterialTable::new(&map.materials, scene.db);
                let c = CELL as f32;
                for cy in (y0 * CELLS) as i32..(y1 * CELLS) as i32 {
                    for cx in (x0 * CELLS) as i32..(x1 * CELLS) as i32 {
                        if map.collision_at(cx as u32 / CELLS, cy as u32 / CELLS) == CollisionOverride::Walkable {
                            continue;
                        }
                        match table.walk_of(map.ground.get(cx, cy)) {
                            Walk::Solid => draw_rectangle(cx as f32 * c, cy as f32 * c, c, c, Color::new(1.0, 0.2, 0.2, 0.3)),
                            Walk::Low => draw_rectangle(cx as f32 * c, cy as f32 * c, c, c, Color::new(1.0, 0.85, 0.1, 0.3)),
                            Walk::Walk => {}
                        }
                    }
                }
            }
            for d in &map.decor {
                let Some(prop) = map.decor_defs.get(d.def as usize).and_then(|id| scene.db.prop(id)).filter(|q| q.solid) else { continue };
                let [w, h] = if prop.turns() { prop.hitbox_facing(d.facing()) } else { prop.hitbox };
                let r = Rect::new(d.x as f32 - w / 2.0, d.y as f32 - h, w, h);
                if r.overlaps(&view) {
                    let c = if prop.low { Color::new(1.0, 0.85, 0.1, 0.9) } else { Color::new(1.0, 0.2, 0.2, 0.9) };
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5 * px, c);
                }
            }
        }
        if self.opts.grid && tile_px >= 8.0 {
            let c = Color::new(1.0, 1.0, 1.0, 0.1);
            for x in x0..=x1 {
                draw_line(x as f32 * ts, y0 as f32 * ts, x as f32 * ts, y1 as f32 * ts, px, c);
            }
            for y in y0..=y1 {
                draw_line(x0 as f32 * ts, y as f32 * ts, x1 as f32 * ts, y as f32 * ts, px, c);
            }
        }
        if self.opts.zones {
            for z in &map.zones {
                let r = z.pixel_rect(ts);
                let c = zone_color(&z.kind);
                let selected = self.sel.contains(&Selection::Zone(z.uid));
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(c.r, c.g, c.b, if selected { 0.34 } else { 0.2 }));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 * px } else { 1.5 * px }, if selected { YELLOW } else { c });
            }
            for e in rustpg2d_engine::map::Edge::ALL {
                if let Some(r) = map.edges.strip_rect(e, map.width, map.height, ts) {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.2, 1.0, 0.4, 0.18));
                }
            }
        }

        // The selection: each thing's frame, and what blocks the way under a single one.
        for s in &self.sel {
            if let Selection::Scenery(d) = s {
                let (r, flat) = scene.decor_rect(map, d);
                let frame = || draw_rectangle_lines(r.x - px, r.y - px, r.w + 2.0 * px, r.h + 2.0 * px, 2.0 * px, YELLOW);
                if flat {
                    frame();
                } else {
                    scene.standing(vec2(d.x as f32, d.y as f32), frame);
                }
            }
            let Selection::Entity(uid) = s else { continue };
            let Some(e) = map.entity(*uid) else { continue };
            if self.sel.len() == 1 {
                if let Some(hb) = scene.entity_hitbox(e) {
                    draw_rectangle(hb.x, hb.y, hb.w, hb.h, Color::new(1.0, 0.5, 0.1, 0.25));
                    draw_rectangle_lines(hb.x, hb.y, hb.w, hb.h, 1.5 * px, ORANGE);
                }
            }
            let r = scene.entity_rect(e);
            scene.standing(vec2(e.x, e.y), || draw_rectangle_lines(r.x - px, r.y - px, r.w + 2.0 * px, r.h + 2.0 * px, 2.0 * px, YELLOW));
        }

        // The selected character's patrol route (joined waypoints, an arrow the way it walks)
        // and daily schedule (dashed, in the order of the hours); the labels are egui's.
        if let [Selection::Entity(uid)] = self.sel.as_slice() {
            if let Some(e) = map.entity(*uid) {
                let mid = |x: i32, y: i32| vec2((x as f32 + 0.5) * ts, (y as f32 + 0.5) * ts);
                if let Some(r) = &e.patrol {
                    let col = Color::new(0.3, 0.95, 1.0, 0.85);
                    let n = r.points.len();
                    let closes = r.mode == rustpg2d_engine::entity::PatrolMode::Loop && n > 2;
                    for k in 0..n.saturating_sub(1) + usize::from(closes) {
                        let (a, b) = (mid(r.points[k].0, r.points[k].1), mid(r.points[(k + 1) % n].0, r.points[(k + 1) % n].1));
                        draw_line(a.x, a.y, b.x, b.y, 2.0 * px, col);
                        let (m, d) = (a.lerp(b, 0.5), (b - a).normalize_or_zero() * 6.0 * px);
                        let nrm = vec2(-d.y, d.x);
                        draw_triangle(m + d, m - d + nrm, m - d - nrm, col);
                    }
                    for p in &r.points {
                        let q = mid(p.0, p.1);
                        draw_circle(q.x, q.y, 5.0 * px, Color::new(0.05, 0.2, 0.3, 0.9));
                        draw_circle_lines(q.x, q.y, 5.0 * px, 1.5 * px, col);
                    }
                }
                let mut stops: Vec<&rustpg2d_engine::entity::ScheduleStop> = e.schedule.iter().filter(|s| s.map.is_empty() || s.map == map.name).collect();
                stops.sort_by(|a, b| a.hour.total_cmp(&b.hour));
                let col = Color::new(1.0, 0.72, 0.25, 0.85);
                let n = stops.len();
                for k in 0..if n > 2 { n } else { n.saturating_sub(1) } {
                    let (a, b) = (mid(stops[k].x, stops[k].y), mid(stops[(k + 1) % n].x, stops[(k + 1) % n].y));
                    let (len, dir) = (a.distance(b), (b - a).normalize_or_zero());
                    let mut d = 0.0;
                    while d < len {
                        let (p0, p1) = (a + dir * d, a + dir * (d + 7.0 * px).min(len));
                        draw_line(p0.x, p0.y, p1.x, p1.y, 2.0 * px, col);
                        d += 12.0 * px;
                    }
                }
                for s in &stops {
                    let q = mid(s.x, s.y);
                    draw_circle(q.x, q.y, 4.0 * px, col);
                }
            }
        }

        // What the tool is about to do.
        if let Some(w) = self.hover {
            let tile = ((w.x / ts).floor() as i32, (w.y / ts).floor() as i32);
            let (fx, fy) = (tile.0 as f32 * ts, tile.1 as f32 * ts);
            match (&self.drag, self.tool) {
                (Some(Drag::NewZone { from, .. }), _) => {
                    let (rx, ry, rw, rh) = rect_of(*from, tile);
                    draw_rectangle(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, Color::new(0.35, 0.75, 1.0, 0.18));
                    draw_rectangle_lines(rx as f32 * ts, ry as f32 * ts, rw as f32 * ts, rh as f32 * ts, 2.0 * px, SKYBLUE);
                }
                (_, Tool::Place) => {
                    let ghost = Color::new(1.0, 1.0, 1.0, 0.55);
                    match &self.place {
                        Some((RefKind::Structure, id)) => {
                            if let Some(def) = scene.db.structure(id) {
                                let (ox, oy) = (tile.0 - (def.w as i32 - 1) / 2, tile.1 - (def.h as i32 - 1) / 2);
                                draw_rectangle(ox as f32 * ts, oy as f32 * ts, def.w as f32 * ts, def.h as f32 * ts, Color::new(0.3, 1.0, 0.5, 0.18));
                                draw_rectangle_lines(ox as f32 * ts, oy as f32 * ts, def.w as f32 * ts, def.h as f32 * ts, 2.0 * px, GREEN);
                            }
                        }
                        Some((RefKind::Scatter, _)) => draw_circle_lines(w.x, w.y, self.scatter_size, 1.5 * px, Color::new(0.4, 1.0, 0.5, 1.0)),
                        Some((RefKind::Prop, id)) if !ws.prop_acts(id) => {
                            if let Some(def) = scene.db.prop(id) {
                                let at = if self.shift { w.round() } else { ws.scenery_spot(id, w) };
                                if let Some(Drag::Row { from }) = &self.drag {
                                    // The row runs along the longer way, as it will be put down.
                                    let a = ws.scenery_spot(id, *from);
                                    let b = if (at.x - a.x).abs() >= (at.y - a.y).abs() { vec2(at.x, a.y) } else { vec2(a.x, at.y) };
                                    draw_line(a.x, a.y, b.x, b.y, 3.0 * px, YELLOW);
                                }
                                let facing = rustpg2d_engine::decor::turn_facing(if def.turns() { self.place_turn } else { 0 });
                                let draw = || scene.assets.draw_prop(scene.db, def, 0, facing, 0, at, false, ghost, time, !look.topdown);
                                if def.flat {
                                    draw();
                                } else {
                                    scene.standing(at, draw);
                                }
                                draw_circle_lines(at.x, at.y, 3.0 * px, 1.5 * px, YELLOW);
                            }
                        }
                        Some((kind, def)) => {
                            if let Some(ek) = entity_kind(*kind) {
                                let feet = ws.snap_feet(ek, def, w, self.shift);
                                scene.standing(feet, || scene.draw_def(ek, def, feet, ghost));
                                draw_circle_lines(feet.x, feet.y, 3.0 * px, 1.5 * px, YELLOW);
                            }
                        }
                        None => draw_rectangle_lines(fx, fy, ts, ts, px, Color::new(1.0, 1.0, 1.0, 0.25)),
                    }
                }
                (_, Tool::Terrain) => {
                    let erase = self.shift || self.material.as_deref() == Some("");
                    let col = if erase { RED } else { Color::new(0.4, 1.0, 0.5, 1.0) };
                    let fill = Color::new(col.r, col.g, col.b, 0.25);
                    let (r, brush) = (self.brush.size, self.brush);
                    let square = |a: Vec2, b: Vec2| {
                        let (lo, hi) = (a.min(b), a.max(b));
                        draw_rectangle(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y, fill);
                        draw_rectangle_lines(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y, 1.5 * px, col);
                    };
                    match (&self.drag, self.ground_mode) {
                        (Some(Drag::GroundShape { from, .. }), GroundMode::Rect) => square(brush.corner(*from), brush.corner(w)),
                        (Some(Drag::GroundShape { from, .. }), _) => {
                            let (a, b) = (brush.line_end(*from), brush.line_end(w));
                            draw_line(a.x, a.y, b.x, b.y, (r * 2.0).max(px), Color::new(col.r, col.g, col.b, 0.35));
                            draw_circle_lines(a.x, a.y, r, 1.5 * px, col);
                            draw_circle_lines(b.x, b.y, r, 1.5 * px, col);
                        }
                        (_, GroundMode::Rect) => {
                            let c = brush.corner(w);
                            draw_circle(c.x, c.y, 3.0 * px, col);
                        }
                        (_, GroundMode::Fill) => {
                            let c = rustpg2d_engine::ground::CELL as f32;
                            draw_rectangle_lines((w.x / c).floor() * c, (w.y / c).floor() * c, c, c, 2.0 * px, col);
                        }
                        (_, GroundMode::Line) => {
                            let a = brush.line_end(w);
                            draw_circle_lines(a.x, a.y, r, 1.5 * px, col);
                        }
                        _ if brush.square => {
                            let (a, b) = brush.square_at(w);
                            square(a, b);
                        }
                        _ => draw_circle_lines(w.x, w.y, r, 1.5 * px, col),
                    }
                }
                (None, Tool::Select) => {
                    // What a click would select.
                    let at = look.to_screen(w);
                    match self.things_at(ws, map, at).first() {
                        Some(Selection::Entity(uid)) if !self.sel.contains(&Selection::Entity(*uid)) => {
                            if let Some(e) = map.entity(*uid) {
                                let r = scene.entity_rect(e);
                                scene.standing(vec2(e.x, e.y), || draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5 * px, Color::new(1.0, 1.0, 1.0, 0.8)));
                            }
                        }
                        Some(Selection::Scenery(d)) if !self.sel.contains(&Selection::Scenery(*d)) => {
                            let (r, flat) = scene.decor_rect(map, d);
                            let frame = || draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5 * px, Color::new(1.0, 1.0, 1.0, 0.8));
                            if flat {
                                frame();
                            } else {
                                scene.standing(vec2(d.x as f32, d.y as f32), frame);
                            }
                        }
                        Some(Selection::Zone(uid)) if !self.sel.contains(&Selection::Zone(*uid)) => {
                            if let Some(z) = map.zones.iter().find(|z| z.uid == *uid) {
                                let r = z.pixel_rect(ts);
                                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5 * px, Color::new(1.0, 1.0, 1.0, 0.8));
                            }
                        }
                        Some(_) => {}
                        None => draw_rectangle_lines(fx, fy, ts, ts, px, Color::new(1.0, 1.0, 1.0, 0.25)),
                    }
                }
                (None, Tool::Zones | Tool::Collision) => draw_rectangle_lines(fx, fy, ts, ts, 1.5 * px, Color::new(1.0, 1.0, 1.0, 0.6)),
                _ => {}
            }
        }
        draw_rectangle_lines(0.0, 0.0, mw, mh, 2.0 * px, Color::new(1.0, 1.0, 1.0, 0.5));
        set_default_camera();
    }

    /// What egui writes and draws over the picture: names, the selection box, the handles.
    fn labels(&mut self, ui: &mut egui::Ui, p: &mut Parts, name: &str) {
        let Some(doc) = doc_of(p.ws, name) else { return };
        let (ts, look) = (p.ws.ts(), self.look());
        let painter = ui.painter_at(self.rect);
        let font = egui::FontId::proportional(12.0);
        let tag = |at: egui::Pos2, text: &str, color: egui::Color32, back: egui::Color32| {
            let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
            let r = egui::Rect::from_center_size(at, galley.size() + egui::vec2(8.0, 3.0));
            painter.rect_filled(r, 3.0, back);
            painter.galley(r.min + egui::vec2(4.0, 1.5), galley, color);
        };
        let back = egui::Color32::from_black_alpha(150);
        if self.opts.zones {
            for z in &doc.map.zones {
                let selected = self.sel.contains(&Selection::Zone(z.uid));
                let r = z.pixel_rect(ts);
                if r.w * look.zoom > 26.0 || selected {
                    let c = zone_color(&z.kind);
                    tag(self.to_points(look.to_screen(r.center())), &zone_label(z), if selected { egui::Color32::YELLOW } else { c32(Color::new(c.r * 0.5 + 0.5, c.g * 0.5 + 0.5, c.b * 0.5 + 0.5, 1.0)) }, back);
                }
            }
        }
        if look.zoom >= 1.2 {
            let view = look.visible();
            for e in doc.map.entities.iter().filter(|e| !e.name.is_empty() && view.contains(vec2(e.x, e.y))) {
                tag(self.to_points(look.to_screen(vec2(e.x, e.y))) + egui::vec2(0.0, 9.0), &e.name, egui::Color32::from_rgb(255, 255, 160), egui::Color32::from_black_alpha(110));
            }
        }
        if let [Selection::Entity(uid)] = self.sel.as_slice() {
            if let Some(e) = doc.map.entity(*uid) {
                let mid = |x: i32, y: i32| vec2((x as f32 + 0.5) * ts, (y as f32 + 0.5) * ts);
                for (k, pt) in e.patrol.iter().flat_map(|r| r.points.iter().enumerate()) {
                    tag(self.to_points(look.to_screen(mid(pt.0, pt.1))) - egui::vec2(0.0, 12.0), &(k + 1).to_string(), egui::Color32::from_rgb(120, 240, 255), back);
                }
                for s in e.schedule.iter().filter(|s| s.map.is_empty() || s.map == doc.map.name) {
                    let text = if s.inside { format!("{} in", s.time_text()) } else { s.time_text() };
                    tag(self.to_points(look.to_screen(mid(s.x, s.y))) - egui::vec2(0.0, 12.0), &text, egui::Color32::from_rgb(255, 200, 110), back);
                }
            }
        }
        for (_, h, _) in self.handles(p.ws, &doc.map) {
            let c = self.to_points(h);
            painter.rect_filled(egui::Rect::from_center_size(c, egui::vec2(9.0, 9.0)), 2.0, egui::Color32::YELLOW);
            painter.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(9.0, 9.0)), 2.0, egui::Stroke::new(1.0, egui::Color32::BLACK), egui::StrokeKind::Inside);
        }
        if let (Some(Drag::Box { from, .. }), Some(w)) = (&self.drag, self.hover) {
            let r = egui::Rect::from_two_pos(self.to_points(*from), self.to_points(look.to_screen(w)));
            painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(255, 220, 60, 28));
            painter.rect_stroke(r, 0.0, egui::Stroke::new(1.5, egui::Color32::YELLOW), egui::StrokeKind::Inside);
        }
        // Where the pointer is, in tiles.
        if let Some(w) = self.hover {
            let tile = ((w.x / ts).floor() as i32, (w.y / ts).floor() as i32);
            if doc.map.in_bounds(tile.0, tile.1) {
                painter.text(self.rect.left_bottom() + egui::vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("{}, {}", tile.0, tile.1), font.clone(), egui::Color32::from_white_alpha(200));
            }
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        view: MapView,
        thumbs: Thumbs,
        lists: Vec<(RefKind, Vec<(String, String)>)>,
        asks: Vec<String>,
        link: Option<(String, String)>,
        dir: std::path::PathBuf,
    }

    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    /// The tab of the village of a scratch copy of the example, without a graphics adapter (the
    /// map itself isn't drawn; its panels and its input work).
    fn village(test: &str) -> Harness<'static, State> {
        let dir = std::env::temp_dir().join(format!("rustpg2d-mapview-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.refresh_refs();
        ws.history_frame(None);
        ws.open_map("village");
        let lists: Vec<(RefKind, Vec<(String, String)>)> = RefKind::ALL.iter().map(|k| (*k, editor_core::defs::list(&ws.project.db, *k))).collect();
        let state = State { ws, view: MapView::default(), thumbs: Thumbs::default(), lists, asks: Vec::new(), link: None, dir };
        Harness::builder().with_size(egui::vec2(1300.0, 1400.0)).build_ui_state(
            |ui, s: &mut State| {
                s.ws.history_frame(None);
                let mut parts = Parts { ws: &mut s.ws, rs: None, lists: &s.lists, thumbs: &mut s.thumbs, show_all: false, link: &mut s.link, asks: Vec::new() };
                s.view.show(ui, &mut parts, "village");
                for ask in parts.asks {
                    s.asks.push(match ask {
                        Ask::OpenDef(k, id) => format!("open {} {id}", k.name()),
                        Ask::OpenMap(m) => format!("map {m}"),
                        Ask::New(k) => format!("new {}", k.name()),
                        Ask::PlayHere(t, setup) => format!("play {},{}{}", t.0, t.1, if setup { " setup" } else { "" }),
                        Ask::Usages(k, id) => format!("usages {} {id}", k.name()),
                    });
                }
            },
            state,
        )
    }

    #[test]
    fn the_outliner_selects_and_the_inspector_changes_and_deletes() {
        let mut h = village("inspector");
        h.run();
        let uid = h.state().ws.doc.as_ref().unwrap().map.entities.iter().find(|e| e.name == "elder").unwrap().uid;
        h.get_by_role_and_label(egui::accesskit::Role::Button, "elder").click();
        h.run();
        assert_eq!(h.state().view.sel, [Selection::Entity(uid)]);
        // Its form: what means something for a character, not a prop's fields.
        for label in ["Placed here", "Daily life", "Name", "Facing", "Schedule"] {
            assert!(h.query_all_by_label(label).next().is_some(), "no \"{label}\"");
        }
        assert!(h.query_all_by_label("Container").next().is_none() && h.query_all_by_label("Lock").next().is_none());
        // Typing a name changes the placed character, as one step with its name.
        let name = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name");
        name.focus();
        name.type_text("_1");
        h.run();
        let now = h.state().ws.doc.as_ref().unwrap().map.entity(uid).unwrap().name.clone();
        assert!(now.contains("_1") && now != "elder", "{now}");
        assert!(h.state().ws.timeline.done.last().unwrap().label.starts_with("Change "));
        // The definition is one click away.
        h.get_by_label("Edit definition").click();
        h.run();
        assert_eq!(h.state().asks, ["open character elder"]);
        // Delete, and undo brings the character back as it was.
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Delete").click();
        h.run();
        assert!(h.state().ws.doc.as_ref().unwrap().map.entity(uid).is_none() && h.state().view.sel.is_empty());
        while h.state_mut().ws.undo().is_some() {}
        assert_eq!(h.state().ws.doc.as_ref().unwrap().map.entity(uid).unwrap().name, "elder");
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_tools_offer_what_they_place() {
        let mut h = village("tools");
        h.run();
        // Zones: every kind the engine has, by name.
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Zones").click();
        h.run();
        for kind in ["Spawn", "Portal", "Trigger", "Spawner", "Transition", "Fishing spot", "Ambience", "Encounter", "Drop", "Trap"] {
            assert!(h.query_all_by_label(kind).next().is_some(), "no zone kind \"{kind}\"");
        }
        h.get_by_label("Encounter").click();
        h.run();
        assert_eq!(h.state().view.zone_kind, "Encounter");
        // Place: the list is searched, and a click arms what the map's clicks put down.
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Place").click();
        h.run();
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search what to place");
        search.focus();
        search.type_text("town guard");
        h.run();
        assert!(h.query_all_by_label("Villager").next().is_none(), "the list is searched");
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Town Guard").click();
        h.run();
        assert_eq!(h.state().view.place, Some((RefKind::Character, "guard".to_string())));
        // Terrain: the materials, and a pick.
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Terrain").click();
        h.run();
        assert!(h.query_all_by_label("Erase (no ground)").next().is_some());
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Cobblestone").click();
        h.run();
        assert_eq!(h.state().view.material.as_deref(), Some("cobblestone"));
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scenery_is_listed_by_what_it_is_and_made_interactive() {
        let mut h = village("scenery");
        h.run();
        // The Outliner's Scenery group (found by the search) selects every piece of a prop.
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search this map");
        search.focus();
        search.type_text("young oak");
        h.run();
        let oaks = h.state().ws.doc.as_ref().unwrap().map.decor.iter().filter(|d| h.state().ws.doc.as_ref().unwrap().map.decor_defs[d.def as usize] == "oak_tree").count();
        assert!(oaks > 1);
        h.get_by_label(&format!("Young Oak x {oaks}")).click();
        h.run();
        assert_eq!(h.state().view.sel.len(), oaks);
        assert!(h.query_all_by_label(&format!("{oaks} things selected")).next().is_some());
        // What is selected, with the ground under it, becomes a structure named by what is typed.
        let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "The structure's name");
        field.focus();
        field.type_text("Oak grove");
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Save as structure").click();
        h.run();
        let grove = h.state().ws.project.db.structure("oak_grove").expect("the structure is made").clone();
        assert!(grove.decor.len() >= oaks && grove.w > 1 && grove.h > 1, "{} decorations on {}x{}", grove.decor.len(), grove.w, grove.h);
        assert_eq!(h.state().asks, ["open structure oak_grove"]);
        assert!(h.state_mut().ws.undo().is_some_and(|m| m.contains("Save Oak grove as a structure")));
        assert!(h.state().ws.project.db.structure("oak_grove").is_none());
        // One piece: its own small form, and the way to an object of the map and back.
        let one = h.state().view.sel[0];
        h.state_mut().view.sel = vec![one];
        h.run();
        let (entities, decor) = (h.state().ws.doc.as_ref().unwrap().map.entities.len(), h.state().ws.doc.as_ref().unwrap().map.decor.len());
        h.get_by_label("Make interactive").click();
        h.run();
        let map = &h.state().ws.doc.as_ref().unwrap().map;
        assert_eq!((map.entities.len(), map.decor.len()), (entities + 1, decor - 1));
        assert!(matches!(h.state().view.sel.as_slice(), [Selection::Entity(_)]));
        assert!(h.query_all_by_label("Placed here").next().is_some(), "an object has the full form");
        h.get_by_label("Make scenery").click();
        h.run();
        let map = &h.state().ws.doc.as_ref().unwrap().map;
        assert_eq!((map.entities.len(), map.decor.len()), (entities, decor));
        assert_eq!(h.state().ws.timeline.done.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["Make Young Oak interactive", "Make Young Oak scenery"]);
        h.state_mut().asks.clear();
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
}
