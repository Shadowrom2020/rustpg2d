//! Races at runtime: the player's race (`WorldState::player_race`, merged with its parent race
//! for a subrace) adds its max HP, speed and defense; a new hero gets its SRD data (scores,
//! proficiencies, languages, resistances, the creator's choices and background:
//! [`crate::races::build_hero`]); characters whose body belongs to a race with a size are drawn
//! and collide at that size.

use std::collections::BTreeMap;
use std::rc::Rc;

use macroquad::prelude::*;

use super::Game;
use crate::races::RaceDef;

/// What the creator chose (a new game without the creator: the player character's race only).
#[derive(Clone, Debug, Default)]
pub(super) struct HeroChoices {
    pub race: String,
    /// Base scores before racial bonuses (`None` = the project's *Rules*).
    pub base: Option<crate::srd::Abilities>,
    pub choices: BTreeMap<String, Vec<String>>,
    pub background: String,
    /// The class ("" = none) and the class skills picked.
    pub class: String,
    pub class_skills: Vec<String>,
}

impl Game {
    /// The player's race, merged with its parent race (cached until the race changes).
    pub(super) fn player_race(&self) -> Option<Rc<RaceDef>> {
        let id = &self.world.player_race;
        if id.is_empty() {
            return None;
        }
        let mut cache = self.race_cache.borrow_mut();
        if cache.as_ref().is_none_or(|(k, _)| k != id) {
            *cache = Some((id.clone(), self.project.db.race_full(id).map(Rc::new)));
        }
        cache.as_ref().and_then(|(_, r)| r.clone())
    }

    /// The player's race at the start of a game (no creator): its SRD data, full HP, its size.
    pub(super) fn begin_as_race(&mut self, race: &str) {
        self.begin_as_hero(HeroChoices { race: race.to_string(), ..Default::default() });
    }

    /// A new hero: race, scores, proficiencies, languages, resistances, choices and background
    /// (with its starting equipment), then full HP, the race's size and its faction's greetings.
    pub(super) fn begin_as_hero(&mut self, c: HeroChoices) {
        self.world.player_race = c.race.clone();
        self.world.player_race_parent = self.project.db.race(&c.race).map(|r| r.parent.clone()).unwrap_or_default();
        let race = self.player_race();
        let background = self.project.db.background(&c.background).cloned();
        let hero = crate::races::build_hero(race.as_deref(), &self.project.config.rules, c.base, &c.choices, background.as_ref());
        *self.world.base_abilities_mut() = hero.abilities;
        self.world.proficiencies = hero.proficiencies;
        self.world.languages = hero.languages;
        self.world.resistances = hero.resistances;
        self.world.choices = c.choices;
        self.world.player_background = background.as_ref().map(|b| b.id.clone()).unwrap_or_default();
        for s in background.iter().flat_map(|b| b.equipment.iter()).filter(|s| !s.item.is_empty() && s.count > 0) {
            self.world.give_item(&s.item, s.count);
        }
        // The class: its proficiencies, the skills picked, its starting equipment; level 1's
        // choices wait for the level-up window.
        if !c.class.is_empty() {
            self.set_hero_class(&c.class);
            for s in &c.class_skills {
                self.world.set_proficient(s, true);
            }
            let gear: Vec<(String, u32)> = self.project.db.class(&c.class).map(|cl| cl.equipment.iter().filter(|s| !s.item.is_empty() && s.count > 0).map(|s| (s.item.clone(), s.count)).collect()).unwrap_or_default();
            for (item, n) in gear {
                self.world.give_item(&item, n);
            }
        }
        self.init_skills(false);
        self.world.player_hp = Some(self.player_max_hp());
        self.refresh_player_size();
        // The race's faction decides who greets and who attacks.
        self.refresh_hostility();
    }

    /// The hero's darkvision in feet (0 = none).
    pub(super) fn darkvision_ft(&self) -> u32 {
        // (Or from an item: Goggles of Night; or a spell: Darkvision.)
        let items = self.hero_traits().map_or(0, |t| t.darkvision_ft);
        let spell = crate::srd::conditions::buffs(&self.world.effects)
            .filter_map(|b| if let crate::srd::conditions::Buff::Sense { sense: crate::srd::conditions::Sense::Darkvision, feet } = b { Some(*feet) } else { None })
            .max()
            .unwrap_or(0);
        self.player_race().map_or(0, |r| r.darkvision_ft).max(items).max(spell)
    }

    /// The player's hitbox at their body's size (the player character's hitbox × the size of the
    /// race their body belongs to).
    pub(super) fn refresh_player_size(&mut self) {
        let base = self.project.db.character(&self.project.config.player).map_or(vec2(self.ts() * 0.75, self.ts() * 0.5), |c| vec2(c.hitbox[0], c.hitbox[1]));
        let size = self.player_appearance().map_or(1.0, |a| self.look_size(&a));
        self.player.hitbox = base * size;
    }

    /// `race=` for `status`: the player's race and its effect ("-" without one).
    pub(super) fn race_status(&self) -> String {
        match self.player_race() {
            Some(r) => format!("{}(hp{:+},speed{:+}%,def{:+},size{})", r.id, r.max_hp, r.speed, r.defense, self.player_appearance().map_or(1.0, |a| self.look_size(&a))),
            None => "-".into(),
        }
    }

    /// `sheet=` for `status`: race|scores|bg:background|lang:…|res:…|dv:feet|speed:feet|choices|
    /// level:N|ac:N|hp:now/max|hd:left/level|cond:poisoned:4.0+exhaustion2.
    pub(super) fn sheet_status(&self) -> String {
        let w = &self.world;
        let join = |s: &std::collections::BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(",");
        let race = self.player_race();
        let choices: Vec<String> = w.choices.iter().map(|(k, v)| format!("{k}={}", v.join("+"))).collect();
        format!(
            "{}|{}|bg:{}|lang:{}|res:{}|dv:{}|speed:{}|choices:{}|level:{}|ac:{}|hp:{}/{}|hd:{}/{}|cond:{}",
            if w.player_race.is_empty() { "-" } else { &w.player_race },
            w.abilities.text(),
            if w.player_background.is_empty() { "-" } else { &w.player_background },
            join(&w.languages),
            join(&w.resistances),
            race.as_ref().map_or(0, |r| r.darkvision_ft),
            self.hero_speed_ft(),
            choices.join(";"),
            w.player_level(),
            self.armor_class(),
            self.hp(),
            self.player_max_hp(),
            self.hit_dice_left(),
            w.player_level(),
            crate::srd::conditions::describe(&w.effects).join("+")
        )
    }
}
