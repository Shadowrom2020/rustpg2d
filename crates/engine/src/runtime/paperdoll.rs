//! Drawing characters: every character is a 3D model rendered with its gear into an image
//! (`render/model.rs`). [`Game::look`] says which: the character's model; a look of an older
//! project that names a figure plays the model of the same id; anything else is the mannequin.

use macroquad::prelude::*;

use super::Game;
pub(super) use crate::render::character::Anim;
use crate::defs::Appearance;
use crate::entity::Facing;
pub(super) use crate::render::model::Gear;
use crate::render::model::{Cell, PoseReq};
use crate::state::Equipment;

impl Game {
    /// What a look is drawn as: its model, else the model named like its (old) figure, else the
    /// mannequin.
    pub(super) fn look(&self, a: &Appearance) -> Appearance {
        self.assets.models.look_of(a)
    }

    /// Size of a character's race (trolls are bigger): races list their models.
    pub(super) fn look_size(&self, a: &Appearance) -> f32 {
        self.project.db.body_size(&self.look(a).model)
    }

    /// The rendered pose of a character (None: not rendered yet, or no model at all).
    pub(super) fn model_cell(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim)) -> Option<Cell> {
        self.model_cell_turned(a, gear, pose, 0)
    }

    /// [`Self::model_cell`] turned a further eighth (`turn`: -1, 0, 1; see `diagonal_turn`).
    pub(super) fn model_cell_turned(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim), turn: i8) -> Option<Cell> {
        let a = &self.look(a);
        let (anim, time, once) = crate::render::model::anim_req(pose.1, macroquad::time::get_time() as f32);
        let req = PoseReq { look: a, gear, anim, time, once, facing: pose.0, turn, bust: false };
        self.assets.models.pose(&self.project.db, &self.assets.sheets, &req)
    }

    /// How far a walker is drawn turned from its facing: an eighth towards the diagonal it walks
    /// along (`ProjectConfig::eight_facings`), else none.
    pub(super) fn walk_turn(&self, body: &super::Body, anim: Anim) -> i8 {
        if !self.project.config.eight_facings || !body.moving || !matches!(anim, Anim::Walking(_)) {
            return 0;
        }
        crate::render::model::diagonal_turn(body.facing, body.heading)
    }

    /// A character's portrait render (head and shoulders, front).
    pub(super) fn model_bust(&self, a: &Appearance) -> Option<Cell> {
        let a = &self.look(a);
        let req = PoseReq { look: a, gear: &Gear::default(), anim: "idle", time: 0.0, once: true, facing: Facing::Down, turn: 0, bust: true };
        self.assets.models.pose(&self.project.db, &self.assets.sheets, &req)
    }

    /// Whether a character's model has a clip for `anim` ("sit", "attack"…).
    pub(super) fn has_anim(&self, a: &Appearance, anim: &str) -> bool {
        self.assets.models.has_anim(&self.look(a).model, anim)
    }

    /// Draws a character with its bottom-center at `feet`. Returns false if nothing could be
    /// drawn (not rendered yet). `pose` = facing and what it does (walk frame, idle, attack, hurt).
    pub(super) fn draw_character(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim), feet: Vec2, tint: Color) -> bool {
        self.draw_character_turned(a, gear, pose, 0, feet, tint)
    }

    /// [`Self::draw_character`] turned a further eighth (see [`Self::walk_turn`]).
    pub(super) fn draw_character_turned(&self, a: &Appearance, gear: &Gear, pose: (Facing, Anim), turn: i8, feet: Vec2, tint: Color) -> bool {
        let Some(cell) = self.model_cell_turned(a, gear, pose, turn) else { return false };
        self.assets.models.draw(&cell, feet, self.look_size(a), tint);
        true
    }

    /// What a character holds: its weapon (or the tool it uses), its left-hand item, a carried prop.
    pub(super) fn character_gear(&self, eq: &Equipment, carrying: &str, tool: Option<&str>) -> Gear {
        Gear::of(eq, carrying, tool)
    }

    /// The far end of held item `item` (the tip of a fishing rod) of a character standing at
    /// `feet`, in the world.
    pub(super) fn held_tip(&self, a: &Appearance, feet: Vec2, gear: &Gear, pose: (Facing, Anim), item: &str) -> Option<Vec2> {
        let cell = self.model_cell(a, gear, pose)?;
        let size = self.look_size(a);
        cell.tips.iter().find(|(id, _)| id == item).map(|(_, p)| feet + (*p - cell.feet) / crate::render::model::PX * size)
    }

    /// Whether a character swings its own attack (its model has an attack clip) instead of the
    /// weapon's swing sprite.
    pub(super) fn figure_animates(&self, a: &Appearance) -> bool {
        self.has_anim(a, "attack")
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
        // A diagonal walker's direction follows its animation: "walk@se".
        let dir = |body: &super::Body, anim: Anim| match self.walk_turn(body, anim) {
            0 => String::new(),
            t => format!("@{}", crate::render::model::compass(body.facing, t)),
        };
        let anim = self.player_anim();
        let mut out = vec![format!("player:{}{}", anim.name(), dir(&self.player, anim))];
        for (i, e) in self.loaded.entities.iter().enumerate() {
            if e.active && e.kind == crate::entity::EntityKind::Character && e.body.pos.distance(self.player.pos) < 8.0 * ts && e.npc_seat.is_none() {
                let anim = self.npc_anim(i);
                out.push(format!("{}:{}{}", if e.name.is_empty() { &e.def } else { &e.name }, anim.name(), dir(&e.body, anim)));
            }
        }
        out
    }

    /// `held=` for `status`: what the player and the characters within 8 tiles hold, as
    /// `who:item@hand` (`R`, `L`, `B` = both; `/back` behind a figure).
    pub(super) fn held_status(&self) -> Vec<String> {
        let line = |who: &str, _a: &Appearance, gear: &Gear, pose: (Facing, Anim)| -> Vec<String> {
            let g = Some(crate::hands::Grip::new(0.0, 0.0, 0.0));
            let held = crate::hands::held_with(&self.project.db, (g, g), &gear.equipment(), &gear.carrying, None, pose.0);
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

    /// Draws a character's front idle frame scaled into `dest` (the creator, windows), with what
    /// it holds.
    pub(super) fn draw_character_in(&self, a: &Appearance, gear: &Gear, dest: Rect) {
        if let Some(cell) = self.model_cell(a, gear, (Facing::Down, Anim::Idle(0))) {
            self.assets.models.draw_in(&cell, dest, WHITE);
        }
    }

    /// What the player holds (fishing: the rod instead of the weapon; carrying a prop).
    pub(super) fn player_gear(&self) -> Gear {
        self.character_gear(&self.world.equipment, &self.world.player_carrying, self.player_tool_in_use().as_deref())
    }

    /// World size of a character's frame (32 x 48 without any model).
    pub(super) fn character_frame_size(&self, a: &Appearance) -> (f32, f32) {
        self.assets.models.frame_size(&self.look(a).model).unwrap_or((32.0, 48.0))
    }

    /// The player's model: the creator's choice, else the player definition's.
    pub(super) fn player_appearance(&self) -> Option<Appearance> {
        if let Some(a) = &self.world.player_appearance {
            return Some(a.clone());
        }
        self.project.db.character(&self.project.config.player).map(|d| d.appearance.clone())
    }
}
