//! Companions' magic: spells they learned from scrolls and books the player gave them (or the
//! *Teach spell* action / `teach_spell()`), cast from their own mana (`CharacterDef::max_mana`,
//! regained by `mana_regen` per second). *Heal* goes to the most hurt of the player, the
//! companions on the map (a knocked-down one first: a heal gets it up) and itself; *Bolt* at the
//! enemy it fights. Learned spells are kept by where the character is placed
//! (`WorldState::npc_spells`), so a dismissed companion doesn't forget them.

use macroquad::prelude::*;

use super::Game;
use crate::skills::SpellEffect;

/// The player or a companion below this share of its HP gets a *Heal*.
const HEAL_BELOW: f32 = 0.5;
/// How far (tiles) a companion's *Heal* reaches.
const HEAL_RANGE: f32 = 6.0;

/// Whether companions cast a spell of this effect (the others need the player's aiming).
pub(super) fn castable(effect: &SpellEffect) -> bool {
    matches!(effect, SpellEffect::Heal { .. } | SpellEffect::Bolt { .. })
}

/// Who gets a *Heal*: the lowest share of HP below [`HEAL_BELOW`] (a knocked-down companion
/// counts as 0); `None` when nobody needs one. Candidates: (who, share of HP, knocked down).
pub(super) fn heal_choice<T: Copy>(candidates: &[(T, f32, bool)]) -> Option<T> {
    candidates
        .iter()
        .filter(|(_, share, down)| *down || *share < HEAL_BELOW)
        .min_by(|a, b| (if a.2 { -1.0 } else { a.1 }).total_cmp(&(if b.2 { -1.0 } else { b.1 })))
        .map(|c| c.0)
}

/// Who a companion heals.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Patient {
    Player,
    Entity(usize),
}

impl Game {
    /// Character `i`'s mana pool (its definition's; 0 = it can't learn spells).
    pub(super) fn npc_max_mana(&self, i: usize) -> f32 {
        self.project.db.character(&self.loaded.entities[i].def).map_or(0, |d| d.max_mana) as f32
    }

    /// Where character `i` is placed: the key of its learned spells.
    fn spell_key(&self, i: usize) -> (String, u32) {
        let e = &self.loaded.entities[i];
        if let Some(l) = &e.companion {
            (l.map.clone(), l.uid)
        } else if let Some(v) = &e.visitor {
            (v.map.clone(), v.uid)
        } else {
            (self.loaded.map.name.clone(), e.uid)
        }
    }

    /// The spells character `i` has learned.
    pub(super) fn npc_spell_list(&self, i: usize) -> Vec<String> {
        let (map, uid) = self.spell_key(i);
        self.world.npc_spells(&map, uid).to_vec()
    }

    /// Teaches character `i` `spell` (*Teach spell*, or an item that teaches it). False, with a
    /// message, when it can't: no mana, a spell companions don't cast, or it knows it already.
    pub(super) fn teach_character(&mut self, i: usize, spell: &str) -> bool {
        let who = self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_else(|| self.loaded.entities[i].name.clone());
        let Some(def) = self.project.db.spell(spell) else {
            eprintln!("[game] Teach spell: there is no spell '{spell}'");
            return false;
        };
        let name = def.name.clone();
        if self.npc_max_mana(i) <= 0.0 || !castable(&def.effect) {
            self.ui.toast(crate::lang::tr_ui("{} can't learn {}.").replacen("{}", &who, 1).replacen("{}", &name, 1));
            return false;
        }
        let (map, uid) = self.spell_key(i);
        if !self.world.teach_npc(&map, uid, spell) {
            return false;
        }
        self.ui.toast(crate::lang::tr_ui("{} learned {}.").replacen("{}", &who, 1).replacen("{}", &name, 1));
        true
    }

    /// Character `i` was given `item` (*Give item to character*, `give_item_to`): a companion
    /// learns the spell a scroll or book teaches.
    pub(super) fn npc_received(&mut self, i: usize, item: &str) {
        self.companion_reads(i, item);
    }

    /// Companion `i` reads `item` it now carries: it learns the spell a scroll or book teaches; a
    /// scroll that is used up when read leaves its bag. True when one was used up.
    pub(super) fn companion_reads(&mut self, i: usize, item: &str) -> bool {
        if self.loaded.entities[i].companion.is_none() {
            return false;
        }
        let Some(d) = self.project.db.item(item) else { return false };
        if d.teaches.is_empty() {
            return false;
        }
        let (spell, consumable) = (d.teaches.clone(), d.consumable);
        if self.npc_spell_list(i).contains(&spell) || !self.teach_character(i, &spell) || !consumable {
            return false;
        }
        let e = &mut self.loaded.entities[i];
        if let Some(k) = e.inventory.iter().position(|s| s.item == item) {
            e.inventory[k].count -= 1;
            if e.inventory[k].count == 0 {
                e.inventory.remove(k);
            }
        }
        self.persist_npc_items(i);
        true
    }

    /// Companion `i`'s magic this frame: mana and cooldowns, then a *Heal* for whoever needs it
    /// or a *Bolt* at its enemy. True when it cast (it stands still this frame).
    pub(super) fn companion_magic(&mut self, i: usize, dt: f32) -> bool {
        let max = self.npc_max_mana(i);
        let regen = self.project.db.character(&self.loaded.entities[i].def).map_or(0.0, |d| d.mana_regen);
        let e = &mut self.loaded.entities[i];
        let stunned = crate::effects::stunned(&e.effects);
        let Some(l) = e.companion.as_mut() else { return false };
        if max <= 0.0 || l.down.is_some() {
            return false;
        }
        l.mana = (l.mana + regen * dt).min(max);
        for c in &mut l.cooldowns {
            c.1 -= dt;
        }
        l.cooldowns.retain(|c| c.1 > 0.0);
        if stunned {
            return false;
        }
        let spells = self.npc_spell_list(i);
        if spells.is_empty() {
            return false;
        }
        for id in spells {
            let Some(def) = self.project.db.spell(&id).cloned() else { continue };
            let l = self.loaded.entities[i].companion.as_ref().unwrap();
            if l.mana < def.mana as f32 || l.cooldowns.iter().any(|c| c.0 == id) {
                continue;
            }
            let cast = match &def.effect {
                SpellEffect::Heal { amount } => match self.companion_patient(i) {
                    Some(Patient::Player) => {
                        self.heal(*amount);
                        true
                    }
                    Some(Patient::Entity(j)) => {
                        self.heal_entity(j, *amount);
                        true
                    }
                    None => false,
                },
                SpellEffect::Bolt { damage, projectile, speed, range } => self.companion_bolt(i, *damage, projectile, *speed, *range),
                _ => false,
            };
            if cast {
                let who = self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_default();
                if let Some(l) = self.loaded.entities[i].companion.as_mut() {
                    l.mana -= def.mana as f32;
                    l.cooldowns.push((id.clone(), def.cooldown.max(0.5)));
                }
                if matches!(def.effect, SpellEffect::Heal { .. }) {
                    self.ui.toast(crate::lang::tr_ui("{} casts {}.").replacen("{}", &who, 1).replacen("{}", &def.name, 1));
                }
                self.loaded.entities[i].body.moving = false;
                return true;
            }
        }
        false
    }

    /// Who companion `i` would heal now (within reach), if anybody needs it.
    fn companion_patient(&self, i: usize) -> Option<Patient> {
        let ts = self.ts();
        let me = self.loaded.entities[i].body.pos;
        let mut candidates: Vec<(Patient, f32, bool)> = Vec::new();
        if self.combat.enabled() && self.player.pos.distance(me) <= HEAL_RANGE * ts {
            candidates.push((Patient::Player, self.hp() as f32 / self.player_max_hp().max(1) as f32, false));
        }
        for (j, o) in self.loaded.entities.iter().enumerate() {
            if o.companion.is_some() && o.active && o.max_hp > 0 && o.body.pos.distance(me) <= HEAL_RANGE * ts {
                candidates.push((Patient::Entity(j), o.hp as f32 / o.max_hp as f32, self.is_down(j)));
            }
        }
        heal_choice(&candidates)
    }

    /// A *Bolt* from companion `i` at the enemy it fights, when it is in range. True if cast.
    fn companion_bolt(&mut self, i: usize, damage: i32, projectile: &crate::defs::SpriteRef, speed: f32, range: f32) -> bool {
        let Some(t) = self.companion_target(i) else { return false };
        let ts = self.ts();
        let e = &self.loaded.entities[i];
        let eye = e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0);
        let o = &self.loaded.entities[t];
        let aim = o.body.pos - vec2(0.0, o.body.hitbox.y / 2.0);
        if aim.distance(eye) > range * ts {
            return false;
        }
        let dir = (aim - eye).normalize_or_zero();
        let uid = e.uid;
        let e = &mut self.loaded.entities[i];
        e.body.facing = crate::entity::Facing::from_vec(dir.x, dir.y).unwrap_or(e.body.facing);
        let ranged = crate::defs::Ranged { projectile: projectile.clone(), speed, range, ammo: String::new() };
        self.audio.play(&self.project.config.sfx.attack);
        self.shoot(eye, dir, &ranged, damage, true, Some(uid), None);
        true
    }

    /// For `status()`: ":mana=a/b:spells=x,y" of companion `i` (empty without magic).
    pub(super) fn companion_magic_status(&self, i: usize) -> String {
        let max = self.npc_max_mana(i);
        let spells = self.npc_spell_list(i);
        let Some(l) = &self.loaded.entities[i].companion else { return String::new() };
        if max <= 0.0 && spells.is_empty() {
            return String::new();
        }
        format!(":mana={:.0}/{:.0}:spells={}", l.mana, max, spells.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_hurt_gets_the_heal() {
        // Nobody below half: no heal.
        assert_eq!(heal_choice(&[("player", 0.8, false), ("guard", 0.6, false)]), None);
        // The lowest share below half.
        assert_eq!(heal_choice(&[("player", 0.4, false), ("guard", 0.2, false), ("dog", 0.45, false)]), Some("guard"));
        // A knocked-down companion first, even next to a badly hurt player.
        assert_eq!(heal_choice(&[("player", 0.1, false), ("dog", 0.0, true)]), Some("dog"));
    }

    #[test]
    fn companions_cast_heal_and_bolt_only() {
        assert!(castable(&SpellEffect::Heal { amount: 3 }));
        assert!(castable(&SpellEffect::Bolt { damage: 2, projectile: Default::default(), speed: 300.0, range: 6.0 }));
        assert!(!castable(&SpellEffect::Blast { damage: 2, radius: 2.0 }));
        assert!(!castable(&SpellEffect::Light { radius: 4.0, seconds: 30.0 }));
        assert!(!castable(&SpellEffect::Script { function: "f".into() }));
    }
}
