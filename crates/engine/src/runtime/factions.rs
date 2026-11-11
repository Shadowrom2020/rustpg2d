//! Factions at runtime: the player's faction and standings, which characters are hostile to the
//! player (`RtEntity::hostile`: the definition's *Hostile*, or a faction the player's standing is
//! hostile with) and which characters are each other's foes (hostile factions).

use super::Game;

impl Game {
    /// The player's faction: their race's (a race without one: none), else the player
    /// character's ("" = none).
    pub(super) fn player_faction(&self) -> String {
        match self.project.db.race(&self.world.player_race) {
            Some(r) => r.faction.clone(),
            None => self.project.db.faction_of(&self.project.config.player),
        }
    }

    /// The player's standing with a faction.
    pub(super) fn standing(&self, faction: &str) -> i32 {
        self.world.standing.get(faction).copied().unwrap_or_else(|| self.project.config.factions.base_standing(&self.player_faction(), faction))
    }

    /// Characters of this definition attack the player: their faction's standing is at
    /// [`HOSTILE_AT`](crate::factions::HOSTILE_AT) or below, or they are *Hostile* and the player
    /// isn't a friend of their faction (no faction: *Hostile* alone decides).
    pub(super) fn hostile_to_player(&self, def: &str) -> bool {
        let marked = self.project.db.character(def).is_some_and(|d| d.hostile);
        let f = self.project.db.faction_of(def);
        let known = !f.is_empty() && self.project.config.factions.get(&f).is_some();
        crate::factions::attacks_player(marked, known.then(|| self.standing(&f)))
    }

    /// Recomputes who is hostile to the player (after a standing or the player's race changed).
    pub(super) fn refresh_hostility(&mut self) {
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if e.kind != crate::entity::EntityKind::Character || e.companion.is_some() {
                continue;
            }
            let hostile = self.hostile_to_player(&e.def.clone());
            let e = &mut self.loaded.entities[i];
            if e.hostile && !hostile {
                // Calmed down: it stops hunting the player.
                if matches!(e.ai.target, Some(super::ai::Foe::Player) | Some(super::ai::Foe::Companion(_))) {
                    e.ai = Default::default();
                }
            }
            e.hostile = hostile;
        }
    }

    /// Changes the player's standing with a faction (clamped to -100..100).
    pub(super) fn change_standing(&mut self, faction: &str, by: i32) {
        if faction.is_empty() {
            return;
        }
        let now = (self.standing(faction) + by).clamp(-100, 100);
        self.world.standing.insert(faction.to_string(), now);
        self.refresh_hostility();
    }

    /// Characters `i` and `j` belong to factions hostile to each other.
    pub(super) fn foes(&self, i: usize, j: usize) -> bool {
        self.factions.hostile(self.loaded.entities[i].faction, self.loaded.entities[j].faction)
    }

    /// `factions=` for `status`: the player's faction and the standings with every faction.
    pub(super) fn factions_status(&self) -> String {
        if self.project.config.factions.is_empty() {
            return "-".into();
        }
        let list: Vec<String> = self.project.config.factions.list.iter().map(|f| format!("{}:{}", f.id, self.standing(&f.id))).collect();
        format!("{}[{}]", self.player_faction(), list.join(" "))
    }
}
