//! Scripted visual effects: sprites that fly to a point and then call a script function
//! (`fly`, the Fly action), and animations played once (`effect`, the Effect action), both
//! optionally glowing; timed lights at a point (Light at point) and area healing (Heal area).
//! Positions in the actions / scripts are tile coordinates.

use macroquad::prelude::*;

use super::Game;
use crate::defs::LightSpec;
use crate::events::FxSprite;
use crate::render::lighting::Light;

pub(super) struct Fx {
    sprite: FxSprite,
    /// World pixels (the sprite's center).
    pos: Vec2,
    age: f32,
    kind: FxKind,
}

/// A light at a point for a while (`light_at`).
pub(super) struct TimedLight {
    pos: Vec2,
    spec: LightSpec,
    left: f32,
}

enum FxKind {
    /// Flies to `target` (world pixels) at `speed` pixels per second, then calls `function`
    /// with `tile` (the target in tile coordinates as given).
    Fly { target: Vec2, tile: Vec2, speed: f32, function: String },
    /// Plays its frames once.
    Once,
}

impl Game {
    /// Number of frames of an effect sprite.
    fn fx_frames(&self, s: &FxSprite) -> u32 {
        if s.first == 0 && s.last == 0 {
            self.assets.sheets.get(&s.sheet).map_or(1, |t| t.count.max(1))
        } else {
            s.last.saturating_sub(s.first) + 1
        }
    }

    /// Where spells leave the player (about chest height).
    pub(super) fn cast_origin(&self) -> Vec2 {
        self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0 + 2.0)
    }

    /// Sends an animated sprite from the player to tile point `tile`; `function(x, y)` runs when
    /// it arrives.
    /// Applies a sprite's named animation (`FxSprite::tag`) from its sheet's tags.
    fn resolve_tag(&self, mut sprite: FxSprite) -> FxSprite {
        if !sprite.tag.is_empty() {
            match self.project.db.sheet(&sprite.sheet).and_then(|d| d.tags.iter().find(|t| t.name == sprite.tag)) {
                Some(t) => (sprite.first, sprite.last, sprite.fps) = (t.first, t.last.max(t.first), t.fps.max(0.1)),
                None => eprintln!("[fx] sheet '{}' has no animation '{}'", sprite.sheet, sprite.tag),
            }
        }
        sprite
    }

    pub(super) fn spawn_fly(&mut self, sprite: FxSprite, tile: Vec2, speed_tiles: f32, function: String) {
        let sprite = self.resolve_tag(sprite);
        if !self.assets.sheets.contains_key(&sprite.sheet) {
            eprintln!("[fx] fly: unknown sprite sheet '{}'", sprite.sheet);
        }
        let ts = self.ts();
        let origin = self.cast_origin();
        let target = tile * ts;
        if let Some(f) = crate::entity::Facing::from_vec(target.x - origin.x, target.y - origin.y) {
            self.player.facing = f;
        }
        let kind = FxKind::Fly { target, tile, speed: speed_tiles.max(0.5) * ts, function };
        self.fx.push(Fx { sprite, pos: origin, age: 0.0, kind });
    }

    /// Plays an animated sprite once, centered on tile point `tile`.
    pub(super) fn spawn_effect(&mut self, sprite: FxSprite, tile: Vec2) {
        let sprite = self.resolve_tag(sprite);
        if !self.assets.sheets.contains_key(&sprite.sheet) {
            eprintln!("[fx] effect: unknown sprite sheet '{}'", sprite.sheet);
        }
        let pos = tile * self.ts();
        // Explosions and the like can shake the screen (a bit harder than a hit).
        self.shake_screen(sprite.shake, 1.3);
        self.fx.push(Fx { sprite, pos, age: 0.0, kind: FxKind::Once });
    }

    /// A light at `pos` (world pixels) for `seconds`; it fades out during its last second.
    pub(super) fn light_at(&mut self, pos: Vec2, radius: f32, seconds: f32, color: [u8; 3], flicker: bool) {
        self.timed_lights.push(TimedLight { pos, spec: LightSpec::new(radius.max(0.1), color, flicker), left: seconds.max(0.05) });
    }

    /// Heals the player and friendly characters (not hostile ones) within `radius` tiles of
    /// `center` (world pixels).
    pub(super) fn heal_area(&mut self, center: Vec2, radius: f32, amount: i32) {
        let r = radius.max(0.5) * self.ts();
        if self.player.pos.distance(center) <= r {
            self.heal(amount);
        }
        for i in 0..self.loaded.entities.len() {
            let e = &mut self.loaded.entities[i];
            if e.active && !e.hostile && e.max_hp > 0 && e.body.pos.distance(center) <= r {
                e.hp = (e.hp + amount).min(e.max_hp);
                self.companion_healed(i);
            }
        }
    }

    /// Tells the script engine where the player is (player_x() / player_y() / ...).
    /// Gives the script host what scripts read about the player and the characters; called right
    /// before a script runs (not every frame: it clones every character's name).
    pub(super) fn sync_script_player(&mut self) {
        let ts = self.ts();
        let info = crate::script::PlayerInfo { x: self.player.pos.x / ts, y: self.player.pos.y / ts, max_hp: self.player_max_hp(), facing: self.player.facing, weather: self.weather(), pose: self.pose_name().into(), sleeping: self.sleeping(), faction: self.player_faction() };
        self.scripts.set_player(info);
        let chars = self.loaded.entities.iter().filter(|e| e.active && e.max_hp > 0).map(|e| (e.uid, e.name.clone(), e.hp)).collect();
        self.scripts.set_characters(chars);
        let merchants = self.merchant_stock();
        self.scripts.set_merchants(merchants);
        let casters = (0..self.loaded.entities.len())
            .filter(|&i| self.loaded.entities[i].kind == crate::entity::EntityKind::Character)
            .map(|i| (self.loaded.entities[i].uid, self.loaded.entities[i].name.clone(), self.npc_spell_list(i)))
            .filter(|c| !c.2.is_empty())
            .collect();
        self.scripts.set_casters(casters);
    }

    /// What a script call may ask about its trigger: the used item (`used_item()`) and the
    /// owner's name (`self_name()`).
    pub(super) fn script_event(&mut self, ctx: &crate::events::runner::ActionCtx) {
        let name = ctx.owner.and_then(|u| self.loaded.entities.iter().find(|e| e.uid == u)).map_or(String::new(), |e| e.name.clone());
        self.scripts.set_event(&ctx.item, &name);
    }

    /// Runs a script function and queues its actions (automated play-throughs).
    pub fn run_script(&mut self, function: &str) {
        self.sync_script_player();
        let actions = self.scripts.call(&mut self.world, function, None);
        self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
    }

    pub(super) fn update_fx(&mut self, dt: f32) {
        for l in &mut self.timed_lights {
            l.left -= dt;
        }
        self.timed_lights.retain(|l| l.left > 0.0);
        let mut arrived = Vec::new();
        let frames: Vec<u32> = self.fx.iter().map(|f| self.fx_frames(&f.sprite)).collect();
        let mut k = 0;
        self.fx.retain_mut(|f| {
            let n = frames[k];
            k += 1;
            f.age += dt;
            match &mut f.kind {
                FxKind::Fly { target, tile, speed, function } => {
                    let d = *target - f.pos;
                    let step = *speed * dt;
                    if d.length() <= step {
                        arrived.push((std::mem::take(function), *tile));
                        false
                    } else {
                        f.pos += d.normalize() * step;
                        true
                    }
                }
                FxKind::Once => f.age * f.sprite.fps < n as f32,
            }
        });
        for (function, tile) in arrived {
            if function.is_empty() {
                continue;
            }
            let args = vec![rhai::Dynamic::from(tile.x as f64), rhai::Dynamic::from(tile.y as f64)];
            let actions = self.scripts.call_args(&mut self.world, &function, args, None);
            if !actions.is_empty() {
                self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
            }
        }
    }

    /// Draws the effects (world space).
    /// Draws the effects played once (explosions, bursts) over the scene; flying sprites are
    /// depth-sorted with characters and props instead ([`Self::flying_fx`]).
    pub(super) fn draw_fx(&self) {
        for (k, f) in self.fx.iter().enumerate() {
            if matches!(f.kind, FxKind::Once) {
                self.draw_one_fx(k);
            }
        }
    }

    /// Flying sprites: index and y (their height in the air is added by the caller).
    pub(super) fn flying_fx(&self) -> impl Iterator<Item = (usize, f32)> + '_ {
        self.fx.iter().enumerate().filter(|(_, f)| matches!(f.kind, FxKind::Fly { .. })).map(|(k, f)| (k, f.pos.y))
    }

    /// Draws effect `k`.
    pub(super) fn draw_one_fx(&self, k: usize) {
        if let Some(f) = self.fx.get(k) {
            let n = self.fx_frames(&f.sprite);
            let step = (f.age * f.sprite.fps) as u32;
            let (frame, angle) = match &f.kind {
                FxKind::Fly { target, .. } => {
                    let d = *target - f.pos;
                    (step % n, d.y.atan2(d.x))
                }
                FxKind::Once => (step.min(n - 1), 0.0),
            };
            if !self.assets.draw_frame_scaled(&f.sprite.sheet, f.sprite.first + frame, f.pos, angle, f.sprite.scale, WHITE) {
                draw_circle(f.pos.x, f.pos.y, 6.0 * f.sprite.scale, ORANGE);
            }
        }
    }

    /// Lights of glowing effects (an effect played once fades out) and timed lights.
    pub(super) fn fx_lights(&self) -> Vec<Light> {
        let timed = self.timed_lights.iter().map(|l| Light::new(l.pos, &LightSpec { intensity: l.spec.intensity * l.left.min(1.0), ..l.spec.clone() }));
        self.fx
            .iter()
            .filter(|f| f.sprite.light > 0.0)
            .map(|f| {
                let fade = match f.kind {
                    FxKind::Fly { .. } => 1.0,
                    FxKind::Once => (1.0 - f.age * f.sprite.fps / self.fx_frames(&f.sprite) as f32).clamp(0.0, 1.0),
                };
                let spec = LightSpec { intensity: fade, ..LightSpec::new(f.sprite.light * (0.5 + 0.5 * fade), f.sprite.glow, true) };
                Light::new(f.pos, &spec)
            })
            .chain(timed)
            .collect()
    }

    /// Effect state for `Game::status()`: "fly:SHEET@x,y" / "once:SHEET" (tiles).
    pub(super) fn fx_status(&self) -> Vec<String> {
        let ts = self.ts();
        self.fx
            .iter()
            .map(|f| match f.kind {
                FxKind::Fly { .. } => format!("fly:{}@{:.1},{:.1}", f.sprite.sheet, f.pos.x / ts, f.pos.y / ts),
                FxKind::Once => format!("once:{}[{}-{}@{}fps]", f.sprite.sheet, f.sprite.first, f.sprite.last, f.sprite.fps),
            })
            .chain(self.timed_lights.iter().map(|l| format!("light@{:.1},{:.1}", l.pos.x / ts, l.pos.y / ts)))
            .collect()
    }
}
