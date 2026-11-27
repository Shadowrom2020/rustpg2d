//! The new editor (ROADMAP.md, *0.36.0 — The Studio*): an eframe window on the engine's own wgpu
//! device, over `editor-core`'s workspace. `shell.rs` is the window, `play.rs` the game in a tab.
//!
//! `rustpg2d-studio [PROJECT_DIR] [--map NAME --spawn NAME]`; without a project the repository's
//! example. `rustpg2d-studio --headless PROJECT [--mcp PORT] [--seconds N]` opens no window: the
//! agent server works on the project (`editor-core`'s workspace). `RUSTPG2D_STUDIO_SCRIPT="wait 3; status; shot /path/a.png; quit"` drives it for tests
//! (`wait SECS`, `status`, `shot PATH` = the whole window, `tab NAME`, `palette TEXT` / `goto
//! TEXT` / `pick N`, `field KIND ID /POINTER VALUE` = a form's field, `setting /POINTER VALUE` = one
//! of the game's settings, `key NAME` = a key for the
//! game, `pause` / `resume`, `quit`; the window's own input: `click X Y [ctrl]`, `hover X Y`, `drag X1 Y1 X2 Y2`, `wheel DY`,
//! `type TEXT`, `press KEY` (egui's key names: Enter, Escape, Tab, ArrowDown…), `size W H`,
//! `allfields on|off`, `new [KIND]` = the New… gallery; the map view by tiles: `mapclick`, `mapdrag`,
//! `maphover`, `mapview`, `maptool`, `mapopt`, `mapplace`, `mapzone`, `mapselect`, `mapact`, `mapshot`; any other line
//! is a command of the editor, e.g. `map.open village` or `edit.undo`).

mod form;
mod gallery;
mod helpview;
mod librarypage;
mod artpage;
mod artshop;
mod buildtool;
mod charshop;
mod mapview;
mod modelshop;
mod namespage;
mod nodesview;
mod packspage;
mod pick;
mod pixeledit;
mod play;
mod propshop;
mod publish;
mod scene;
mod scriptedit;
mod sheet;
mod shell;
mod thumbs;
mod timeline;
mod tourpanel;
mod worldview;

use std::collections::VecDeque;
use std::path::PathBuf;

use eframe::egui;

struct Script {
    cmds: VecDeque<Vec<String>>,
    wait: f32,
    /// Input the script makes, a batch a frame (a click is a press, then a release).
    input: VecDeque<Vec<egui::Event>>,
    pointer: egui::Pos2,
}

impl Script {
    fn from_env() -> Script {
        let text = std::env::var("RUSTPG2D_STUDIO_SCRIPT").unwrap_or_default();
        Script { cmds: text.split(';').map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>()).filter(|c| !c.is_empty()).collect(), wait: 0.0, input: VecDeque::new(), pointer: egui::Pos2::ZERO }
    }
}

struct Studio {
    shell: shell::Shell,
    script: Script,
    frames: u32,
    fps_time: f64,
    fps: f32,
}

impl Studio {
    fn new(cc: &eframe::CreationContext<'_>, project_dir: Option<PathBuf>, start: rustpg2d_engine::runtime::StartOptions) -> Studio {
        // The engine draws with eframe's device: what it draws is a texture egui can show.
        let rs = cc.wgpu_render_state.as_ref().expect("eframe was started with the wgpu renderer");
        gfx::init(rs.device.clone(), rs.queue.clone(), 16, 16, &gfx::conf::Conf::default());
        let mut shell = shell::Shell::open(project_dir, start);
        shell.ws.looking_at = "the overview".into();
        Studio { shell, script: Script::from_env(), frames: 0, fps_time: 0.0, fps: 0.0 }
    }

    fn run_script(&mut self, ctx: &egui::Context, dt: f32) {
        self.script.wait -= dt;
        // (Input the script made is taken in first: a batch a frame.)
        while self.script.wait <= 0.0 && self.script.input.is_empty() {
            let Some(cmd) = self.script.cmds.pop_front() else { return };
            let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
            match arg(0) {
                "wait" => self.script.wait = arg(1).parse().unwrap_or(1.0),
                "status" => {
                    let game = self.shell.play.as_ref().map(|p| p.status()).unwrap_or_else(|| "play=none".into());
                    println!("[status] fps={:.0} {} | {}", self.fps, self.shell.status(), &game[..game.len().min(300)]);
                    // (The running game's own line in full: its clock, flags, variables…)
                    if self.shell.play.is_some() {
                        println!("[game] {game}");
                    }
                    let map = self.shell.map_status();
                    if !map.is_empty() {
                        println!("[mapview] {map}");
                    }
                    println!("[names] {}", self.shell.names.status());
                    println!("[publish] {}", self.shell.publish.status());
                    let script = self.shell.script_status();
                    if !script.is_empty() {
                        println!("[script] {script}");
                    }
                    let sound = self.shell.sound_word(&[], ctx.input(|i| i.time));
                    if !sound.is_empty() {
                        println!("[sound] {sound}");
                    }
                    let sheet = self.shell.sheet_word(&[]);
                    if !sheet.is_empty() {
                        println!("[sheet] {sheet}");
                    }
                    let figure = self.shell.char_word(&[]);
                    if !figure.is_empty() {
                        println!("[char] {figure}");
                    }
                    let prop = self.shell.prop_word(&[]);
                    if !prop.is_empty() {
                        println!("[prop] {prop}");
                    }
                    let nodes = self.shell.nodes_word(&[]);
                    if !nodes.is_empty() {
                        println!("[nodes] {nodes}");
                    }
                    // (What the editor last told the person.)
                    println!("[said] {}", self.shell.ws.status.0);
                    println!("[world] {} link={}", self.shell.world_status(), self.shell.link.as_ref().map_or("-".to_string(), |(m, z)| format!("{m}:{z}")));
                }
                // Projects: `project new PATH`, `project open PATH`, `project close`.
                "project" => {
                    let path = PathBuf::from(cmd[2.min(cmd.len())..].join(" "));
                    let r = match arg(1) {
                        // (`project new PATH`: with the basics from the library, as the home screen
                        // makes one; `project empty PATH`: nothing in it.)
                        "new" => self.shell.new_project(&path, "", true),
                        "empty" => self.shell.new_project(&path, "", false),
                        "open" => self.shell.switch_project(&path),
                        "close" => self.shell.close_project(),
                        other => Err(format!("project {other}: new PATH, empty PATH, open PATH, close")),
                    };
                    println!("[project] {}", r.map_or_else(|e| format!("failed: {e}"), |()| if self.shell.ws.project_open { format!("{} at {}", self.shell.ws.project.config.name, self.shell.ws.project_location().display()) } else { "none".to_string() }));
                }
                // The Library tab: `library tick KIND ID`, `library add` (what is ticked, into the project).
                "library" => match arg(1) {
                    "tick" => match rustpg2d_engine::refactor::RefKind::parse(arg(2)) {
                        Some(kind) if self.shell.library.tick(&mut self.shell.ws, kind, arg(3)) => {}
                        _ => println!("[script] the library has no {} {}", arg(2), arg(3)),
                    },
                    "add" => {
                        self.shell.library.add(&mut self.shell.ws);
                        println!("[library] {}", self.shell.library.status(&self.shell.ws));
                    }
                    _ => println!("[library] {}", self.shell.library.status(&self.shell.ws)),
                },
                // A cut-scene's timeline (its tab in front): `timeline time SECS`, `timeline cue TRACK CUE`,
                // `timeline add TRACK ACTION…`, `timeline play`; alone it prints where everyone is.
                "timeline" => println!("[timeline] {}", self.shell.timeline_word(&cmd[1..])),
                // A file dropped on the window: `dropfile PATH` opens *New…* with it, as a real drop does.
                "dropfile" => {
                    let path = PathBuf::from(cmd[1.min(cmd.len())..].join(" "));
                    self.shell.dropped(&path);
                }
                // The packs of the Library tab: `packs file PATH` (a pack file looked at), `packs same keep|replace|copy`,
                // `packs install`, `packs cancel`, `packs add ADDRESS` / `packs remove ADDRESS` (a source).
                "packs" => println!("[packs] {}", self.shell.packs_word(&cmd[1..])),
                // What the next file dialog answers (a script never opens a real one): `pickanswer PATH`.
                "pickanswer" => pick::answer_next(PathBuf::from(cmd[1.min(cmd.len())..].join(" "))),
                // A sound's tab in front: `sound play|loop|stop`; a sprite sheet's: `sheet cut ACROSS DOWN`, `sheet frame K`.
                "sound" => println!("[sound] {}", self.shell.sound_word(&cmd[1..], ctx.input(|i| i.time))),
                "sheet" => println!("[sheet] {}", self.shell.sheet_word(&cmd[1..])),
                // A character's workshop (its tab in front): `char model ID`, `char part SLOT ID|-|own`,
                // `char color FEATURE RRGGBB|-`, `char random`, `char face front|left|back|right|turn`, `char anim walk|idle|attack|hurt`.
                "char" => println!("[char] {}", self.shell.char_word(&cmd[1..])),
                // A guided tour: `tour start first-map|talking-character|a-quest`, `tour skip`, `tour end`; `tour` prints
                // `ID done/steps [x - .] now=STEP at=X,Y` (where the control it points at is).
                "tour" => println!("[tour] {}", self.shell.tour_word(ctx, &cmd[1..])),
                // The Help tab: `help topic` (as F1: the manual for the tab in front), `help open FILE [ANCHOR]`,
                // `help search TEXT…`, `help hit K`, `help back`.
                "help" => println!("[help] {}", self.shell.help_word(&cmd[1..])),
                // Generate art: `art kind KIND`, `art id ID`, `art describe TEXT…`, `art for CHARACTER`, `art attack NAME`,
                // `art extras A,B`, `art draw` (the cost confirmed), `art redo JOB PICTURE`, `art add JOB [ID]`.
                "art" => println!("[art] {}", self.shell.art_word(&cmd[1..])),
                // The pixel editor of the sprite sheet's tab in front: `pixels open|close|frame K|tool NAME|color RRGGBB [2]|dot X Y [2]|
                // line|rect X1 Y1 X2 Y2 [2]|fill X Y [2]|undo|redo|copy|paste|flip|clear|nudge DX DY|replace FROM TO [all]|save`.
                "pixels" => println!("[pixels] {}", self.shell.pixels_word(&cmd[1..])),
                // A 3D model's workshop (its tab in front): `model play ANIM CLIP|-`, `model fps ANIM N`, `model guess`,
                // `model rig TEMPLATE|- [TURN]`, `model face …`, `model anim NAME`, `model check`.
                "model" => println!("[model] {}", self.shell.model_word(&cmd[1..])),
                // A prop's workshop (its tab in front): `prop state NAME`, `prop turn`, `prop dark on|off`; prints
                // where its handles are in the window (for `drag`).
                "prop" => println!("[prop] {}", self.shell.prop_word(&cmd[1..])),
                // A visual script's canvas (its tab in front): `nodes add KIND X Y`, `nodes link FROM OUT TO IN`,
                // `nodes value NODE INPUT VALUE`, `nodes select N…`, `nodes delete|group|copy|paste [X Y]|code`.
                "nodes" => println!("[nodes] {}", self.shell.nodes_word(&cmd[1..])),
                // The Publishing page: `publish name|out VALUE`, `publish linux|windows|macos|web|zip|pack|appimage|flatpak on|off`,
                // `publish check` prints the pre-flight, `publish go` exports.
                "publish" => match arg(1) {
                    "go" => {
                        self.shell.save_scripts();
                        self.shell.publish.export(&mut self.shell.ws);
                        match &self.shell.publish.result {
                            Some(Ok(m)) => m.lines().for_each(|l| println!("[publish] {l}")),
                            Some(Err(e)) => println!("[publish] failed: {e}"),
                            None => {}
                        }
                    }
                    "check" => {
                        let pre = self.shell.publish.preflight(&self.shell.ws).clone();
                        pre.stops.iter().for_each(|s| println!("[publish] stop: {s}"));
                        pre.notes.iter().for_each(|s| println!("[publish] note: {s}"));
                        pre.writes.iter().for_each(|s| println!("[publish] writes: {s}"));
                    }
                    what => {
                        if let Err(e) = self.shell.publish.set(&self.shell.ws, what, &cmd[2..].join(" ")) {
                            println!("[script] {e}");
                        }
                    }
                },
                // The Flags and variables page: `names open KIND NAME` opens a name's row,
                // `names rename KIND OLD NEW` renames it everywhere (KIND: flag, variable, text).
                "names" => {
                    let Some(kind) = editor_core::names::NameKind::parse(arg(2)) else {
                        println!("[script] names open|rename flag|variable|text NAME [NEW]");
                        continue;
                    };
                    match arg(1) {
                        "rename" => println!("[names] {}", self.shell.ws.rename_name(kind, arg(3), arg(4)).unwrap_or_else(|e| format!("not renamed: {e}"))),
                        _ => self.shell.names.pick(&self.shell.ws, kind, arg(3)),
                    }
                }
                // A picture of tiles of the map being worked on, as agents get it:
                // `mapshot X Y W H PATH [ZOOM]`.
                "mapshot" => {
                    let n = |i: usize| arg(i).parse::<u32>().unwrap_or(1);
                    let spec = editor_core::mcp::exec::ShotSpec { rect: Some((n(1), n(2), n(3), n(4))), zoom: arg(6).parse().ok(), grid: None, collision: None, zones: None, text: String::new() };
                    let r = mapview::MapView::shot(&mut self.shell.ws, &spec).and_then(|img| img.encode().map_err(|e| e.to_string())).and_then(|png| std::fs::write(arg(5), png).map_err(|e| e.to_string()));
                    println!("[mapshot] {}", r.map_or_else(|e| format!("failed: {e}"), |()| arg(5).to_string()));
                }
                // The view of the map being worked on. Places are tiles (fractions allowed; a whole
                // number is the tile's middle): `mapclick X Y [ctrl|shift|right|double]`,
                // `mapdrag X1 Y1 X2 Y2`, `maphover X Y` use the window's pointer for real;
                // `mapview CX CY ZOOM`, `maptool NAME`, `mapopt grid|zones|collision|light|weather|above on|off`,
                // `mapplace KIND ID|-`, `mapzone KIND`, `mapbuild MODE …` (`BuildTool::script`),
                // `mapselect NAME…|-`, `mapact delete|duplicate|turn`.
                "mapclick" | "maphover" | "mapdrag" | "mapview" | "maptool" | "mapopt" | "mapplace" | "mapzone" | "mapground" | "mapbuild" | "mapselect" | "mapact" => {
                    let ts = self.shell.ws.ts();
                    let num = |i: usize| arg(i).parse::<f32>().unwrap_or(0.0);
                    if arg(0) == "mapact" {
                        self.shell.map_act(&cmd[1..].join(" "));
                        continue;
                    }
                    if arg(0) == "mapselect" {
                        let what = cmd[1..].join(" ");
                        let found = self.shell.ws.find_object(&what);
                        if let Some((view, _)) = self.shell.map_view() {
                            match found {
                                Some(s) => view.select(s),
                                None => view.sel.clear(),
                            }
                        }
                        if found.is_none() && what != "-" {
                            println!("[script] nothing on the map is called '{what}'");
                        }
                        continue;
                    }
                    let Some((view, on)) = self.shell.map_view() else {
                        println!("[script] no map is open");
                        continue;
                    };
                    match arg(0) {
                        "mapview" => view.look_at(gfx::math::vec2((num(1) + 0.5) * ts, (num(2) + 0.5) * ts), if arg(3).is_empty() { 1.0 } else { num(3) }),
                        "maptool" => match mapview::Tool::named(arg(1)) {
                            Some(t) if !mapview::Tool::offered(&on).any(|o| o == t) => println!("[script] a structure's map has no {} tool", t.label()),
                            Some(t) => view.tool = t,
                            None => println!("[script] no tool '{}'", arg(1)),
                        },
                        "mapopt" => {
                            let on = arg(2) != "off";
                            match arg(1) {
                                "grid" => view.opts.grid = on,
                                "zones" => view.opts.zones = on,
                                "collision" => view.opts.collision = on,
                                "light" => {
                                    view.opts.lighting = on;
                                    if let Ok(h) = arg(3).parse() {
                                        view.opts.hour = h;
                                    }
                                }
                                "weather" => view.opts.weather = on,
                                "above" => view.opts.topdown = on,
                                "ground" => view.layers[0] = on,
                                "scenery" => view.layers[1] = on,
                                "things" => view.layers[2] = on,
                                other => println!("[script] no view option '{other}'"),
                            }
                        }
                        "mapplace" => {
                            let kind = rustpg2d_engine::refactor::RefKind::ALL.iter().copied().find(|k| k.name() == arg(1));
                            view.place = kind.map(|k| (k, arg(2).to_string()));
                            view.tool = mapview::Tool::Place;
                        }
                        // The Terrain tool: `mapground MATERIAL|erase [brush|line|rect|fill] [WIDTH_TILES] [square] [snap]`.
                        "mapground" => {
                            view.tool = mapview::Tool::Terrain;
                            view.material = Some(if arg(1) == "erase" { String::new() } else { arg(1).to_string() });
                            if let Some(m) = mapview::GroundMode::named(arg(2)) {
                                view.ground_mode = m;
                            }
                            if let Ok(w) = arg(3).parse::<f32>() {
                                view.brush.size = w * ts / 2.0;
                            }
                            view.brush.square = cmd.iter().any(|a| a == "square");
                            view.brush.snap = cmd.iter().any(|a| a == "snap");
                        }
                        "mapzone" => {
                            view.zone_kind = arg(1).to_string();
                            view.tool = mapview::Tool::Zones;
                        }
                        "mapbuild" => {
                            view.tool = mapview::Tool::Build;
                            if let Err(e) = view.build.script(&cmd[1..]) {
                                println!("[script] {e}");
                            }
                        }
                        _ => {
                            let a = view.window_pos(ts, num(1), num(2));
                            let how = if arg(0) == "mapclick" { arg(3) } else { "" };
                            let modifiers = match how {
                                "ctrl" => egui::Modifiers::COMMAND | egui::Modifiers::CTRL,
                                "shift" => egui::Modifiers::SHIFT,
                                _ => egui::Modifiers::NONE,
                            };
                            let button = if how == "right" { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
                            let press = |pos, pressed| egui::Event::PointerButton { pos, button, pressed, modifiers };
                            self.script.pointer = a;
                            self.script.input.push_back(vec![egui::Event::ModifiersChanged(modifiers), egui::Event::PointerMoved(a)]);
                            match arg(0) {
                                "mapclick" => {
                                    for _ in 0..if how == "double" { 2 } else { 1 } {
                                        self.script.input.push_back(vec![press(a, true)]);
                                        self.script.input.push_back(vec![press(a, false)]);
                                    }
                                    self.script.input.push_back(vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)]);
                                }
                                "mapdrag" => {
                                    let b = view.window_pos(ts, num(3), num(4));
                                    self.script.input.push_back(vec![press(a, true)]);
                                    for k in 1..=4 {
                                        self.script.input.push_back(vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / 4.0))]);
                                    }
                                    self.script.input.push_back(vec![press(b, false)]);
                                    self.script.pointer = b;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "shot" => {
                    let path = arg(1).to_string();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img: std::sync::Arc<egui::ColorImage>| {
                        let px: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                        let png = rustpg2d_engine::imageio::Rgba { w: img.size[0] as u32, h: img.size[1] as u32, px }.encode();
                        match png.map_err(|e| e.to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
                            Ok(()) => println!("[shot] {path}"),
                            Err(e) => println!("[shot] {path} failed: {e}"),
                        }
                    })));
                    // (The picture is taken with the next frame.)
                    self.script.wait = 0.2;
                }
                // A tab by its name (`overview`, `play`, `problems`, `history`, `def:item:potion`, `map:village`).
                "tab" => match self.shell.tab_named(arg(1)) {
                    Some(t) => self.shell.show_tab(t),
                    None => println!("[script] no tab '{}'", arg(1)),
                },
                // The palettes: `palette TEXT…` (commands) and `goto TEXT…`; `pick N` = Enter on entry N.
                "palette" | "goto" => {
                    let kind = if arg(0) == "palette" { shell::PaletteKind::Commands } else { shell::PaletteKind::Search };
                    self.shell.open_palette(kind, &cmd[1..].join(" "));
                    for (k, (label, detail, _)) in self.shell.palette_entries().iter().enumerate().take(8) {
                        println!("[palette] {k} {label} | {detail}");
                    }
                }
                "pick" => {
                    if self.shell.palette_pick(arg(1).parse().unwrap_or(0)) {
                        self.shell.palette.open = None;
                    }
                    if let Some(a) = &self.shell.palette.answer {
                        println!("[palette] answer: {a:?}");
                    }
                }
                // A form's field: `field KIND ID /POINTER VALUE…` (the value as data: 12, "text", true).
                "field" => {
                    let kind = rustpg2d_engine::refactor::RefKind::ALL.iter().copied().find(|k| k.name() == arg(1));
                    let r = match kind {
                        Some(k) => self.shell.set_field(k, arg(2), arg(3), &cmd[4.min(cmd.len())..].join(" ")),
                        None => Err(format!("'{}' is not a kind", arg(1))),
                    };
                    println!("[field] {}", r.unwrap_or_else(|e| format!("failed: {e}")));
                }
                // One of the game's settings: `setting /POINTER VALUE…`.
                "setting" => println!("[setting] {}", self.shell.set_setting(arg(1), &cmd[2.min(cmd.len())..].join(" ")).unwrap_or_else(|e| format!("failed: {e}"))),
                "key" => {
                    if let (Some(p), Some(k)) = (self.shell.play.as_mut(), play::key_named(arg(1))) {
                        p.tap(k);
                    }
                }
                // `keyhold NAME SECS`: a key held down for the game (walking: a tap is one frame).
                "keyhold" => {
                    if let (Some(p), Some(k)) = (self.shell.play.as_mut(), play::key_named(arg(1))) {
                        p.hold(k, arg(2).parse().unwrap_or(0.3));
                    }
                }
                "pause" | "resume" => {
                    if let Some(p) = self.shell.play.as_mut() {
                        p.paused = arg(0) == "pause";
                    }
                }
                // The Play tab: `play title|start|here|again|window|MAP X Y [setup]`, `playstep` (one step
                // while paused), `playspeed N`, `playflag NAME on|off`, `playvar NAME VALUE`,
                // `playstate on|off` (the State panel), `playinspect on|off`.
                // (A word with a tile after it is a map, also when the map is called `start`.)
                "play" => match arg(1) {
                    "title" if arg(2).is_empty() => self.shell.play_from(shell::PlayFrom::Title),
                    "start" if arg(2).is_empty() => self.shell.play_from(shell::PlayFrom::Start),
                    "again" if arg(2).is_empty() => self.shell.play_from(shell::PlayFrom::Again),
                    "here" | "" => self.shell.play_here(),
                    "window" if arg(2).is_empty() => self.shell.play_window(),
                    map => self.shell.play_from(shell::PlayFrom::Tile(map.to_string(), (arg(2).parse().unwrap_or(0), arg(3).parse().unwrap_or(0)), arg(4) == "setup")),
                },
                // The running game's state becomes the test setup (`playkeep`; `playkeep -` forgets it).
                "playkeep" => {
                    let setup = if arg(1) == "-" { Some(Default::default()) } else { self.shell.play.as_ref().map(|p| p.as_setup()) };
                    match setup {
                        Some(s) => {
                            println!("[testsetup] {}", s.summary());
                            self.shell.ws.set_test_setup(s);
                        }
                        None => println!("[script] no game is running"),
                    }
                }
                "playstep" | "playspeed" | "playflag" | "playvar" | "playstate" | "playinspect" => match self.shell.play.as_mut() {
                    Some(p) => match arg(0) {
                        "playstep" => p.step(),
                        "playspeed" => p.speed = arg(1).parse().unwrap_or(1.0),
                        "playflag" => p.set_flag(arg(1), arg(2) != "off"),
                        "playvar" => p.set_var(arg(1), arg(2).parse().unwrap_or(0)),
                        "playstate" => p.show_state = arg(1) != "off",
                        _ => p.inspect = arg(1) != "off",
                    },
                    None => println!("[script] no game is running"),
                },
                // The window's own input, as a mouse and a keyboard would make it.
                "click" | "hover" => {
                    let pos = egui::pos2(arg(1).parse().unwrap_or(0.0), arg(2).parse().unwrap_or(0.0));
                    self.script.pointer = pos;
                    // (`click X Y ctrl`: with Ctrl held.)
                    let modifiers = if arg(0) == "click" && arg(3) == "ctrl" { egui::Modifiers::COMMAND | egui::Modifiers::CTRL } else { egui::Modifiers::NONE };
                    self.script.input.push_back(vec![egui::Event::ModifiersChanged(modifiers), egui::Event::PointerMoved(pos)]);
                    if arg(0) == "click" {
                        let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers };
                        self.script.input.push_back(vec![button(true)]);
                        self.script.input.push_back(vec![button(false)]);
                        self.script.input.push_back(vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)]);
                    }
                }
                // A drag across the window: `drag X1 Y1 X2 Y2` (pressed at the first place, let go at the second).
                "drag" => {
                    let n = |i: usize| arg(i).parse::<f32>().unwrap_or(0.0);
                    let (a, b) = (egui::pos2(n(1), n(2)), egui::pos2(n(3), n(4)));
                    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                    self.script.input.push_back(vec![egui::Event::PointerMoved(a)]);
                    self.script.input.push_back(vec![button(a, true)]);
                    for k in 1..=6 {
                        self.script.input.push_back(vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / 6.0))]);
                    }
                    self.script.input.push_back(vec![button(b, false)]);
                    self.script.pointer = b;
                }
                "wheel" => {
                    let delta = egui::vec2(0.0, arg(1).parse().unwrap_or(0.0));
                    self.script.input.push_back(vec![egui::Event::PointerMoved(self.script.pointer)]);
                    self.script.input.push_back(vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown, modifiers: egui::Modifiers::NONE }]);
                }
                "type" => self.script.input.push_back(vec![egui::Event::Text(cmd[1..].join(" "))]),
                "press" => match egui::Key::from_name(arg(1)) {
                    Some(key) => {
                        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
                        self.script.input.push_back(vec![event(true)]);
                        self.script.input.push_back(vec![event(false)]);
                    }
                    None => println!("[script] no key '{}'", arg(1)),
                },
                "size" => ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(arg(1).parse().unwrap_or(1500.0), arg(2).parse().unwrap_or(800.0)))),
                "allfields" => self.shell.show_all = arg(1) != "off",
                // The *New…* gallery (Ctrl+N), with a kind picked: `new [KIND]`.
                "new" => self.shell.new_asset = Some(gallery::NewAsset::of(rustpg2d_engine::refactor::RefKind::ALL.iter().copied().find(|k| k.name() == arg(1)))),
                "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                // Anything else is a command of the editor (`map.open village`, `edit.undo`…).
                _ => {
                    let line = cmd.join(" ");
                    match self.shell.run(&line) {
                        Ok(m) => println!("[cmd] {line}: {}", m.replace('\n', " | ")),
                        Err(e) => println!("[cmd] {line}: failed: {e}"),
                    }
                }
            }
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        gfx::begin_frame();
        let dt = gfx::time::get_frame_time();
        self.frames += 1;
        self.fps_time += dt as f64;
        if self.fps_time >= 0.5 {
            self.fps = self.frames as f32 / self.fps_time as f32;
            self.frames = 0;
            self.fps_time = 0.0;
        }
        self.run_script(&ctx, dt);
        // Agents work between frames, like script commands.
        self.shell.ws.mcp_drain();
        self.shell.ui(ui, frame.wgpu_render_state(), self.fps);
        self.shell.ws.mcp_answer_shots(Some(&mut |ws, spec| mapview::MapView::shot(ws, spec)));
        gfx::end_frame();
        // The game and the agents move on by themselves: a frame after every frame.
        ctx.request_repaint();
    }

    /// The script's clicks and keys join what the window got.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(batch) = self.script.input.pop_front() {
            raw.events.extend(batch);
        }
    }
}

/// `--headless PROJECT [--mcp PORT]`: the editor without a window. The project is opened and the
/// agent server answers until the process is stopped (or for `--seconds N`): agents and scripts
/// work on a project where there is no display. No textures are loaded and nothing is drawn, so
/// screenshots and thumbnails aren't available; play-tests start the game as usual.
fn headless(project: PathBuf, port: u16, seconds: Option<f64>) -> std::process::ExitCode {
    use editor_core::{Gpu, Workspace};
    let mut ws = match Workspace::open_path(project.clone(), Gpu::No) {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[studio] {}: {e:#}", project.display());
            return std::process::ExitCode::FAILURE;
        }
    };
    ws.looking_at = "nothing (no window)".into();
    ws.refresh_refs();
    if let Err(e) = ws.mcp.start(port) {
        eprintln!("[studio] the agent server: {e}");
        return std::process::ExitCode::FAILURE;
    }
    let started = std::time::Instant::now();
    let mut last_tick = 0.0;
    while seconds.is_none_or(|s| started.elapsed().as_secs_f64() < s) {
        ws.mcp_drain();
        ws.mcp_answer_shots(None);
        let now = started.elapsed().as_secs_f64();
        if now - last_tick > 1.0 {
            last_tick = now;
            ws.watch_project();
            ws.project_file_tick();
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    ws.mcp.stop();
    ws.finish_project_file();
    std::process::ExitCode::SUCCESS
}

fn main() -> std::process::ExitCode {
    // Started in a checkout of R(ust)PG2D: the asset library is the starter library's folder.
    editor_core::repo_mode::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if let Some(project) = opt("--headless") {
        let port = opt("--mcp").and_then(|p| p.parse().ok()).unwrap_or(editor_core::DEFAULT_PORT);
        return headless(PathBuf::from(project), port, opt("--seconds").and_then(|s| s.parse().ok()));
    }
    match window(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[studio] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn window(args: &[String]) -> eframe::Result {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    // The project named; without one the example of a checkout started in, else the home screen.
    let project_dir = args.first().filter(|a| !a.starts_with("--")).map(PathBuf::from).or_else(|| Some(PathBuf::from("example_project")).filter(|p| p.join("project.ron").is_file()));
    let start = rustpg2d_engine::runtime::StartOptions { map: opt("--map"), spawn: opt("--spawn"), title: opt("--map").is_none(), dev: true, ..Default::default() };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("R(ust)PG2D Studio").with_inner_size([1500.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        // An X11 window where there is an X server, as the game's (`gfx::window`): a Wayland
        // compositor hands no frames to a covered window, and scripted runs would stand still.
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                use winit::platform::x11::EventLoopBuilderExtX11;
                let x11 = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
                if x11 && !std::env::var_os("RUSTPG2D_WAYLAND").is_some_and(|v| v == "1") {
                    builder.with_x11();
                }
            }
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            let _ = builder;
        })),
        ..Default::default()
    };
    eframe::run_native("rustpg2d-studio", options, Box::new(move |cc| Ok(Box::new(Studio::new(cc, project_dir, start)))))
}
