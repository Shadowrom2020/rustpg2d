//! Pictures over the game (*Show image* / `show_image()`): a sprite sheet drawn whole, at its
//! scale or scaled down to fit the window with its aspect ratio kept. A *scrolling* picture that
//! is taller than the window is fitted to the window's width and pans down and back up; the
//! player can scroll it by hand. A *waiting* picture pauses the event (and the world) until the
//! player closes it; one without waiting stays under the following dialogs until *Hide image* or
//! until the event is over.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::Game;

/// Seconds a scrolling picture rests at the top and the bottom.
const END_PAUSE: f32 = 1.0;
/// Hand scrolling (arrow keys, stick): screen pixels per second; the wheel moves 60 per notch.
const HAND_SPEED: f32 = 420.0;
const WHEEL_STEP: f32 = 60.0;

pub(super) struct ImageView {
    pub sheet: String,
    pub wait: bool,
    pub scroll: bool,
    speed: f32,
    scale: f32,
    /// The picture's size in pixels.
    size: Vec2,
    /// A smooth (linear filtered) copy for drawing it scaled down; the sheet's own texture is
    /// pixel-sharp.
    smooth: Option<Texture2D>,
    /// Screen pixels scrolled down from the top; direction +1 down / -1 up; rest at an end.
    offset: f32,
    dir: f32,
    rest: f32,
    /// The player scrolled by hand: no more automatic panning.
    manual: bool,
}

/// How a picture of `size` pixels at `scale` is drawn in a window of `win`: the scale it gets and
/// how far it can scroll (screen pixels; 0 = it fits).
pub(super) fn layout(size: Vec2, scale: f32, win: Vec2, scroll: bool) -> (f32, f32) {
    if size.x <= 0.0 || size.y <= 0.0 || win.x <= 0.0 || win.y <= 0.0 {
        return (1.0, 0.0);
    }
    let s = scale.max(0.01).min(win.x / size.x);
    if scroll && size.y * s > win.y {
        return (s, size.y * s - win.y);
    }
    (s.min(win.y / size.y), 0.0)
}

/// Whether a picture drawn at `scale` needs the smooth copy: scaled down, or by a fraction.
fn needs_smooth(scale: f32) -> bool {
    scale < 0.999 || (scale - scale.round()).abs() > 0.001
}

impl Game {
    /// The *Show image* action: false when the sheet doesn't exist (nothing to wait for).
    pub(super) fn show_image(&mut self, sheet: &str, wait: bool, scroll: bool, speed: f32, scale: f32) -> bool {
        let Some(tex) = self.assets.sheets.get(sheet) else {
            eprintln!("[game] Show image: there is no sprite sheet '{sheet}'");
            return false;
        };
        let size = vec2(tex.image.width() as f32, tex.image.height() as f32);
        let smooth = Texture2D::from_image(&tex.image);
        smooth.set_filter(FilterMode::Linear);
        self.image = Some(ImageView {
            sheet: sheet.to_string(),
            wait,
            scroll,
            speed: if speed > 0.0 { speed } else { crate::events::image_speed() },
            scale: if scale > 0.0 { scale } else { 1.0 },
            size,
            smooth: Some(smooth),
            offset: 0.0,
            dir: 1.0,
            rest: END_PAUSE,
            manual: false,
        });
        true
    }

    /// A waiting picture is open (the event waits for the player to close it).
    pub(super) fn image_waiting(&self) -> bool {
        self.image.as_ref().is_some_and(|v| v.wait)
    }

    /// Scrolls the picture and, for a waiting one, takes the player's input: closing it or
    /// scrolling by hand. True while a waiting picture pauses the world.
    pub(super) fn update_image(&mut self, dt: f32) -> bool {
        let Some(v) = &mut self.image else { return false };
        let (_, range) = layout(v.size, v.scale, vec2(screen_width(), screen_height()), v.scroll);
        if v.wait {
            if input::any_act_pressed(&[Act::Confirm, Act::Cancel, Act::Pause, Act::Attack]) || input::mouse_pressed(MouseButton::Left) {
                self.image = None;
                return true;
            }
            let hand = (input::act_down(Act::Down) as i32 - input::act_down(Act::Up) as i32) as f32 * HAND_SPEED * dt - input::wheel() * WHEEL_STEP;
            if hand != 0.0 && range > 0.0 {
                v.manual = true;
                v.offset += hand;
            }
        }
        if range > 0.0 && !v.manual {
            if v.rest > 0.0 {
                v.rest -= dt;
            } else {
                v.offset += v.dir * v.speed * dt;
                if v.offset >= range || v.offset <= 0.0 {
                    v.dir = -v.dir;
                    v.rest = END_PAUSE;
                }
            }
        }
        v.offset = v.offset.clamp(0.0, range);
        v.wait
    }

    /// Takes away a picture that doesn't wait once its event is over.
    pub(super) fn end_image_with_event(&mut self) {
        if self.image.as_ref().is_some_and(|v| !v.wait) && !self.runner.busy() && !self.ui.dialog_open() {
            self.image = None;
        }
    }

    /// Draws the picture over the game, under the dialog box.
    pub(super) fn draw_image(&self) {
        let Some(v) = &self.image else { return };
        let Some(sheet) = self.assets.sheets.get(&v.sheet) else { return };
        let win = vec2(screen_width(), screen_height());
        let (s, range) = layout(v.size, v.scale, win, v.scroll);
        draw_rectangle(0.0, 0.0, win.x, win.y, Color::new(0.0, 0.0, 0.0, 0.78));
        let (w, h) = (v.size.x * s, v.size.y * s);
        let x = ((win.x - w) / 2.0).round();
        let y = if range > 0.0 { -v.offset.round() } else { ((win.y - h) / 2.0).round() };
        let tex = if needs_smooth(s) { v.smooth.as_ref().unwrap_or(&sheet.texture) } else { &sheet.texture };
        draw_texture_ex(tex, x, y, WHITE, DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() });
    }

    /// For `status()`: "sheet:wait|behind[:scroll@offset/max]" or "-".
    pub(super) fn image_status(&self) -> String {
        let Some(v) = &self.image else { return "-".into() };
        let (_, range) = layout(v.size, v.scale, vec2(screen_width(), screen_height()), v.scroll);
        let mode = if v.wait { "wait" } else { "behind" };
        if range > 0.0 {
            format!("{}:{mode}:scroll@{:.0}/{:.0}{}", v.sheet, v.offset, range, if v.manual { ":hand" } else { "" })
        } else {
            format!("{}:{mode}", v.sheet)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: Vec2 = Vec2::new(1280.0, 720.0);

    #[test]
    fn pictures_fit_the_window_with_their_aspect_ratio() {
        // Smaller than the window: drawn at its scale.
        assert_eq!(layout(vec2(200.0, 100.0), 1.0, WIN, false), (1.0, 0.0));
        assert_eq!(layout(vec2(200.0, 100.0), 3.0, WIN, false), (3.0, 0.0));
        // Taller: scaled down to the window's height (and so keeps its proportions).
        let (s, range) = layout(vec2(864.0, 1536.0), 1.0, WIN, false);
        assert!((s - 720.0 / 1536.0).abs() < 1e-6 && range == 0.0);
        // Wider: to the width.
        let (s, _) = layout(vec2(2560.0, 400.0), 1.0, WIN, false);
        assert!((s - 0.5).abs() < 1e-6);
        // Both: the tighter fit wins.
        let (s, _) = layout(vec2(1024.0, 1024.0), 1.0, WIN, false);
        assert!((s - 720.0 / 1024.0).abs() < 1e-6);
    }

    #[test]
    fn scrolling_pictures_keep_their_width() {
        // Taller than the window: its own size, scrolling 1536 - 720 pixels.
        assert_eq!(layout(vec2(864.0, 1536.0), 1.0, WIN, true), (1.0, 816.0));
        // Wider too: fitted to the width, then scrolling what is left.
        let (s, range) = layout(vec2(2560.0, 3000.0), 1.0, WIN, true);
        assert!((s - 0.5).abs() < 1e-6 && (range - (1500.0 - 720.0)).abs() < 1e-3);
        // Fits: nothing to scroll, centred like a still picture.
        assert_eq!(layout(vec2(400.0, 300.0), 1.0, WIN, true), (1.0, 0.0));
        let (s, range) = layout(vec2(1024.0, 700.0), 1.0, WIN, true);
        assert!(s == 1.0 && range == 0.0);
    }

    #[test]
    fn smooth_drawing_only_when_scaled_down_or_by_a_fraction() {
        assert!(!needs_smooth(1.0) && !needs_smooth(2.0) && !needs_smooth(3.0));
        assert!(needs_smooth(0.47) && needs_smooth(1.5));
    }
}
