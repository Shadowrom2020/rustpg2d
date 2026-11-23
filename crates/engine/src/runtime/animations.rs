//! Plays animation assets (`animations.rs`, `defs/animations.ron`) at a point: their cels and
//! particles drawn over the scene, their flashes (the target's figure or the screen) and sounds
//! started at their moments. Spells' impacts, *Play effect* and `effect()` play one where they
//! would play a sheet; weapon and stat-block attack hits, items used and class actions play
//! their own.

use macroquad::prelude::*;

use super::Game;
use crate::defs::LightSpec;
use crate::render::lighting::Light;

/// Whom an animation is about (its target flashes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnimTarget {
    Player,
    /// A character on the map by uid.
    Npc(u32),
}

/// An animation playing.
pub(super) struct AnimRun {
    id: String,
    /// World pixels: the point it plays at.
    pos: Vec2,
    target: Option<AnimTarget>,
    age: f32,
    len: f32,
    next_flash: usize,
    next_sound: usize,
}

/// A target's figure shining in a colour (an animation's flash).
pub(super) struct TintFlash {
    target: AnimTarget,
    color: [f32; 4],
    left: f32,
    len: f32,
}

impl Game {
    fn sheet_frames(&self, sheet: &str) -> u32 {
        self.assets.sheets.get(sheet).map_or(1, |s| s.count.max(1))
    }

    /// Plays animation `id` at `pos` (world pixels), about `target`; false when there is no
    /// such animation.
    pub(super) fn play_animation(&mut self, id: &str, pos: Vec2, target: Option<AnimTarget>) -> bool {
        let Some(d) = self.project.db.animation(id) else { return false };
        let len = d.length(|s| self.sheet_frames(s));
        self.anims.push(AnimRun { id: id.to_string(), pos, target, age: 0.0, len, next_flash: 0, next_sound: 0 });
        true
    }

    /// Where an animation about a character plays: the middle of its body.
    pub(super) fn anim_point(&self, feet: Vec2) -> Vec2 {
        feet - vec2(0.0, self.ts() * 0.6)
    }

    /// A combatant as an animation's target.
    pub(super) fn anim_target(&self, who: super::turns::Who) -> AnimTarget {
        match who {
            super::turns::Who::Player => AnimTarget::Player,
            super::turns::Who::Npc(uid) => AnimTarget::Npc(uid),
        }
    }

    /// Plays animation `id` on the player or the character with `uid` (nothing for "").
    pub(super) fn animate_on(&mut self, id: &str, target: AnimTarget) {
        if id.is_empty() {
            return;
        }
        let feet = match target {
            AnimTarget::Player => Some(self.player.pos),
            AnimTarget::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map(|e| e.body.pos),
        };
        if let Some(feet) = feet {
            let at = self.anim_point(feet);
            if !self.play_animation(id, at, Some(target)) {
                eprintln!("[fx] no animation '{id}'");
            }
        }
    }

    pub(super) fn update_animations(&mut self, dt: f32) {
        for f in &mut self.tint_flashes {
            f.left -= dt;
        }
        self.tint_flashes.retain(|f| f.left > 0.0);
        let mut flashes = Vec::new();
        let mut sounds = Vec::new();
        for run in &mut self.anims {
            run.age += dt;
            let Some(d) = self.project.db.animation(&run.id) else {
                run.age = f32::INFINITY;
                continue;
            };
            let (fl, so) = d.moments();
            while let Some(f) = fl.get(run.next_flash).filter(|f| f.at <= run.age) {
                flashes.push(((*f).clone(), run.target));
                run.next_flash += 1;
            }
            while let Some(s) = so.get(run.next_sound).filter(|s| s.at <= run.age) {
                sounds.push(s.sound.clone());
                run.next_sound += 1;
            }
        }
        self.anims.retain(|r| r.age <= r.len);
        for (f, target) in flashes {
            if f.screen {
                self.flash_screen(f.color, f.seconds);
            } else if let Some(t) = target {
                let len = f.seconds.max(0.05);
                self.tint_flashes.retain(|x| x.target != t);
                self.tint_flashes.push(TintFlash { target: t, color: f.color.map(|v| v as f32 / 255.0), left: len, len });
            }
        }
        for s in sounds {
            self.audio.play(&s);
        }
    }

    /// The colour a target's figure shines in now (an animation's flash), if any.
    pub(super) fn tint_flash_of(&self, target: AnimTarget) -> Option<Color> {
        self.tint_flashes.iter().find(|f| f.target == target).map(|f| {
            let k = (f.left / f.len).clamp(0.0, 1.0);
            Color::new(f.color[0], f.color[1], f.color[2], f.color[3] * k)
        })
    }

    /// Draws the animations playing (world space, over the scene).
    pub(super) fn draw_animations(&self) {
        let additive = super::pictures::additive();
        for run in &self.anims {
            let Some(d) = self.project.db.animation(&run.id) else { continue };
            for c in &d.cels {
                let Some(p) = c.pose(run.age, self.sheet_frames(&c.sheet)) else { continue };
                let mat = if c.additive { additive.as_ref() } else { None };
                if let Some(m) = mat {
                    gl_use_material(m);
                }
                let at = run.pos + vec2(p.offset[0], p.offset[1]);
                self.assets.draw_frame_scaled(&c.sheet, p.frame, at, p.rotation.to_radians(), p.scale, Color::new(1.0, 1.0, 1.0, p.opacity));
                if mat.is_some() {
                    gl_use_default_material();
                }
            }
            for e in &d.emitters {
                let parts = e.particles(run.age);
                if parts.is_empty() {
                    continue;
                }
                let mat = if e.additive { additive.as_ref() } else { None };
                if let Some(m) = mat {
                    gl_use_material(m);
                }
                for p in parts {
                    let c = Color::new(p.color[0], p.color[1], p.color[2], p.color[3]);
                    draw_circle(run.pos.x + p.pos[0], run.pos.y + p.pos[1], (p.size / 2.0).max(0.5), c);
                }
                if mat.is_some() {
                    gl_use_default_material();
                }
            }
        }
    }

    /// The lights of the animations' cels (fading with their opacity).
    pub(super) fn animation_lights(&self) -> Vec<Light> {
        let mut out = Vec::new();
        for run in &self.anims {
            let Some(d) = self.project.db.animation(&run.id) else { continue };
            for c in d.cels.iter().filter(|c| c.light > 0.0) {
                if let Some(p) = c.pose(run.age, self.sheet_frames(&c.sheet)) {
                    let spec = LightSpec { intensity: p.opacity, ..LightSpec::new(c.light, c.glow, true) };
                    out.push(Light::new(run.pos + vec2(p.offset[0], p.offset[1]), &spec));
                }
            }
        }
        out
    }

    /// For `status` (`fx=`): `anim:ID@x,y:SECONDS` (tiles) per animation playing.
    pub(super) fn animation_status(&self) -> Vec<String> {
        let ts = self.ts();
        self.anims.iter().map(|r| format!("anim:{}@{:.1},{:.1}:{:.2}", r.id, r.pos.x / ts, r.pos.y / ts, r.age)).collect()
    }
}
