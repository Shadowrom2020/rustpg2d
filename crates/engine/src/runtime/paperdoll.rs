//! Drawing characters: a layered model (body + hair, recolored skin / hair) with clothing,
//! armor and the items in their hands on top (see [`character::layers`] for the order).

use macroquad::prelude::*;

use super::Game;
pub(super) use crate::render::character::{self, Layer, LAYOUT_FRAMES};
use crate::defs::Appearance;
use crate::entity::Facing;
use crate::state::Equipment;

impl Game {
    /// The layers of a character in drawing order (see [`character::layers`]).
    pub(super) fn character_layers(&self, a: &Appearance, eq: &Equipment, facing: Facing, col: u32, hide_weapon: bool) -> Vec<Layer> {
        character::layers(&self.project.db, a, eq, facing, col, hide_weapon)
    }

    /// Draws a character with its bottom-center at `feet`. Returns false if nothing could be
    /// drawn (missing sheets). `pose` = facing and walk frame.
    pub(super) fn draw_character(&self, a: &Appearance, eq: &Equipment, pose: (Facing, u32), feet: Vec2, tint: Color, hide_weapon: bool) -> bool {
        let mut any = false;
        for l in self.character_layers(a, eq, pose.0, pose.1, hide_weapon) {
            any |= self.assets.draw_frame_colored(&l.sheet, l.frame, feet, tint, l.skin, l.hair);
        }
        any
    }

    /// Draws a character's front idle frame scaled into `dest` (portraits, the creator).
    pub(super) fn draw_character_in(&self, a: &Appearance, eq: &Equipment, dest: Rect) {
        for l in self.character_layers(a, eq, Facing::Down, 0, false) {
            self.assets.draw_frame_colored_in(&l.sheet, l.frame, dest, l.skin, l.hair);
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
