//! Picture layers (*Show picture* / *Move picture* / *Erase picture*): up to 100 numbered pictures
//! over the map, each a frame of a sprite sheet placed on the screen (by an anchor, like HUD
//! elements) or on the map, moved, faded, scaled, rotated and tinted with easing. They are kept in
//! `WorldState::pictures` (saved, a move in progress included); `runtime/pictures.rs` draws them
//! above the map and below the HUD, in layer order.

use serde::{Deserialize, Serialize};

use crate::script_ui::Anchor;

/// The highest layer number.
pub const MAX_LAYER: u32 = 100;

/// How a move speeds up and slows down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Easing {
    #[default]
    Linear,
    /// Starts slowly.
    In,
    /// Ends slowly.
    Out,
    /// Starts and ends slowly.
    InOut,
}

impl Easing {
    pub const ALL: [Easing; 4] = [Easing::Linear, Easing::In, Easing::Out, Easing::InOut];

    pub fn name(self) -> &'static str {
        match self {
            Easing::Linear => "linear",
            Easing::In => "in",
            Easing::Out => "out",
            Easing::InOut => "in_out",
        }
    }

    pub fn parse(s: &str) -> Option<Easing> {
        let k: String = s.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        Easing::ALL.into_iter().find(|e| e.name().replace('_', "") == k || (k == "easein" && *e == Easing::In) || (k == "easeout" && *e == Easing::Out) || (k == "easeinout" && *e == Easing::InOut))
    }

    /// Progress `t` (0 to 1) eased.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::In => t * t,
            Easing::Out => t * (2.0 - t),
            Easing::InOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// How a picture mixes with what is under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Blend {
    #[default]
    Normal,
    /// Adds its light (glows, flashes of magic).
    Additive,
}

/// What a move changes: the place (screen pixels from the anchor, or map tiles), size, opacity
/// (0 to 1), rotation (degrees, clockwise) and tint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PictureLook {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub rotation: f32,
    pub tint: [u8; 3],
}

impl Default for PictureLook {
    fn default() -> Self {
        PictureLook { x: 0.0, y: 0.0, scale: 1.0, opacity: 1.0, rotation: 0.0, tint: [255, 255, 255] }
    }
}

impl PictureLook {
    /// The look `k` (0 to 1) of the way from `self` to `to`.
    pub fn lerp(&self, to: &PictureLook, k: f32) -> PictureLook {
        let l = |a: f32, b: f32| a + (b - a) * k;
        let c = |a: u8, b: u8| l(a as f32, b as f32).round().clamp(0.0, 255.0) as u8;
        PictureLook {
            x: l(self.x, to.x),
            y: l(self.y, to.y),
            scale: l(self.scale, to.scale),
            opacity: l(self.opacity, to.opacity),
            rotation: l(self.rotation, to.rotation),
            tint: [c(self.tint[0], to.tint[0]), c(self.tint[1], to.tint[1]), c(self.tint[2], to.tint[2])],
        }
    }
}

/// A move in progress.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PictureMove {
    pub from: PictureLook,
    pub to: PictureLook,
    pub elapsed: f32,
    pub seconds: f32,
    #[serde(default)]
    pub easing: Easing,
}

/// A picture on a layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Picture {
    pub sheet: String,
    #[serde(default)]
    pub frame: u32,
    /// Where on the screen its place counts from (ignored on the map: x, y are a tile there and
    /// the picture's middle sits on it).
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub on_map: bool,
    #[serde(default)]
    pub blend: Blend,
    pub look: PictureLook,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moving: Option<PictureMove>,
}

impl Picture {
    /// Advances its move by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        let Some(m) = &mut self.moving else { return };
        m.elapsed += dt;
        let k = if m.seconds <= 0.0 { 1.0 } else { m.easing.apply(m.elapsed / m.seconds) };
        self.look = m.from.lerp(&m.to, k);
        if m.elapsed >= m.seconds {
            self.look = m.to;
            self.moving = None;
        }
    }

    /// Starts a move to `to` over `seconds` (at once with 0).
    pub fn move_to(&mut self, to: PictureLook, seconds: f32, easing: Easing) {
        if seconds <= 0.0 {
            self.look = to;
            self.moving = None;
        } else {
            self.moving = Some(PictureMove { from: self.look, to, elapsed: 0.0, seconds, easing });
        }
    }
}

/// The targets of *Move picture*: each `None` keeps the current value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PictureTargets {
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    #[serde(default)]
    pub scale: Option<f32>,
    #[serde(default)]
    pub opacity: Option<f32>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub tint: Option<[u8; 3]>,
}

impl PictureTargets {
    /// `base` with the targets set.
    pub fn applied(&self, base: &PictureLook) -> PictureLook {
        PictureLook {
            x: self.x.unwrap_or(base.x),
            y: self.y.unwrap_or(base.y),
            scale: self.scale.unwrap_or(base.scale),
            opacity: self.opacity.unwrap_or(base.opacity).clamp(0.0, 1.0),
            rotation: self.rotation.unwrap_or(base.rotation),
            tint: self.tint.unwrap_or(base.tint),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_curves() {
        for e in Easing::ALL {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
            assert_eq!(Easing::parse(e.name()), Some(e));
        }
        assert!(Easing::In.apply(0.5) < 0.5 && Easing::Out.apply(0.5) > 0.5);
        assert_eq!(Easing::InOut.apply(0.5), 0.5);
        assert_eq!(Easing::parse("ease in out"), Some(Easing::InOut));
    }

    #[test]
    fn a_move_eases_to_its_targets_and_ends() {
        let mut p = Picture { sheet: "card".into(), frame: 0, anchor: Anchor::Center, on_map: false, blend: Blend::Normal, look: PictureLook { opacity: 0.0, ..Default::default() }, moving: None };
        let to = PictureTargets { opacity: Some(1.0), x: Some(100.0), ..Default::default() }.applied(&p.look);
        assert_eq!((to.opacity, to.x, to.scale), (1.0, 100.0, 1.0));
        p.move_to(to, 2.0, Easing::Linear);
        p.update(1.0);
        assert_eq!((p.look.opacity, p.look.x), (0.5, 50.0));
        let text = ron::to_string(&p).unwrap();
        assert_eq!(ron::from_str::<Picture>(&text).unwrap(), p, "a move in progress is saved");
        p.update(1.5);
        assert!(p.moving.is_none());
        assert_eq!(p.look, to);
    }
}
