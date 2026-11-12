//! Races at runtime: the player's race (`WorldState::player_race`) adds its max HP, speed and
//! defense, gives its starting skill levels at the start of a game, and characters whose body
//! belongs to a race with a size are drawn and collide at that size.

use macroquad::prelude::*;

use super::Game;
use crate::races::RaceDef;

impl Game {
    /// The player's race definition.
    pub(super) fn player_race(&self) -> Option<&RaceDef> {
        self.project.db.race(&self.world.player_race)
    }

    /// The player's race at the start of a game: its starting skills (they are the player's
    /// skills then), full HP, its size.
    pub(super) fn begin_as_race(&mut self, race: &str) {
        self.world.player_race = race.to_string();
        let skills = self.project.db.race(race).map(|r| r.skills.clone()).unwrap_or_default();
        self.world.skills = skills.into_iter().filter(|s| s.level > 0).map(|s| (s.skill, s.level)).collect();
        // Script skills (e.g. Scavenging) apply their effect.
        self.init_skills(false);
        self.world.player_hp = Some(self.player_max_hp());
        self.refresh_player_size();
        // The race's faction decides who greets and who attacks.
        self.refresh_hostility();
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
}
