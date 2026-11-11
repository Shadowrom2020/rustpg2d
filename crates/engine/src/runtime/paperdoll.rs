//! Drawing characters: a figure (a complete picture) in the colours chosen for its colour
//! features (see [`character::figure`]), and what it holds at its hands' grips
//! ([`crate::hands::held`]): behind the figure, then the figure, then in front of it.

use macroquad::prelude::*;

use super::Game;
pub(super) use crate::render::character::{self, Anim, Layer, ANIM_ROWS, LAYOUT_FRAMES};
use crate::defs::Appearance;
use crate::entity::Facing;
use crate::hands::HeldLayer;
use crate::state::Equipment;

impl Game {
    /// A character's figure frame (see [`character::figure`]).
    pub(super) fn character_figure(&self, a: &Appearance, facing: Facing, anim: impl Into<Anim>) -> Layer {
        character::figure(&self.project.db, a, facing, anim)
    }

    /// The frame of a layer its sheet has (the standing frame when it lacks the animation's row).
    pub(super) fn layer_frame(&self, l: &Layer) -> u32 {
        self.assets.sheets.get(&l.sheet).map_or(l.fallback, |s| l.frame_in(s.count))
    }

    /// Draws a character with its bottom-center at `feet`. Returns false if nothing could be
    /// drawn (missing sheets). `pose` = facing and what it does (walk frame, idle, attack, hurt).
    pub(super) fn draw_character(&self, a: &Appearance, held: &[HeldLayer], pose: (Facing, Anim), feet: Vec2, tint: Color) -> bool {
        // Bigger (or smaller) races: the figure at the race's size.
        let size = self.project.db.body_size(&a.body);
        let l = self.character_figure(a, pose.0, pose.1);
        self.draw_held(a, held, false, feet, tint);
        let drawn = if size == 1.0 {
            self.assets.draw_frame_colored(&l.sheet, self.layer_frame(&l), feet, tint, &l.colors)
        } else {
            self.assets.draw_frame_colored_sized(&l.sheet, self.layer_frame(&l), feet, tint, &l.colors, size)
        };
        self.draw_held(a, held, true, feet, tint);
        drawn
    }

    /// What a character holds this frame (see [`crate::hands::held`]).
    pub(super) fn character_held(&self, a: &Appearance, eq: &Equipment, carrying: &str, tool: Option<&str>, pose: (Facing, Anim)) -> Vec<HeldLayer> {
        crate::hands::held(&self.project.db, &a.body, eq, carrying, tool, pose.0, pose.1)
    }

    /// The world point where a held item's grip is, for a figure standing at `feet`.
    fn held_at(&self, a: &Appearance, feet: Vec2, h: &HeldLayer) -> Option<Vec2> {
        let s = self.assets.sheets.get(&a.body)?;
        let size = self.project.db.body_size(&a.body);
        Some(feet + vec2(h.at.0 - s.frame_w / 2.0, h.at.1 - s.frame_h) / s.scale * size)
    }

    /// The far end of a held item (the tip of a fishing rod) in the world: the opaque pixel of its
    /// picture farthest from the hold point, turned as drawn.
    pub(super) fn held_tip(&self, a: &Appearance, feet: Vec2, h: &HeldLayer) -> Option<Vec2> {
        let at = self.held_at(a, feet, h)?;
        let (sheet, (cw, ch)) = (self.assets.sheets.get(&h.sprite.sheet)?, h.sprite.cells());
        let src = sheet.region(h.sprite.frame, cw, ch);
        let size = sheet.world_of(src) * h.scale * self.project.db.body_size(&a.body);
        let k = vec2(size.x / src.w.max(1.0), size.y / src.h.max(1.0));
        let g = vec2(h.grip[0] * src.w, h.grip[1] * src.h);
        let (mut far, mut best) = (None, -1.0);
        for y in 0..src.h as u32 {
            for x in 0..src.w as u32 {
                let (ix, iy) = (src.x as u32 + x, src.y as u32 + y);
                if ix >= sheet.image.width() as u32 || iy >= sheet.image.height() as u32 || sheet.image.get_pixel(ix, iy).a < 0.5 {
                    continue;
                }
                // Pixel centres; the picture is mirrored top to bottom when the layer is flipped.
                let p = vec2(x as f32 + 0.5, if h.flip { src.h - y as f32 - 0.5 } else { y as f32 + 0.5 }) - g;
                if p.length_squared() > best {
                    (far, best) = (Some(p * k), p.length_squared());
                }
            }
        }
        let far = far?;
        let (sn, cs) = h.rotation.to_radians().sin_cos();
        Some(at + vec2(far.x * cs - far.y * sn, far.x * sn + far.y * cs))
    }

    /// Draws the held items in front of (or behind) a figure standing at `feet`.
    fn draw_held(&self, a: &Appearance, held: &[HeldLayer], front: bool, feet: Vec2, tint: Color) {
        let Some(s) = self.assets.sheets.get(&a.body) else { return };
        let size = self.project.db.body_size(&a.body);
        for l in held.iter().filter(|l| l.front == front) {
            let at = feet + vec2(l.at.0 - s.frame_w / 2.0, l.at.1 - s.frame_h) / s.scale * size;
            self.assets.draw_held_sprite(l, at, l.scale * size, tint);
        }
    }

    /// A figure with attack rows: its painted swing replaces the weapon's swing sprite.
    pub(super) fn figure_animates(&self, a: &Appearance) -> bool {
        self.assets.sheets.get(&a.body).is_some_and(|s| s.count >= ANIM_ROWS * LAYOUT_FRAMES)
    }

    /// Breathing frame (0 / 1) of a character standing still; `seed` keeps neighbours apart.
    fn breath(seed: u32) -> u32 {
        ((macroquad::time::get_time() * 1.25 + (seed % 97) as f64 * 0.37) as u32) % 2
    }

    /// What the player's frame shows: a swing, being hurt, walking or breathing.
    pub(super) fn player_anim(&self) -> Anim {
        let c = &self.combat;
        if c.swing > 0.0 {
            let p = 1.0 - c.swing / super::combat::SWING_TIME;
            return Anim::Attack(if p < 0.34 { 1 } else if p < 0.67 { 2 } else { 3 });
        }
        if c.invuln > super::combat::HURT_INVULN - 0.25 {
            return Anim::Hurt;
        }
        if self.player.moving {
            Anim::Walk((self.player.anim * 8.0) as u32 % LAYOUT_FRAMES)
        } else {
            Anim::Idle(Self::breath(0))
        }
    }

    /// What character `i`'s frame shows: its attack, being hurt, walking or breathing.
    pub(super) fn npc_anim(&self, i: usize) -> Anim {
        let e = &self.loaded.entities[i];
        if let Some(a) = e.attack.and_then(|a| a.anim()) {
            return a;
        }
        if e.hurt > 0.0 {
            return Anim::Hurt;
        }
        if e.body.moving {
            Anim::Walk((e.body.anim * 8.0) as u32 % LAYOUT_FRAMES)
        } else {
            Anim::Idle(Self::breath(e.uid))
        }
    }

    /// `anims=` for `status`: the player's and every nearby character's animation.
    pub(super) fn anims_status(&self) -> Vec<String> {
        let ts = self.ts();
        let mut out = vec![format!("player:{}", self.player_anim().name())];
        for (i, e) in self.loaded.entities.iter().enumerate() {
            if e.active && e.kind == crate::entity::EntityKind::Character && e.body.pos.distance(self.player.pos) < 8.0 * ts && e.npc_seat.is_none() {
                out.push(format!("{}:{}", if e.name.is_empty() { &e.def } else { &e.name }, self.npc_anim(i).name()));
            }
        }
        out
    }

    /// `held=` for `status`: what the player and the characters within 8 tiles hold, as
    /// `who:item@hand` (`R`, `L`, `B` = both; `front` / `back` of the figure).
    pub(super) fn held_status(&self) -> Vec<String> {
        let fmt = |who: &str, held: Vec<HeldLayer>| held.into_iter().map(|h| format!("{who}:{}@{}{}", h.id, &format!("{:?}", h.hand)[..1], if h.front { "" } else { "/back" })).collect::<Vec<_>>();
        let mut out = fmt("player", self.player_held((self.player.facing, self.player_anim())));
        let ts = self.ts();
        for (i, e) in self.loaded.entities.iter().enumerate() {
            if e.active && e.kind == crate::entity::EntityKind::Character && e.body.pos.distance(self.player.pos) < 8.0 * ts {
                let who = if e.name.is_empty() { &e.def } else { &e.name };
                out.extend(fmt(who, self.character_held(&e.appearance, &e.equipment, &e.carrying, self.activity_tool(i).as_deref(), (e.body.facing, self.npc_anim(i)))));
            }
        }
        out
    }

    /// Draws a character's front idle frame scaled into `dest` (portraits, the creator).
    pub(super) fn draw_character_in(&self, a: &Appearance, held: &[HeldLayer], dest: Rect) {
        let l = self.character_figure(a, Facing::Down, 0);
        let Some(s) = self.assets.sheets.get(&a.body) else { return };
        // Screen pixels per image pixel.
        let k = dest.w / s.frame_w;
        let place = |front: bool| {
            for h in held.iter().filter(|h| h.front == front) {
                self.assets.draw_held_sprite(h, vec2(dest.x + h.at.0 * k, dest.y + h.at.1 * k), h.scale * k * s.scale, WHITE);
            }
        };
        place(false);
        self.assets.draw_frame_colored_in(&l.sheet, self.layer_frame(&l), dest, &l.colors);
        place(true);
    }

    /// What the player holds (fishing: the rod instead of the weapon; carrying a prop).
    pub(super) fn player_held(&self, pose: (Facing, Anim)) -> Vec<HeldLayer> {
        let Some(a) = self.player_appearance() else { return Vec::new() };
        self.character_held(&a, &self.world.equipment, &self.world.player_carrying, self.player_tool_in_use().as_deref(), pose)
    }

    /// World size of a model's body frame (32 x 48 if the sheet is missing).
    pub(super) fn character_frame_size(&self, a: &Appearance) -> (f32, f32) {
        self.assets.sheets.get(&a.body).map_or((32.0, 48.0), |t| t.world_size().into())
    }

    /// The player's model: the creator's choice, else the player definition's.
    pub(super) fn player_appearance(&self) -> Option<Appearance> {
        if let Some(a) = &self.world.player_appearance {
            return Some(a.clone());
        }
        self.project.db.character(&self.project.config.player).map(|d| d.appearance.clone())
    }
}
