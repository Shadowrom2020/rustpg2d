//! Drawing characters: a figure (a complete picture) in the colours chosen for its colour
//! features (see [`character::figure`]).

use macroquad::prelude::*;

use super::Game;
pub(super) use crate::render::character::{self, Anim, Layer, ANIM_ROWS, LAYOUT_FRAMES};
use crate::defs::Appearance;
use crate::entity::Facing;

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
    pub(super) fn draw_character(&self, a: &Appearance, pose: (Facing, Anim), feet: Vec2, tint: Color) -> bool {
        // Bigger (or smaller) races: the figure at the race's size.
        let size = self.project.db.body_size(&a.body);
        let l = self.character_figure(a, pose.0, pose.1);
        if size == 1.0 {
            self.assets.draw_frame_colored(&l.sheet, self.layer_frame(&l), feet, tint, &l.colors)
        } else {
            self.assets.draw_frame_colored_sized(&l.sheet, self.layer_frame(&l), feet, tint, &l.colors, size)
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

    /// Draws a character's front idle frame scaled into `dest` (portraits, the creator).
    pub(super) fn draw_character_in(&self, a: &Appearance, dest: Rect) {
        let l = self.character_figure(a, Facing::Down, 0);
        self.assets.draw_frame_colored_in(&l.sheet, self.layer_frame(&l), dest, &l.colors);
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
