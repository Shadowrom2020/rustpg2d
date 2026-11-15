//! Combat states: who is battle-ready, and the battle music.
//!
//! A character is **in combat** while it hunts someone (`ai.rs`: aggro), attacks, or (a companion)
//! fights; the player while an enemy close by hunts them or a companion, while they strike or
//! raise a shield, and when hurt. The state lasts [`HOLD`] seconds after the last of these, so it
//! doesn't flicker. In combat characters stand in their weapon's stance and hold a one-handed
//! weapon (out of combat it hangs at the hip: `render/model.rs`, `Gear::ready`). Enemies
//! engaging the player or a companion switch the music to the battle track
//! (`ProjectConfig::battle_music`, a map may name another or none) and back to the map's own
//! when it is over.

use super::Game;

/// Seconds a combat state lasts after its last reason.
pub(super) const HOLD: f32 = 4.0;
/// Enemies farther than this (tiles) don't put the player in combat however they hunt.
const NEAR: f32 = 16.0;

#[derive(Default)]
pub(super) struct Battle {
    /// Seconds the player stays in combat.
    pub player_t: f32,
    /// Seconds the battle music stays on: like `player_t`, but only for enemies (a swing at
    /// nothing in town draws the sword, it doesn't start the battle music).
    pub threat_t: f32,
    /// The music outside combat: the map's track, or one a *Play music* action chose.
    pub peace: String,
}

impl Game {
    /// Counts the combat states down and up again (once a frame, with the world's update).
    pub(super) fn update_battle(&mut self, dt: f32) {
        let ts = self.ts();
        let player = self.player.pos;
        let mut threat = false;
        for e in &mut self.loaded.entities {
            let fighting = e.active && (e.ai.aggro > 0.0 && !e.ai.returning || e.attack.is_some() || e.companion.as_ref().is_some_and(|l| l.fighting));
            e.combat_t = if fighting { HOLD } else { (e.combat_t - dt).max(0.0) };
            threat |= fighting && e.hostile && e.companion.is_none() && e.max_hp > 0 && e.body.pos.distance(player) <= NEAR * ts;
        }
        // (Defeated: the fight is over, whatever the enemies still think.)
        let threat = threat && self.game_over.is_none();
        let me = self.game_over.is_none() && (self.combat.strike > 0.0 || self.combat.block.is_some() || self.combat.invuln > 0.0);
        self.battle.player_t = if threat || me { HOLD } else { (self.battle.player_t - dt).max(0.0) };
        self.battle.threat_t = if threat { HOLD } else { (self.battle.threat_t - dt).max(0.0) };
        self.update_battle_music(dt);
    }

    pub(super) fn player_in_combat(&self) -> bool {
        self.battle.player_t > 0.0
    }

    pub(super) fn npc_in_combat(&self, i: usize) -> bool {
        self.loaded.entities.get(i).is_some_and(|e| e.combat_t > 0.0)
    }

    /// The battle track of this map: its own (`""` = none), else the project's.
    pub(super) fn battle_music(&self) -> &str {
        self.loaded.map.battle_music.as_deref().unwrap_or(&self.project.config.battle_music)
    }

    /// Fades to the battle track while the player is in combat, else to the peace track.
    fn update_battle_music(&mut self, dt: f32) {
        let battle = self.battle_music();
        let fight = self.battle.threat_t > 0.0;
        let wanted = if fight && !battle.is_empty() && self.project.db.sound(battle).is_some() { battle.to_string() } else { self.battle.peace.clone() };
        // (A map without music of its own keeps what plays: nothing to come back to.)
        if !wanted.is_empty() || fight || self.audio.current_music() == Some(battle) {
            self.audio.fade_music_to(&wanted);
        }
        self.audio.tick_music(dt);
    }

    /// The music outside combat (map enter, *Play music*): played now unless a fight is on.
    pub(super) fn set_peace_music(&mut self, id: &str) {
        self.battle.peace = id.to_string();
        if self.battle.threat_t <= 0.0 || self.battle_music().is_empty() {
            self.audio.play_music(id);
        }
    }

    /// `battle=` for `status`: the player's combat (`on:SECS` / `off`) and the characters in it.
    pub(super) fn battle_status(&self) -> String {
        let who: Vec<String> = self.loaded.entities.iter().filter(|e| e.active && e.combat_t > 0.0).map(|e| if e.name.is_empty() { e.def.clone() } else { e.name.clone() }).collect();
        let me = if self.player_in_combat() { format!("on:{:.1}", self.battle.player_t) } else { "off".into() };
        format!("{me}[{}]", who.join(","))
    }
}
