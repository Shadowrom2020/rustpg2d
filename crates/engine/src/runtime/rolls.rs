//! The SRD rolls on screen: a short line for every new ability check and saving throw ("Persuasion:
//! 16 vs DC 12 - success"), the journal's *Rolls* page and the `rolls=` status.

use macroquad::prelude::*;

use super::text::{draw_text, measure_text};
use super::{skin, Game};
use crate::srd::abilities::{self, Ability};
use crate::srd::RollRecord;

const PAD: f32 = 16.0;
const ROW: f32 = 26.0;

/// A roll's name in the current language: skills and abilities translated, a saving throw as
/// "WIS save", a tool check as "DEX (Thieves' Tools)"; a plain roll as it was written.
pub(super) fn roll_label(r: &RollRecord) -> String {
    let id = r.id.as_str();
    if r.success.is_none() {
        return r.label.clone();
    }
    if let Some(a) = id.strip_prefix("save:").and_then(Ability::parse) {
        return crate::lang::tr_ui(&format!("{} saving throw", a.short())).into_owned();
    }
    if let Some((a, tool)) = id.split_once(':') {
        let a = Ability::parse(a).map_or(a.to_uppercase(), |a| a.short().to_string());
        return format!("{a} ({})", crate::lang::tr_ui(&abilities::proficiency_name(tool)));
    }
    if let Some((_, name, _)) = abilities::skill(id) {
        return crate::lang::tr_ui(name).into_owned();
    }
    if let Some(a) = Ability::parse(id) {
        return crate::lang::tr_ui(a.name()).into_owned();
    }
    r.label.clone()
}

/// The line shown when a check is rolled.
pub(super) fn roll_toast(r: &RollRecord) -> String {
    let label = roll_label(r);
    let ok = r.success == Some(true);
    let text = match (ok, r.natural) {
        (true, 20) => format!("{label}: {} vs DC {} - success (natural 20)", r.total, r.dc),
        (false, 1) => format!("{label}: {} vs DC {} - failure (natural 1)", r.total, r.dc),
        (true, _) => format!("{label}: {} vs DC {} - success", r.total, r.dc),
        (false, _) => format!("{label}: {} vs DC {} - failure", r.total, r.dc),
    };
    crate::lang::tr_ui(&text).into_owned()
}

impl Game {
    /// Once per frame: a line on screen for each check rolled since the last frame (by the
    /// game, the action runner or a script), when the project shows them.
    pub(super) fn show_new_rolls(&mut self) {
        let count = self.world.roll_count;
        if count == self.rolls_shown {
            return;
        }
        let new = (count.saturating_sub(self.rolls_shown) as usize).min(self.world.rolls.len());
        self.rolls_shown = count;
        // (In a fight the fight's log shows every roll.)
        if !self.project.config.rules.show_checks || self.fight.is_some() {
            return;
        }
        let lines: Vec<String> = self.world.rolls[self.world.rolls.len() - new..].iter().filter(|r| r.success.is_some()).map(roll_toast).collect();
        for l in lines {
            self.ui.toast(l);
        }
    }

    /// For `status()`: the last six rolls, oldest first (`persuasion:16/12:ok`, `2d6:7`).
    pub(super) fn rolls_status(&self) -> String {
        let r = &self.world.rolls;
        r[r.len().saturating_sub(6)..].iter().map(|r| r.status()).collect::<Vec<_>>().join(",")
    }

    /// The journal's *Rolls* page: the roll log, newest at the bottom, scrolled with the
    /// journal's log scroll.
    pub(super) fn draw_roll_log(&self, r: Rect, scroll: usize) -> usize {
        let log = &self.world.rolls;
        if log.is_empty() {
            draw_text("No rolls yet. Ability checks, saving throws and dice rolls are kept here.", r.x + PAD, r.y + 80.0, 22.0, skin::dim());
            return 0;
        }
        let (top, bottom) = (r.y + 64.0, r.y + r.h - 44.0);
        let rows = (((bottom - top) / ROW).floor() as usize).max(1);
        let max_scroll = log.len().saturating_sub(rows);
        let scroll = scroll.min(max_scroll);
        let end = log.len() - scroll;
        let start = end.saturating_sub(rows);
        let x = r.x + PAD + 8.0;
        let cols = [x, x + 260.0, x + 420.0, x + 480.0, x + 560.0];
        for (k, h) in ["Roll", "Dice", "Total", "DC", "Result"].iter().enumerate() {
            draw_text(h, cols[k], r.y + 58.0, 16.0, skin::disabled());
        }
        for (row, rec) in log[start..end].iter().enumerate() {
            let y = top + 18.0 + row as f32 * ROW;
            let label = roll_label(rec);
            let mut l = label.clone();
            while l.chars().count() > 3 && measure_text(&l, None, 19, 1.0).width > 250.0 {
                l.pop();
            }
            draw_text(&l, cols[0], y, 19.0, skin::normal());
            let dice = if rec.modifier != 0 { format!("{} {}", rec.dice, abilities::signed(rec.modifier)) } else { rec.dice.clone() };
            draw_text(&dice, cols[1], y, 18.0, skin::dim());
            draw_text(format!("{}", rec.total), cols[2], y, 19.0, skin::highlight());
            if let Some(ok) = rec.success {
                draw_text(format!("{}", rec.dc), cols[3], y, 18.0, skin::dim());
                let (word, col) = match (ok, rec.natural) {
                    (true, 20) => ("natural 20", skin::text(skin::Text::Positive, Color::new(0.7, 0.95, 0.7, 1.0))),
                    (false, 1) => ("natural 1", skin::text(skin::Text::Negative, Color::new(1.0, 0.55, 0.5, 1.0))),
                    (true, _) => ("success", skin::text(skin::Text::Positive, Color::new(0.7, 0.95, 0.7, 1.0))),
                    (false, _) => ("failure", skin::text(skin::Text::Negative, Color::new(1.0, 0.55, 0.5, 1.0))),
                };
                draw_text(word, cols[4], y, 18.0, col);
            }
        }
        max_scroll
    }
}
