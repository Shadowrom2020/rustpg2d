//! The game without a window: the example project's house is loaded, played for a second and a
//! half and drawn off screen, and the picture is compared with a kept one (the painted ground, the
//! 3D figures, text and the UI's skin in one picture).
//!
//! Needs a graphics adapter. Without one the test is skipped, unless `RUSTPG2D_GPU_TESTS=1` asks
//! for it (CI does, with Vulkan's software renderer: `RUSTPG2D_ALLOW_SOFTWARE=1`).
//! `RUSTPG2D_BLESS=1` writes the kept picture anew (look at it before committing it).

use std::path::{Path, PathBuf};

use rustpg2d_engine::gfx;
use rustpg2d_engine::project::Project;
use rustpg2d_engine::runtime::{Game, StartOptions};

/// A scratch copy of the example project (without save games).
fn example_copy(to: &Path) {
    fn copy(from: &Path, to: &Path) {
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
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project"), to);
}

/// Share of pixels (in percent) of which a channel differs by more than `tolerance`.
fn differing(a: &[u8], b: &[u8], tolerance: u8) -> f32 {
    let n = a.len() / 4;
    let off = a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0).filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > tolerance)).count();
    off as f32 * 100.0 / n.max(1) as f32
}

#[test]
fn the_example_draws_without_a_window() {
    let required = std::env::var("RUSTPG2D_GPU_TESTS").is_ok_and(|v| v == "1");
    if let Err(e) = gfx::init_headless(1280, 720) {
        assert!(!required, "RUSTPG2D_GPU_TESTS=1, but: {e}");
        eprintln!("skipped (no graphics adapter): {e}");
        return;
    }
    let tmp: PathBuf = std::env::temp_dir().join(format!("rustpg2d-headless-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    example_copy(&tmp.join("proj"));
    // (This test is the only one of its binary: the environment is its own.)
    std::env::set_var("RUSTPG2D_MUTE", "1");
    std::env::set_var("RUSTPG2D_SETTINGS_DIR", tmp.join("settings"));

    let project = Project::load(&tmp.join("proj")).expect("the example project loads");
    gfx::time::set_fixed_step(Some(1.0 / 60.0));
    rustpg2d_engine::srd::dice::seed(1);
    let start = StartOptions { map: Some("house".into()), spawn: Some("start".into()), title: false, ..Default::default() };
    let mut game = Game::new(project, start).expect("the game starts");
    for _ in 0..90 {
        gfx::begin_frame();
        game.frame();
        gfx::end_frame();
    }
    let status = game.status();
    assert!(status.contains("map=house") && status.contains("menu=-"), "{}", &status[..status.len().min(400)]);
    let stats = gfx::window::frame_stats();
    assert!(stats.draw_calls > 10 && stats.passes >= 1, "{stats:?}");

    // The screen comes bottom row first.
    let shot = gfx::texture::get_screen_data();
    let (w, h) = (shot.width as usize, shot.height as usize);
    let mut rgba = Vec::with_capacity(shot.bytes.len());
    for y in (0..h).rev() {
        rgba.extend_from_slice(&shot.bytes[y * w * 4..(y + 1) * w * 4]);
    }
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/house.png");
    if std::env::var("RUSTPG2D_BLESS").is_ok_and(|v| v == "1") {
        std::fs::write(&golden, rustpg2d_engine::imageio::Rgba { w: w as u32, h: h as u32, px: rgba.clone() }.encode().unwrap()).expect("writing the kept picture");
        eprintln!("wrote {}", golden.display());
    }
    let kept = rustpg2d_engine::imageio::Rgba::decode(&std::fs::read(&golden).expect("tests/golden/house.png (RUSTPG2D_BLESS=1 writes it)")).expect("a PNG");
    assert_eq!((kept.w as usize, kept.h as usize), (w, h));
    // Graphics cards round edges differently, and the software renderer filters a little
    // differently: a few percent of pixels may be off, a broken shader or a flipped target is not.
    let off = differing(&kept.px, &rgba, 24);
    if off > 3.0 {
        let actual = std::env::temp_dir().join("rustpg2d-headless-house.png");
        if let Ok(png) = (rustpg2d_engine::imageio::Rgba { w: w as u32, h: h as u32, px: rgba }).encode() {
            let _ = std::fs::write(&actual, png);
        }
        panic!("{off:.2} % of the pixels differ from tests/golden/house.png (this run's picture: {})", actual.display());
    }

    // The same game in a view of its own, with its own input: how an editor's panel shows it.
    let mut view = gfx::Screen::new(640, 360, 1.0);
    let mut input = gfx::input::Input::new();
    gfx::with_screen(&mut view, &mut input, || {
        assert_eq!((gfx::window::screen_width(), gfx::window::screen_height()), (640.0, 360.0));
        game.frame();
    });
    assert_eq!(gfx::window::screen_width(), 1280.0, "the screen is the window's again");
    let small = view.texture().get_texture_data();
    assert_eq!((small.width, small.height), (640, 360));
    let lit = small.bytes.as_chunks::<4>().0.iter().filter(|p| p[0] > 90 && p[1] > 60).count();
    assert!(lit > 2000, "the view shows the room ({lit} bright pixels)");
    let _ = std::fs::remove_dir_all(&tmp);
}
