//! Cut-scenes: a Rhai function played with the *Play cutscene* action (or `cutscene("f")`).
//! While one plays the HUD is hidden, black bars slide in at the top and bottom, and the
//! action runner keeps the player and the other characters still apart from what the scene
//! moves (`walk_to`, `approach`, `place`, `move_by`, `face`, dialogs, waits ...). Its end
//! (`EndCutscene`, queued after its actions) restores the HUD.
//!
//! Positions are tiles: `walk_to("guard", 12, 8)` walks onto tile 12, 8 (feet near its bottom,
//! like patrol points).

use macroquad::prelude::*;

use super::{Game, RtEntity};
use crate::entity::Facing;
use crate::events::runner::ActionCtx;

/// Height of the black bars (fraction of the screen height).
const BAR: f32 = 0.09;
/// Seconds the bars take to slide in / out.
const SLIDE: f32 = 0.4;

#[derive(Default)]
pub(super) struct CutsceneState {
    /// The cut-scene playing (None = none).
    playing: Option<Playing>,
    /// How far the black bars are in (0..1).
    bars: f32,
    /// Characters to turn once all scripted walking is done (from `Approach`).
    face_after: Vec<(String, Option<u32>, Facing)>,
    /// `Dim`: how dark the scene is (0..1), where it is going, and how fast (per second).
    dim: f32,
    dim_target: f32,
    dim_rate: f32,
}

/// Color of a fully dimmed scene (like a deep night outdoors).
const DIM_COLOR: Vec3 = Vec3::new(0.1, 0.12, 0.26);

struct Playing {
    name: String,
    /// Nested cut-scenes (one started from another) end with the outer one.
    depth: u32,
    /// The HUD was already hidden (H) before.
    ui_was_hidden: bool,
}

impl Game {
    pub(super) fn cutscene_active(&self) -> bool {
        self.cutscene.playing.is_some()
    }

    pub(super) fn begin_cutscene(&mut self, name: &str) {
        match &mut self.cutscene.playing {
            Some(p) => p.depth += 1,
            None => {
                self.cutscene.playing = Some(Playing { name: name.to_string(), depth: 1, ui_was_hidden: self.ui_hidden });
                self.ui_hidden = true;
                // Windows and an aim in progress make way for the scene.
                self.inv.close();
                self.close_window();
                self.cancel_aim();
            }
        }
    }

    pub(super) fn end_cutscene(&mut self) {
        let Some(p) = &mut self.cutscene.playing else { return };
        p.depth = p.depth.saturating_sub(1);
        if p.depth == 0 {
            self.ui_hidden = p.ui_was_hidden;
            self.cutscene.playing = None;
            // A scene left dimmed brightens again.
            if self.cutscene.dim_target > 0.0 {
                self.dim_to(0.0, 1.0);
            }
        }
    }

    /// Forgets a cut-scene (new game, loading a save).
    pub(super) fn reset_cutscene(&mut self) {
        self.cutscene.playing = None;
        self.cutscene.face_after.clear();
        (self.cutscene.dim, self.cutscene.dim_target) = (0.0, 0.0);
    }

    /// `Dim`: darken (or brighten) the scene to `amount` over `seconds`.
    pub(super) fn dim_to(&mut self, amount: f32, seconds: f32) {
        let c = &mut self.cutscene;
        c.dim_target = amount.clamp(0.0, 1.0);
        c.dim_rate = if seconds <= 0.0 { f32::INFINITY } else { (c.dim_target - c.dim).abs() / seconds };
    }

    /// The scene's ambient light with the dimming applied (None = no darkness at all).
    pub(super) fn scene_ambient(&self, ambient: Option<Color>) -> Option<Color> {
        let d = self.cutscene.dim;
        if d <= 0.001 {
            return ambient;
        }
        let base = ambient.map_or(Vec3::ONE, |c| vec3(c.r, c.g, c.b));
        let c = base.lerp(base.min(DIM_COLOR), d);
        Some(Color::new(c.x, c.y, c.z, 1.0))
    }

    /// Bars in / out; a cut-scene whose actions were dropped (e.g. by loading) ends.
    pub(super) fn update_cutscene(&mut self, dt: f32) {
        if self.cutscene.playing.is_some() && !self.runner.busy() {
            if let Some(p) = &mut self.cutscene.playing {
                p.depth = 1;
            }
            self.end_cutscene();
        }
        let c = &mut self.cutscene;
        let step = (c.dim_rate * dt).min((c.dim_target - c.dim).abs());
        c.dim += step * (c.dim_target - c.dim).signum();
        let target = if self.cutscene.playing.is_some() { 1.0 } else { 0.0 };
        let step = dt / SLIDE;
        self.cutscene.bars = if self.cutscene.bars < target { (self.cutscene.bars + step).min(1.0) } else { (self.cutscene.bars - step).max(0.0) };
    }

    /// The black bars (screen space).
    pub(super) fn draw_cutscene_bars(&self) {
        let t = self.cutscene.bars;
        if t <= 0.0 {
            return;
        }
        let (sw, sh) = (screen_width(), screen_height());
        let h = (sh * BAR * t * (2.0 - t)).round(); // ease out
        draw_rectangle(0.0, 0.0, sw, h, BLACK);
        draw_rectangle(0.0, sh - h, sw, h, BLACK);
    }

    /// Where a target ("player", "self", a name) stands.
    fn target_pos(&self, target: &str, ctx: &ActionCtx) -> Option<Vec2> {
        if target == "player" {
            return Some(self.player.pos);
        }
        self.entity_index(target, ctx.owner).map(|i| self.loaded.entities[i].body.pos)
    }

    /// Starts walking a target to `dest` (scripted: no collisions). False if there is no such
    /// target.
    fn walk_target(&mut self, target: &str, ctx: &ActionCtx, dest: Vec2) -> bool {
        if target == "player" {
            self.player.move_target = Some(dest);
            return true;
        }
        match self.entity_index(target, ctx.owner) {
            Some(i) => {
                self.loaded.entities[i].body.move_target = Some(dest);
                true
            }
            None => {
                eprintln!("[cutscene] no character '{target}' on this map");
                false
            }
        }
    }

    /// `WalkTo`: true = wait for the arrival.
    pub(super) fn walk_to(&mut self, target: &str, ctx: &ActionCtx, x: i32, y: i32) -> bool {
        let dest = self.tile_spot((x, y));
        self.walk_target(target, ctx, dest)
    }

    /// `Approach`: walks next to `to` (on the side the walker comes from, along the longer
    /// axis); both face each other once all walking is done. True = wait for the arrival.
    pub(super) fn approach(&mut self, target: &str, to: &str, ctx: &ActionCtx) -> bool {
        let (Some(a), Some(b)) = (self.target_pos(target, ctx), self.target_pos(to, ctx)) else {
            eprintln!("[cutscene] approach: '{target}' or '{to}' isn't on this map");
            return false;
        };
        let d = a - b;
        let ts = self.ts();
        let off = if d.x.abs() >= d.y.abs() { vec2(d.x.signum() * ts, 0.0) } else { vec2(0.0, d.y.signum() * ts) };
        let off = if off == Vec2::ZERO { vec2(0.0, ts) } else { off };
        let dest = b + off;
        if !self.walk_target(target, ctx, dest) {
            return false;
        }
        let toward = |v: Vec2| Facing::from_vec(v.x, v.y).unwrap_or(Facing::Down);
        let to_uid = (to != "player").then(|| self.entity_index(to, ctx.owner).map(|i| self.loaded.entities[i].uid)).flatten();
        let target_uid = (target != "player").then(|| self.entity_index(target, ctx.owner).map(|i| self.loaded.entities[i].uid)).flatten();
        self.cutscene.face_after.push((target.to_string(), target_uid, toward(-off)));
        self.cutscene.face_after.push((to.to_string(), to_uid, toward(off)));
        true
    }

    /// `Place`: puts a target on a tile at once.
    pub(super) fn place(&mut self, target: &str, ctx: &ActionCtx, x: i32, y: i32) {
        let p = self.tile_spot((x, y));
        if target == "player" {
            self.player.pos = p;
            self.player.move_target = None;
            return;
        }
        match self.entity_index(target, ctx.owner) {
            Some(i) => {
                let e: &mut RtEntity = &mut self.loaded.entities[i];
                e.body.pos = p;
                e.body.move_target = None;
                e.home = p;
                self.occupy(i);
                self.persist(i);
            }
            None => eprintln!("[cutscene] place: no character '{target}' on this map"),
        }
    }

    /// Turns the characters `Approach` left waiting (once no one walks any more).
    pub(super) fn apply_face_after(&mut self) {
        for (target, uid, facing) in std::mem::take(&mut self.cutscene.face_after) {
            if target == "player" {
                self.player.facing = facing;
            } else if let Some(e) = self.loaded.entities.iter_mut().find(|e| Some(e.uid) == uid) {
                e.body.facing = facing;
            }
        }
    }

    /// For `Game::status()`: the cut-scene playing, or "-".
    pub(super) fn cutscene_status(&self) -> String {
        let name = self.cutscene.playing.as_ref().map_or("-".into(), |p| p.name.clone());
        format!("{name} dim={:.2}", self.cutscene.dim)
    }
}
