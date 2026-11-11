//! R(ust)PG2D editor: maps, entities, zones, transitions, definitions and scripts.
//!
//! Usage: rustpg2d-editor [PROJECT_DIR]   (without a folder the welcome screen shows)
//!        rustpg2d-editor --library [DIR]  (the Asset Library window; default: the shared library)
// No console window on Windows (logs go to the terminal it was started from, if any).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod art;
mod builder;
mod canvas;
mod clipboard;
mod database;
mod db_undo;
mod doc;
mod export;
mod package;
mod files;
mod graph;
mod import;
mod library_ui;
mod library_update;
mod library_window;
mod maps;
mod mcp;
mod multisel;
mod models;
mod models_tab;
mod new_project;
mod frame_ops;
mod pixel;
mod problems;
mod script_editor;
mod search;
mod structures;
mod terrain;
mod ui;
mod wizard;
mod widgets;

use std::collections::VecDeque;

use app::{Editor, Mode, Pending, Selection, Tool};
use macroquad::prelude::*;

thread_local! {
    /// Scripted mouse position held by `hover X Y` (until the next `click`).
    static HOVER: std::cell::Cell<Option<canvas::Mouse>> = const { std::cell::Cell::new(None) };
}

/// `--library [DIR]`: run as the Asset Library window (DIR defaults to the shared library).
fn library_arg() -> Option<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "--library")?;
    Some(args.get(i + 1).filter(|a| !a.starts_with("--")).map_or_else(rustpg2d_engine::library::library_dir, std::path::PathBuf::from))
}

fn window_conf() -> macroquad::conf::Conf {
    rustpg2d_engine::desktop::startup("The R(ust)PG2D editor");
    let library = library_arg().is_some();
    let window = Conf {
        icon: Some(rustpg2d_engine::icon::window_icon(&rustpg2d_engine::icon::emblem())),
        window_title: if library { "R(ust)PG2D – Asset Library" } else { "R(ust)PG2D editor" }.to_owned(),
        window_width: if library { 1300 } else { 1600 },
        window_height: 900,
        high_dpi: true,
        window_resizable: true,
        ..Default::default()
    };
    macroquad::conf::Conf {
        miniquad_conf: window,
        update_on: None,
        default_filter_mode: FilterMode::Linear,
        // 3D character models are drawn as one mesh each (render/model.rs).
        draw_call_vertex_capacity: rustpg2d_engine::render::model::DRAW_VERTICES,
        draw_call_index_capacity: rustpg2d_engine::render::model::DRAW_INDICES,
    }
}

/// egui-macroquad divides wheel deltas by 90 before handing them to egui, but macroquad on X11
/// reports one wheel notch as 1.0 — so with egui's defaults a notch scrolled half a pixel.
const EMQ_WHEEL_DIVISOR: f32 = 90.0;
/// Scroll distance of one mouse-wheel notch in egui panels (about three lines of text).
const WHEEL_POINTS_PER_NOTCH: f32 = 50.0;
/// What macroquad reports for one wheel notch: 1 on Linux, Windows' `WHEEL_DELTA` (120) there,
/// about 10 on macOS (miniquad multiplies line deltas by 10; trackpads report points, many small
/// events). The map and pixel editor zoom by `delta / WHEEL_NOTCH` steps.
pub const WHEEL_NOTCH: f32 = if cfg!(windows) {
    120.0
} else if cfg!(target_os = "macos") {
    10.0
} else {
    1.0
};
/// egui scroll distance per unit macroquad reports: a notch scrolls `WHEEL_POINTS_PER_NOTCH`; on
/// macOS a trackpad's point deltas scroll 2 points each (a wheel notch, ~10, then 20).
const WHEEL_POINTS_PER_UNIT: f32 = if cfg!(target_os = "macos") { 2.0 } else { WHEEL_POINTS_PER_NOTCH / WHEEL_NOTCH };
/// One wheel notch in egui's `raw_scroll_delta` (points).
pub const EGUI_NOTCH: f32 = WHEEL_POINTS_PER_UNIT * WHEEL_NOTCH;

#[macroquad::main(window_conf)]
async fn main() {
    // With a folder argument that project opens directly; otherwise the welcome screen shows.
    // (macOS passes `-psn_…` to apps started from Finder on some versions: not a project.)
    let mut ed = match (library_arg(), std::env::args().nth(1).filter(|a| !a.starts_with("-psn_"))) {
        (Some(dir), _) => Editor::library_window(dir.clone()).unwrap_or_else(|e| {
            let mut ed = Editor::empty();
            ed.set_status(format!("Cannot open the asset library at {}: {e:#}", dir.display()));
            ed
        }),
        (None, Some(root)) => Editor::new(root.clone().into()).unwrap_or_else(|e| {
            let mut ed = Editor::empty();
            ed.set_status(format!("Cannot open project '{root}': {e:#}"));
            ed
        }),
        (None, None) => Editor::empty(),
    };
    prevent_quit();
    let settings = files::load_settings();
    ed.mcp.port = settings.mcp_port;
    ed.mcp.autostart = settings.mcp_autostart;
    ed.art_settings = art::ArtSettings { model: settings.meshy_model.clone(), budget: settings.meshy_budget, ask: settings.meshy_ask };
    ed.mcp_autostart();
    egui_macroquad::cfg(|ctx| {
        ctx.options_mut(|o| o.line_scroll_speed = WHEEL_POINTS_PER_UNIT * EMQ_WHEEL_DIVISOR);
        ui::install_fonts(ctx);
    });
    // Debug aid: RUSTPG2D_EDITOR_SCRIPT="tool entity; select entity 3; wait 0.5; shot a.png; quit"
    let mut script: VecDeque<Vec<String>> = std::env::var("RUSTPG2D_EDITOR_SCRIPT")
        .map(|s| s.split(';').map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>()).filter(|c| !c.is_empty()).collect())
        .unwrap_or_default();
    let mut wait = 0.3f32;
    let mut shot: Option<String> = None;
    let mut mouse_frames: VecDeque<canvas::Mouse> = VecDeque::new();
    loop {
        if is_quit_requested() {
            ed.request(Pending::Quit);
        }
        if !script.is_empty() || !mouse_frames.is_empty() {
            wait -= get_frame_time();
            while wait <= 0.0 && shot.is_none() && mouse_frames.is_empty() {
                let Some(cmd) = script.pop_front() else { break };
                run_script_cmd(&mut ed, &cmd, &mut wait, &mut shot, &mut mouse_frames);
            }
        }
        // AI agents' tool calls (Assets > AI agent server...), like script commands.
        ed.mcp_drain();
        ed.assets.models.begin_frame();
        if let Some(id) = database::take_icon_request() {
            let m = ed.render_item_icon(&id);
            ed.set_status(m);
        }
        canvas::inject_mouse(mouse_frames.pop_front().or(HOVER.with(|h| h.get())));
        egui_macroquad::ui(|ctx| ed.ui(ctx));
        ed.update_canvas();
        ed.draw_canvas();
        egui_macroquad::draw();
        if let Some(p) = shot.take() {
            get_screen_data().export_png(&p);
        }
        ed.mcp_capture_shots();
        if ed.quit {
            ed.mcp.stop();
            break;
        }
        next_frame().await;
    }
}

fn run_script_cmd(ed: &mut Editor, cmd: &[String], wait: &mut f32, shot: &mut Option<String>, mouse: &mut VecDeque<canvas::Mouse>) {
    let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
    let num = |i: usize| arg(i).parse::<f32>().unwrap_or(0.0);
    // Screen position of the center of tile (x, y).
    let at = |ed: &Editor, x: f32, y: f32| {
        let ts = ed.ts();
        ed.camera().world_to_screen(vec2((x + 0.5) * ts, (y + 0.5) * ts))
    };
    let frame = |pos: Vec2, b: usize, down: bool, pressed: bool, released: bool| {
        let mut m = canvas::Mouse { pos, ..Default::default() };
        m.down[b] = down;
        m.pressed[b] = pressed;
        m.released[b] = released;
        m
    };
    match arg(0) {
        // click X Y [right]  /  drag X0 Y0 X1 Y1   (tile coordinates)
        "click" => {
            HOVER.with(|h| h.set(None));
            let b = if arg(3) == "right" { 1 } else { 0 };
            let p = at(ed, num(1), num(2));
            mouse.push_back(frame(p, b, true, true, false));
            mouse.push_back(frame(p, b, false, false, true));
            mouse.push_back(frame(p, b, false, false, false));
        }
        // hover X Y: keep the mouse over a tile for a while (tool previews)
        "hover" => {
            let p = at(ed, num(1), num(2));
            HOVER.with(|h| h.set(Some(frame(p, 0, false, false, false))));
        }
        "drag" => {
            let (a, b) = (at(ed, num(1), num(2)), at(ed, num(3), num(4)));
            mouse.push_back(frame(a, 0, true, true, false));
            for i in 1..=12 {
                mouse.push_back(frame(a.lerp(b, i as f32 / 12.0), 0, true, false, false));
            }
            mouse.push_back(frame(b, 0, false, false, true));
            mouse.push_back(frame(b, 0, false, false, false));
        }
        "brush" => {
            ed.brush.x = num(1) as u32;
            ed.brush.y = num(2) as u32;
            ed.brush.w = (num(3) as u32).max(1);
            ed.brush.h = (num(4) as u32).max(1);
        }
        "layer" => ed.layer = num(1) as usize,
        // containeradd ITEM COUNT / containersize N: edit the selected entity's container
        "containeradd" | "containersize" => {
            if let (Some(doc), Selection::Entity(uid)) = (&mut ed.doc, ed.sel) {
                let before = doc::Objects::of(&doc.map);
                let def = ed.project.db.clone();
                if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                    let default = def.container_of(e).cloned().unwrap_or_default();
                    let spec = e.container.get_or_insert(default);
                    if arg(0) == "containeradd" {
                        spec.items.push(rustpg2d_engine::inventory::ItemStack::new(arg(1), num(2) as u32));
                    } else {
                        spec.size = num(1) as u32;
                    }
                }
                doc.record_objects(before, "script");
            }
        }
        // spawnermode never|enter|SECONDS  (selected zone)
        "spawnermode" => {
            use rustpg2d_engine::map::{Respawn, ZoneKind};
            if let (Some(doc), Selection::Zone(uid)) = (&mut ed.doc, ed.sel) {
                if let Some(ZoneKind::Spawner { respawn, .. }) = doc.map.zones.iter_mut().find(|z| z.uid == uid).map(|z| &mut z.kind) {
                    *respawn = match arg(1) {
                        "never" => Respawn::Never,
                        "enter" => Respawn::OnEnter,
                        s => Respawn::After { seconds: s.parse().unwrap_or(10.0) },
                    };
                    doc.mark_changed();
                }
            }
        }
        "place" => {
            ed.place_kind = match arg(1) {
                "character" => rustpg2d_engine::entity::EntityKind::Character,
                "item" => rustpg2d_engine::entity::EntityKind::Item,
                _ => rustpg2d_engine::entity::EntityKind::Prop,
            };
            ed.place_def = arg(2).to_string();
        }
        // unplace: stop placing (like right-click / Esc)
        "unplace" => ed.disarm(),
        // cycle +1|-1: next / previous definition in the palette
        "cycle" => ed.cycle_place(num(1) as i32),
        // filter TEXT / tabkind all|prop|character|item
        "filter" => ed.place_filter = arg(1).to_string(),
        "zonekind" => {
            ed.zone_template = match arg(1) {
                "portal" => app::ZoneTemplate::Portal,
                "trigger" => app::ZoneTemplate::Trigger,
                "spawner" => app::ZoneTemplate::Spawner,
                "transition" => app::ZoneTemplate::Transition,
                _ => app::ZoneTemplate::Spawn,
            }
        }
        // Clipboard: region X Y W H (select tiles) · copy · cut · paste X Y (top-left corner)
        // · paste (arm it: the ghost follows `hover`, clicks paste) · unpaste
        "region" => {
            if let Some(r) = ed.doc.as_ref().and_then(|d| clipboard::clip_rect(&d.map, (num(1) as i32, num(2) as i32), (num(1) as i32 + num(3) as i32 - 1, num(2) as i32 + num(4) as i32 - 1))) {
                ed.sel = Selection::Region(r);
            }
        }
        "copy" => {
            ed.copy_selection();
            println!("[script] {}", ed.status.0);
        }
        "cut" => {
            ed.cut_selection();
            println!("[script] {}", ed.status.0);
        }
        "paste" if cmd.len() >= 3 => {
            ed.paste_at((num(1) as i32, num(2) as i32));
            println!("[script] {}", ed.status.0);
        }
        "paste" => ed.start_paste(),
        "unpaste" => ed.stop_paste(),
        // terrain NAME: arms a terrain (any tileset) and the Terrain tool
        "terrain" => {
            if !ed.select_terrain_named(&cmd[1..].join(" ")) {
                println!("[script] terrain: no terrain named '{}'", cmd[1..].join(" "));
            }
        }
        // terrainpaint X Y [erase]: one Terrain-tool click (erase = Shift+click), one undo step
        "terrainpaint" => {
            if let Some(d) = &mut ed.doc {
                d.begin_stroke(ed.layer, false);
            }
            ed.paint_terrain_at((num(1) as i32, num(2) as i32), arg(3) == "erase");
            if let Some(d) = &mut ed.doc {
                d.end_stroke();
            }
        }
        // terrainedit K [block]: Database › Tilesets: edit terrain K of the selected tileset
        // tilepick low|ladder|light|off: the Tilesets tab's grid pick mode
        "tilepick" => {
            ed.db.tile_low_pick = arg(1) == "low";
            ed.db.tile_ladder_pick = arg(1) == "ladder";
            ed.db.tile_light_pick = arg(1) == "light";
        }
        "terrainedit" => ed.db.terrain_pick = Some(terrain::TerrainPick { terrain: num(1) as usize, slot: 0, block: arg(2) == "block" }),
        // terrainblob TILESET NAME on|off: the terrain's Corners (47 tiles) toggle
        "terrainblob" => {
            let name = cmd.get(2..cmd.len().saturating_sub(1)).map(|n| n.join(" ")).unwrap_or_default();
            let found = ed.project.db.tilesets.iter_mut().find(|d| d.id == arg(1)).and_then(|d| d.terrains.iter_mut().find(|t| t.name.eq_ignore_ascii_case(&name)));
            match found {
                Some(t) => {
                    t.set_blob(cmd.last().is_some_and(|l| *l != "off"));
                    ed.defs_dirty = true;
                }
                None => println!("[script] terrainblob: no terrain {name} in {}", arg(1)),
            }
        }
        // Events mode: graph scope map|selection · graph functions map|all · graphnode N
        // (select node N, -1 = none) · graphedit N (toggle its editors) · graphtext N K TEXT
        // (text of action K of trigger node N) · graphdump (nodes with summaries)
        "graph" => match (arg(1), arg(2)) {
            ("scope", s) => ed.graph.focus_selection = s == "selection",
            ("functions", f) => ed.graph.all_functions = f == "all",
            _ => println!("[script] graph scope map|selection · graph functions map|all"),
        },
        "graphview" => ed.graph_view_node(num(1) as usize),
        "graphnode" => ed.graph.selected = arg(1).parse::<usize>().ok().and_then(|i| ed.graph.order.get(i).cloned()),
        "graphedit" => {
            if let Some(k) = arg(1).parse::<usize>().ok().and_then(|i| ed.graph.order.get(i).cloned()) {
                if !ed.graph.editing.remove(&k) {
                    ed.graph.editing.insert(k);
                }
            }
        }
        "graphtext" => {
            let ok = ed.graph_set_text(num(1) as usize, num(2) as usize, &cmd[3..].join(" "));
            println!("[script] graphtext: {ok}");
        }
        "graphdump" => {
            for l in ed.graph_dump() {
                println!("[graph] {l}");
            }
        }
        "undo" => {
            if let Some(d) = &mut ed.doc {
                d.undo(&ed.assets);
            }
        }
        "redo" => {
            if let Some(d) = &mut ed.doc {
                d.redo(&ed.assets);
            }
        }
        // dbundo / dbredo: undo / redo a definition change (the Database mode's Ctrl+Z / Ctrl+Y)
        "dbundo" => ed.db_undo(),
        "dbredo" => ed.db_redo(),
        "save" => ed.save_all(),
        // play [start]: F5 (from the camera / selected spawn) or F6 (project start); the game
        // inherits the environment, so RUSTPG2D_AUTOPLAY can drive the play-test
        "play" => ed.play(arg(1) != "start"),
        // Same code paths as the file dialogs, without the dialog.
        // import tiled FILE [NAME] · import lpc FILE [ID] [body|sprite] ·
        // import aseprite FILE [ID]: the File › Import… imports (result printed and shown)
        "import" if matches!(arg(1), "tiled" | "lpc" | "aseprite") => {
            use import::ImportKind;
            use rustpg2d_engine::defs::SheetKind;
            let kind = match arg(1) {
                "tiled" => ImportKind::Tiled,
                "lpc" => ImportKind::Lpc,
                _ => ImportKind::Aseprite,
            };
            let sheet_kind = match arg(4) {
                "sprite" => SheetKind::Sprite,
                _ => SheetKind::Body,
            };
            // (import KIND FILE [ID] [SHEETKIND] [scale=N])
            let scale = cmd.iter().find_map(|a| a.strip_prefix("scale=")).and_then(|v| v.parse().ok()).unwrap_or(1);
            ed.import = Some(import::ImportDialog { kind, id: arg(3).to_string(), sheet_kind, female: false, scale });
            ed.import_and_report(kind, std::path::Path::new(arg(2)));
            println!("[import] {}", ed.import_report.as_ref().map_or(String::new(), |r| r.1.clone()));
        }
        // importdialog: opens File › Import…
        "importdialog" => ed.import = Some(Default::default()),
        "import" => {
            let purpose = match arg(1) {
                "tileset" => files::Purpose::TilesetImage(arg(2).into()),
                "sheet" => files::Purpose::SheetImage(arg(2).into()),
                _ => files::Purpose::SoundFile(arg(2).into()),
            };
            ed.handle_file(purpose, arg(3).into());
        }
        // 3D models: modelimport FILE (a new model) · modelclips ID FILE (more clips) · modelfile ID FILE
        "modelimport" => ed.handle_file(files::Purpose::ModelFile { id: String::new(), clips: false }, arg(1).into()),
        "modelclips" => ed.handle_file(files::Purpose::ModelFile { id: arg(1).into(), clips: true }, arg(2).into()),
        "modelfile" => ed.handle_file(files::Purpose::ModelFile { id: arg(1).into(), clips: false }, arg(2).into()),
        // itemmodel ITEM FILE: the item's 3D model (where it is held estimated).
        "itemmodel" => ed.handle_file(files::Purpose::ItemModel(arg(1).into()), arg(2).into()),
        // itemicon ITEM: its inventory icon rendered from its 3D model.
        "itemicon" => {
            let m = ed.render_item_icon(arg(1));
            println!("[script] {m}");
        }
        "newproject" => ed.handle_file(files::Purpose::NewProject, arg(1).into()),
        "openproject" => ed.handle_file(files::Purpose::OpenProject, arg(1).into()),
        "closeproject" => ed.request(Pending::CloseProject),
        // Maps: linkarea X Y W H · linknext TARGET · goto MAP [ZONE] · mapnext · zones
        "linkarea" => ed.link_area((num(1) as u32, num(2) as u32, num(3) as u32, num(4) as u32)),
        "linknext" => ed.link_next(arg(1)),
        "goto" => ed.goto_map(arg(1), (!arg(2).is_empty()).then_some(arg(2))),
        "mapnext" => ed.cycle_map(1),
        // trdraw ZONE MAP · trlink ZONE MAP OTHER · trunlink ZONE (transition inspector actions)
        "trdraw" | "trlink" | "trunlink" => {
            let r = match arg(0) {
                "trdraw" => maps::TransitionReq::DrawOther { zone: arg(1).into(), map: arg(2).into() },
                "trlink" => maps::TransitionReq::LinkBack { zone: arg(1).into(), map: arg(2).into(), other: arg(3).into() },
                _ => maps::TransitionReq::Unlink { zone: arg(1).into() },
            };
            maps::request_transition(r);
            ed.handle_transition_request();
            println!("[script] {}", ed.status.0);
        }
        "quickswitch" => ed.quick.open = true,
        "zones" => {
            if let Some(d) = &ed.doc {
                for z in &d.map.zones {
                    println!("[zones] {}: #{} {} {:?} at {},{} {}x{}", d.map.name, z.uid, z.name, z.kind, z.x, z.y, z.w, z.h);
                }
            }
        }
        // chooser FIELD_ID: opens a sprite chooser (e.g. item_icon)
        "chooser" => widgets::request_chooser(arg(1)),
        // art on|off · newframe [W H] · newcharsheet [TEMPLATE_SHEET]  (Database item / prop / character)
        "art" => ed.db.art_open = arg(1) != "off",
        "newframe" => {
            let cells = if num(1) > 0.0 { (num(1) as u32, num(2) as u32) } else { (1, 1) };
            let tab = ed.db.tab;
            ed.art_action(database::ArtAction::NewFrame { cells }, tab);
            println!("[script] {}", ed.status.0);
        }
        // Character editor: charnew [ID NAME] (New character…) · charcustom (selected placed NPC) ·
        // charset KEY VALUE (body|color FEATURE RRGGBB|-|clothing|head|armor|weapon|offhand|
        // carry ITEMxN|random|id|name|source existing|draw|import|ai|size SIZE|from SHEET|-|
        // female on|off; on the open window, else the selected character) · charrandom ·
        // chartemplate ID (project or library character) · charcreate / charapply · charcancel ·
        // charreset (selected NPC uses the definition's look)
        "charnew" => {
            ed.open_builder(builder::Target::New);
            if !arg(1).is_empty() {
                if let Some(b) = &mut ed.builder {
                    b.id = arg(1).to_string();
                    b.name = cmd[2..].join(" ");
                }
            }
        }
        "charcustom" => {
            if let Selection::Entity(uid) = ed.sel {
                ed.open_builder(builder::Target::Instance(uid));
            }
        }
        "charset" => {
            if let Err(e) = ed.builder_set(arg(1), &cmd[2..].join(" ")) {
                println!("[script] charset: {e}");
            }
        }
        "charrandom" => {
            if let Err(e) = ed.builder_set("random", "") {
                println!("[script] charrandom: {e}");
            }
        }
        "chartemplate" => {
            use crate::models::{ModelPick, ModelTarget};
            let target = if ed.builder.is_some() {
                ModelTarget::Builder
            } else if let Selection::Entity(uid) = ed.sel {
                ModelTarget::Instance(uid)
            } else {
                ModelTarget::Def(ed.project.db.characters.get(ed.db.sel).map(|d| d.id.clone()).unwrap_or_default())
            };
            let pick = if ed.project.db.character(arg(1)).is_some() { ModelPick::Project(arg(1).to_string()) } else { ModelPick::Library(arg(1).to_string()) };
            let r = ed.apply_model(&target, &pick);
            println!("[script] chartemplate: {r:?}");
            ed.show_result(r);
        }
        "charcreate" | "charapply" => match ed.apply_builder() {
            Ok(m) => {
                println!("[script] {m}");
                ed.set_status(m);
            }
            Err(e) => println!("[script] {}: {e:#}", arg(0)),
        },
        "charcancel" => ed.builder = None,
        // playable SHEET on|off: may players choose that body in the character creator?
        "playable" => builder::request_playable(arg(1), arg(2) != "off"),
        "charreset" => {
            if let Selection::Entity(uid) = ed.sel {
                ed.reset_instance_look(uid);
            }
        }
        // exportgame OUT_DIR [NAME]  /  exportdialog
        // exportpkg OUT_DIR appimage|flatpak|all [APP_ID]: the folder plus Linux packages
        // exportgame OUT [NAME] (Linux) · exportwin (Windows) · exportmac (macOS) · exportweb (web)
        // · exportall (all four); all with zips
        "exportgame" | "exportpkg" | "exportwin" | "exportmac" | "exportweb" | "exportall" => {
            let mut opts = export::ExportOpts { zip: arg(0) != "exportpkg", ..Default::default() };
            if arg(0) == "exportwin" {
                (opts.linux, opts.windows) = (false, true);
            } else if arg(0) == "exportmac" {
                (opts.linux, opts.macos) = (false, true);
            } else if arg(0) == "exportweb" {
                (opts.linux, opts.web) = (false, true);
            } else if arg(0) == "exportall" {
                (opts.linux, opts.windows, opts.macos, opts.web) = (true, true, true, true);
            }
            let mut name = ed.project.config.name.clone();
            if arg(0) != "exportpkg" {
                if !arg(2).is_empty() {
                    name = cmd[2..].join(" ");
                }
            } else {
                opts.appimage = matches!(arg(2), "appimage" | "all");
                opts.flatpak = matches!(arg(2), "flatpak" | "all");
                opts.app_id = if arg(3).is_empty() { package::default_app_id(&rustpg2d_engine::release::slug(&name)) } else { arg(3).to_string() };
            }
            match ed.run_export(&name, arg(1), &opts) {
                Ok(m) => println!("[export] {m}"),
                Err(e) => println!("[export] error: {e}"),
            }
        }
        // langtemplate CODE: Database › Project › Languages › Export translation template
        "langtemplate" => match ed.export_translation(arg(1)) {
            Ok(m) => println!("[lang] {m}"),
            Err(e) => println!("[lang] error: {e}"),
        },
        // playlang CODE|-: the play-test language
        "playlang" => ed.play_lang = if arg(1) == "-" { String::new() } else { arg(1).to_string() },
        // exportdialog [appimage] [flatpak] [windows] [macos] [web]: opens File → Export game with those packages ticked
        // example: the welcome screen's "Example project" (packaged R(ust)PG2D only)
        "example" => ed.open_example(),
        // browse open|folder|icon: opens that file dialog like its button (a person answers it)
        "browse" => {
            ed.browse_request = Some(match arg(1) {
                "folder" => files::Purpose::ExportGameFolder,
                "icon" => files::Purpose::ExportIcon,
                _ => files::Purpose::OpenProject,
            })
        }
        "exportdialog" => {
            ed.open_export();
            if let Some(d) = &mut ed.export {
                d.opts.web = cmd.iter().any(|a| a == "web");
                if cmd.iter().any(|a| a == "windows") {
                    d.opts.windows = true;
                }
                if cmd.iter().any(|a| a == "macos") {
                    d.opts.macos = true;
                }
                d.opts.appimage = cmd.iter().any(|a| a == "appimage");
                d.opts.flatpak = cmd.iter().any(|a| a == "flatpak");
            }
        }
        // pending save|discard|cancel: answers the "Unsaved changes" question
        "pending" => {
            if let Some(p) = ed.pending.take() {
                let c = match arg(1) {
                    "save" => app::Choice::Save,
                    "discard" => app::Choice::Discard,
                    _ => app::Choice::Cancel,
                };
                ed.settle_pending(p, c);
            }
        }
        // mcp on [PORT] | off: the AI agent server; mcpwindow opens its window
        "mcp" => match arg(1) {
            "on" => {
                let port = if arg(2).is_empty() { ed.mcp.port } else { num(2) as u16 };
                if let Err(e) = ed.mcp.start(port) {
                    println!("[mcp] error: {e}");
                }
            }
            "off" => ed.mcp.stop(),
            _ => println!("[mcp] usage: mcp on [PORT] | off"),
        },
        "mcpwindow" => ed.mcp.window_open = true,
        // watchanswer reload|keep: answers the "changed on disk" banner
        "watchanswer" => ed.watch_answer(arg(1) == "reload"),
        "status" => println!(
            "[status] script_pick={} open={} project={} pending={} map={:?} fog={} dirty={} library_mode={} defs_dirty={} dbundo={} multi={} lib_dialog={} builder={} preview={:?} music={:?} sel={:?} schedule={} clipboard={} pasting={} graph={},{} mcp={} watch={} notices={} search={} usages={} problems={} libupdate={} art={} pixel={} mode={:?} dbtab={:?} dbsel={} script={:?} msg={}",
            ed.lib.script_pick.as_ref().map_or("closed".to_string(), |p| format!("{:?} needed={:?} picked={:?} missing={:?}", p.to, p.needed.keys().collect::<Vec<_>>(), p.picked, p.missing)),
            ed.project_open,
            ed.project.root.display(),
            ed.pending.is_some(),
            ed.doc.as_ref().map(|d| d.map.name.clone()),
            ed.doc.as_ref().is_some_and(|d| d.map.fog),
            ed.doc.as_ref().is_some_and(|d| d.is_dirty()),
            ed.library_mode,
            ed.defs_dirty,
            ed.db_hist.status(),
            ed.selected_set().len(),
            ed.lib.add.as_ref().map_or("closed".to_string(), |d| format!("{:?}:{}", d.kind, d.picked.len())),
            ed.builder_status(),
            ed.audio.previewing(),
            ed.audio.current_music(),
            ed.sel,
            schedule_text(ed),
            ed.clipboard.as_ref().map_or("empty".to_string(), |c| format!("{}x{},{}ents", c.w, c.h, c.entities.len())),
            ed.pasting,
            ed.graph.counts.0,
            ed.graph.counts.1,
            ed.mcp.status(),
            ed.watch.status(),
            ed.notices.len(),
            ed.search.status(),
            ed.usages.status(),
            ed.problems.status(),
            ed.lib_update.status(),
            ed.art.status(),
            ed.px.status(),
            ed.mode,
            ed.db.tab,
            ed.db.sel,
            ed.db.script_file.as_ref().and_then(|f| f.file_name()).map(|f| f.to_string_lossy().to_string()),
            ed.status.0
        ),
        "filedialog" => ed.open_file_dialog(files::Purpose::OpenProject),
        "rename" => match rustpg2d_engine::refactor::RefKind::parse(arg(1)) {
            Some(kind) => ed.rename_id(kind, arg(2), arg(3)),
            None => eprintln!("[script] rename: unknown kind {:?} (tileset, sheet, item, prop, character, sound, map, structure, skill, spell, recipe, quest, achievement)", arg(1)),
        },
        "newmap" => {
            ed.new_map = Some(app::NewMapDialog { name: arg(1).into(), width: num(2) as u32, height: num(3) as u32, fill: true });
        }
        "dump" => {
            if let Some(d) = &ed.doc {
                let m = &d.map;
                let tile = |l: usize, x: u32, y: u32| m.layers[l].get(x, y).parts().map_or("-".to_string(), |(_, i)| i.to_string());
                println!(
                    "[dump] map={} {}x{} dirty={} sel={:?} ents={} zones={:?} tile(L{},{},{})={} col={}",
                    m.name,
                    m.width,
                    m.height,
                    d.is_dirty(),
                    ed.sel,
                    m.entities.iter().map(|e| format!("{}@{},{}", e.def, e.x, e.y)).collect::<Vec<_>>().join(" "),
                    m.zones.iter().map(|z| format!("{}:{}({},{} {}x{})", z.name, z.kind.label(), z.x, z.y, z.w, z.h)).collect::<Vec<_>>(),
                    num(1),
                    num(2),
                    num(3),
                    tile(num(1) as usize, num(2) as u32, num(3) as u32),
                    m.collision[(num(3) as u32 * m.width + num(2) as u32) as usize],
                );
            }
        }
        "wait" => *wait = num(1),
        "shot" => *shot = Some(arg(1).to_string()),
        "quit" => ed.quit = true,
        "open" => ed.open_map(arg(1)),
        "mode" => {
            ed.mode = match arg(1) {
                "db" => Mode::Database,
                "library" | "browse" => Mode::Library,
                "pixel" => Mode::Pixel,
                "events" => Mode::Events,
                _ => Mode::Map,
            }
        }
        // Library / packages: libsel KIND ID · libadd · tolib KIND ID · libexport PATH ·
        // libimport PATH (into the library) · projimport PATH (into the project)
        // libfilter KIND|all: the Browse view's kind filter
        "libfilter" => ed.lib.filter = library_ui::parse_kind(arg(1)),
        "libsel" => {
            ed.lib.ensure_loaded();
            if let Some(k) = library_ui::parse_kind(arg(1)) {
                ed.lib.selected.insert((k, arg(2).to_string()));
            }
        }
        // libremove: the "Remove from library" button (opens the confirmation) · libconfirm yes|no
        "libremove" => ed.lib.confirm_remove = Some(ed.lib.selected.iter().cloned().collect()),
        "libconfirm" => {
            if let Some(list) = ed.lib.confirm_remove.take() {
                if arg(1) == "yes" {
                    ed.remove_from_library(&list);
                }
            }
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        // Asset Library window / add dialog: libwindow · libdialog [KIND] · libpick KIND ID ·
        // libdialogadd · libdialogclose · libedit KIND ID (library mode: edit that asset)
        "libwindow" => ed.open_library_window(),
        // mapfog on|off: the open map's fog of war (like the inspector checkbox, one undo step)
        "mapfog" => {
            if let Some(doc) = &mut ed.doc {
                let before = doc::Objects::of(&doc.map);
                doc.map.fog = arg(1) != "off";
                doc.record_objects(before, "inspector");
            }
        }
        // lighting on|off [HOUR]: the map view's lighting preview
        "lighting" => {
            ed.view.lighting = arg(1) != "off";
            if !arg(2).is_empty() {
                ed.view.hour = num(2);
            }
        }
        // Structures: structure ID (arm it) · stplace X Y (top-left corner) ·
        // stcapture X Y W H ID [noground] · stinfo
        "structure" => ed.arm_structure(arg(1)),
        "stplace" => ed.place_structure_at((num(1) as i32, num(2) as i32)),
        "stcapture" => {
            let rect = (num(1) as u32, num(2) as u32, num(3) as u32, num(4) as u32);
            let r = ed.create_structure(rect, arg(5), arg(5), "", arg(6) != "noground");
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        // stnew · stlayer N · sttool stamp|keep|clear|solid|walkable|auto|prop · ststamp TS X Y W H ·
        // stprop ID · stpaint X Y (cell of the open structure in Database → Structures) · stsize W H
        "stnew" => ed.new_structure(),
        "stlayer" => ed.db.st_edit.layer = num(1) as usize,
        "sttool" => {
            use structures::StTool;
            ed.db.st_edit.tool = match arg(1) {
                "keep" => StTool::Keep,
                "clear" => StTool::Clear,
                "solid" => StTool::Collision(1),
                "walkable" => StTool::Collision(2),
                "auto" => StTool::Collision(rustpg2d_engine::defs::StructureDef::COLLISION_AUTO),
                "prop" => StTool::Prop,
                _ => StTool::Stamp,
            }
        }
        "ststamp" => {
            ed.db.st_edit.tileset = arg(1).to_string();
            ed.db.st_edit.stamp = (num(2) as u32, num(3) as u32, (num(4) as u32).max(1), (num(5) as u32).max(1));
        }
        "stprop" => ed.db.st_edit.prop = arg(1).to_string(),
        "stpaint" | "stsize" => {
            let i = ed.db.sel;
            let db = ed.project.db.clone();
            if let Some(d) = ed.project.db.structures.get_mut(i) {
                if arg(0) == "stsize" {
                    d.resize(num(1) as u32, num(2) as u32);
                    d.prune();
                } else {
                    ed.db.st_edit.apply(d, (num(1) as u32, num(2) as u32), &db, &ed.assets);
                }
                ed.defs_dirty = true;
            }
        }
        "stinfo" => {
            let ids: Vec<&str> = ed.project.db.structures.iter().map(|d| d.id.as_str()).collect();
            let ents = ed.doc.as_ref().map_or(0, |d| d.map.entities.len());
            println!("[structures] {} defined: {} | armed={:?} tool={:?} map_entities={ents}", ids.len(), ids.join(","), ed.place_structure, ed.tool);
        }
        "libdialog" => ed.open_add_dialog(library_ui::parse_kind(arg(1))),
        "libpick" => {
            if let (Some(d), Some(k)) = (&mut ed.lib.add, library_ui::parse_kind(arg(1))) {
                d.picked.insert((k, arg(2).to_string()));
            }
        }
        "libdialogadd" => {
            ed.add_picked_from_library();
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        "libdialogclose" => ed.lib.add = None,
        "libedit" => {
            if let Some(k) = library_ui::parse_kind(arg(1)) {
                ed.edit_library_asset(&(k, arg(2).to_string()));
            }
        }
        "libadd" => {
            let sel: Vec<_> = ed.lib.selected.iter().cloned().collect();
            let r = ed.library_to_project(&sel);
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        "tolib" => {
            if let Some(k) = library_ui::parse_kind(arg(1)) {
                // Assets using scripts open the "Script functions" dialog (scriptpick…).
                ed.send_project_assets(vec![(k, arg(2).to_string())], library_ui::SendTo::Library);
                println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
            }
        }
        // projexport KIND ID PATH: a project asset to a package (via the script dialog if needed)
        "projexport" => {
            if let Some(k) = library_ui::parse_kind(arg(1)) {
                ed.send_project_assets(vec![(k, arg(2).to_string())], library_ui::SendTo::Package(arg(3).into()));
                println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
            }
        }
        // The "Script functions" dialog: scriptpick FN on|off · scriptpickok · scriptpickcancel
        "scriptpick" => {
            if let Some(p) = &mut ed.lib.script_pick {
                if arg(2) == "off" {
                    p.picked.remove(arg(1));
                } else {
                    p.picked.insert(arg(1).to_string());
                }
            }
        }
        "scriptpickok" | "scriptpickcancel" => {
            ed.close_script_pick(arg(0) == "scriptpickok");
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        "libexport" => {
            let sel: Vec<_> = ed.lib.selected.iter().cloned().collect();
            let r = ed.export_package(&sel, std::path::Path::new(arg(1)));
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        "libimport" | "projimport" => {
            let r = ed.import_package(arg(0) == "libimport", std::path::Path::new(arg(1)));
            ed.show_result(r);
            println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
        }
        // newasset KIND ID [W H]: blank image (prop: tiles, sheet/character: frame px)
        "newasset" => {
            if let Some(k) = wizard::WKind::parse(arg(1)) {
                let mut w = wizard::Wizard::new(&ed.project);
                w.kind = k;
                w.id = arg(2).to_string();
                if num(3) > 0.0 {
                    w.cells = (num(3) as u32, num(4) as u32);
                    w.frame = (num(3) as u32, num(4) as u32);
                    w.grid = (num(3) as u32, num(4) as u32);
                }
                let r = ed.create_asset(&w);
                ed.show_result(r);
                println!("[script] {}", ed.lib.report.clone().unwrap_or_default());
            }
        }
        // model def ID | model builder | model inst UID|NAME: open the template chooser
        "model" => {
            use crate::models::{request_model, ModelTarget};
            match arg(1) {
                "def" => request_model(ModelTarget::Def(arg(2).to_string())),
                "builder" => request_model(ModelTarget::Builder),
                _ => {
                    let uid = arg(2).parse::<u32>().ok().or_else(|| ed.doc.as_ref()?.map.entities.iter().find(|e| e.name == arg(2)).map(|e| e.uid));
                    if let Some(uid) = uid {
                        request_model(ModelTarget::Instance(uid));
                    }
                }
            }
        }
        // modelpick lib CHAR | modelpick proj SHEET | modelpick lib: preview in the open chooser
        "modelpick" => {
            use crate::models::ModelPick;
            let lib = arg(1) == "lib";
            let pick = match (lib, arg(2)) {
                (_, "") => None,
                (true, c) => Some(ModelPick::Library(c.to_string())),
                (false, s) => Some(ModelPick::Project(s.to_string())),
            };
            ed.model_script(lib, pick);
        }
        "modeluse" => ed.model_script_use(),
        // New project wizard: newwizard · wiz name|parent|folder|sex|creator|starter|map|size|scale VALUE... · wizcreate
        "newwizard" => ed.request(app::Pending::NewProject(std::path::PathBuf::new())),
        "wiz" => {
            if let Some(w) = &mut ed.new_project {
                let rest = cmd[2..].join(" ");
                match arg(1) {
                    "name" => {
                        w.name = rest;
                        if !w.folder_edited {
                            w.folder = rustpg2d_engine::release::slug(&w.name);
                        }
                    }
                    "parent" => w.parent = rest,
                    "folder" => {
                        w.folder = rest;
                        w.folder_edited = true;
                    }
                    "sex" => {
                        w.sex = match arg(2) {
                            "male" => rustpg2d_engine::project::SexChoice::Male,
                            "female" => rustpg2d_engine::project::SexChoice::Female,
                            _ => rustpg2d_engine::project::SexChoice::Both,
                        }
                    }
                    "creator" => w.creator = arg(2) != "off",
                    "starter" => w.starter = arg(2) != "off",
                    "map" => w.map_name = rest,
                    "size" => {
                        w.width = num(2) as u32;
                        w.height = num(3) as u32;
                    }
                    "scale" => w.scale = num(2),
                    _ => println!("[script] wiz: unknown field {}", arg(1)),
                }
                println!("[script] wizard problem: {:?}", w.problem());
            }
        }
        "wizcreate" => {
            let r = ed.create_from_wizard().map(|d| format!("created {}", d.display()));
            println!("[script] {r:?}");
        }
        // routeedit on|off: edit the selected character's patrol route on the map
        "routeedit" => {
            ed.route_kind = app::RouteKind::Patrol;
            ed.route_edit = match (arg(1), ed.sel) {
                ("on", app::Selection::Entity(uid)) => Some(uid),
                _ => None,
            }
        }
        // scheduleedit on|off: edit the selected character's schedule on the map (on: adds a
        // schedule with one stop at its tile when it has none, like "Add a schedule")
        "scheduleedit" => {
            ed.route_edit = None;
            if let (Some(doc), app::Selection::Entity(uid), "on") = (&mut ed.doc, ed.sel, arg(1)) {
                let ts = rustpg2d_engine::project::TILE_SIZE as f32;
                let before = doc::Objects::of(&doc.map);
                if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid && e.kind == rustpg2d_engine::entity::EntityKind::Character) {
                    if e.schedule.is_empty() {
                        let tile = ((e.x / ts).floor() as i32, ((e.y - 1.0) / ts).floor() as i32);
                        e.schedule.push(rustpg2d_engine::entity::ScheduleStop::new(8.0, tile.0, tile.1));
                    }
                    ed.route_edit = Some(uid);
                    ed.route_kind = app::RouteKind::Schedule;
                }
                doc.record_objects(before, "schedule");
            }
        }
        // stop K hour H | facing up|down|left|right|any | inside on|off | map NAME|- | prop NAME|- |
        // tile X Y: edit stop K of the selected character's schedule (as the inspector's row)
        "stop" => {
            if let (Some(doc), app::Selection::Entity(uid)) = (&mut ed.doc, ed.sel) {
                let before = doc::Objects::of(&doc.map);
                if let Some(s) = doc.map.entities.iter_mut().find(|e| e.uid == uid).and_then(|e| e.schedule.get_mut(num(1) as usize)) {
                    match arg(2) {
                        "hour" => s.hour = num(3).rem_euclid(24.0),
                        "facing" => s.facing = rustpg2d_engine::entity::Facing::ALL.into_iter().find(|f| f.label().eq_ignore_ascii_case(arg(3))),
                        "inside" => s.inside = arg(3) == "on",
                        "map" => s.map = if arg(3) == "-" { String::new() } else { arg(3).to_string() },
                        "prop" => s.prop = if arg(3) == "-" { String::new() } else { arg(3).to_string() },
                        "tile" => (s.x, s.y) = (num(3) as i32, num(4) as i32),
                        _ => println!("[script] stop: unknown field {}", arg(2)),
                    }
                }
                doc.record_objects(before, "schedule");
            }
        }
        // search TEXT: Ctrl+P with that query (prints the hits) · searchpick N: Enter on hit N
        "search" => {
            let q = cmd[1..].join(" ");
            ed.open_search(Some(&q));
            for k in 0..ed.search.hits.len() {
                if let Some(e) = ed.search.hit(k) {
                    println!("[search] {k}: {} {} — {}", e.what.name(), e.label, e.detail);
                }
            }
            println!("[search] {} hit(s) for {q:?}", ed.search.hits.len());
        }
        "searchpick" => {
            if let Some(t) = ed.search.hit(num(1) as usize).map(|e| e.target.clone()) {
                ed.search.open = false;
                ed.jump_to(&t);
            }
        }
        // usages KIND ID: Find usages (prints them, opens the window) · usagepick N: click usage N
        "usages" => match rustpg2d_engine::refactor::RefKind::parse(arg(1)) {
            Some(kind) => {
                ed.show_usages(kind, arg(2));
                for (k, e) in ed.usages.list.iter().enumerate() {
                    println!("[usages] {k}: {} {} — {}", e.what.name(), e.label, e.detail);
                }
                println!("[usages] {} usage(s) of {} {:?}", ed.usages.list.len(), kind.name(), arg(2));
            }
            None => eprintln!("[script] usages: unknown kind {:?}", arg(1)),
        },
        "usagepick" => {
            if let Some(t) = ed.usages.list.get(num(1) as usize).map(|e| e.target.clone()) {
                ed.jump_to(&t);
            }
        }
        // libupdate: Assets › Update asset library… (prints the lists) · libupdatepick edited|retired KIND ID on|off · libupdateok · libupdatecancel
        "libupdate" => {
            ed.open_library_update();
            let u = &ed.lib_update;
            if let Some(e) = &u.error {
                println!("[libupdate] error: {e}");
            }
            if let Some(p) = &u.plan {
                let ids = |v: &[rustpg2d_engine::library::AssetRef]| v.iter().map(|a| format!("{}:{}", a.0.name(), a.1)).collect::<Vec<_>>().join(" ");
                println!("[libupdate] added: {}", ids(&p.added));
                println!("[libupdate] updated: {}", ids(&p.updated));
                println!("[libupdate] edited: {}", ids(&p.edited));
                for r in &p.retired {
                    println!("[libupdate] retired: {}:{} untouched={} used_by={:?} delete={}", r.asset.0.name(), r.asset.1, r.untouched, r.used_by, u.delete.contains(&r.asset));
                }
                println!("[libupdate] {}", p.counts());
            }
        }
        "libupdatepick" => match rustpg2d_engine::refactor::RefKind::parse(arg(2)) {
            Some(kind) => {
                let a = (kind, arg(3).to_string());
                let set = if arg(1) == "retired" { &mut ed.lib_update.delete } else { &mut ed.lib_update.take };
                if arg(4) == "off" {
                    set.remove(&a);
                } else {
                    set.insert(a);
                }
            }
            None => eprintln!("[script] libupdatepick: unknown kind {:?}", arg(2)),
        },
        "libupdateok" => {
            ed.apply_library_update();
            println!("[libupdate] {}", ed.lib_update.report.clone().or_else(|| ed.lib_update.error.clone()).unwrap_or_default());
        }
        "libupdatecancel" => ed.lib_update.open = false,
        // Generate art: artwindow / settingswindow open the windows · artgen KIND ID SIZE ATTACK DESCRIPTION… starts a
        // job (the cost confirmed) · artkind KIND = the window's kind · artjobs prints them · artapply JOB [ID] · artredo JOB PICTURE · status prints art=
        "artwindow" => ed.art.window.open = true,
        "artkind" => match art::ArtKind::parse(arg(1)) {
            Some(k) => {
                ed.art.window.open = true;
                ed.art.window.kind = k;
            }
            None => println!("[art] unknown kind {:?}", arg(1)),
        },
        "settingswindow" => ed.art.settings_open = true,
        "artgen" => {
            let kind = art::ArtKind::parse(arg(1));
            match kind {
                Some(kind) => {
                    // 3D characters take their options in the SIZE place: female / male, from=SHEET (comma separated).
                    let opts: Vec<&str> = arg(3).split(',').collect();
                    let from = opts.iter().find_map(|o| o.strip_prefix("from=")).unwrap_or("").to_string();
                    let req = art::ArtRequest { kind, id: arg(2).to_string(), size: arg(3).to_string(), attack: arg(4).to_string(), description: cmd[5.min(cmd.len())..].join(" "), tiles: (1, 1), portrait_for: String::new(), by_agent: false, female: opts.contains(&"female"), from };
                    match ed.start_art(req) {
                        Ok(id) => println!("[art] started job {id}"),
                        Err(e) => println!("[art] refused: {e:#}"),
                    }
                }
                None => println!("[art] unknown kind {:?}", arg(1)),
            }
        }
        // artbalance: Edit › Settings › Meshy › Test key (prints the balance, never the key)
        "artbalance" => match art::meshy::balance(&art::api_key()) {
            Ok(b) => println!("[art] balance {b}"),
            Err(e) => println!("[art] balance failed: {e:#}"),
        },
        "artjobs" => {
            for j in &ed.art.jobs {
                println!("[art] {}", j.lock().unwrap().line());
            }
        }
        "artapply" => {
            let job = num(1) as u32;
            let id = if arg(2).is_empty() { ed.art.job(job).map(|j| j.lock().unwrap().req.id.clone()).unwrap_or_default() } else { arg(2).to_string() };
            match ed.apply_art(job, &id) {
                Ok(m) => println!("[art] {m}"),
                Err(e) => println!("[art] apply refused: {e:#}"),
            }
        }
        "artredo" => match ed.redo_art(num(1) as u32, arg(2)) {
            Ok(()) => println!("[art] redrawing"),
            Err(e) => println!("[art] redo refused: {e:#}"),
        },
        // problems [AREA]: check the project, print the list (opens the Problems window) · problempick K: click entry K
        "problems" => {
            ed.refresh_problems();
            ed.problems.open = true;
            ed.problems.area = crate::problems::Area::parse(arg(1));
            let shown: Vec<String> = ed.problems.shown().iter().map(|p| p.line()).collect();
            for (k, l) in shown.iter().enumerate() {
                println!("[problems] {k}: {l}");
            }
            let (e, w) = ed.problems.counts();
            println!("[problems] {} shown, {e} error(s), {w} warning(s)", shown.len());
        }
        "problempick" => {
            if let Some(t) = ed.problems.shown().get(num(1) as usize).map(|p| p.target.clone()) {
                ed.jump_to(&t);
            }
        }
        // charanim walk|idle|attack|hurt: what the character builder's preview plays
        "charanim" => builder::set_preview_anim(arg(1)),
        // scriptsel FILE: what clicking a file in Database > Scripts does
        "scriptsel" => {
            let f = ed.project.scripts_dir().join(arg(1));
            ed.open_script(&f, None);
        }
        // probe X Y: print the egui layer under that screen point (next frame)
        "probe" => ed.probe.push(egui_macroquad::egui::pos2(num(1), num(2))),
        "wizard" => {
            if arg(1) == "character" {
                ed.open_builder(builder::Target::New);
            } else {
                ed.wizard = Some(wizard::Wizard::new(&ed.project));
            }
        }
        // Pixel editor: pxopen REL FW FH · pxtool NAME · pxcolor RRGGBB · pxat X Y [right] · pxsave
        // pxopen REL FW FH: a figure's image opens with the character layout
        "pxopen" => {
            let figure = ed.project.db.sheets.iter().any(|s| s.image == arg(1) && s.kind == rustpg2d_engine::defs::SheetKind::Body);
            ed.open_pixel(arg(1), (num(2) as u32, num(3) as u32), if figure { pixel::Layout::Character } else { pixel::Layout::Plain });
        }
        "pxtool" => {
            if let Some(t) = pixel::PxTool::parse(arg(1)) {
                ed.px.tool = t;
            }
        }
        "pxcolor" => {
            if let Ok(v) = u32::from_str_radix(arg(1), 16) {
                ed.px.colors[0] = [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255];
            }
        }
        "pxat" => ed.px.apply_at(if arg(3) == "right" { 1 } else { 0 }, (num(1) as i32, num(2) as i32)),
        "pxsave" => ed.save_pixel(),
        // pxframe N: select frame N · pxclip copy|cut|paste|mirror (Ctrl+C / X / V)
        "pxframe" => ed.px.sel_frame = num(1) as u32,
        "pxclip" => ed.frame_clipboard(arg(1)),
        // Figures: pxanim walk|idle|attack|hurt down|left|right|up · pxplay on|off · pxonion on|off
        // [next] · pxdup · pxnudge DX DY · pxrow copy|paste|clear|mirror [all] · pxfill standing ·
        // pxreplace FROM TO frame|row|sheet (RRGGBB colours) · pxundo; `status` prints `pixel=`
        "pxanim" => match (frame_ops::anim_index(arg(1)), frame_ops::dir_index(arg(2))) {
            (Some(a), Some(d)) => ed.px.show_anim(a, d),
            _ => println!("[script] pxanim: walk|idle|attack|hurt down|left|right|up"),
        },
        "pxplay" => ed.px.playing = arg(1) != "off",
        // pxgrip right|left X Y ANGLE [SCALE] [front|back] (the selected frame) · pxgrip right|left -
        // removes it · pxhands estimate|estimateall|clear · pxpreview WEAPON [LEFTHANDITEM|-]
        "pxgrip" => {
            let hand = if arg(1).starts_with('l') { rustpg2d_engine::hands::Hand::Left } else { rustpg2d_engine::hands::Hand::Right };
            let grip = (arg(2) != "-").then(|| rustpg2d_engine::hands::Grip { x: num(2), y: num(3), angle: num(4), scale: if arg(5).is_empty() || arg(5).parse::<f32>().is_err() { 1.0 } else { num(5) }, front: !cmd.iter().any(|a| *a == "back") });
            match ed.px.frame_op(pixel::FrameOp::SetGrip { hand, grip }) {
                Some(m) => println!("[script] {m}"),
                None => println!("[script] pxgrip: nothing open"),
            }
        }
        "pxhands" => {
            let sheet = ed.px.doc.as_ref().and_then(|d| d.sheet.clone()).and_then(|id| ed.project.db.sheet(&id).cloned());
            let op = match arg(1) {
                "clear" => Some(pixel::FrameOp::ClearGrips),
                "swap" => Some(pixel::FrameOp::SwapHands(arg(2) == "all")),
                "estimate" | "estimateall" => sheet.map(|s| pixel::FrameOp::EstimateHands { all: arg(1) == "estimateall", skin: s.feature("skin").map(|f| f.shades.clone()).unwrap_or_default(), scale: s.scale }),
                _ => None,
            };
            match op.and_then(|op| ed.px.frame_op(op)) {
                Some(m) => println!("[script] {m}"),
                None => println!("[script] pxhands: open a figure (estimate|estimateall|clear|swap [all])"),
            }
        }
        "pxpreview" => {
            ed.px.preview = [arg(1).to_string(), if arg(2) == "-" { String::new() } else { arg(2).to_string() }];
        }
        "pxundo" => {
            if let Some(d) = &mut ed.px.doc {
                d.undo();
            }
        }
        "pxonion" => {
            ed.px.onion = arg(1) != "off";
            ed.px.onion_next = ed.px.onion && arg(2) == "next";
        }
        "pxdup" | "pxnudge" | "pxrow" | "pxfill" | "pxreplace" => {
            let rgb = |v: &str| u32::from_str_radix(v.trim_start_matches('#'), 16).ok().map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
            let op = match (arg(0), arg(1)) {
                ("pxdup", _) => Some(pixel::FrameOp::Duplicate),
                ("pxnudge", _) => Some(pixel::FrameOp::Nudge(num(1) as i32, num(2) as i32)),
                ("pxrow", "copy") => Some(pixel::FrameOp::RowCopy),
                ("pxrow", "paste") => Some(pixel::FrameOp::RowPaste),
                ("pxrow", "clear") => Some(pixel::FrameOp::RowClear),
                ("pxrow", "mirror") => Some(pixel::FrameOp::Mirror(arg(2) == "all")),
                ("pxfill", _) => Some(pixel::FrameOp::FillStanding),
                ("pxreplace", _) => match (rgb(arg(1)), rgb(arg(2))) {
                    (Some(from), Some(to)) => {
                        let scope = match arg(3) {
                            "frame" => pixel::Scope::Frame,
                            "row" => pixel::Scope::Row,
                            _ => pixel::Scope::Sheet,
                        };
                        Some(pixel::FrameOp::Replace { from, to: [to[0], to[1], to[2], 255], scope })
                    }
                    _ => None,
                },
                _ => None,
            };
            match op.and_then(|op| ed.px.frame_op(op)) {
                Some(m) => println!("[script] {m}"),
                None => println!("[script] {}: nothing done", arg(0)),
            }
        }
        // dbscroll Y: scrolls the Database form to Y px (a large number = the end)
        "dbscroll" => ed.db.form_scroll = Some(num(1)),
        "tab" => {
            use database::Tab;
            ed.db.tab = match arg(1) {
                "props" => Tab::Props,
                "characters" => Tab::Characters,
                "tilesets" => Tab::Tilesets,
                "sheets" => Tab::Sheets,
                "project" => Tab::Project,
                "quests" => Tab::Quests,
                "scripts" => Tab::Scripts,
                "sounds" => Tab::Sounds,
                "structures" => Tab::Structures,
                "skills" => Tab::Skills,
                "spells" => Tab::Spells,
                "recipes" => Tab::Recipes,
                "achievements" => Tab::Achievements,
                "races" => Tab::Races,
                "models" => Tab::Models,
                _ => Tab::Items,
            };
            // tab KIND N, or tab KIND ID (sheets, tilesets, items, props, characters).
            let db = &ed.project.db;
            let by_id = match ed.db.tab {
                Tab::Sheets => db.sheets.iter().position(|d| d.id == arg(2)),
                Tab::Tilesets => db.tilesets.iter().position(|d| d.id == arg(2)),
                Tab::Items => db.items.iter().position(|d| d.id == arg(2)),
                Tab::Props => db.props.iter().position(|d| d.id == arg(2)),
                Tab::Characters => db.characters.iter().position(|d| d.id == arg(2)),
                Tab::Recipes => db.recipes.iter().position(|d| d.id == arg(2)),
                Tab::Achievements => db.achievements.iter().position(|d| d.id == arg(2)),
                Tab::Races => db.races.iter().position(|d| d.id == arg(2)),
                Tab::Models => db.models.iter().position(|d| d.id == arg(2)),
                _ => None,
            };
            ed.db.sel = by_id.unwrap_or(num(2) as usize);
        }
        // sndplay / sndstop [ID]: the Sounds form's Play / Stop (default: the selected sound).
        "sndplay" | "sndstop" => {
            let id = if arg(1).is_empty() { ed.project.db.sounds.get(ed.db.sel).map(|d| d.id.clone()).unwrap_or_default() } else { arg(1).to_string() };
            if arg(0) == "sndplay" {
                ed.audio.play_preview(&id);
            } else {
                ed.audio.stop(&id);
            }
        }
        "tool" => {
            ed.tool = match arg(1) {
                "select" => Tool::Select,
                "entity" => Tool::Entity,
                "zone" => Tool::Zone,
                "collision" => Tool::Collision,
                "rect" => Tool::Rect,
                "fill" => Tool::Fill,
                "erase" => Tool::Erase,
                "structure" => Tool::Structure,
                "terrain" => Tool::Terrain,
                _ => Tool::Brush,
            }
        }
        // selectadd entity N | zone N | named NAME: Ctrl+click on that object (into the
        // selection or out of it) · boxselect X Y W H: Ctrl+drag over tiles · movesel DX DY:
        // drags the selection by whole tiles (one undo step)
        "selectadd" => {
            let s = match arg(1) {
                "entity" => Selection::Entity(num(2) as u32),
                "zone" => Selection::Zone(num(2) as u32),
                "named" => ed.doc.as_ref().and_then(|d| d.map.entities.iter().find(|e| e.name == arg(2))).map_or(Selection::None, |e| Selection::Entity(e.uid)),
                _ => Selection::None,
            };
            ed.toggle_selected(s);
        }
        "delete" => ed.delete_selection(),
        "duplicate" => ed.duplicate_selection(),
        "boxselect" => ed.box_select((num(1) as u32, num(2) as u32, num(3) as u32, num(4) as u32)),
        "movesel" => ed.move_selection(num(1) as i32, num(2) as i32),
        "select" => {
            ed.sel = match arg(1) {
                "entity" => Selection::Entity(num(2) as u32),
                "zone" => Selection::Zone(num(2) as u32),
                // select named NAME: the entity with that instance name
                "named" => ed.doc.as_ref().and_then(|d| d.map.entities.iter().find(|e| e.name == arg(2))).map_or(Selection::None, |e| Selection::Entity(e.uid)),
                // select zonenamed NAME: the zone with that name
                "zonenamed" => ed.doc.as_ref().and_then(|d| d.map.zones.iter().find(|z| z.name == arg(2))).map_or(Selection::None, |z| Selection::Zone(z.uid)),
                _ => Selection::None,
            }
        }
        "view" => {
            ed.cam.center = vec2(num(1), num(2));
            ed.cam.zoom = num(3).max(0.01);
        }
        // mapweather none|rain|snow|fog [PERCENT]: the open map's weather (map settings, undoable)
        "mapweather" => {
            if let (Some(doc), Some(k)) = (ed.doc.as_mut(), rustpg2d_engine::map::WeatherKind::from_id(arg(1))) {
                let before = doc::Objects::of(&doc.map);
                doc.map.weather = rustpg2d_engine::map::Weather::new(k, if arg(2).is_empty() { 0.6 } else { num(2) / 100.0 });
                doc.record_objects(before, "script");
                println!("[script] weather of {}: {:?}", doc.map.name, doc.map.weather);
            }
        }
        // viewopt zones|grid|collision|weather on|off: the View toggles
        "viewopt" => {
            let on = arg(2) != "off";
            match arg(1) {
                "zones" => ed.view.zones = on,
                "grid" => ed.view.grid = on,
                "collision" => ed.view.collision = on,
                "weather" => ed.view.weather = on,
                "parallax" => ed.view.parallax = on,
                _ => println!("[script] viewopt: zones|grid|collision|weather|parallax on|off"),
            }
        }
        // actionsheet SHEET: the sprite sheet of the last Effect / Fly action of the selected
        // entity's first trigger (with addaction, to test the action's animation picker)
        "actionsheet" => {
            if let (Selection::Entity(uid), Some(doc)) = (ed.sel, ed.doc.as_mut()) {
                let before = doc::Objects::of(&doc.map);
                use rustpg2d_engine::events::Action as A;
                let sprite = doc.map.entities.iter_mut().find(|e| e.uid == uid).and_then(|e| e.triggers.first_mut()).and_then(|t| {
                    t.actions.iter_mut().rev().find_map(|a| match a {
                        A::Effect { sprite, .. } | A::Fly { sprite, .. } => Some(sprite),
                        _ => None,
                    })
                });
                if let Some(sprite) = sprite {
                    sprite.sheet = arg(1).to_string();
                }
                doc.record_objects(before, "script");
            }
        }
        // addaction LABEL...: appends the action template with that label (e.g. "Play effect") to
        // the selected entity's first trigger (testing the action editors)
        "addaction" => {
            let label = cmd[1..].join(" ");
            let tpl = rustpg2d_engine::events::Action::templates().into_iter().find(|a| a.label() == label);
            if let (Some(tpl), Selection::Entity(uid), Some(doc)) = (tpl, ed.sel, ed.doc.as_mut()) {
                let before = doc::Objects::of(&doc.map);
                if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                    if e.triggers.is_empty() {
                        e.triggers.push(rustpg2d_engine::events::Trigger::new(rustpg2d_engine::events::TriggerOn::Interact));
                    }
                    e.triggers[0].actions.push(tpl);
                }
                doc.record_objects(before, "inspector");
            } else {
                eprintln!("[editor script] addaction: no template '{label}' or no entity selected");
            }
        }
        // entequip ITEM: the selected NPC wears / holds ITEM (its own equipment, in the item's slot)
        // entcolor FEATURE RRGGBB|-: the selected NPC gets its own look with that feature colour
        "entequip" | "entcolor" => {
            if let (Selection::Entity(uid), Some(doc)) = (ed.sel, ed.doc.as_mut()) {
                let before = doc::Objects::of(&doc.map);
                let db = &ed.project.db;
                if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                    let def = db.character(&e.def).cloned();
                    if arg(0) == "entequip" {
                        if let (Some(slot), Some(d)) = (db.item(arg(1)).and_then(|i| i.equip_slot()), &def) {
                            let eq = e.equipment.get_or_insert_with(|| d.equipment.clone());
                            *eq.slot_mut(slot) = Some(rustpg2d_engine::inventory::ItemStack::new(arg(1), 1));
                        }
                    } else if let Some(d) = &def {
                        // entcolor FEATURE RRGGBB|-
                        let a = e.appearance.get_or_insert_with(|| d.appearance.clone());
                        match u32::from_str_radix(arg(2), 16) {
                            Ok(v) => {
                                a.colors.insert(arg(1).to_string(), [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
                            }
                            Err(_) => {
                                a.colors.remove(arg(1));
                            }
                        }
                    }
                }
                doc.record_objects(before, "inspector");
            }
        }
        "fit" => ed.fit_view(),
        "collision" => ed.view.collision = arg(1) != "off",
        other => eprintln!("[editor script] unknown command '{other}'"),
    }
}

/// The selected character's schedule for `status`: "HH:MM@x,y[in][facing]" per stop, "-" if
/// none; "(editing)" while edited on the map.
fn schedule_text(ed: &Editor) -> String {
    let Some(e) = ed.doc.as_ref().zip(match ed.sel {
        app::Selection::Entity(uid) => Some(uid),
        _ => None,
    }).and_then(|(d, uid)| d.map.entity(uid)) else {
        return "-".into();
    };
    if e.schedule.is_empty() {
        return "-".into();
    }
    let stops: Vec<String> = e
        .schedule
        .iter()
        .map(|s| {
            let map = if s.map.is_empty() { String::new() } else { format!(">{}", s.map) };
            let prop = if s.prop.is_empty() { String::new() } else { format!("[on {}]", s.prop) };
            format!("{}{map}@{},{}{}{}{prop}", s.time_text(), s.x, s.y, if s.inside { "[in]" } else { "" }, s.facing.map_or(String::new(), |f| format!("[{}]", f.label())))
        })
        .collect();
    let editing = ed.route_edit == Some(e.uid) && ed.route_kind == app::RouteKind::Schedule;
    format!("[{}]{}", stops.join(" "), if editing { "(editing)" } else { "" })
}
