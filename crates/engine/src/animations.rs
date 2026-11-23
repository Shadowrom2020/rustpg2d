//! Animation assets (`defs/animations.ron`): what spells, attacks, class actions, items and
//! *Play effect* show where they land. An [`AnimationDef`] holds timed **cels** (frames of a
//! sprite sheet with place, size, turn, opacity and blend, optionally moving to end values),
//! **particle emitters**, **flashes** of the target or the screen and **sounds**, all at moments
//! in seconds from its start.
//!
//! Everything here is worked out from the time alone (particles too: each one's start, direction
//! and speed come from its number), so the editor's preview and the game draw the same thing
//! (`runtime/animations.rs` plays them). Places are pixels at the 32 px tile, from the point the
//! animation plays at (the middle of the target's body); +x is right, +y down.

use serde::{Deserialize, Serialize};

/// An animation asset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationDef {
    pub id: String,
    /// Name shown in the editor.
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cels: Vec<Cel>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub emitters: Vec<Emitter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flashes: Vec<AnimFlash>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<AnimSound>,
}

/// Frames of a sprite sheet shown from a moment on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cel {
    /// Seconds from the start.
    pub at: f32,
    pub sheet: String,
    /// The frames it plays, first to last (both 0 = all of the sheet's).
    pub first: u32,
    pub last: u32,
    /// Frames per second.
    pub fps: f32,
    /// How many times it plays its frames (at least once).
    pub loops: u32,
    /// Where (pixels from the point), how big (1 = the frame's size), turned (degrees,
    /// clockwise) and how opaque (0 to 1).
    pub offset: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
    /// Adds its light to what is under it (fire, magic) instead of covering it.
    pub additive: bool,
    /// A light around it (radius in tiles, 0 = none) of this colour, fading with its opacity.
    #[serde(skip_serializing_if = "is_zero")]
    pub light: f32,
    pub glow: [u8; 3],
    /// Moves to these values over its time (None = stays).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<CelEnd>,
}

impl Default for Cel {
    fn default() -> Self {
        Cel { at: 0.0, sheet: String::new(), first: 0, last: 0, fps: 12.0, loops: 1, offset: [0.0, 0.0], scale: 1.0, rotation: 0.0, opacity: 1.0, additive: false, light: 0.0, glow: [255, 170, 90], end: None }
    }
}

/// Where a cel ends up at its last frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CelEnd {
    pub offset: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
}

impl Default for CelEnd {
    fn default() -> Self {
        CelEnd { offset: [0.0, 0.0], scale: 1.0, rotation: 0.0, opacity: 1.0 }
    }
}

/// A cel at a moment: which frame of its sheet (counted from the sheet's start) and how.
#[derive(Clone, Debug, PartialEq)]
pub struct CelPose {
    pub frame: u32,
    pub offset: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
}

/// Particles sent out from a point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Emitter {
    pub at: f32,
    /// Seconds it keeps sending them, evenly (0 = all at once).
    pub duration: f32,
    /// How many in all.
    pub count: u32,
    /// Where they start (pixels from the point), and how far around it at random (a radius).
    pub offset: [f32; 2],
    pub area: f32,
    /// Their direction (degrees: 0 = right, 90 = down, -90 = up) and how wide the spread is
    /// around it (360 = every way).
    pub direction: f32,
    pub spread: f32,
    /// Pixels a second, and how much slower some are at random (0 to 1).
    pub speed: f32,
    pub speed_var: f32,
    /// Seconds each one lives.
    pub lifetime: f32,
    /// Size (pixels across) at the start and at the end of its life.
    pub size: f32,
    pub size_end: f32,
    /// Colour (RGBA) at the start and at the end of its life.
    pub color: [u8; 4],
    pub color_end: [u8; 4],
    /// Pulls them down (pixels a second each second; negative = up, like flames).
    pub gravity: f32,
    pub additive: bool,
}

impl Default for Emitter {
    fn default() -> Self {
        Emitter {
            at: 0.0,
            duration: 0.0,
            count: 20,
            offset: [0.0, 0.0],
            area: 0.0,
            direction: -90.0,
            spread: 360.0,
            speed: 60.0,
            speed_var: 0.5,
            lifetime: 0.6,
            size: 4.0,
            size_end: 1.0,
            color: [255, 220, 140, 255],
            color_end: [255, 90, 30, 0],
            gravity: 0.0,
            additive: true,
        }
    }
}

/// One particle at a moment: where (pixels from the point), how big, its colour (0 to 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub pos: [f32; 2],
    pub size: f32,
    pub color: [f32; 4],
}

/// A flash of the target (its figure shines in the colour) or of the whole screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimFlash {
    pub at: f32,
    pub screen: bool,
    /// RGBA: alpha = how strong.
    pub color: [u8; 4],
    pub seconds: f32,
}

impl Default for AnimFlash {
    fn default() -> Self {
        AnimFlash { at: 0.0, screen: false, color: [255, 255, 255, 200], seconds: 0.3 }
    }
}

/// A sound at a moment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimSound {
    pub at: f32,
    pub sound: String,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A number from 0 to 1 that only depends on `n` and `salt` (a particle's dice).
fn rand01(n: u32, salt: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0xFF_FFFF) as f32 / 0x100_0000 as f32
}

impl Cel {
    /// How many frames it plays from a sheet of `sheet_frames`.
    pub fn frames(&self, sheet_frames: u32) -> u32 {
        if self.first == 0 && self.last == 0 {
            sheet_frames.max(1)
        } else {
            self.last.saturating_sub(self.first) + 1
        }
    }

    /// Seconds it shows.
    pub fn seconds(&self, sheet_frames: u32) -> f32 {
        self.frames(sheet_frames) as f32 * self.loops.max(1) as f32 / self.fps.max(0.1)
    }

    /// How it shows at `t` seconds from the animation's start (None = not now).
    pub fn pose(&self, t: f32, sheet_frames: u32) -> Option<CelPose> {
        let age = t - self.at;
        let len = self.seconds(sheet_frames);
        if age < 0.0 || age >= len {
            return None;
        }
        let n = self.frames(sheet_frames);
        // (A hair over: 0.4 s at 10 fps is frame 4, not 3.999…)
        let step = (age * self.fps.max(0.1) + 1e-3) as u32 % n;
        let k = (age / len).clamp(0.0, 1.0);
        let (offset, scale, rotation, opacity) = match &self.end {
            Some(e) => (
                [lerp(self.offset[0], e.offset[0], k), lerp(self.offset[1], e.offset[1], k)],
                lerp(self.scale, e.scale, k),
                lerp(self.rotation, e.rotation, k),
                lerp(self.opacity, e.opacity, k),
            ),
            None => (self.offset, self.scale, self.rotation, self.opacity),
        };
        Some(CelPose { frame: self.first + step, offset, scale, rotation, opacity: opacity.clamp(0.0, 1.0) })
    }
}

impl Emitter {
    /// Seconds until its last particle is gone (from the animation's start).
    pub fn end(&self) -> f32 {
        self.at + self.duration.max(0.0) + self.lifetime.max(0.0)
    }

    /// Its particles alive at `t` seconds from the animation's start.
    pub fn particles(&self, t: f32) -> Vec<Particle> {
        let mut out = Vec::new();
        let n = self.count.min(2000);
        let life = self.lifetime.max(0.01);
        let c0 = self.color.map(|v| v as f32 / 255.0);
        let c1 = self.color_end.map(|v| v as f32 / 255.0);
        for i in 0..n {
            let born = self.at + if n > 1 { self.duration.max(0.0) * i as f32 / (n - 1) as f32 } else { 0.0 };
            let age = t - born;
            if age < 0.0 || age >= life {
                continue;
            }
            let k = age / life;
            let angle = (self.direction + (rand01(i, 1) - 0.5) * self.spread).to_radians();
            let speed = self.speed * (1.0 - self.speed_var.clamp(0.0, 1.0) * rand01(i, 2));
            let (ra, rr) = (rand01(i, 3) * std::f32::consts::TAU, self.area.max(0.0) * rand01(i, 4).sqrt());
            let start = [self.offset[0] + ra.cos() * rr, self.offset[1] + ra.sin() * rr];
            let pos = [start[0] + angle.cos() * speed * age, start[1] + angle.sin() * speed * age + 0.5 * self.gravity * age * age];
            let color = [lerp(c0[0], c1[0], k), lerp(c0[1], c1[1], k), lerp(c0[2], c1[2], k), lerp(c0[3], c1[3], k)];
            out.push(Particle { pos, size: lerp(self.size, self.size_end, k).max(0.0), color });
        }
        out
    }
}

impl AnimationDef {
    /// The editor's "+ New" value: a short burst of sparks.
    pub fn fresh(id: &str) -> Self {
        AnimationDef { id: id.to_string(), name: id.to_string(), emitters: vec![Emitter::default()], ..Default::default() }
    }

    /// Seconds until everything in it is over (`frames` = a sheet's frame count by id).
    pub fn length(&self, frames: impl Fn(&str) -> u32) -> f32 {
        let cels = self.cels.iter().map(|c| c.at + c.seconds(frames(&c.sheet)));
        let emitters = self.emitters.iter().map(Emitter::end);
        let flashes = self.flashes.iter().map(|f| f.at + f.seconds);
        let sounds = self.sounds.iter().map(|s| s.at);
        cels.chain(emitters).chain(flashes).chain(sounds).fold(0.0, f32::max)
    }

    /// Its flashes and sounds sorted by time (the order the player starts them).
    pub fn moments(&self) -> (Vec<&AnimFlash>, Vec<&AnimSound>) {
        let mut f: Vec<&AnimFlash> = self.flashes.iter().collect();
        f.sort_by(|a, b| a.at.total_cmp(&b.at));
        let mut s: Vec<&AnimSound> = self.sounds.iter().collect();
        s.sort_by(|a, b| a.at.total_cmp(&b.at));
        (f, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cel_plays_its_frames_and_moves_to_its_end() {
        let c = Cel { at: 0.5, fps: 10.0, end: Some(CelEnd { offset: [0.0, -20.0], scale: 2.0, opacity: 0.0, ..Default::default() }), ..Default::default() };
        // 8 frames at 10 fps: 0.8 s from 0.5 s on.
        assert!(c.pose(0.4, 8).is_none() && c.pose(1.31, 8).is_none());
        let p = c.pose(0.9, 8).unwrap();
        assert_eq!(p.frame, 4);
        assert!((p.offset[1] + 10.0).abs() < 1e-3 && (p.scale - 1.5).abs() < 1e-3 && (p.opacity - 0.5).abs() < 1e-3);
        // A part of the sheet, twice.
        let part = Cel { first: 3, last: 5, loops: 2, fps: 6.0, ..Default::default() };
        assert!((part.seconds(8) - 1.0).abs() < 1e-4);
        assert_eq!(part.pose(0.6, 8).unwrap().frame, 3);
    }

    #[test]
    fn particles_come_from_the_time_alone() {
        let e = Emitter { at: 0.2, count: 10, duration: 0.0, lifetime: 0.5, gravity: 100.0, ..Default::default() };
        assert!(e.particles(0.1).is_empty());
        let a = e.particles(0.4);
        assert_eq!(a.len(), 10);
        assert_eq!(a, e.particles(0.4), "the same every time");
        assert!(e.particles(0.71).is_empty());
        // Spread over a second: fewer alive early on.
        let spread = Emitter { duration: 1.0, ..e.clone() };
        assert!(spread.particles(0.3).len() < 10);
        let d = AnimationDef { emitters: vec![e], flashes: vec![AnimFlash { at: 0.1, seconds: 1.0, ..Default::default() }], ..Default::default() };
        assert!((d.length(|_| 1) - 1.1).abs() < 1e-4);
    }

    #[test]
    fn an_animation_round_trips_and_defaults() {
        let d = AnimationDef { id: "fire".into(), cels: vec![Cel { sheet: "fx_fire".into(), additive: true, ..Default::default() }], sounds: vec![AnimSound { at: 0.0, sound: "fireball".into() }], ..AnimationDef::fresh("fire") };
        let text = ron::to_string(&d).unwrap();
        assert_eq!(ron::from_str::<AnimationDef>(&text).unwrap(), d);
        let bare: AnimationDef = ron::from_str(r#"(id: "x", cels: [(sheet: "s")])"#).unwrap();
        assert_eq!((bare.cels[0].fps, bare.cels[0].scale, bare.cels[0].opacity, bare.cels[0].loops), (12.0, 1.0, 1.0, 1));
    }
}
