//! Drawing characters: a layered model (body + hair, recolored skin / hair) with clothing,
//! armor and the items in their hands on top (see [`character::layers`] for the order).

use macroquad::prelude::*;

use super::Game;
pub(super) use crate::render::character::{self, Anim, Layer, ANIM_ROWS, LAYOUT_FRAMES};
use crate::defs::Appearance;
use crate::entity::Facing;
use crate::state::Equipment;

impl Game {
    /// The layers of a character in drawing order (see [`character::layers`]).
    pub(super) fn character_layers(&self, a: &Appearance, eq: &Equipment, facing: Facing, anim: impl Into<Anim>, hide_weapon: bool) -> Vec<Layer> {
        character::layers(&self.project.db, a, eq, facing, anim, hide_weapon)
    }

    /// The frame of a layer its sheet has (the standing frame when it lacks the animation's row).
    pub(super) fn layer_frame(&self, l: &Layer) -> u32 {
        self.assets.sheets.get(&l.sheet).map_or(l.fallback, |s| l.frame_in(s.count))
    }

    /// Draws a character with its bottom-center at `feet`. Returns false if nothing could be
    /// drawn (missing sheets). `pose` = facing and what it does (walk frame, idle, attack, hurt).
    pub(super) fn draw_character(&self, a: &Appearance, eq: &Equipment, pose: (Facing, Anim), feet: Vec2, tint: Color, hide_weapon: bool) -> bool {
        let mut any = false;
        for l in self.character_layers(a, eq, pose.0, pose.1, hide_weapon) {
            any |= self.assets.draw_frame_colored(&l.sheet, self.layer_frame(&l), feet, tint, l.skin, l.hair);
        }
        any
    }

    /// A complete figure with attack rows: its painted swing replaces the weapon's swing sprite.
    pub(super) fn figure_animates(&self, a: &Appearance) -> bool {
        self.project.db.complete_body(&a.body) && self.assets.sheets.get(&a.body).is_some_and(|s| s.count >= ANIM_ROWS * LAYOUT_FRAMES)
    }

    /// The weapon in `eq` has a held sheet with attack rows (it swings in the hand: no separate
    /// swing sprite).
    pub(super) fn weapon_animates(&self, eq: &Equipment) -> bool {
        let held = eq.item(crate::defs::EquipSlot::Weapon).and_then(|id| self.project.db.item(id)).map(|d| d.held.clone()).unwrap_or_default();
        self.assets.sheets.get(&held).is_some_and(|s| s.count >= ANIM_ROWS * LAYOUT_FRAMES)
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

    /// Draws a character's front idle frame scaled into `dest` (portraits, the creator).
    pub(super) fn draw_character_in(&self, a: &Appearance, eq: &Equipment, dest: Rect) {
        for l in self.character_layers(a, eq, Facing::Down, 0, false) {
            self.assets.draw_frame_colored_in(&l.sheet, self.layer_frame(&l), dest, l.skin, l.hair);
        }
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
