//! Experience and levels at runtime: XP for defeated characters and completed quests,
//! level-ups (max HP, mana, skill points) and the numbers the HUD and the character window
//! show. The settings and the level math are in [`crate::progression`].

use super::Game;

impl Game {
    /// Experience and levels are on for this project.
    pub(super) fn progression_on(&self) -> bool {
        self.project.config.progression.enabled
    }

    /// The player's max HP: the player character's, plus the levels' bonus (0 = no combat).
    pub(super) fn player_max_hp(&self) -> i32 {
        let base = self.combat.max_hp;
        if base <= 0 {
            return base;
        }
        // The race's extra (or fewer) HP.
        let base = (base + self.player_race().map_or(0, |r| r.max_hp)).max(1);
        if !self.progression_on() {
            return base;
        }
        let p = &self.project.config.progression;
        (base + (self.world.player_level() as i32 - 1) * p.hp_per_level).max(1)
    }

    /// Extra max mana from levels (with magic).
    pub(super) fn level_mana(&self) -> u32 {
        if !self.progression_on() {
            return 0;
        }
        (self.world.player_level() - 1) * self.project.config.progression.mana_per_level
    }

    /// Adds (negative: removes) experience and raises the level as far as it reaches; every
    /// level gives max HP (healed by as much), mana and skill points, with one message each.
    /// Losing experience never lowers the level.
    pub(super) fn give_xp(&mut self, n: i64) {
        if !self.progression_on() || n == 0 {
            return;
        }
        self.world.xp = if n > 0 { self.world.xp.saturating_add(n as u64) } else { self.world.xp.saturating_sub(n.unsigned_abs()) };
        let p = self.project.config.progression.clone();
        while self.world.player_level() < p.max_level.max(1) && self.world.xp >= p.xp_for_level(self.world.player_level() + 1) {
            self.level_up(&p);
        }
    }

    fn level_up(&mut self, p: &crate::progression::Progression) {
        self.world.level = self.world.player_level() + 1;
        let mut gains = Vec::new();
        if self.combat.enabled() && p.hp_per_level != 0 {
            let max = self.player_max_hp();
            let hp = self.world.player_hp.unwrap_or(max);
            self.world.player_hp = Some((hp + p.hp_per_level).clamp(1, max));
            gains.push(format!("{:+} HP", p.hp_per_level));
        }
        if self.magic_on() && p.mana_per_level > 0 {
            self.world.mana = (self.world.mana + p.mana_per_level as f32).min(self.max_mana() as f32);
            gains.push(format!("+{} mana", p.mana_per_level));
        }
        if self.skills_on() && p.skill_points_per_level > 0 {
            let n = p.skill_points_per_level;
            self.world.skill_points += n;
            gains.push(format!("+{n} skill point{}", if n == 1 { "" } else { "s" }));
        }
        let msg = if gains.is_empty() { format!("Level {}!", self.world.level) } else { format!("Level {}!  {}", self.world.level, gains.join(", ")) };
        self.ui.toast(msg);
        self.audio.play(&self.project.config.sfx.pickup);
    }

    /// The XP a defeated character gives.
    pub(super) fn xp_of(&self, def: &str) -> u32 {
        self.project.db.character(def).map_or(0, |d| d.xp)
    }

    /// Progress inside the current level for the HUD bar: (XP since the level, XP to the next).
    pub(super) fn xp_progress(&self) -> (u64, u64) {
        self.project.config.progression.progress(self.world.player_level(), self.world.xp)
    }

    /// For `status()`.
    pub(super) fn progression_status(&self) -> String {
        format!("xp={} level={}", self.world.xp, self.world.player_level())
    }
}
