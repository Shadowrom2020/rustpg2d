//! Characters' spells outside the hero: what a party member knows (its class's spells, the
//! saved `Companion::spells`), teaching one (*Teach spell*, `teach_spell()`, a scroll handed to
//! it) under the SRD's rule (only a class whose list has the spell learns it: a known caster adds
//! it, a wizard copies it into its spellbook), and spells told to other characters for the story
//! (`WorldState::npc_spells`, what `npc_knows_spell` reads). Party members cast in fights when
//! the player plays their turns; the AI's casting is 0.32. (Until 0.29 companions healed from
//! their own mana in real time.)

use super::turns::Who;
use super::Game;

impl Game {
    /// Mana went with the SRD's spell slots (0.30): no character has any.
    pub(super) fn npc_max_mana(&self, _i: usize) -> f32 {
        0.0
    }

    /// Where character `i` is placed: the key of its story spells.
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

    /// The spells character `i` knows: a party member's class spells, and any told to it.
    pub(super) fn npc_spell_list(&self, i: usize) -> Vec<String> {
        let (map, uid) = self.spell_key(i);
        let mut v = self.world.npc_spells(&map, uid).to_vec();
        if self.loaded.entities[i].companion.is_some() {
            for s in self.known_of(Who::Npc(self.loaded.entities[i].uid)) {
                if !v.contains(&s) {
                    v.push(s);
                }
            }
        }
        v
    }

    /// Teaches character `i` `spell` (*Teach spell*, a scroll given to it): a party member whose
    /// class lists the spell learns it; anyone else can't. False, with a message, when it can't.
    pub(super) fn teach_character(&mut self, i: usize, spell: &str) -> bool {
        let uid = self.loaded.entities[i].uid;
        let who = self.entity_display_name(uid).unwrap_or_else(|| self.loaded.entities[i].name.clone());
        let Some(def) = self.project.db.spell(spell).cloned() else {
            eprintln!("[game] Teach spell: there is no spell '{spell}'");
            return false;
        };
        let name = crate::lang::tr(&def.name);
        let member = self.loaded.entities[i].companion.is_some();
        if !member || !self.on_list(Who::Npc(uid), &def) {
            self.ui.toast(crate::lang::tr_ui("{} can't learn {}.").replacen("{}", &who, 1).replacen("{}", &name, 1));
            return false;
        }
        if self.known_of(Who::Npc(uid)).iter().any(|s| s == spell) {
            return false;
        }
        self.add_known(Who::Npc(uid), &[spell.to_string()]);
        self.ui.toast(crate::lang::tr_ui("{} learned {}.").replacen("{}", &who, 1).replacen("{}", &name, 1));
        self.auto_emote(i, "joy");
        true
    }

    /// Character `i` was given `item` (*Give item to character*, `give_item_to`): a party member
    /// learns the spell a scroll or book teaches, when its class can.
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

    /// Party members no longer cast in real time (their spells are cast in fights' turns).
    pub(super) fn companion_magic(&mut self, _i: usize, _dt: f32) -> bool {
        false
    }

    /// For `status()`: ":spells=x,y:slots=1:2/2" of companion `i` (empty without spells).
    pub(super) fn companion_magic_status(&self, i: usize) -> String {
        let spells = self.npc_spell_list(i);
        let who = Who::Npc(self.loaded.entities[i].uid);
        let max = self.slot_max(who);
        if spells.is_empty() && max.iter().all(|n| *n == 0) {
            return String::new();
        }
        let left = self.slots_left(who);
        let slots: Vec<String> = (0..9).filter(|k| max[*k] > 0).map(|k| format!("{}:{}/{}", k + 1, left[k], max[k])).collect();
        format!(":spells={}:slots={}", spells.join(","), slots.join(","))
    }
}
