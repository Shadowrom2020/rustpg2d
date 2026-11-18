//! Factions at runtime: the player's faction and standings, which characters are hostile to the
//! player (`RtEntity::hostile`: the definition's *Hostile*, or a faction the player's standing is
//! hostile with) and which characters are each other's foes (hostile factions).

use super::Game;

impl Game {
    /// The player's faction: the one they joined (*Set faction*), else their race's (a race
    /// without one: none), else the player character's ("" = none).
    pub(super) fn player_faction(&self) -> String {
        if !self.world.player_faction.is_empty() {
            return self.world.player_faction.clone();
        }
        match self.player_race() {
            Some(r) => r.faction.clone(),
            None => self.project.db.faction_of(&self.project.config.player),
        }
    }

    /// A character's faction id: what *Set faction* gave it, else the placed character's own,
    /// else its definition's or race's ("" = none).
    pub(super) fn faction_id(&self, e: &super::RtEntity) -> String {
        match &e.faction_set {
            Some(f) => f.clone(),
            None if !e.faction_placed.is_empty() => e.faction_placed.clone(),
            None => self.project.db.faction_of(&e.def),
        }
    }

    /// Puts a character into the faction [`faction_id`](Self::faction_id) says: its index, and
    /// whether it attacks the player (hostile characters overlap the player instead of blocking).
    pub(super) fn apply_faction(&self, e: &mut super::RtEntity) {
        let (index, hostile) = self.faction_effect(e);
        e.faction = index;
        if e.companion.is_none() {
            e.hostile = hostile;
            e.solid = !hostile;
        }
    }

    /// A character's faction index and whether it attacks the player.
    fn faction_effect(&self, e: &super::RtEntity) -> (Option<u16>, bool) {
        let id = self.faction_id(e);
        (self.factions.index(&id), self.hostile_to_player(&e.def, &id))
    }

    /// *Set faction*: `target` ("player", "self" or a name on this map) joins `faction`; "" = back
    /// to its own.
    pub(super) fn set_faction(&mut self, target: &str, faction: &str, owner: Option<u32>) {
        if !faction.is_empty() && self.project.config.factions.get(faction).is_none() {
            eprintln!("[events] Set faction: no faction '{faction}' in Database › Factions");
        }
        if target == "player" {
            self.world.player_faction = faction.to_string();
            self.refresh_hostility();
            return;
        }
        let Some(i) = self.entity_index(target, owner).filter(|&i| self.loaded.entities[i].kind == crate::entity::EntityKind::Character) else {
            eprintln!("[events] Set faction: '{target}' is not a character on this map");
            return;
        };
        self.loaded.entities[i].faction_set = (!faction.is_empty()).then(|| faction.to_string());
        let (index, hostile) = self.faction_effect(&self.loaded.entities[i]);
        let e = &mut self.loaded.entities[i];
        e.faction = index;
        if e.companion.is_none() {
            // Calmed down: it stops hunting the player.
            if e.hostile && !hostile && matches!(e.ai.target, Some(super::ai::Foe::Player) | Some(super::ai::Foe::Companion(_))) {
                e.ai = Default::default();
            }
            e.hostile = hostile;
            e.solid = !hostile;
        }
        self.persist(i);
    }

    /// The player's standing with a faction.
    pub(super) fn standing(&self, faction: &str) -> i32 {
        self.world.standing.get(faction).copied().unwrap_or_else(|| self.project.config.factions.base_standing(&self.player_faction(), faction))
    }

    /// Characters of this definition attack the player: their faction's standing is at
    /// [`HOSTILE_AT`](crate::factions::HOSTILE_AT) or below, or they are *Hostile* and the player
    /// isn't a friend of their faction (no faction: *Hostile* alone decides).
    pub(super) fn hostile_to_player(&self, def: &str, faction: &str) -> bool {
        let marked = self.project.db.character(def).is_some_and(|d| d.hostile);
        let known = !faction.is_empty() && self.project.config.factions.get(faction).is_some();
        crate::factions::attacks_player(marked, known.then(|| self.standing(faction)))
    }

    /// Recomputes who is hostile to the player (after a standing or the player's race changed).
    pub(super) fn refresh_hostility(&mut self) {
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if e.kind != crate::entity::EntityKind::Character || e.companion.is_some() {
                continue;
            }
            let hostile = self.hostile_to_player(&e.def, &self.faction_id(e));
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
        // Characters whose faction isn't their definition's (placed so, or changed in play).
        let own: Vec<String> = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.kind == crate::entity::EntityKind::Character && (e.faction_set.is_some() || !e.faction_placed.is_empty()))
            .map(|e| format!("{}={}{}", if e.name.is_empty() { format!("#{}", e.uid) } else { e.name.clone() }, self.faction_id(e), if e.faction_set.is_some() { "*" } else { "" }))
            .collect();
        let own = if own.is_empty() { String::new() } else { format!("{{{}}}", own.join(" ")) };
        format!("{}[{}]{own}", self.player_faction(), list.join(" "))
    }
}
