//! Characters use what they carry: a healing item when their health is critical, a cure when
//! poisoned or burning, a buff (strength, haste, ...) in a fight. Companions (items the player
//! gave them) and any other character with items, enemies included. Ranged companions also
//! shoot ammunition from their bag when the player gave them the bow. What an item does comes
//! from its *on use* actions, run with the character as "self" (`healing::redirect_heals`);
//! dialog lines of the item (meant for the player) are left out.

use super::Game;
use super::Source;
use crate::effects::EffectKind;
use crate::events::runner::ActionCtx;
use crate::events::{Action, GameEvent};

/// At or below this share of its HP a character's health is critical.
const CRITICAL: f32 = 0.35;
/// Seconds between two items a character uses.
const USE_COOLDOWN: f32 = 4.0;
/// An enemy hunting this close (tiles) makes a friendly character use a buff.
const BUFF_RANGE: f32 = 5.0;

/// What an item does when used (from its *on use* actions).
#[derive(Default, Debug)]
struct Uses {
    heals: i32,
    cures: Vec<Option<EffectKind>>,
    buffs: Vec<EffectKind>,
}

fn scan(actions: &[Action], u: &mut Uses) {
    crate::events::walk_actions(actions, &mut |a| match a {
        Action::Heal { amount } if *amount > 0 => u.heals += amount,
        Action::CureEffect { target, kind } if target == "player" => u.cures.push(*kind),
        Action::ApplyEffect { target, effect } if target == "player" && is_buff(effect.kind) => u.buffs.push(effect.kind),
        _ => {}
    });
}

/// An item's action without what is meant for the player to read (dialogs, choices).
fn silent(a: Action) -> Option<Action> {
    match a {
        Action::Dialog { .. } | Action::Choice { .. } => None,
        Action::If { conditions, then, otherwise } => Some(Action::If { conditions, then: then.into_iter().filter_map(silent).collect(), otherwise: otherwise.into_iter().filter_map(silent).collect() }),
        a => Some(a),
    }
}

fn is_buff(k: EffectKind) -> bool {
    matches!(k, EffectKind::Regen | EffectKind::Haste | EffectKind::Strength | EffectKind::Shield)
}

impl Game {
    fn item_uses(&self, item: &str) -> Uses {
        let mut u = Uses::default();
        let ev = GameEvent::UseItem { item: item.to_string() };
        if let Some(d) = self.project.db.item(item) {
            for t in d.on_use.iter().filter(|t| t.on.matches(&ev)) {
                scan(&t.actions, &mut u);
            }
        }
        u
    }

    /// Character `i` uses something it carries if that helps now (once per cooldown): health
    /// is critical when at most 35 % is left or the next hit like the last would fell it.
    pub(super) fn npc_use_items(&mut self, i: usize, dt: f32) {
        let down = self.is_down(i);
        let e = &mut self.loaded.entities[i];
        e.item_cd = (e.item_cd - dt).max(0.0);
        if e.item_cd > 0.0 || e.inventory.is_empty() || e.max_hp <= 0 || e.hp <= 0 || down || crate::effects::stunned(&e.effects) {
            return;
        }
        let e = &self.loaded.entities[i];
        let hurt = e.hp < e.max_hp && ((e.hp as f32) <= CRITICAL * e.max_hp as f32 || e.hp <= e.last_hit);
        let bad: Vec<EffectKind> = e.effects.iter().map(|x| x.kind).filter(|k| matches!(k, EffectKind::Poison | EffectKind::Burn | EffectKind::Slow | EffectKind::Weakness)).collect();
        let ts = self.ts();
        // In a fight: hostile characters hunting the player; friendly ones with a hunting enemy close.
        let danger = if e.hostile { e.ai.aggro > 0.0 } else { self.loaded.entities.iter().any(|o| o.active && o.hostile && o.max_hp > 0 && o.ai.aggro > 0.0 && o.body.pos.distance(e.body.pos) <= BUFF_RANGE * ts) };
        let have: Vec<EffectKind> = e.effects.iter().map(|x| x.kind).collect();
        let items: Vec<String> = e.inventory.iter().filter(|s| s.count > 0).map(|s| s.item.clone()).collect();
        let pick = items.into_iter().find(|item| {
            let u = self.item_uses(item);
            (hurt && u.heals > 0)
                || (!bad.is_empty() && u.cures.iter().any(|c| c.is_none_or(|k| bad.contains(&k))))
                || (danger && !u.buffs.is_empty() && u.buffs.iter().any(|b| !have.contains(b)))
        });
        match pick {
            Some(item) => self.npc_use(i, &item),
            // Nothing needed: look again in a quarter second, not every frame (the look scans
            // the inventory and, for friendly characters, every enemy).
            None => self.loaded.entities[i].item_cd = 0.25,
        }
    }

    /// Character `i` uses one `item` it carries: its *on use* actions run on it (without the
    /// item's dialog lines, which are written for the player).
    fn npc_use(&mut self, i: usize, item: &str) {
        let uid = self.loaded.entities[i].uid;
        let actions = self.collect(&GameEvent::UseItem { item: item.to_string() }, &Source::Item(item.to_string()));
        for (a, _) in actions {
            if let Some(a) = silent(super::healing::redirect_heals(a)) {
                self.runner.push(&[a], &ActionCtx { owner: Some(uid), ..Default::default() });
            }
        }
        let consumable = self.project.db.item(item).is_some_and(|d| d.consumable);
        if consumable {
            self.npc_give(i, item, -1);
        }
        self.loaded.entities[i].item_cd = USE_COOLDOWN;
        // Companions and friendly characters say what they do; enemies show it on the map.
        let who = self.entity_display_name(uid).unwrap_or_default();
        let what = self.project.db.item(item).map_or(item.to_string(), |d| d.name.clone());
        self.ui.toast(format!("{who} uses {what}."));
    }

    /// The ammunition companion `i`'s ranged weapon needs (only a weapon the player gave it:
    /// a character's own weapon needs none) and how much it carries.
    pub(super) fn companion_ammo(&self, i: usize) -> Option<(String, u32)> {
        let e = &self.loaded.entities[i];
        e.companion.as_ref()?;
        let id = e.equipment.item(crate::defs::EquipSlot::Weapon)?;
        let ammo = self.project.db.item(id)?.weapon.as_ref()?.ranged.as_ref()?.ammo.clone();
        if ammo.is_empty() {
            return None;
        }
        let n = e.inventory.iter().filter(|s| s.item == ammo).map(|s| s.count).sum();
        Some((ammo, n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectSpec;

    #[test]
    fn item_uses_from_on_use_actions() {
        let mut u = Uses::default();
        let actions = vec![
            Action::Heal { amount: 6 },
            Action::If { conditions: vec![], then: vec![Action::CureEffect { target: "player".into(), kind: Some(EffectKind::Poison) }], otherwise: vec![] },
            Action::ApplyEffect { target: "player".into(), effect: EffectSpec::new(EffectKind::Strength, 25.0, 30.0) },
            // Not for the user: an effect on someone else, a harmful one, damage.
            Action::ApplyEffect { target: "guard".into(), effect: EffectSpec::new(EffectKind::Haste, 25.0, 30.0) },
            Action::ApplyEffect { target: "player".into(), effect: EffectSpec::new(EffectKind::Poison, 1.0, 5.0) },
            Action::Heal { amount: -2 },
        ];
        scan(&actions, &mut u);
        assert_eq!(u.heals, 6);
        assert_eq!(u.cures, vec![Some(EffectKind::Poison)]);
        assert_eq!(u.buffs, vec![EffectKind::Strength]);
    }
}
