//! Furniture the player uses (`PropDef::use_spot`): sitting on chairs and benches (a short
//! rest: HP and mana come back faster), standing at counters, and sleeping in beds (the night
//! passes: the clock jumps, HP and mana are restored, the world moves on). Moving, attacking or
//! E stands the player up again. The Sleep action / `sleep(hours)` sleeps without a bed.

use macroquad::prelude::*;

use super::{Game, Mover, Source};
use crate::defs::{Pose, SpriteRef};
use crate::entity::Facing;
use crate::events::runner::{conditions_met, ActionCtx};
use crate::events::{Action, CmpOp, Cond, Condition, GameEvent};
use super::text::{draw_text, measure_text};

/// Seconds of the sleep sequence: fading out, dark, fading in.
const FADE_OUT: f32 = 0.8;
const DARK: f32 = 1.4;
const FADE_IN: f32 = 0.8;
/// Seconds per HP regained while sitting.
const SIT_REGEN: f32 = 4.0;
/// Hostile characters this close (tiles) keep the player awake.
const UNSAFE: f32 = 8.0;
/// The variable the bed's "Sleep until …?" choice answers into.
const SLEEP_ANSWER: &str = "_sleep_answer";

/// The player using a piece of furniture.
#[derive(Clone, Debug)]
pub(super) struct Seat {
    /// The prop (entity uid).
    pub uid: u32,
    pub pose: Pose,
    /// The player's feet while using it, and the way they look.
    pub pos: Vec2,
    pub facing: Facing,
    /// Where the player stands up (where they stood before).
    pub back: Vec2,
    /// Drawn over the player (a bed's blanket), anchored at the prop's feet.
    cover: SpriteRef,
    prop_feet: Vec2,
    regen: f32,
}

/// A character using a piece of furniture (a schedule stop with a prop).
#[derive(Clone, Debug)]
pub(super) struct NpcSeat {
    pub uid: u32,
    pub pose: Pose,
    pub pos: Vec2,
    pub facing: Facing,
    pub cover: SpriteRef,
    pub prop_feet: Vec2,
    /// Where it stood before (it gets up there).
    pub stand: Vec2,
}

impl NpcSeat {
    /// Lying (in a bed).
    pub fn lying(&self) -> bool {
        self.pose == crate::defs::Pose::Lie
    }
}

/// The sleep sequence in progress.
#[derive(Clone, Debug)]
pub(super) struct Sleeping {
    t: f32,
    hours: f32,
    /// The bed slept in (its Woke up triggers run too).
    bed: Option<u32>,
    applied: bool,
}

impl Game {
    /// The player's pose: "sit", "lie", "stand" (at furniture) or "" (on their feet).
    pub(super) fn pose_name(&self) -> &'static str {
        match self.seat.as_ref().map(|s| s.pose) {
            Some(Pose::Sit) => "sit",
            Some(Pose::Lie) => "lie",
            Some(Pose::Stand) => "stand",
            None => "",
        }
    }

    /// For `status()`: "sit#uid" (the furniture), "lie#uid", "stand#uid" or "-".
    pub(super) fn pose_status(&self) -> String {
        match &self.seat {
            Some(s) => format!("{}#{}", self.pose_name(), s.uid),
            None => "-".into(),
        }
    }

    /// Interacting with prop `i` after its own triggers: uses it if it is furniture and nothing
    /// else started (a dialog, an event, a container).
    pub(super) fn try_use_furniture(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        let Some(spot) = self.project.db.prop(&e.def).and_then(|d| d.use_spot.clone()) else { return };
        if self.runner.busy() || self.ui.dialog_open() || self.inventory_open() || self.seat.is_some() {
            return;
        }
        let (uid, feet) = (e.uid, e.body.pos);
        if self.furniture_taken(uid) {
            self.ui.toast("Someone is using it.".into());
            return;
        }
        if !spot.conditions.is_empty() && !conditions_met(self, &spot.conditions, &ActionCtx { owner: Some(uid) }) {
            let msg = if spot.refuse_text.is_empty() { "You can't use that now.".to_string() } else { self.world.expand_text(&spot.refuse_text) };
            self.ui.toast(msg);
            return;
        }
        if spot.pose == Pose::Lie {
            if let Some(why) = self.cant_sleep() {
                self.ui.toast(why.into());
                return;
            }
            // Asks first; the answer's Sleep action lies the player down in this bed.
            let hours = (spot.sleep_until - self.world.hour).rem_euclid(24.0);
            let hours = if hours < 0.5 { hours + 24.0 } else { hours };
            let until = crate::entity::hour_text(spot.sleep_until);
            self.bed = Some(uid);
            let ask = Action::Choice { speaker: String::new(), text: format!("Sleep until {until}?"), options: vec!["Sleep".into(), "Not now".into()], var: SLEEP_ANSWER.into() };
            let then = Action::If {
                conditions: vec![Condition::new(Cond::Var { name: SLEEP_ANSWER.into(), op: CmpOp::Eq, value: 0 })],
                then: vec![Action::Sleep { hours }],
                otherwise: vec![],
            };
            self.runner.push(&[ask, then], &ActionCtx { owner: Some(uid) });
            return;
        }
        self.take_seat(uid, &spot, feet);
    }

    /// Puts the player on furniture `uid` (feet at `prop_feet`).
    fn take_seat(&mut self, uid: u32, spot: &crate::defs::UseSpot, prop_feet: Vec2) {
        let k = self.ts() / 32.0;
        let pos = prop_feet + vec2(spot.offset[0], spot.offset[1]) * k;
        self.seat = Some(Seat { uid, pose: spot.pose, pos, facing: spot.facing, back: self.player.pos, cover: spot.cover.clone(), prop_feet, regen: 0.0 });
        self.order = None;
        self.player.move_target = None;
        self.player.pos = pos;
        self.player.facing = spot.facing;
        self.player.moving = false;
    }

    /// Back on one's feet where the player stood before.
    pub(super) fn stand_up(&mut self) {
        if let Some(s) = self.seat.take() {
            self.player.pos = s.back;
            self.player.moving = false;
        }
    }

    /// While using furniture: stays put; a movement key, attack or E stands up. Sitting
    /// regains HP (and mana twice as fast). Returns true while the player is seated.
    pub(super) fn update_seat(&mut self, dt: f32, key_consumed: bool) -> bool {
        let Some(seat) = &mut self.seat else { return false };
        use super::input::{self, Act};
        // A movement key held (not just pressed: held keys from before don't count, the player
        // got here with E), an attack or E again.
        let moving = [Act::Left, Act::Right, Act::Up, Act::Down].into_iter().any(input::act_down);
        let leave = !key_consumed && (moving || input::any_act_pressed(&[Act::Attack, Act::Confirm]));
        if leave || self.order.is_some() {
            self.order = None;
            self.stand_up();
            return true;
        }
        self.player.pos = seat.pos;
        self.player.facing = seat.facing;
        self.player.moving = false;
        if seat.pose == Pose::Sit {
            seat.regen += dt;
            let tick = seat.regen >= SIT_REGEN;
            if tick {
                seat.regen = 0.0;
            }
            if tick && self.combat.enabled() && self.hp() < self.player_max_hp() {
                self.heal(1);
            }
            if self.magic_on() {
                // Mana regenerates once more on top of the usual rate.
                let max = self.max_mana() as f32;
                self.world.mana = (self.world.mana + self.project.config.skills.mana_regen * dt).min(max);
            }
        }
        true
    }

    /// Why the player can't sleep right now (None = they can).
    fn cant_sleep(&self) -> Option<&'static str> {
        let ts = self.ts();
        let danger = self.loaded.entities.iter().any(|e| e.active && e.hostile && e.max_hp > 0 && e.body.pos.distance(self.player.pos) < UNSAFE * ts);
        if danger {
            Some("You can't sleep with enemies nearby.")
        } else if self.cutscene_active() {
            Some("Not now.")
        } else {
            None
        }
    }

    /// The Sleep action: starts the sleep sequence (in the bed the player chose, if any).
    /// Returns false if the player can't sleep now.
    pub(super) fn start_sleep(&mut self, hours: f32) -> bool {
        let bed = self.bed.take();
        if let Some(why) = self.cant_sleep() {
            self.ui.toast(why.into());
            return false;
        }
        if let Some(uid) = bed {
            let found = self.loaded.entities.iter().find(|e| e.uid == uid).and_then(|e| Some((self.project.db.prop(&e.def)?.use_spot.clone()?, e.body.pos)));
            if let Some((spot, feet)) = found {
                self.take_seat(uid, &spot, feet);
            }
        }
        self.sleeping = Some(Sleeping { t: 0.0, hours: hours.clamp(0.0, 48.0), bed, applied: false });
        true
    }

    pub(super) fn sleeping(&self) -> bool {
        self.sleeping.is_some()
    }

    /// Runs the sleep sequence. Returns true while it lasts (the world is paused).
    pub(super) fn update_sleep(&mut self, dt: f32) -> bool {
        let Some(s) = &mut self.sleeping else { return false };
        s.t += dt;
        let (t, hours, bed) = (s.t, s.hours, s.bed);
        if t >= FADE_OUT && !s.applied {
            s.applied = true;
            self.pass_time(hours);
        }
        if t >= FADE_OUT + DARK + FADE_IN {
            self.sleeping = None;
            self.stand_up();
            self.ui.toast("You feel rested.".into());
            self.fire(GameEvent::Sleep, Source::Map);
            if let Some(uid) = bed {
                self.fire(GameEvent::Sleep, Source::Entity(uid));
            }
            self.autosave_pending = true;
        }
        true
    }

    /// What a night's sleep does: the clock moves on, HP and mana are restored, timed effects
    /// run out, enemies due to respawn are back, scheduled characters are where the hour wants.
    fn pass_time(&mut self, hours: f32) {
        self.world.advance_hours(hours);
        self.water_crops(hours);
        if self.combat.enabled() {
            self.world.player_hp = Some(self.player_max_hp());
        }
        if self.magic_on() {
            self.world.mana = self.max_mana() as f32;
        }
        self.world.effects.clear();
        self.spawners_catch_up();
        for i in 0..self.loaded.entities.len() {
            let e = &mut self.loaded.entities[i];
            let Some(sch) = &mut e.schedule else { continue };
            sch.current = None;
            if e.away {
                e.away = false;
                e.active = !e.hidden;
            }
            self.place_on_schedule(i);
        }
    }

    /// The darkness of the sleep sequence (0..1), drawn over everything but the menus.
    pub(super) fn draw_sleep(&self) {
        let Some(s) = &self.sleeping else { return };
        let a = if s.t < FADE_OUT {
            s.t / FADE_OUT
        } else if s.t < FADE_OUT + DARK {
            1.0
        } else {
            1.0 - (s.t - FADE_OUT - DARK) / FADE_IN
        };
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.02, a.clamp(0.0, 1.0)));
        if s.t >= FADE_OUT * 0.5 && s.t < FADE_OUT + DARK + FADE_IN * 0.5 {
            let z = "Z z z";
            let w = measure_text(z, None, 40, 1.0).width;
            draw_text(z, (sw - w) / 2.0, sh / 2.0, 40.0, Color::new(0.7, 0.75, 1.0, a.clamp(0.0, 1.0)));
        }
    }

    /// Where the player sorts among the map's objects: on furniture, right after it (what is
    /// drawn over the user, a blanket or a chair's back, comes with the player).
    pub(super) fn player_sort_y(&self) -> f32 {
        match &self.seat {
            Some(s) => s.prop_feet.y + 0.5,
            None => self.player.pos.y,
        }
    }

    /// Draws the player using furniture (instead of the normal drawing). Returns false when the
    /// player isn't seated.
    pub(super) fn draw_seated_player(&self, tint: Color) -> bool {
        let Some(seat) = &self.seat else { return false };
        let Some(a) = self.player_appearance() else { return false };
        self.draw_on_furniture(&a, (seat.pose, seat.pos, seat.facing), (&seat.cover, seat.prop_feet), tint);
        true
    }

    /// Draws a character using furniture: `pose` with the feet at `pos` facing `facing`, then
    /// what the furniture draws over its user (`cover` at the prop's feet).
    pub(super) fn draw_on_furniture(&self, a: &crate::defs::Appearance, (pose, pos, facing): (Pose, Vec2, Facing), (cover, prop_feet): (&SpriteRef, Vec2), tint: Color) {
        struct S {
            pos: Vec2,
            facing: Facing,
        }
        let seat = S { pos, facing };
        let a = a.clone();
        let (fw, fh) = self.character_frame_size(&a);
        let k = self.ts() / 32.0;
        match pose {
            Pose::Sit => {
                // Lower, with the legs out of sight below the seat.
                let part = Rect::new(0.0, 0.0, 1.0, 0.8);
                let drop = 6.0 * k;
                let dest = Rect::new(seat.pos.x - fw / 2.0, seat.pos.y - fh + drop, fw, fh * part.h);
                let l = self.character_figure(&a, seat.facing, 0);
                self.assets.draw_frame_part_in(&l.sheet, self.layer_frame(&l), part, dest, &l.colors);
            }
            Pose::Lie => {
                // `pos` is where the feet are; the head lies towards `facing`.
                let (angle, head) = match seat.facing {
                    Facing::Up => (0.0, vec2(0.0, -1.0)),
                    Facing::Left => (-std::f32::consts::FRAC_PI_2, vec2(-1.0, 0.0)),
                    Facing::Right => (std::f32::consts::FRAC_PI_2, vec2(1.0, 0.0)),
                    Facing::Down => (std::f32::consts::PI, vec2(0.0, 1.0)),
                };
                let center = seat.pos + head * fh / 2.0;
                let l = self.character_figure(&a, Facing::Down, 0);
                self.assets.draw_frame_colored_rotated(&l.sheet, self.layer_frame(&l), center, angle, &l.colors, WHITE);
            }
            Pose::Stand => {
                self.draw_character(&a, (seat.facing, 0.into()), seat.pos, tint);
            }
        }
        if !cover.sheet.is_empty() {
            self.assets.draw_sprite(cover, 0, prop_feet, WHITE);
        }
    }

    /// Whether a character (or the player) is using prop `uid`.
    pub(super) fn furniture_taken(&self, uid: u32) -> bool {
        self.seat.as_ref().is_some_and(|s| s.uid == uid) || self.loaded.entities.iter().any(|e| e.active && e.npc_seat.as_ref().is_some_and(|s| s.uid == uid))
    }

    /// Character `i` uses the prop named `prop` on this map (its schedule stop): snaps to the
    /// prop's spot and takes its pose. False if there is no such free furniture.
    pub(super) fn npc_use_furniture(&mut self, i: usize, prop: &str) -> bool {
        let Some(p) = self.loaded.entities.iter().position(|e| e.active && e.kind == crate::entity::EntityKind::Prop && e.name == prop) else {
            eprintln!("[game] schedule: no prop named '{prop}' on {}", self.loaded.map.name);
            return false;
        };
        let (puid, feet) = (self.loaded.entities[p].uid, self.loaded.entities[p].body.pos);
        let Some(spot) = self.project.db.prop(&self.loaded.entities[p].def).and_then(|d| d.use_spot.clone()) else {
            eprintln!("[game] schedule: '{prop}' is no furniture (no use spot)");
            return false;
        };
        if self.furniture_taken(puid) {
            return false;
        }
        let pos = feet + Vec2::from(spot.offset);
        let e = &mut self.loaded.entities[i];
        e.npc_seat = Some(NpcSeat { uid: puid, pose: spot.pose, pos, facing: spot.facing, cover: spot.cover.clone(), prop_feet: feet, stand: e.body.pos });
        e.body.pos = pos;
        e.body.facing = spot.facing;
        e.body.moving = false;
        self.occupy(i);
        true
    }

    /// Character `i` gets up from its furniture (where it stood before).
    pub(super) fn npc_stand_up(&mut self, i: usize) {
        if let Some(s) = self.loaded.entities[i].npc_seat.take() {
            // (Clear of the furniture, or it couldn't move.)
            let (body, uid) = (self.loaded.entities[i].body.clone(), self.loaded.entities[i].uid);
            let p = self.free_spot(&body, s.stand, Mover::Npc(uid));
            self.loaded.entities[i].body.pos = p;
            self.occupy(i);
        }
    }

    /// "sit" / "lie" / "stand" of character `i` on furniture, "" on its feet.
    pub(super) fn npc_pose(&self, i: usize) -> &'static str {
        match self.loaded.entities[i].npc_seat.as_ref().map(|s| s.pose) {
            Some(Pose::Sit) => "sit",
            Some(Pose::Lie) => "lie",
            Some(Pose::Stand) => "stand",
            None => "",
        }
    }
}
