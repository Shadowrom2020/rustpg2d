//! Draws a little of everything off screen and writes it as a PNG: `cargo run -p rustpg2d-gfx
//! --example offscreen -- OUT.png`.
use gfx::prelude::*;
use rustpg2d_gfx as gfx;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "offscreen.png".into());
    gfx::init_headless(320, 200).expect("a graphics adapter");
    gfx::begin_frame();
    clear_background(Color::new(0.1, 0.12, 0.2, 1.0));
    draw_rectangle(10.0, 10.0, 60.0, 40.0, RED);
    draw_rectangle_lines(80.0, 10.0, 60.0, 40.0, 2.0, YELLOW);
    draw_circle(180.0, 30.0, 20.0, GREEN);
    draw_line(210.0, 10.0, 300.0, 50.0, 3.0, SKYBLUE);
    draw_triangle(vec2(10.0, 120.0), vec2(60.0, 70.0), vec2(110.0, 120.0), ORANGE);
    draw_text("Hello, R(ust)PG2D! äöü", 10.0, 150.0, 16.0, WHITE);
    draw_text("Bigger text", 10.0, 180.0, 32.0, GOLD);

    // A checkered texture, drawn plain, scaled with nearest pixels, and through a render target.
    let mut img = Image::gen_image_color(8, 8, WHITE);
    for y in 0..8 {
        for x in 0..8 {
            if (x + y) % 2 == 0 {
                img.set_pixel(x, y, if y < 4 { BLUE } else { MAGENTA });
            }
        }
    }
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Nearest);
    draw_texture(&tex, 130.0, 70.0, WHITE);
    draw_texture_ex(&tex, 150.0, 70.0, WHITE, DrawTextureParams { dest_size: Some(vec2(48.0, 48.0)), ..Default::default() });

    let rt = render_target(64, 64);
    rt.texture.set_filter(FilterMode::Nearest);
    set_camera(&Camera2D { zoom: vec2(2.0 / 64.0, 2.0 / 64.0), target: vec2(32.0, 32.0), render_target: Some(rt.clone()), ..Default::default() });
    clear_background(DARKGRAY);
    draw_texture_ex(&tex, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(32.0, 32.0)), ..Default::default() });
    draw_text("RT", 34.0, 60.0, 16.0, WHITE);
    set_default_camera();
    draw_texture(&rt.texture, 210.0, 70.0, WHITE);
    draw_texture_ex(&rt.texture, 280.0, 70.0, WHITE, DrawTextureParams { dest_size: Some(vec2(32.0, 32.0)), rotation: 0.5, ..Default::default() });
    gfx::end_frame();
    get_screen_data().export_png(&out);
    let s = gfx::window::frame_stats();
    println!("{out}: {} draw calls, {} passes", s.draw_calls, s.passes);
}
