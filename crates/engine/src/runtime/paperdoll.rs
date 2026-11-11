//! Drawing characters: a 3D model rendered with its gear into an image (`render/model.rs`), or a
//! figure (a complete picture) in the colours chosen for its colour features (see
//! [`character::figure`]) with what it holds at its hands' grips ([`crate::hands::held`]).

use macroquad::prelude::*;

use super::Game;
pub(super) use crate::render::character::{self, Anim, Layer, ANIM_ROWS, LAYOUT_FRAMES};
use crate::defs::Appearance;
use crate::entity::Facing;
use crate::hands::HeldLayer;
pub(super) use crate::render::model::Gear;
use crate::render::model::{Cell, PoseReq};
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

    /// Whether a character is drawn from a 3D model.
    pub(super) fn uses_model(&self, a: &Appearance) -> bool {
        !a.model.is_empty() && self.assets.models.has(&a.model)
    }

    /// The rendered pose of a model character (None: no model, or not rendered yet).
    pub(super) fn model_cell(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim)) -> Option<Cell> {
        if !self.uses_model(a) {
            return None;
        }
        let (anim, time, once) = match pose.1 {
            Anim::Walk(c) => ("walk", (c as f32 + 0.5) / LAYOUT_FRAMES as f32, true),
            Anim::Walking(t) => ("walk", t as f32 / 100.0, false),
            Anim::Idle(_) => ("idle", macroquad::time::get_time() as f32, false),
            // Wind-up, swing, follow-through, recover: points of the clip (strikes sit in its
            // middle; the player's swing shows the last three).
            Anim::Attack(k) => ("attack", [0.22, 0.42, 0.58, 0.8][k.min(3) as usize], true),
            Anim::Hurt => ("hurt", 0.4, true),
            Anim::Extra(n, _) => (n, macroquad::time::get_time() as f32, false),
        };
        let req = PoseReq { look: a, gear, anim, time, once, facing: pose.0, bust: false };
        self.assets.models.pose(&self.project.db, &self.assets.sheets, &req)
    }

    /// A model character's portrait render (head and shoulders, front).
    pub(super) fn model_bust(&self, a: &Appearance) -> Option<Cell> {
        if !self.uses_model(a) {
            return None;
        }
        let req = PoseReq { look: a, gear: &Gear::default(), anim: "idle", time: 0.0, once: true, facing: Facing::Down, bust: true };
        self.assets.models.pose(&self.project.db, &self.assets.sheets, &req)
    }

    /// Draws a character with its bottom-center at `feet`. Returns false if nothing could be
    /// drawn (missing art). `pose` = facing and what it does (walk frame, idle, attack, hurt).
    pub(super) fn draw_character(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim), feet: Vec2, tint: Color) -> bool {
        if self.uses_model(a) {
            let Some(cell) = self.model_cell(a, gear, pose) else { return false };
            self.assets.models.draw(&cell, feet, self.project.db.body_size(&a.body), tint);
            return true;
        }
        // Bigger (or smaller) races: the figure at the race's size.
        let size = self.project.db.body_size(&a.body);
        let held = self.sheet_held(a, gear, pose);
        let l = self.character_figure(a, pose.0, pose.1);
        self.draw_held(a, &held, false, feet, tint);
        let drawn = if size == 1.0 {
            self.assets.draw_frame_colored(&l.sheet, self.layer_frame(&l), feet, tint, &l.colors)
        } else {
            self.assets.draw_frame_colored_sized(&l.sheet, self.layer_frame(&l), feet, tint, &l.colors, size)
        };
        self.draw_held(a, &held, true, feet, tint);
        drawn
    }

    /// What a character holds: its weapon (or the tool it uses), its left-hand item, a carried prop.
    pub(super) fn character_gear(&self, eq: &Equipment, carrying: &str, tool: Option<&str>) -> Gear {
        Gear::of(eq, carrying, tool)
    }

    /// A figure's held items at its hand marks (see [`crate::hands::held`]).
    fn sheet_held(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim)) -> Vec<HeldLayer> {
        crate::hands::held(&self.project.db, &a.body, &gear.equipment(), &gear.carrying, None, pose.0, pose.1)
    }

    /// The far end of held item `item` (the tip of a fishing rod) of a character standing at
    /// `feet`, in the world.
    pub(super) fn held_tip(&self, a: &Appearance, feet: Vec2, gear: &Gear, pose: (Facing, Anim), item: &str) -> Option<Vec2> {
        if let Some(cell) = self.model_cell(a, gear, pose) {
            let size = self.project.db.body_size(&a.body);
            return cell.tips.iter().find(|(id, _)| id == item).map(|(_, p)| feet + (*p - cell.feet) / crate::render::model::PX * size);
        }
        let held = self.sheet_held(a, gear, pose);
        held.iter().find(|h| h.id == item).and_then(|h| self.sheet_held_tip(a, feet, h))
    }

    /// The world point where a held item's grip is, for a figure standing at `feet`.
    fn held_at(&self, a: &Appearance, feet: Vec2, h: &HeldLayer) -> Option<Vec2> {
        let s = self.assets.sheets.get(&a.body)?;
        let size = self.project.db.body_size(&a.body);
        Some(feet + vec2(h.at.0 - s.frame_w / 2.0, h.at.1 - s.frame_h) / s.scale * size)
    }

    /// The far end of a figure's held item in the world: the opaque pixel of its picture farthest
    /// from the hold point, turned as drawn.
    fn sheet_held_tip(&self, a: &Appearance, feet: Vec2, h: &HeldLayer) -> Option<Vec2> {
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
            Anim::Walking((self.player.anim * 100.0) as u32)
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
            Anim::Walking((e.body.anim * 100.0) as u32)
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
    /// `who:item@hand` (`R`, `L`, `B` = both; `/back` behind a figure).
    pub(super) fn held_status(&self) -> Vec<String> {
        let line = |who: &str, a: &Appearance, gear: &Gear, pose: (Facing, Anim)| -> Vec<String> {
            let held = if self.uses_model(a) {
                let g = Some(crate::hands::Grip::new(0.0, 0.0, 0.0));
                crate::hands::held_with(&self.project.db, (g, g), &gear.equipment(), &gear.carrying, None, pose.0)
            } else {
                self.sheet_held(a, gear, pose)
            };
            held.into_iter().map(|h| format!("{who}:{}@{}{}", h.id, &format!("{:?}", h.hand)[..1], if h.front { "" } else { "/back" })).collect()
        };
        let mut out = match self.player_appearance() {
            Some(a) => line("player", &a, &self.player_gear(), (self.player.facing, self.player_anim())),
            None => Vec::new(),
        };
        let ts = self.ts();
        for (i, e) in self.loaded.entities.iter().enumerate() {
            if e.active && e.kind == crate::entity::EntityKind::Character && e.body.pos.distance(self.player.pos) < 8.0 * ts {
                let who = if e.name.is_empty() { &e.def } else { &e.name };
                let gear = self.character_gear(&e.equipment, &e.carrying, self.activity_tool(i).as_deref());
                out.extend(line(who, &e.appearance, &gear, (e.body.facing, self.npc_anim(i))));
            }
        }
        out
    }

    /// Draws a character's front idle frame scaled into `dest` (portraits, the creator), with
    /// what it holds.
    pub(super) fn draw_character_in(&self, a: &Appearance, gear: &Gear, dest: Rect) {
        if self.uses_model(a) {
            if let Some(cell) = self.model_cell(a, gear, (Facing::Down, Anim::Idle(0))) {
                self.assets.models.draw_in(&cell, dest, WHITE);
            }
            return;
        }
        let held = self.sheet_held(a, gear, (Facing::Down, Anim::Walk(0)));
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
    pub(super) fn player_gear(&self) -> Gear {
        self.character_gear(&self.world.equipment, &self.world.player_carrying, self.player_tool_in_use().as_deref())
    }

    /// World size of a character's frame (32 x 48 if its art is missing).
    pub(super) fn character_frame_size(&self, a: &Appearance) -> (f32, f32) {
        if self.uses_model(a) {
            return self.assets.models.frame_size(&a.model).unwrap_or((32.0, 48.0));
        }
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
