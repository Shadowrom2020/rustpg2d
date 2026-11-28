//! Checks outside fights (the SRD's *Using Ability Scores*): the light at the hero for checks by
//! sight (dim light: disadvantage, darkness: a failure; darkvision sees one step lighter),
//! companions helping (working together: advantage), group checks (half the party succeeds) and
//! contests against a character's stat block. The rolls themselves are `WorldState::check`'s.

use super::turns::Who;
use super::Game;
use crate::srd::check::Sight;

/// Tiles within which a companion helps the hero's checks.
const HELP_TILES: f32 = 6.0;
/// Tiles around the hero a search reaches (props and characters; zones: those they stand in).
const SEARCH_TILES: f32 = 2.0;

impl Game {
    /// Once a frame: the light at the hero for checks by sight, and who of the party helps with
    /// which skill (out of fights; in fights *Help* is an action).
    pub(super) fn refresh_senses(&mut self) {
        let sight = match super::turns::vision::obscured_by_light(self.light_at_player()) {
            super::turns::vision::Obscured::None => Sight::Bright,
            super::turns::vision::Obscured::Light => Sight::Dim,
            super::turns::vision::Obscured::Heavy => Sight::Dark,
        };
        self.world.training.sight = if self.darkvision_ft() > 0 { sight.lighter() } else { sight };
        self.world.training.sight_penalty = self.disease_sight_penalty();
        let mut help = std::collections::BTreeMap::new();
        if self.fight.is_none() {
            let ts = self.ts();
            for (uid, name, pos) in self.party_members() {
                if pos.distance(self.player.pos) > HELP_TILES * ts {
                    continue;
                }
                // (Skills and tools: what a check can be made with; not armor, weapons or saves.)
                for p in self.who_block(Who::Npc(uid)).map(|b| b.proficiencies).unwrap_or_default() {
                    let id = crate::srd::abilities::proficiency_id(&p);
                    if crate::srd::abilities::skill(&id).is_some() || crate::srd::abilities::tool(&id).is_some() {
                        help.entry(id).or_insert_with(|| name.clone());
                    }
                }
            }
        }
        self.world.training.help = help;
    }

    /// The companions on this map who can act (not down): uid, the name messages use, position.
    fn party_members(&self) -> Vec<(u32, String, gfx::prelude::Vec2)> {
        (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && e.companion.as_ref().is_some_and(|c| c.down.is_none())
            })
            .map(|i| (self.loaded.entities[i].uid, self.display_name(i), self.loaded.entities[i].body.pos))
            .collect()
    }

    /// The companions on the map who can act, with their stat blocks (group checks).
    pub(super) fn party_blocks(&self) -> Vec<(String, crate::srd::StatBlock)> {
        self.party_members().into_iter().filter_map(|(uid, name, _)| Some((name, self.who_block(Who::Npc(uid))?))).collect()
    }

    /// A group check (*Group check*): the hero and every companion on the map who can act.
    pub(super) fn group_check(&mut self, what: &str, dc: i32) -> bool {
        let others = self.party_blocks();
        self.world.group_check(what, dc, &others)
    }

    /// A contest (*Contest*) against the character `target` on this map (false without it).
    pub(super) fn contest(&mut self, what: &str, target: &str, against: &str, owner: Option<u32>) -> bool {
        let Some(i) = self.entity_index(target, owner) else {
            eprintln!("[events] Contest: no character '{target}' on this map");
            return false;
        };
        let (uid, name) = (self.loaded.entities[i].uid, self.display_name(i));
        let Some(block) = self.who_block(Who::Npc(uid)) else { return false };
        self.world.contest(what, &name, &block, against)
    }

    /// Search (the action bar): the hero rolls Perception (dim light, darkness and help apply), a
    /// minute passes, and the *Searched* triggers of the zones they stand in and of the props and
    /// characters within 2 tiles fire (*Search found* compares the roll). "You find nothing."
    /// when none of them runs.
    pub(super) fn search(&mut self) {
        if self.fight.is_some() {
            self.ui.toast(crate::lang::tr_ui("In a fight: use the action bar").into_owned());
            return;
        }
        if self.runner.busy() {
            return;
        }
        let total = self.world.roll_check("perception").map_or(0, |r| r.total);
        self.world.last_search = Some(total);
        self.world.advance_hours(1.0 / 60.0);
        let ts = self.ts();
        let mut zones: Vec<u32> = self.zones_inside.iter().copied().collect();
        zones.sort();
        let near: Vec<u32> = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.active && matches!(e.kind, crate::entity::EntityKind::Prop | crate::entity::EntityKind::Character) && e.companion.is_none() && e.body.pos.distance(self.player.pos) <= SEARCH_TILES * ts)
            .map(|e| e.uid)
            .collect();
        // Traps within reach whose DC the roll reaches are found.
        let mut found = self.search_traps(total);
        for uid in zones {
            found |= self.fire(crate::events::GameEvent::Search, super::Source::Zone(uid));
        }
        for uid in near {
            found |= self.fire(crate::events::GameEvent::Search, super::Source::Entity(uid));
        }
        if !found {
            self.ui.toast(crate::lang::tr_ui("You find nothing.").into_owned());
        }
    }

    /// `search=` for status: the last search's total (`-` = none yet).
    pub(super) fn search_status(&self) -> String {
        self.world.last_search.map_or("-".into(), |t| t.to_string())
    }

    /// `senses=` for status: the light at the hero for sight checks (`light:dim`, `:darkvision`)
    /// and who helps with what (`help:athletics=Finn,…`).
    pub(super) fn senses_status(&self) -> String {
        let t = &self.world.training;
        let dv = if self.darkvision_ft() > 0 { ":darkvision" } else { "" };
        let help: Vec<String> = t.help.iter().map(|(k, v)| format!("{k}={v}")).collect();
        // (The hero's special senses: spells, items.)
        let see: Vec<String> = crate::srd::conditions::Sense::ALL
            .into_iter()
            .filter(|s| *s != crate::srd::conditions::Sense::Darkvision)
            .filter_map(|s| match self.sense_of(super::turns::Who::Player, s) {
                0 => None,
                u32::MAX => Some(s.id().replace(' ', "_")),
                f => Some(format!("{}{f}", s.id().replace(' ', "_"))),
            })
            .collect();
        format!("light:{}{dv} help:{}{}", t.sight.word(), if help.is_empty() { "-".into() } else { help.join(",") }, if see.is_empty() { String::new() } else { format!(" see:{}", see.join(",")) })
    }
}
