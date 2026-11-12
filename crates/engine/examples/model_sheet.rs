//! Renders a 3D model the way the game draws it into a contact sheet: walk, idle, attack and hurt
//! (rows) facing down, left, right and up, four moments each (columns), on grass. For checking
//! models and rig templates without a project.
//!
//! `cargo run -p rustpg2d-engine --example model_sheet -- OUT.png MODEL.glb [--clips FILE.glb]…
//! [--template TEMPLATE.glb [--turn DEGREES]] [--height PIXELS] [--anims walk,idle,…] [--facing left]
//! [--moments N]`
use std::collections::HashMap;

use macroquad::prelude::*;
use rustpg2d_engine::defs::{Appearance, Database, ModelDef, RigFit};
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::model::guess_clips;
use rustpg2d_engine::render::model::{Gear, Models, PoseReq};

fn conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf { window_title: "model_sheet".into(), window_width: 1600, window_height: 900, ..Default::default() },
        update_on: None,
        default_filter_mode: FilterMode::Nearest,
        draw_call_vertex_capacity: rustpg2d_engine::render::model::DRAW_VERTICES,
        draw_call_index_capacity: rustpg2d_engine::render::model::DRAW_INDICES,
    }
}

fn clip_names(path: &str) -> Vec<String> {
    std::fs::read(path).ok().and_then(|b| rustpg2d_engine::model::clip_names(&b).ok()).unwrap_or_default()
}

#[macroquad::main(conf)]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(out), Some(file)) = (args.first(), args.get(1)) else {
        eprintln!("usage: model_sheet OUT.png MODEL.glb [--clips FILE.glb]… [--template TEMPLATE.glb [--turn DEGREES]] [--height PIXELS] [--anims walk,idle,…]");
        std::process::exit(2);
    };
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let clip_files: Vec<String> = args.windows(2).filter(|w| w[0] == "--clips").map(|w| w[1].clone()).collect();
    let mut def = ModelDef { clip_files: clip_files.clone(), ..ModelDef::new("m", file) };
    def.height = opt("--height").and_then(|h| h.parse().ok()).unwrap_or(40.0);
    let mut db = Database::default();
    if let Some(t) = opt("--template") {
        db.models.push(ModelDef { clips: guess_clips(&clip_names(&t)), ..ModelDef::new("t", &t) });
        def.rig = Some(RigFit { template: "t".into(), turn: opt("--turn").and_then(|v| v.parse().ok()).unwrap_or(0.0), joints: Vec::new() });
    } else {
        let mut names = clip_names(file);
        for f in &clip_files {
            names.extend(clip_names(f));
        }
        def.clips = guess_clips(&names);
    }
    db.models.push(def);
    db.reindex();
    let models = Models::load(&db, &|p: &str| std::fs::read(p));
    for e in &models.errors {
        eprintln!("{e}");
    }
    let anims: Vec<String> = opt("--anims").unwrap_or_else(|| "walk,idle,attack,hurt".into()).split(',').map(String::from).collect();
    // `--facing left` and `--moments N`: one facing, N moments (looking through a long clip).
    let facings: Vec<Facing> = match opt("--facing").as_deref() {
        Some("down") => vec![Facing::Down],
        Some("left") => vec![Facing::Left],
        Some("right") => vec![Facing::Right],
        Some("up") => vec![Facing::Up],
        _ => vec![Facing::Down, Facing::Left, Facing::Right, Facing::Up],
    };
    let moments: usize = opt("--moments").and_then(|m| m.parse().ok()).unwrap_or(4);
    let look = Appearance { model: "m".into(), ..Default::default() };
    let gear = Gear::default();
    let sheets = HashMap::new();
    let (cw, ch) = (96.0, 112.0);
    let mut done = 0;
    for frame in 0..60 {
        models.begin_frame();
        // Render first (that sets render-target cameras), then draw with the screen's camera.
        let mut placed = Vec::new();
        for (r, anim) in anims.iter().enumerate() {
            for (fi, f) in facings.iter().enumerate() {
                for m in 0..moments {
                    let req = PoseReq { look: &look, gear: &gear, anim, time: (m as f32 + 0.5) / moments as f32, once: true, facing: *f, bust: false };
                    if let Some(cell) = models.pose(&db, &sheets, &req) {
                        placed.push((cell, vec2(((fi * moments + m) as f32 + 0.5) * cw, r as f32 * ch + 16.0 + ch - 12.0)));
                    }
                }
            }
        }
        set_default_camera();
        clear_background(Color::from_rgba(70, 110, 60, 255));
        for (cell, feet) in &placed {
            models.draw(cell, *feet, 2.0, WHITE);
        }
        for (r, anim) in anims.iter().enumerate() {
            draw_text(anim, 4.0, r as f32 * ch + 14.0, 18.0, WHITE);
        }
        let cells = placed.len();
        // The renderer makes a few dozen poses a frame and stands in cached ones for the rest:
        // after enough frames every pose is its own. Read the screen before it is presented.
        let needed = anims.len() * facings.len() * moments;
        if cells == needed {
            done += 1;
            if done >= needed / rustpg2d_engine::render::model::RENDERS_PER_FRAME as usize + 3 {
                let w = (facings.len() * moments) as u32 * cw as u32;
                let h = anims.len() as u32 * ch as u32 + 16;
                let shot = get_screen_data();
                let mut im = Image::gen_image_color(w as u16, h as u16, BLANK);
                // Screen data is bottom-up and `export_png` flips it: keep the screen's top rows
                // in that order.
                let top = shot.height() as u32 - h;
                for y in 0..h {
                    for x in 0..w {
                        let c = shot.get_pixel(x, top + y);
                        im.set_pixel(x, y, Color { a: 1.0, ..c });
                    }
                }
                im.export_png(out);
                println!("wrote {out} ({frame} frames)");
                return;
            }
        }
        next_frame().await;
    }
    eprintln!("the poses didn't all render");
}
