//! Photo mode (P, or *Photo mode* in the pause menu): the world stands still (nothing is updated),
//! the HUD hides and the camera moves freely over the map: arrows / the left stick or a mouse
//! drag, the wheel or + / − to zoom. H hides the player, [ / ] (LT / RT) show another hour's light.
//! E / Enter / Space (A) takes a picture of the world alone, saved as a PNG in the player's
//! Pictures folder (`<Pictures>/<game>/`; the web game offers it as a download); Esc / P (B)
//! leaves, back to the clock as it was.

use macroquad::prelude::*;

use super::input::{self, Act, Device, Pad};
use super::text::{draw_text, measure_text};
use super::Game;

const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_STEP: f32 = 1.25;
/// Camera speed in screen pixels per second (twice that with Run).
const PAN: f32 = 420.0;
/// How long the white flash and the "saved" line show (seconds).
const FLASH: f32 = 0.25;
const SAVED_LINE: f32 = 4.0;

#[derive(Default)]
pub(super) struct Photo {
    pub on: bool,
    /// The camera's centre (world pixels) and zoom (times the game's own view scale).
    pub center: Vec2,
    pub zoom: f32,
    /// The player isn't drawn.
    pub hide_player: bool,
    /// The clock when photo mode began: restored when it ends (the hour shown may change).
    hour: f32,
    /// A picture is taken while the next frame is drawn (the world only).
    take: bool,
    /// A mouse drag: where it began and the camera's centre then.
    drag: Option<(Vec2, Vec2)>,
    /// The last picture: where it went, and seconds since (the flash, the "saved" line).
    pub last: String,
    since: f32,
}

impl Game {
    pub(super) fn photo_on(&self) -> bool {
        self.photo.on
    }

    /// Photo mode can begin: playing, nothing open, nothing scripted running.
    pub(super) fn can_photo(&self) -> bool {
        !self.on_title() && self.game_over.is_none() && !self.ui.dialog_open() && !self.inventory_open() && !self.world_map_open() && !self.cutscene_active() && self.creator.is_none() && !self.aiming()
    }

    pub(super) fn start_photo(&mut self) {
        let center = self.player.pos - vec2(0.0, 8.0);
        self.photo = Photo { on: true, center, zoom: 1.0, hour: self.world.hour, last: std::mem::take(&mut self.photo.last), since: SAVED_LINE, ..Default::default() };
    }

    fn end_photo(&mut self) {
        self.world.hour = self.photo.hour;
        self.photo.on = false;
        self.photo.drag = None;
    }

    /// Photo mode's input (instead of the game's update: the world stands still).
    pub(super) fn update_photo(&mut self, dt: f32) {
        let real = get_frame_time().min(0.1);
        self.photo.since += real;
        if input::act_pressed(Act::Cancel) || input::act_pressed(Act::Photo) {
            self.end_photo();
            return;
        }
        let scale = self.view_scale() * self.photo.zoom;
        // Moving: keys and the stick, faster with Run; a mouse drag pulls the world along.
        let mut dir = Vec2::ZERO;
        for (a, d) in [(Act::Left, vec2(-1.0, 0.0)), (Act::Right, vec2(1.0, 0.0)), (Act::Up, vec2(0.0, -1.0)), (Act::Down, vec2(0.0, 1.0))] {
            if input::act_down(a) {
                dir += d;
            }
        }
        let fast = if input::act_down(Act::Run) && input::device() != Device::Pad { 2.0 } else { 1.0 };
        self.photo.center += dir.normalize_or_zero() * PAN * fast * real.max(dt) / scale;
        let m = input::mouse_pos();
        if input::mouse_pressed(MouseButton::Left) {
            self.photo.drag = Some((m, self.photo.center));
        }
        if let Some((from, center)) = self.photo.drag {
            self.photo.center = center - (m - from) / scale;
            if !input::mouse_down(MouseButton::Left) {
                self.photo.drag = None;
            }
        }
        // Zoom: the wheel, + / −, LB / RB.
        let wheel = input::wheel();
        if wheel > 0.0 || input::act_pressed(Act::ZoomIn) {
            self.photo.zoom = (self.photo.zoom * ZOOM_STEP).min(ZOOM_MAX);
        } else if wheel < 0.0 || input::act_pressed(Act::ZoomOut) {
            self.photo.zoom = (self.photo.zoom / ZOOM_STEP).max(ZOOM_MIN);
        }
        if input::act_pressed(Act::HideUi) || input::pad_pressed(Pad::North) {
            self.photo.hide_player = !self.photo.hide_player;
        }
        // Another hour's light (the clock comes back when photo mode ends).
        let step = if input::pressed(KeyCode::RightBracket) || input::pad_pressed(Pad::Rt) {
            1.0
        } else if input::pressed(KeyCode::LeftBracket) || input::pad_pressed(Pad::Lt) {
            -1.0
        } else {
            0.0
        };
        if step != 0.0 {
            self.world.hour = (self.world.hour.floor() + step).rem_euclid(24.0);
        }
        if input::act_pressed(Act::Confirm) || input::pressed(KeyCode::Space) {
            self.photo.take = true;
        }
        // Over the map (a little past its edges).
        let ts = self.ts();
        let (w, h) = (self.loaded.map.width as f32 * ts, self.loaded.map.height as f32 * ts);
        self.photo.center = self.photo.center.clamp(Vec2::ZERO, vec2(w, h));
    }

    /// The camera's centre and scale in photo mode.
    pub(super) fn photo_view(&self) -> Option<(Vec2, f32)> {
        self.photo.on.then(|| (self.photo.center, self.view_scale() * self.photo.zoom))
    }

    /// While drawing, right after the world (no HUD, no hint): takes the asked-for picture.
    pub(super) fn capture_photo(&mut self) {
        if !std::mem::take(&mut self.photo.take) {
            return;
        }
        let shot = get_screen_data();
        let (w, h) = (shot.width as u32, shot.height as u32);
        let mut img = crate::imageio::Rgba::new(w, h);
        // (OpenGL reads rows bottom-up.)
        for y in 0..h {
            let row = ((h - 1 - y) * w * 4) as usize;
            for x in 0..w {
                let i = row + (x * 4) as usize;
                img.set(x as i32, y as i32, [shot.bytes[i], shot.bytes[i + 1], shot.bytes[i + 2], 255]);
            }
        }
        let name = format!("{}-{}.png", file_stem(&self.project.config.name), stamp(crate::sys::unix_now()));
        // Into the Pictures folder, else (no permission, a sandbox) beside the save games.
        let saved = img.encode().map_err(|e| e.to_string()).and_then(|png| {
            crate::sys::give_player(&photos_dir(self).join(&name), &png).or_else(|e| match fallback_dir(self) {
                Some(dir) => crate::sys::give_player(&dir.join(&name), &png),
                None => Err(e),
            }).map_err(|e| e.to_string())
        });
        match saved {
            Ok(place) => self.photo.last = place,
            Err(e) => {
                eprintln!("[game] photo {name}: {e}");
                self.photo.last = format!("could not be saved ({e})");
            }
        }
        self.photo.since = 0.0;
        // (A click like a camera's shutter.)
        self.audio.play(&self.project.config.sfx.pickup);
    }

    /// The flash of a picture taken and the hint bar (not in the picture).
    pub(super) fn draw_photo_hint(&self) {
        let (sw, sh) = (screen_width(), screen_height());
        let flash = 1.0 - self.photo.since / FLASH;
        if flash > 0.0 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(1.0, 1.0, 1.0, flash * 0.6));
        }
        let pad = input::device() == Device::Pad;
        let hour = format!("{:02}:00", self.world.hour.floor() as u32 % 24);
        let lines = [
            if pad {
                format!("Photo mode   Stick: move   LB / RB: zoom {:.1}x   LT / RT: time {hour}   Y: hide yourself   A: take picture   B: back", self.photo.zoom)
            } else {
                format!("Photo mode   Arrows / drag: move   Wheel / + -: zoom {:.1}x   [ ]: time {hour}   H: hide yourself   E / Space: take picture   Esc: back", self.photo.zoom)
            },
            if self.photo.since < SAVED_LINE && !self.photo.last.is_empty() { format!("Photo: {}", self.photo.last) } else { String::new() },
        ];
        let size = 16.0;
        let bar = 12.0 + size * if lines[1].is_empty() { 1.0 } else { 2.2 };
        draw_rectangle(0.0, sh - bar, sw, bar, Color::new(0.0, 0.0, 0.0, 0.55));
        for (k, l) in lines.iter().filter(|l| !l.is_empty()).enumerate() {
            let w = measure_text(l, None, size as u16, 1.0).width;
            draw_text(l, ((sw - w) / 2.0).max(8.0), sh - bar + 4.0 + size * (k as f32 * 1.2 + 0.9), size, if k == 0 { WHITE } else { Color::new(0.75, 0.9, 1.0, 1.0) });
        }
    }

    /// `photo=` for `status`: off, or on:zoom:hour[:hidden]@centre tile, and the last picture.
    pub(super) fn photo_status(&self) -> String {
        let last = if self.photo.last.is_empty() { String::new() } else { format!(" photo_last={}", self.photo.last) };
        if !self.photo.on {
            return format!("off{last}");
        }
        let c = self.photo.center / self.ts();
        format!("on:{:.2}x:{:02}h{}@{:.0},{:.0}{last}", self.photo.zoom, self.world.hour.floor() as u32 % 24, if self.photo.hide_player { ":hidden" } else { "" }, c.x, c.y)
    }
}

/// Where pictures go: `RUSTPG2D_PHOTOS_DIR`, else `<Pictures>/<game name>`, else a `photos`
/// folder beside the save games.
fn photos_dir(g: &Game) -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("RUSTPG2D_PHOTOS_DIR").filter(|d| !d.is_empty()) {
        return d.into();
    }
    match crate::paths::pictures() {
        Some(p) if !cfg!(target_arch = "wasm32") => p.join(file_stem(&g.project.config.name)),
        _ => saves_photos(g),
    }
}

/// The `photos` folder beside the save games.
fn saves_photos(g: &Game) -> std::path::PathBuf {
    let saves = g.project.saves_dir();
    saves.parent().map_or_else(|| saves.join("photos"), |p| p.join("photos"))
}

/// Where a picture goes when the usual folder can't be written (None when that already was it).
fn fallback_dir(g: &Game) -> Option<std::path::PathBuf> {
    let d = saves_photos(g);
    (d != photos_dir(g)).then_some(d)
}

/// A game's name as a file name: letters, digits, `-` and `_` (spaces become `_`).
fn file_stem(name: &str) -> String {
    let s: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if s.trim_matches('_').is_empty() { "photo".into() } else { s }
}

/// `YYYY-MM-DD_HH-MM-SS` (UTC) of a Unix time.
fn stamp(secs: u64) -> String {
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}_{:02}-{:02}-{:02}", rest / 3600, rest % 3600 / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_names_have_the_date_and_a_safe_stem() {
        assert_eq!(stamp(0), "1970-01-01_00-00-00");
        assert_eq!(stamp(1_794_729_123), "2026-11-15_07-52-03");
        assert_eq!(stamp(951_782_400), "2000-02-29_00-00-00");
        assert_eq!(file_stem("Emberbrook: A Tale"), "Emberbrook__A_Tale");
        assert_eq!(file_stem("  "), "photo");
    }
}
