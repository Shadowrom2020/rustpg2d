//! Detection spells (0.32, the SRD's Detect Magic, Detect Poison and Disease, Detect Evil and
//! Good): while the hero has a `Buff::Detect`, what it senses within 30 ft glows on the map:
//! `magic` (items lying there, characters carrying or wearing magic items, containers holding
//! them), `poison` (poisons lying there or carried, poisonous creatures), creature types
//! (`undead`, `fiend`…).

use macroquad::prelude::*;

use super::Game;
use crate::entity::EntityKind;

/// How far the senses reach (tiles: 30 ft).
const REACH: f32 = 6.0;

impl Game {
    /// What the hero senses now (the Detect boons' kinds).
    fn detect_kinds(&self) -> Vec<String> {
        crate::srd::conditions::buffs(&self.world.effects).filter_map(|b| if let crate::srd::conditions::Buff::Detect { what } = b { Some(what.clone()) } else { None }).flatten().map(|w| w.to_ascii_lowercase()).collect()
    }

    /// Whether item `id` is what `kind` senses (magic, poison).
    fn item_sensed(&self, id: &str, kind: &str) -> bool {
        self.project.db.item(id).is_some_and(|d| match kind {
            "magic" => d.magic.is_some(),
            "poison" => d.poison.is_some(),
            _ => false,
        })
    }

    /// The entities within reach that the hero's detection senses (indices).
    fn detected(&self) -> Vec<usize> {
        let kinds = self.detect_kinds();
        if kinds.is_empty() {
            return Vec::new();
        }
        let reach = REACH * self.ts();
        let map = self.loaded.map.name.clone();
        (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                if !e.active || e.body.pos.distance(self.player.pos) > reach {
                    return false;
                }
                kinds.iter().any(|k| match e.kind {
                    EntityKind::Item => self.item_sensed(&e.def, k),
                    EntityKind::Character => {
                        let carried = e.inventory.iter().any(|s| self.item_sensed(&s.item, k)) || crate::defs::EquipSlot::ALL.iter().any(|s| e.equipment.item(*s).is_some_and(|id| self.item_sensed(id, k)));
                        let block = self.stat_block(e);
                        let kind = block.map(|b| b.creature_type.to_ascii_lowercase()).unwrap_or_default();
                        let poisonous = k == "poison" && block.is_some_and(|b| b.attacks.iter().any(|a| a.damage_type == Some(crate::srd::DamageType::Poison) || a.extra.iter().any(|x| x.damage_type == Some(crate::srd::DamageType::Poison))));
                        carried || poisonous || (!kind.is_empty() && kind.contains(k.as_str()))
                    }
                    EntityKind::Prop => {
                        let opened = self.world.containers.get(&map).and_then(|m| m.get(&e.uid));
                        match opened {
                            Some(inv) => inv.slots.iter().flatten().any(|s| self.item_sensed(&s.item, k)),
                            None => self.loaded.map.entity(e.uid).and_then(|p| self.project.db.container_of(p)).is_some_and(|c| c.items.iter().any(|s| self.item_sensed(&s.item, k))),
                        }
                    }
                })
            })
            .collect()
    }

    /// Draws the glow over what the hero senses (world space).
    pub(super) fn draw_detected(&self) {
        let t = get_time() as f32;
        let pulse = 0.55 + 0.25 * (t * 3.0).sin();
        for i in self.detected() {
            let e = &self.loaded.entities[i];
            let p = e.body.pos;
            let h = e.body.hitbox.y.max(self.ts() * 0.5);
            draw_circle_lines(p.x, p.y - h / 2.0, h * 0.75, 2.0, Color::new(0.75, 0.55, 1.0, pulse));
            draw_circle(p.x, p.y - h / 2.0, h * 0.7, Color::new(0.75, 0.55, 1.0, pulse * 0.25));
        }
    }

    /// `detect=` for `status`: what the hero senses (`def@x,y`), `-` without a Detect boon.
    pub(super) fn detect_status(&self) -> String {
        if self.detect_kinds().is_empty() {
            return "-".into();
        }
        let ts = self.ts();
        let v: Vec<String> = self
            .detected()
            .into_iter()
            .map(|i| {
                let e = &self.loaded.entities[i];
                let t = crate::grid::feet_tile(e.body.pos, ts);
                format!("{}@{},{}", e.def, t.0, t.1)
            })
            .collect();
        format!("[{}]", v.join(","))
    }
}
