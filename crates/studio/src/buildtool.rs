//! The Build tool's choices and its panel: what it builds (walls, a room, a house, a roof, doors
//! and windows) and of which props. The building itself is `editor-core`'s (`build.rs`); the
//! map tab (`mapview.rs`) says where the pointer pressed and dragged.

use std::collections::BTreeSet;

use eframe::egui;
use editor_core::build::{self, Put};
use rustpg2d_engine::defs::{Database, PropDef, RoofKind};
use rustpg2d_engine::refactor::RefKind;

use crate::mapview::{Ask, Parts};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BuildMode {
    /// Drag along tile edges: a wall that follows the pointer.
    #[default]
    Walls,
    /// Drag a rectangle: a floor and walls around it.
    Room,
    /// Drag a rectangle: a building of that size.
    House,
    /// Drag a rectangle (or mark tiles): a roof alone, over a room of walls.
    Roof,
    /// Click a wall or a house: a door, a window or a gap.
    Doors,
}

impl BuildMode {
    pub const ALL: [BuildMode; 5] = [BuildMode::Walls, BuildMode::Room, BuildMode::House, BuildMode::Roof, BuildMode::Doors];

    pub fn label(self) -> &'static str {
        match self {
            BuildMode::Walls => "Walls",
            BuildMode::Room => "Room",
            BuildMode::House => "House",
            BuildMode::Roof => "Roof",
            BuildMode::Doors => "Doors",
        }
    }

    fn help(self) -> &'static str {
        match self {
            BuildMode::Walls => "Drag: the wall follows the pointer along the tile edges, round corners too (Shift: a straight line). Thin walls and fences stand on the edges, thick walls fill the tiles. Right-click a piece to take it away.",
            BuildMode::Room => "Drag the room's floor. Its back walls (north and west) are built, or all four with a doorway in the south wall; with a roof, it gets one over it.",
            BuildMode::House => "Drag the house's footprint: a new building in the chosen look, its door in the middle of the front, windows along the walls. Dragged against another house it becomes a wing of that one: an L-shaped house under one roof.",
            BuildMode::Roof => "A roof alone over walls you built, in the look of the chosen building, as high as the walls under it. Drag a rectangle, or mark its tiles (paint them, or click inside a room of walls) and Build roof: any shape, joined into one roof. In play it fades out in front of the hero like anything else.",
            BuildMode::Doors => "Click a house's wall: a door or window there. Click a wall piece: a door (it opens in play), a window piece or a gap. Right-click a house's door or window, or a door in a wall, to take it away.",
        }
    }

    pub fn named(name: &str) -> Option<BuildMode> {
        BuildMode::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(name))
    }
}

/// How the Roof mode marks where the roof goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RoofDraw {
    /// Drag a rectangle: one roof.
    #[default]
    Rectangle,
    /// Paint the tiles (any shape), then *Build roof*.
    Paint,
    /// Click inside walls: the tiles they enclose (and the walls' own), then *Build roof*.
    Fill,
}

const ROOFS: [RoofKind; 4] = [RoofKind::Gable, RoofKind::Hip, RoofKind::Shed, RoofKind::Flat];

pub struct BuildTool {
    pub mode: BuildMode,
    /// The wall prop (Walls, Room; Roof: as high as it; Doors: what comes back for a door).
    pub wall: String,
    /// The floor's material (Room; "" = the ground stays).
    pub floor: String,
    /// Room: walls on all four sides (with a doorway in the south wall) instead of the back two.
    pub all_sides: bool,
    /// Room: a roof over it too.
    pub room_roof: bool,
    /// House, Roof: the building prop whose look a new one takes.
    pub style: String,
    /// House: storeys (1-3).
    pub storeys: u8,
    pub roof: RoofKind,
    pub roof_draw: RoofDraw,
    /// Roof, painted or filled: the tiles marked for the roof so far.
    pub cells: BTreeSet<(i32, i32)>,
    /// Doors: what a click puts in, and the door prop / window wall prop for wall pieces.
    pub put: Put,
    pub door: String,
    pub window: String,
    /// *Build roof* was pressed: the map tab builds the marked tiles' roof.
    pub make: bool,
}

impl Default for BuildTool {
    fn default() -> BuildTool {
        BuildTool { mode: BuildMode::Walls, wall: String::new(), floor: String::new(), all_sides: false, room_roof: false, style: String::new(), storeys: 1, roof: RoofKind::Gable, roof_draw: RoofDraw::Rectangle, cells: BTreeSet::new(), put: Put::Door, door: String::new(), window: String::new(), make: false }
    }
}

/// Wall props as the lists offer them (the pieces a wall set turns for its sides aren't picked
/// by hand).
fn offered_wall(p: &PropDef) -> bool {
    build::is_wall(p) && !p.id.ends_with("_side")
}

/// Buildings to build like (those built here are copies of them).
fn offered_look(p: &PropDef) -> bool {
    build::is_house(p) && !p.id.contains("_built_")
}

impl BuildTool {
    /// What isn't picked yet (or is gone) becomes the first there is, so the tool builds at once.
    pub fn defaults(&mut self, db: &Database) {
        let first = |chosen: &mut String, keep: &dyn Fn(&PropDef) -> bool| {
            if !db.prop(chosen).is_some_and(keep) {
                *chosen = db.props.iter().find(|p| keep(p)).map(|p| p.id.clone()).unwrap_or_default();
            }
        };
        first(&mut self.wall, &build::is_wall);
        first(&mut self.style, &build::is_house);
        first(&mut self.door, &|p| p.door);
        first(&mut self.window, &build::is_window);
    }

    /// `mode:wall:…` for the script's `status`.
    pub fn status(&self) -> String {
        let mut s = format!("{}:{}", self.mode.label(), self.wall).to_lowercase();
        match self.mode {
            BuildMode::Walls => {}
            BuildMode::Room => s += &format!(":{}:{}{}", if self.floor.is_empty() { "-" } else { &self.floor }, if self.all_sides { "all" } else { "back" }, if self.room_roof { ":roof" } else { "" }),
            BuildMode::House => s += &format!(":{}:storeys{}", self.style, self.storeys),
            BuildMode::Roof => s += &format!(":{}:{:?}:{:?}:{}", self.style, self.roof, self.roof_draw, self.cells.len()).to_lowercase(),
            BuildMode::Doors => s += &format!(":{:?}:{}:{}", self.put, self.door, self.window).to_lowercase(),
        }
        s
    }

    /// The script's `mapbuild MODE …`: `walls [WALL]`, `room [WALL|-] [FLOOR|-] [all|back] [roof]`,
    /// `house [LOOK|-] [STOREYS]`, `roof [LOOK|-] [gable|hip|shed|flat] [rectangle|paint|fill]`,
    /// `doors door|window|gap [PROP]`, `make` (the marked tiles' roof), `clear`.
    pub fn script(&mut self, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).filter(|a| *a != "-").unwrap_or("");
        let has = |w: &str| args.iter().any(|a| a.eq_ignore_ascii_case(w));
        match arg(0) {
            "make" => self.make = true,
            "clear" => self.cells.clear(),
            mode => {
                self.mode = BuildMode::named(mode).ok_or_else(|| format!("'{mode}' is no way to build: walls, room, house, roof, doors, make, clear"))?;
                match self.mode {
                    BuildMode::Walls => {
                        if !arg(1).is_empty() {
                            self.wall = arg(1).to_string();
                        }
                    }
                    BuildMode::Room => {
                        if !arg(1).is_empty() {
                            self.wall = arg(1).to_string();
                        }
                        self.floor = arg(2).to_string();
                        self.all_sides = has("all");
                        self.room_roof = has("roof");
                    }
                    BuildMode::House => {
                        if !arg(1).is_empty() {
                            self.style = arg(1).to_string();
                        }
                        self.storeys = arg(2).parse().unwrap_or(1).clamp(1, 3);
                    }
                    BuildMode::Roof => {
                        if !arg(1).is_empty() {
                            self.style = arg(1).to_string();
                        }
                        if let Some(k) = ROOFS.into_iter().find(|k| has(&format!("{k:?}"))) {
                            self.roof = k;
                        }
                        self.roof_draw = if has("paint") {
                            RoofDraw::Paint
                        } else if has("fill") {
                            RoofDraw::Fill
                        } else {
                            RoofDraw::Rectangle
                        };
                    }
                    BuildMode::Doors => {
                        self.put = match arg(1) {
                            "window" => Put::Window,
                            "gap" => Put::Gap,
                            _ => Put::Door,
                        };
                        if !arg(2).is_empty() {
                            match self.put {
                                Put::Window => self.window = arg(2).to_string(),
                                _ => self.door = arg(2).to_string(),
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// The tool's panel beside the map.
    pub fn panel(&mut self, ui: &mut egui::Ui, p: &mut Parts) {
        self.defaults(&p.ws.project.db);
        ui.horizontal_wrapped(|ui| {
            for m in BuildMode::ALL {
                if ui.selectable_label(self.mode == m, m.label()).on_hover_text(m.help()).clicked() {
                    self.mode = m;
                }
            }
        });
        ui.weak(self.mode.help());
        ui.separator();
        let rows = |p: &Parts, keep: &dyn Fn(&PropDef) -> bool| -> Vec<(String, String)> { p.ws.project.db.props.iter().filter(|d| keep(d)).map(|d| (d.id.clone(), d.name.clone())).collect() };
        // (Each list as high as what is under it leaves room for.)
        let list = |ui: &mut egui::Ui, p: &mut Parts, title: &str, rows: &[(String, String)], chosen: &mut String, last: bool| {
            ui.label(title);
            if rows.is_empty() {
                ui.weak("This project has none yet.");
                return;
            }
            let area = egui::ScrollArea::vertical().id_salt(title).auto_shrink([false, !last]);
            let area = if last { area } else { area.max_height(190.0) };
            area.show(ui, |ui| {
                for (id, label) in rows {
                    let r = ui
                        .horizontal(|ui| {
                            if let Some(rs) = p.rs {
                                crate::thumbs::boxed(ui, 30.0, || p.thumbs.of(p.ws, rs, RefKind::Prop, id));
                            }
                            ui.add(egui::Button::selectable(chosen == id, if label.is_empty() { id.as_str() } else { label.as_str() }).truncate()).on_hover_text(id)
                        })
                        .inner;
                    if r.clicked() {
                        *chosen = id.clone();
                    }
                    if r.double_clicked() {
                        p.asks.push(Ask::OpenDef(RefKind::Prop, id.clone()));
                    }
                }
            });
        };
        match self.mode {
            BuildMode::Walls => {
                let walls = rows(p, &offered_wall);
                list(ui, p, "Wall", &walls, &mut self.wall, true);
            }
            BuildMode::Room => {
                ui.checkbox(&mut self.all_sides, "Walls on all four sides").on_hover_text("Off: only the back walls (north and west), open to the viewer");
                ui.checkbox(&mut self.room_roof, "With a roof").on_hover_text("A roof over the room, in the look and of the kind chosen in the Roof mode");
                let materials = p.lists.iter().find(|(k, _)| *k == RefKind::Material).map(|(_, l)| l.as_slice()).unwrap_or(&[]);
                let shown = match materials.iter().find(|(id, _)| *id == self.floor) {
                    Some((id, label)) => if label.is_empty() { id.clone() } else { label.clone() },
                    None => "The ground stays".to_string(),
                };
                egui::ComboBox::from_label("Floor").selected_text(shown).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.floor, String::new(), "The ground stays");
                    for (id, label) in materials {
                        ui.selectable_value(&mut self.floor, id.clone(), if label.is_empty() { id } else { label });
                    }
                });
                let walls = rows(p, &offered_wall);
                list(ui, p, "Wall", &walls, &mut self.wall, true);
            }
            BuildMode::House => {
                ui.horizontal(|ui| {
                    ui.label("Storeys");
                    for n in 1..=3u8 {
                        ui.selectable_value(&mut self.storeys, n, n.to_string());
                    }
                });
                let looks = rows(p, &offered_look);
                list(ui, p, "Built like", &looks, &mut self.style, true);
            }
            BuildMode::Roof => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Mark");
                    ui.selectable_value(&mut self.roof_draw, RoofDraw::Rectangle, "Rectangle").on_hover_text("Drag a rectangle: a roof over it at once");
                    ui.selectable_value(&mut self.roof_draw, RoofDraw::Paint, "Paint tiles").on_hover_text("Paint the roof's tiles by hand (any shape: an L, a T, a cross), then Build roof. A drag that starts on a marked tile clears instead; right-click clears one");
                    ui.selectable_value(&mut self.roof_draw, RoofDraw::Fill, "Inside walls").on_hover_text("Click inside a room you built of walls: its tiles and those of its walls are marked; then Build roof");
                });
                if self.roof_draw != RoofDraw::Rectangle {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} tiles marked", self.cells.len()));
                        if ui.add_enabled(!self.cells.is_empty(), egui::Button::new("Build roof")).on_hover_text("The marked tiles get a roof: cut into rectangles, joined where they meet so their roofs run into each other (Enter)").clicked() {
                            self.make = true;
                        }
                        if ui.add_enabled(!self.cells.is_empty(), egui::Button::new("Clear")).clicked() {
                            self.cells.clear();
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Kind");
                    for k in ROOFS {
                        ui.selectable_value(&mut self.roof, k, format!("{k:?}"));
                    }
                });
                let looks = rows(p, &offered_look);
                list(ui, p, "Roofed like", &looks, &mut self.style, false);
                let walls = rows(p, &|d| offered_wall(d) && d.block > 0.0);
                list(ui, p, "As high as (where no wall stands under it)", &walls, &mut self.wall, true);
            }
            BuildMode::Doors => {
                // (Named apart from the props in the lists below: a door prop is called "Door".)
                ui.horizontal_wrapped(|ui| {
                    ui.label("Put in");
                    ui.selectable_value(&mut self.put, Put::Door, "A door");
                    ui.selectable_value(&mut self.put, Put::Window, "A window");
                    ui.selectable_value(&mut self.put, Put::Gap, "A gap");
                });
                match self.put {
                    Put::Door => {
                        let doors = rows(p, &|d| d.door);
                        list(ui, p, "Door for wall pieces (houses take their own)", &doors, &mut self.door, false);
                    }
                    Put::Window => {
                        let windows = rows(p, &|d| build::is_window(d) && !d.id.ends_with("_side"));
                        list(ui, p, "Window piece for walls (houses take their own)", &windows, &mut self.window, false);
                    }
                    Put::Gap => {
                        ui.weak("A click takes the wall piece away: a doorway.");
                    }
                }
                let walls = rows(p, &offered_wall);
                list(ui, p, "Wall that comes back when a door is taken away", &walls, &mut self.wall, true);
            }
        }
    }
}
