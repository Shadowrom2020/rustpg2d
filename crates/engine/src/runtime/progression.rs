//! Experience and levels at runtime: XP for defeated characters and completed quests,
//! level-ups (SRD hit points, mana) and the numbers the HUD and the character window
//! show. The settings and the level math are in [`crate::progression`].

use super::Game;

impl Game {
    /// Experience and levels are on for this project.
    pub(super) fn progression_on(&self) -> bool {
        self.project.config.progression.enabled
    }

    /// The player's max HP (SRD): the hit die (a d10 until classes) at level 1 plus the CON
    /// modifier, each level's roll or average plus CON, the race's per-level bonus and its old
    /// extra HP. 0 = the game has no combat (the player character has no HP).
    pub(super) fn player_max_hp(&self) -> i32 {
        if self.combat.max_hp <= 0 {
            return self.combat.max_hp;
        }
        let race = self.player_race();
        let level = if self.progression_on() { self.world.player_level() } else { 1 };
        let con = self.world.abilities.modifier(crate::srd::Ability::Con);
        let hp = crate::progression::max_hp(level, con, &self.world.hp_rolls, race.as_ref().map_or(0, |r| r.hp_per_level));
        let hp = (hp + race.as_ref().map_or(0, |r| r.max_hp)).max(1);
        // Exhaustion 4 and up halves it.
        if crate::srd::conditions::exhaustion(&self.world.effects) >= 4 {
            (hp / 2).max(1)
        } else {
            hp
        }
    }

    /// Hit dice the hero has left (rests spend and regain them).
    pub(super) fn hit_dice_left(&self) -> u32 {
        let level = self.world.player_level();
        self.world.hit_dice.unwrap_or(level).min(level)
    }

    /// A character's SRD stat block: the placed character's own, else its definition's.
    pub(super) fn stat_block(&self, e: &super::RtEntity) -> Option<&crate::srd::StatBlock> {
        if e.kind != crate::entity::EntityKind::Character {
            return None;
        }
        let placed = self.loaded.map.entities.iter().find(|p| p.uid == e.uid).and_then(|p| p.stats.as_ref());
        placed.or_else(|| self.project.db.character(&e.def).and_then(|c| c.stats.as_ref()))
    }

    /// `statblocks=` for `status`: characters within 8 tiles that have a stat block,
    /// `name:template:hp/ac` (the definition id when unnamed, `-` for a hand-made block).
    pub(super) fn statblocks_status(&self) -> Vec<String> {
        let reach = 8.0 * self.ts();
        self.loaded
            .entities
            .iter()
            .filter(|e| e.active && e.body.pos.distance(self.player.pos) <= reach)
            .filter_map(|e| {
                let s = self.stat_block(e)?;
                Some(format!("{}:{}:{}/{}", if e.name.is_empty() { &e.def } else { &e.name }, if s.template.is_empty() { "-" } else { &s.template }, s.hp, s.ac))
            })
            .collect()
    }

    /// Extra max mana from levels (with magic).
    pub(super) fn level_mana(&self) -> u32 {
        if !self.progression_on() {
            return 0;
        }
        (self.world.player_level() - 1) * self.project.config.progression.mana_per_level
    }

    /// Adds (negative: removes) experience and raises the level as far as it reaches; every
    /// level gives max HP (healed by as much) and mana, with one message each.
    /// Losing experience never lowers the level.
    pub(super) fn give_xp(&mut self, n: i64) {
        if !self.progression_on() || n == 0 {
            return;
        }
        // Script hook: the amount can change.
        let n = if self.hooked("xp") { crate::hooks::int(&self.hook("xp", crate::hooks::map([("amount", n.into())])), "amount", n) } else { n };
        if n == 0 {
            return;
        }
        self.world.xp = if n > 0 { self.world.xp.saturating_add(n as u64) } else { self.world.xp.saturating_sub(n.unsigned_abs()) };
        let p = self.project.config.progression.clone();
        while self.world.player_level() < p.max_level.max(1) && self.world.xp >= p.xp_for_level(self.world.player_level() + 1) {
            self.level_up(&p);
        }
    }

    fn level_up(&mut self, p: &crate::progression::Progression) {
        let before = self.world.player_level();
        self.world.level = before + 1;
        // The hit die for this level: its average or a roll (the SRD); the levels before that a
        // save of an older version has no roll for count as averages.
        let avg = crate::progression::HIT_DIE / 2 + 1;
        while self.world.hp_rolls.len() + 1 < before as usize {
            self.world.hp_rolls.push(avg);
        }
        let die = match p.hit_points {
            crate::progression::HitPoints::Average => avg,
            // (Into the roll log, like every other roll.)
            crate::progression::HitPoints::Rolled => self.world.roll_dice(&format!("1d{}", crate::progression::HIT_DIE)).map_or(avg, |r| r.total as i32),
        };
        let con = self.world.abilities.modifier(crate::srd::Ability::Con);
        let per_level = self.player_race().map_or(0, |r| r.hp_per_level);
        // Script hook: what the level gives can change.
        let (mut hp_gain, mut mana_gain) = ((die + con).max(1) + per_level, p.mana_per_level);
        if self.hooked("level_up") {
            use crate::hooks::{int, map};
            let level = self.world.level as i64;
            let m = self.hook("level_up", map([("level", level.into()), ("hp", (hp_gain as i64).into()), ("mana", (mana_gain as i64).into())]));
            hp_gain = int(&m, "hp", hp_gain as i64) as i32;
            mana_gain = int(&m, "mana", mana_gain as i64).max(0) as u32;
        }
        // (A hook's gain: the die part that gives it, so the max HP follows.)
        self.world.hp_rolls.push(hp_gain - per_level - con);
        if let Some(n) = &mut self.world.hit_dice {
            *n += 1;
        }
        let mut gains = Vec::new();
        if self.combat.enabled() && hp_gain != 0 {
            let max = self.player_max_hp();
            let hp = self.world.player_hp.unwrap_or(max);
            self.world.player_hp = Some((hp + hp_gain).clamp(1, max));
            gains.push(format!("{:+} HP", hp_gain));
        }
        if self.magic_on() && mana_gain > 0 {
            self.world.mana = (self.world.mana + mana_gain as f32).min(self.max_mana() as f32);
            gains.push(format!("+{} mana", mana_gain));
        }
        let msg = if gains.is_empty() { format!("Level {}!", self.world.level) } else { format!("Level {}!  {}", self.world.level, gains.join(", ")) };
        self.ui.toast(msg);
        // The project's level-up fanfare (a music effect), else the pickup sound.
        let fanfare = self.project.config.sfx.level_up.clone();
        if !fanfare.is_empty() && self.project.db.sound(&fanfare).is_some() {
            self.audio.play_music_effect(&fanfare);
        } else {
            self.audio.play(&self.project.config.sfx.pickup);
        }
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
