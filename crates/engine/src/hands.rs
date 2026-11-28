//! What characters hold in their hands: the weapon (or a tool in use) and what is in the left
//! hand, each in the hand its kind puts it in ([`Hand`]). Items are shown in the hand by their
//! own 3D model (`ItemDef.model`, placed on the hand bones by `render::model`); an item without
//! one is held but not shown. A carried prop (`Action::Carry`) fills both hands.

use crate::defs::Database;
use crate::state::Equipment;

/// Which hand holds an item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hand {
    /// The weapon hand (the default for weapons and tools).
    #[default]
    Right,
    /// The shield hand (left-hand items; bows).
    Left,
    /// Both hands: between them (two-handed weapons, carried props).
    Both,
}

/// Something in a character's hands.
#[derive(Clone, Debug, PartialEq)]
pub struct InHand {
    /// Item id, or the prop id of a carried prop.
    pub id: String,
    pub hand: Hand,
    /// A carried prop (not an item).
    pub carried: bool,
}

/// What a character holds: a carried prop alone (in both hands), else the weapon (`tool`, a
/// tool in use, instead of it) and the left hand's item, each in the hand its kind puts it in:
/// left-hand items and bows the left, two-handed weapons (and long ones while the left hand is
/// free) both, others the right.
pub fn in_hands(db: &Database, eq: &Equipment, carrying: &str, tool: Option<&str>) -> Vec<InHand> {
    if !carrying.is_empty() {
        return db.prop(carrying).map(|p| InHand { id: p.id.clone(), hand: Hand::Both, carried: true }).into_iter().collect();
    }
    let weapon = tool.or(eq.item(crate::defs::EquipSlot::Weapon));
    let mut out = Vec::new();
    for (id, offhand) in [(weapon, false), (eq.item(crate::defs::EquipSlot::Offhand), true)] {
        let Some(d) = id.and_then(|id| db.item(id)) else { continue };
        let hand = if offhand {
            Hand::Left
        } else {
            match &d.weapon {
                Some(w) if w.ranged.is_some() => Hand::Left,
                Some(w) if w.two_handed || (w.long() && eq.item(crate::defs::EquipSlot::Offhand).is_none()) => Hand::Both,
                _ => Hand::Right,
            }
        };
        out.push(InHand { id: d.id.clone(), hand, carried: false });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{ItemDef, SpriteRef, Weapon};
    use crate::inventory::ItemStack;

    fn db() -> Database {
        let mut db = Database::default();
        let item = |id: &str| ItemDef { id: id.into(), name: id.into(), icon: SpriteRef::new("items", 1), ..Default::default() };
        db.items.push(ItemDef { weapon: Some(Weapon::melee(2, 0.4)), ..item("sword") });
        let mut bow = ItemDef { weapon: Some(Weapon::melee(1, 0.5)), ..item("bow") };
        bow.weapon.as_mut().unwrap().ranged = Some(crate::defs::Ranged { projectile: SpriteRef::new("items", 2), speed: 100.0, range: 6.0, ammo: String::new(), long_range: 0.0 });
        db.items.push(bow);
        db.items.push(item("shield"));
        db.items.push(ItemDef { weapon: Some(Weapon { two_handed: true, ..Weapon::melee(2, 0.5) }), ..item("spear") });
        // The SRD's spear: versatile, a long reach, not two-handed.
        db.items.push(ItemDef { weapon: Some(Weapon { reach: 1.9, ..Weapon::melee(2, 0.5) }), ..item("srd_spear") });
        db.props.push(crate::defs::PropDef { id: "crate".into(), sprite: SpriteRef::new("props", 3), ..crate::defs::PropDef::fresh("crate") });
        db
    }

    #[test]
    fn items_go_into_their_hands() {
        let db = db();
        let hands = |eq: &Equipment, carrying: &str, tool: Option<&str>| in_hands(&db, eq, carrying, tool).into_iter().map(|h| (h.id, h.hand, h.carried)).collect::<Vec<_>>();
        let eq = Equipment { weapon: Some(ItemStack::new("sword", 1)), offhand: Some(ItemStack::new("shield", 1)), ..Default::default() };
        assert_eq!(hands(&eq, "", None), vec![("sword".into(), Hand::Right, false), ("shield".into(), Hand::Left, false)]);
        // A bow goes into the left hand; a tool replaces the weapon; a carried prop replaces all.
        let bow = Equipment { weapon: Some(ItemStack::new("bow", 1)), ..Default::default() };
        assert_eq!(hands(&bow, "", None)[0].1, Hand::Left);
        assert_eq!(hands(&eq, "", Some("bow"))[0].0, "bow");
        assert_eq!(hands(&eq, "crate", None), vec![("crate".into(), Hand::Both, true)]);
        // Two-handed: both hands.
        let spear = Equipment { weapon: Some(ItemStack::new("spear", 1)), ..Default::default() };
        assert_eq!(hands(&spear, "", None)[0].1, Hand::Both);
        // A long weapon that isn't two-handed: both hands while the left is free, else the right.
        let long = Equipment { weapon: Some(ItemStack::new("srd_spear", 1)), ..Default::default() };
        assert_eq!(hands(&long, "", None)[0].1, Hand::Both);
        let long_shield = Equipment { offhand: Some(ItemStack::new("shield", 1)), ..long };
        assert_eq!(hands(&long_shield, "", None)[0].1, Hand::Right);
    }
}
