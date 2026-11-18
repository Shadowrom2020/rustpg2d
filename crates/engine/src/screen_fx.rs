//! Screen tint and fades (*Tint screen*, *Fade out*, *Fade in*): colours over the whole screen
//! that change over seconds and stay until changed, saved in `WorldState::screen` (a fade-out
//! lasts across map changes until *Fade in*). Flashes are short and not saved (the runtime's).

use serde::{Deserialize, Serialize};

/// A colour over the screen, on its way from one colour to another (RGBA, 0 to 1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    pub from: [f32; 4],
    pub to: [f32; 4],
    #[serde(default)]
    pub elapsed: f32,
    #[serde(default)]
    pub seconds: f32,
}

impl Overlay {
    /// From `from` to `to` over `seconds`.
    pub fn new(from: [f32; 4], to: [f32; 4], seconds: f32) -> Self {
        Overlay { from, to, elapsed: 0.0, seconds: seconds.max(0.0) }
    }

    /// The colour now.
    pub fn now(&self) -> [f32; 4] {
        let k = if self.seconds <= 0.0 { 1.0 } else { (self.elapsed / self.seconds).clamp(0.0, 1.0) };
        std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * k)
    }

    pub fn update(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt).min(self.seconds.max(0.0));
    }

    pub fn done(&self) -> bool {
        self.elapsed >= self.seconds
    }
}

/// The saved screen colours.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenFx {
    /// *Tint screen*: over the lit world and the pictures, under the HUD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<Overlay>,
    /// *Fade out* / *Fade in*: covers the world, the pictures and the tint (dialogs and the HUD
    /// stay over it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Overlay>,
}

impl ScreenFx {
    pub fn is_empty(&self) -> bool {
        self.tint.is_none() && self.cover.is_none()
    }

    /// Advances the changes; a tint or cover that went fully clear is dropped.
    pub fn update(&mut self, dt: f32) {
        for o in [&mut self.tint, &mut self.cover] {
            if let Some(v) = o {
                v.update(dt);
                if v.done() && v.to[3] <= 0.0 {
                    *o = None;
                }
            }
        }
    }

    /// *Tint screen*: to `rgba` (alpha 0 = no tint) over `seconds`.
    pub fn tint_to(&mut self, rgba: [f32; 4], seconds: f32) {
        let from = self.tint.map_or([rgba[0], rgba[1], rgba[2], 0.0], |t| t.now());
        self.tint = Some(Overlay::new(from, rgba, seconds));
    }

    /// *Fade out* to `rgb` over `seconds`.
    pub fn fade_out(&mut self, rgb: [f32; 3], seconds: f32) {
        let from = self.cover.map_or([rgb[0], rgb[1], rgb[2], 0.0], |c| c.now());
        self.cover = Some(Overlay::new(from, [rgb[0], rgb[1], rgb[2], 1.0], seconds));
    }

    /// *Fade in* over `seconds` (nothing to do when the screen isn't faded).
    pub fn fade_in(&mut self, seconds: f32) {
        if let Some(c) = self.cover {
            let now = c.now();
            self.cover = Some(Overlay::new(now, [now[0], now[1], now[2], 0.0], seconds));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_changes_and_clears() {
        let mut s = ScreenFx::default();
        s.tint_to([1.0, 0.5, 0.0, 0.4], 2.0);
        s.update(1.0);
        assert_eq!(s.tint.unwrap().now(), [1.0, 0.5, 0.0, 0.2]);
        s.update(1.5);
        assert_eq!(s.tint.unwrap().now()[3], 0.4);
        s.tint_to([1.0, 0.5, 0.0, 0.0], 0.0);
        s.update(0.0);
        assert!(s.tint.is_none() && s.is_empty());
    }

    #[test]
    fn a_fade_out_stays_until_faded_in() {
        let mut s = ScreenFx::default();
        s.fade_out([0.0, 0.0, 0.0], 1.0);
        s.update(5.0);
        assert_eq!(s.cover.unwrap().now()[3], 1.0, "stays covered");
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<ScreenFx>(&text).unwrap(), s, "saved");
        s.fade_in(0.5);
        s.update(0.25);
        assert_eq!(s.cover.unwrap().now()[3], 0.5);
        s.update(0.5);
        assert!(s.cover.is_none());
    }
}
