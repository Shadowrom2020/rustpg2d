//! The survival rules in play (`srd/survival.rs`): the hero's load (always: the plain rule or
//! the variant encumbrance), food and water at the end of each day and the forced march (each
//! an option of *Database › Project › Rules*), the fast pace's −5 to passive Perception.

use super::Game;
use crate::srd::survival::{capacity, Encumbrance, Load};
use crate::srd::{Ability, Condition, ConditionSpec, Mode};

impl Game {
    /// Pounds the hero carries: the bag's stacks and what they wear and hold (the purse holds a
    /// value, not coins: it weighs nothing).
    pub(super) fn carried_weight(&self) -> f32 {
        let db = &self.project.db;
        let weigh = |s: &crate::inventory::ItemStack| db.item(&s.item).map_or(0.0, |d| d.weight * s.count as f32);
        self.world.inventory.slots.iter().flatten().map(weigh).sum::<f32>() + self.world.equipment.stacks().map(weigh).sum::<f32>()
    }

    fn encumbrance_rule(&self) -> Encumbrance {
        self.project.config.rules.encumbrance
    }

    /// The hero's load now (worked out once a frame by [`Game::refresh_load`]).
    pub(super) fn hero_load(&self) -> Load {
        self.moves.load
    }

    /// Once a frame: the load and the flags checks read (a heavy load's disadvantage).
    pub(super) fn refresh_load(&mut self) {
        let weight = self.carried_weight();
        let load = Load::of(weight, self.world.abilities.score(Ability::Str), self.encumbrance_rule());
        self.moves.carried = weight;
        self.moves.load = load;
        self.world.training.heavy_load = load.disadvantage(self.encumbrance_rule());
    }

    /// What the load leaves of walking speed (1 = unhindered, 0 = can't move).
    pub(super) fn load_factor(&self) -> f32 {
        match self.moves.load {
            Load::Light => 1.0,
            l => l.speed(30) as f32 / 30.0,
        }
    }

    /// Whether the hero travels at the fast pace (running): −5 to their passive Perception.
    pub(super) fn hero_fast_pace(&self) -> bool {
        !self.sneaking() && self.player.moving && super::input::act_down(super::input::Act::Run)
    }

    /// The hero's passive Perception: 10 + Perception, −5 at the fast pace.
    pub(super) fn hero_passive_perception(&self) -> i32 {
        10 + self.world.check_bonus("perception", false).unwrap_or(0) as i32 - if self.hero_fast_pace() { 5 } else { 0 }
    }

    /// Eats `food` and drinks `water` days' worth (*Eat / drink*, `eat()`, `drink()`).
    pub(super) fn nourish(&mut self, food: f32, water: f32) {
        self.world.survival.food += food.max(0.0);
        self.world.survival.water += water.max(0.0);
    }

    /// The hero walked for `dt` seconds out of fights: the forced march's hours.
    pub(super) fn march_step(&mut self, dt: f32) {
        let day = self.project.config.day_minutes;
        if !self.project.config.rules.forced_march || day <= 0.0 || self.fight.is_some() || self.hero_jumping() {
            return;
        }
        self.world.survival.march += dt * 24.0 / (day * 60.0);
        for dc in self.world.survival.march_saves_due() {
            let ok = self.world.check("con", dc, Mode::Normal, true).is_some_and(|r| r.success == Some(true));
            if !ok {
                self.ui.toast(crate::lang::tr_ui("The march wears you out.").into_owned());
                self.add_exhaustion(1);
            }
        }
    }

    /// A long rest: the march is over; exhaustion goes down only after some food and water that
    /// day (when they count). Returns whether the rest lowers exhaustion.
    pub(super) fn rest_survival(&mut self) -> bool {
        let fed = self.world.survival.fed();
        self.world.survival.rested();
        !self.project.config.rules.food_water || fed
    }

    /// Once a frame: the end of each day that passed (food and water, when they count).
    pub(super) fn update_survival(&mut self) {
        let today = self.world.day;
        if !self.project.config.rules.food_water || self.on_title() {
            self.world.survival.day = Some(today);
            return;
        }
        let Some(mut day) = self.world.survival.day else {
            self.world.survival.day = Some(today);
            return;
        };
        while day < today && self.game_over.is_none() {
            day += 1;
            let con = self.world.abilities.modifier(Ability::Con);
            let exhausted = crate::srd::conditions::exhaustion(&self.world.effects) > 0;
            let end = self.world.survival.end_day(con, exhausted);
            let mut levels = end.exhaustion;
            if end.water_save {
                let mode = if exhausted { Mode::Disadvantage } else { Mode::Normal };
                if !self.world.check("con", 15, mode, true).is_some_and(|r| r.success == Some(true)) {
                    levels += 1;
                }
            }
            if end.hungry {
                self.ui.toast(crate::lang::tr_ui("You go hungry.").into_owned());
            }
            if end.thirsty {
                self.ui.toast(crate::lang::tr_ui("You are parched.").into_owned());
            }
            if levels > 0 {
                self.add_exhaustion(levels);
            }
            self.world.survival.day = Some(day);
        }
        self.world.survival.day = Some(today.max(day));
    }

    /// Gives the hero `levels` of exhaustion.
    fn add_exhaustion(&mut self, levels: u8) {
        let mut spec = ConditionSpec::new(Condition::Exhaustion, 0.0);
        spec.level = levels;
        self.apply_player_effect(&spec);
    }

    /// `survival=` for status: `food:1.0 water:0.5 unfed:0 march:3.2h` (the parts whose rule is
    /// on), `-` with both off.
    pub(super) fn survival_status(&self) -> String {
        let r = &self.project.config.rules;
        let s = &self.world.survival;
        let mut parts = Vec::new();
        if r.food_water {
            parts.push(format!("food:{:.2} water:{:.2} unfed:{}", s.food, s.water, s.unfed));
        }
        if r.forced_march {
            parts.push(format!("march:{:.1}h", s.march));
        }
        if parts.is_empty() {
            "-".into()
        } else {
            parts.join(" ")
        }
    }

    /// `load=` for status: pounds carried / capacity and the load's word (`42/240`,
    /// `300/240:dragging`).
    pub(super) fn load_status(&self) -> String {
        let cap = capacity(self.world.abilities.score(Ability::Str));
        let word = self.moves.load.word();
        let base = format!("{}/{}", self.moves.carried.round() as i64, cap as i64);
        if word.is_empty() {
            base
        } else {
            format!("{base}:{word}")
        }
    }
}
