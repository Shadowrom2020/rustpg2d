//! Rests (SRD): the short rest's window, where the hero spends hit dice (each heals d10 + CON,
//! rolled into the roll log) before an hour passes, and what a long rest does (a night's sleep,
//! the *Long rest* action): full HP, half the level's hit dice back, one level of exhaustion
//! less, the conditions with a duration over.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::skin;
use super::text::{draw_text, measure_text, role, Role};
use super::Game;

/// The short rest's window.
pub(super) struct ShortRest {
    /// 0 = *Spend a hit die*, 1 = *Done*.
    sel: usize,
    /// Hit dice spent in this rest and the HP they gave.
    spent: u32,
    healed: i32,
}

/// Hours awake between two long rests that both give their benefits (the SRD: one long rest in
/// 24 hours, eight of them asleep).
const AWAKE_BETWEEN: f64 = 16.0;

const W: f32 = 460.0;
const H: f32 = 236.0;

impl Game {
    /// Why the hero can't rest now (None = they can): enemies close or a fight going on.
    pub(super) fn cant_rest(&self) -> Option<&'static str> {
        let ts = self.ts();
        let danger = self.loaded.entities.iter().any(|e| e.active && e.hostile && e.max_hp > 0 && e.hp > 0 && e.body.pos.distance(self.player.pos) < 8.0 * ts);
        if danger || self.player_in_combat() {
            Some("You can't rest with enemies nearby.")
        } else {
            None
        }
    }

    /// Opens the short rest's window (*Short rest*, R on the sheet, `short_rest()`).
    pub(super) fn open_short_rest(&mut self) {
        if !self.combat.enabled() {
            return;
        }
        if let Some(why) = self.cant_rest() {
            self.ui.toast(why.into());
            return;
        }
        self.rest = Some(ShortRest { sel: if self.can_spend_hit_die() { 0 } else { 1 }, spent: 0, healed: 0 });
    }

    pub(super) fn rest_open(&self) -> bool {
        self.rest.is_some()
    }

    fn can_spend_hit_die(&self) -> bool {
        self.hit_dice_left() > 0 && self.hp() < self.player_max_hp()
    }

    /// Spends a hit die: d10 + CON (at least 0) HP, into the roll log.
    fn spend_hit_die(&mut self) {
        if !self.can_spend_hit_die() {
            return;
        }
        let left = self.hit_dice_left();
        self.world.hit_dice = Some(left - 1);
        let gain = self.world.roll_dice(&format!("1d{}+CON", self.hit_die())).map_or(0, |r| r.total.max(0) as i32);
        let max = self.player_max_hp();
        let hp = (self.hp() + gain).min(max);
        self.world.player_hp = Some(hp);
        if let Some(r) = &mut self.rest {
            r.spent += 1;
            r.healed += gain;
        }
    }

    /// Ends the short rest: an hour passes.
    fn finish_short_rest(&mut self) {
        self.rest = None;
        self.rest_resources(false);
        self.rest_slots(false);
        self.world.advance_hours(1.0);
        self.ui.toast("An hour passes.".into());
    }

    /// The window's input; true while it is open (it pauses the game).
    pub(super) fn update_rest(&mut self) -> bool {
        if self.rest.is_none() {
            return false;
        }
        let m = input::mouse_pos();
        let rows = self.rest_rows();
        let Some(r) = &mut self.rest else { return false };
        if input::act_pressed(Act::Up) || input::act_pressed(Act::Down) {
            r.sel = 1 - r.sel;
        }
        let hovered = rows.iter().position(|row| row.contains(m));
        if let Some(h) = hovered {
            if mouse_delta_position() != Vec2::ZERO {
                r.sel = h;
            }
        }
        let press = if input::act_pressed(Act::Confirm) { Some(r.sel) } else if input::mouse_pressed(MouseButton::Left) { hovered } else { None };
        if input::act_pressed(Act::Cancel) || press == Some(1) {
            self.finish_short_rest();
        } else if press == Some(0) {
            self.spend_hit_die();
        }
        true
    }

    fn rest_panel(&self) -> Rect {
        let (sw, sh) = (screen_width(), screen_height());
        Rect::new(((sw - W) / 2.0).round(), ((sh - H) / 2.0).round(), W, H)
    }

    /// The two buttons: *Spend a hit die*, *Done*.
    fn rest_rows(&self) -> [Rect; 2] {
        let p = self.rest_panel();
        [Rect::new(p.x + 24.0, p.y + 126.0, W - 48.0, 30.0), Rect::new(p.x + 24.0, p.y + 162.0, W - 48.0, 30.0)]
    }

    /// The window.
    pub(super) fn draw_rest(&self) {
        let Some(r) = &self.rest else { return };
        let _font = role(Role::Dialog);
        let p = self.rest_panel();
        skin::panel(p.x, p.y, p.w, p.h);
        draw_text("Short rest", p.x + 24.0, p.y + 36.0, 26.0, skin::title());
        let con = self.world.abilities.modifier(crate::srd::Ability::Con);
        let dice = format!("Hit dice {} / {}   (d{} {})", self.hit_dice_left(), self.world.player_level(), self.hit_die(), crate::srd::abilities::signed(con as i64));
        draw_text(&dice, p.x + 24.0, p.y + 70.0, 19.0, skin::normal());
        let hp = format!("Hit points {} / {}", self.hp(), self.player_max_hp());
        draw_text(&hp, p.x + 24.0, p.y + 96.0, 19.0, skin::normal());
        if r.spent > 0 {
            let t = format!("+{} HP", r.healed);
            let tw = measure_text(&t, None, 19, 1.0).width;
            draw_text(&t, p.right() - 24.0 - tw, p.y + 96.0, 19.0, skin::text(skin::Text::Positive, Color::new(0.6, 0.95, 0.6, 1.0)));
        }
        let can = self.can_spend_hit_die();
        for (k, (row, label)) in self.rest_rows().iter().zip(["Spend a hit die", "Done"]).enumerate() {
            if k == r.sel && !skin::cursor(*row, Color::new(0.85, 0.7, 0.3, 0.22)) {
                draw_rectangle_lines(row.x, row.y, row.w, row.h, 1.5, skin::highlight());
            }
            let c = if k == 0 && !can { skin::disabled() } else if k == r.sel { skin::highlight() } else { skin::normal() };
            draw_text(label, row.x + 12.0, row.y + 21.0, 20.0, c);
        }
        super::glyphs::draw_hint_in("{key:up}/{key:down} + {key:confirm} or click · {key:cancel}: done", p.x + 24.0, p.bottom() - 12.0, 16.0, W - 48.0, skin::disabled());
    }

    /// For `status()`: `closed`, or `short:LEFT/LEVEL` while the window is open.
    pub(super) fn rest_status(&self) -> String {
        let long = self.world.last_long_rest.map_or(String::new(), |t| format!(":long:{:.1}", self.world.total_hours() - t));
        match &self.rest {
            None => format!("closed{long}"),
            Some(_) => format!("short:{}/{}{long}", self.hit_dice_left(), self.world.player_level()),
        }
    }

    /// A long rest's effect (a night's sleep of `hours`; nothing when the last one ended less than
    /// 16 hours before this one began): HP and mana full, half the level's hit dice back
    /// (at least one), one level of exhaustion less, the conditions with a duration over.
    pub(super) fn long_rest_effects(&mut self, hours: f32) {
        // One long rest in 24 hours: this one starts at least 16 hours after the last one ended.
        let now = self.world.total_hours();
        if self.world.last_long_rest.is_some_and(|t| now - hours as f64 - t < AWAKE_BETWEEN) {
            self.ui.toast(crate::lang::tr_ui("You rested less than a day ago.").into_owned());
            return;
        }
        self.world.last_long_rest = Some(now);
        if self.combat.enabled() {
            let level = self.world.player_level();
            let back = (level / 2).max(1);
            self.world.hit_dice = Some((self.hit_dice_left() + back).min(level));
        }
        self.rest_resources(true);
        // (With food and water counting, only after eating and drinking that day.)
        if self.rest_survival() {
            crate::srd::conditions::lower_exhaustion(&mut self.world.effects, 1);
        }
        crate::srd::conditions::end_timed(&mut self.world.effects);
        if self.combat.enabled() {
            self.world.player_hp = Some(self.player_max_hp());
        }
        self.rest_slots(true);
        // (A night's rest lasts past dawn: the items' charges come back.)
        self.recharge_items();
    }
}
