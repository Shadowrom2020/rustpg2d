//! The SRD's equipment rules, pure: armor class from body armor and a shield, weapon and armor
//! proficiency, money in coins (worth in copper pieces, change, how a price reads).

use crate::defs::{Armor, ArmorCategory, Weapon, WeaponCategory};

/// The SRD's ammunition (by SRD index): what goes into the ammunition slot.
pub const AMMUNITION: [&str; 4] = ["arrow", "crossbow-bolt", "sling-bullet", "blowgun-needle"];

/// What a caster casts spells with material components through (by SRD index): a component
/// pouch, an arcane focus, a holy symbol, a druidic focus, or a bard's musical instrument. (The
/// SRD ties each focus to its classes; any of them does here.)
pub const FOCUS: [&str; 23] = [
    "component-pouch", "crystal", "orb", "rod", "staff", "wand", "amulet", "emblem", "reliquary", "sprig-of-mistletoe", "totem", "wooden-staff", "yew-wand",
    "bagpipes", "drum", "dulcimer", "flute", "lute", "lyre", "horn", "pan-flute", "shawm", "viol",
];

/// What the armor worn gives: (armor class without a shield, wears body armor, the shield's
/// bonus). Body armor: its base + DEX up to its cap; none: 10 + DEX.
pub fn armor_class(body: Option<&Armor>, shield: Option<&Armor>, dex: i32) -> (i32, bool, i32) {
    let body = body.filter(|a| matches!(a.category, Some(ArmorCategory::Light | ArmorCategory::Medium | ArmorCategory::Heavy)));
    let ac = match body {
        // (Heavy armor: no DEX at all, neither bonus nor penalty.)
        Some(a) => a.base_ac + match a.dex_max {
            Some(0) => 0,
            Some(m) => dex.min(m),
            None => dex,
        },
        None => 10 + dex,
    };
    let shield = shield.filter(|a| a.category == Some(ArmorCategory::Shield)).map_or(0, |a| a.base_ac);
    (ac, body.is_some(), shield)
}

/// Whether proficiencies (`has`) cover an armor category (`all-armor` covers light, medium and
/// heavy; shields need `shields`; clothing needs nothing).
pub fn armor_proficient(has: impl Fn(&str) -> bool, cat: ArmorCategory) -> bool {
    match cat.proficiency() {
        None => true,
        Some(p) => has(p) || (cat != ArmorCategory::Shield && has("all-armor")),
    }
}

/// Whether proficiencies (`has`) cover a weapon: by its category (`simple-weapons`,
/// `martial-weapons`) or by its SRD name (`srd` "longsword": `longswords`; `crossbow-light`:
/// `crossbows-light`; `crossbow-hand`: `hand-crossbows`). A weapon without a category (a
/// monster's, an old one) counts as proficient.
pub fn weapon_proficient(has: impl Fn(&str) -> bool, w: &Weapon, srd: &str) -> bool {
    match w.category {
        None => true,
        Some(WeaponCategory::Simple) if has("simple-weapons") => true,
        Some(WeaponCategory::Martial) if has("martial-weapons") => true,
        _ if srd.is_empty() => false,
        _ => {
            let mut names = vec![format!("{srd}s")];
            if let Some((first, rest)) = srd.split_once('-') {
                names.push(format!("{first}s-{rest}"));
                names.push(format!("{rest}-{first}s"));
            }
            names.iter().any(|n| has(n))
        }
    }
}

/// Change in the fewest coins of `kinds` (worth in cp, the largest first): (worth, how many).
/// Electrum and platinum aren't given as change (players count in gold, silver and copper),
/// unless they are the only coins there are.
pub fn change(cp: u32, kinds: &[u32]) -> Vec<(u32, u32)> {
    let mut usual: Vec<u32> = kinds.iter().copied().filter(|w| *w != 50 && *w != 1000).collect();
    if usual.is_empty() {
        usual = kinds.to_vec();
    }
    usual.sort_unstable_by(|a, b| b.cmp(a));
    let mut left = cp;
    let mut out = Vec::new();
    for w in usual {
        if w == 0 {
            continue;
        }
        let n = left / w;
        if n > 0 {
            out.push((w, n));
            left -= n * w;
        }
    }
    out
}

/// A price in copper pieces as it reads: `15 gp`, `1 gp 5 sp`, `2 cp`, `0 cp` (abbreviations
/// in `names`: gold, silver, copper).
pub fn format_cp(cp: u32, names: [&str; 3]) -> String {
    if cp == 0 {
        return format!("0 {}", names[2]);
    }
    let (gp, sp, c) = (cp / 100, cp % 100 / 10, cp % 10);
    let mut parts = Vec::new();
    for (n, name) in [(gp, names[0]), (sp, names[1]), (c, names[2])] {
        if n > 0 {
            parts.push(format!("{n} {name}"));
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{EquipSlot, WeaponProperty};

    fn armor(cat: ArmorCategory, base: i32, dex_max: Option<i32>) -> Armor {
        Armor { category: Some(cat), base_ac: base, dex_max, ..Armor::clothing(if cat == ArmorCategory::Shield { EquipSlot::Offhand } else { EquipSlot::Body }) }
    }

    #[test]
    fn armor_class_follows_the_armor_table() {
        let leather = armor(ArmorCategory::Light, 11, None);
        let half_plate = armor(ArmorCategory::Medium, 15, Some(2));
        let plate = armor(ArmorCategory::Heavy, 18, Some(0));
        let shield = armor(ArmorCategory::Shield, 2, None);
        assert_eq!(armor_class(None, None, 3), (13, false, 0), "unarmored: 10 + DEX");
        assert_eq!(armor_class(Some(&leather), None, 3), (14, true, 0));
        assert_eq!(armor_class(Some(&half_plate), Some(&shield), 4), (17, true, 2), "medium: DEX up to 2");
        assert_eq!(armor_class(Some(&plate), None, 3).0, 18, "heavy: no DEX");
        assert_eq!(armor_class(Some(&plate), None, -1).0, 18, "heavy: no DEX penalty either");
        // A helmet (clothing) in the body slot gives nothing.
        assert_eq!(armor_class(Some(&Armor::clothing(EquipSlot::Body)), None, 2), (12, false, 0));
    }

    #[test]
    fn proficiency_by_category_and_name() {
        let profs = ["simple-weapons", "longswords", "crossbows-light", "light-armor", "all-armor"];
        let has = |p: &str| profs.contains(&p);
        let mut w = Weapon::melee(3, 1.0);
        w.category = Some(WeaponCategory::Martial);
        assert!(weapon_proficient(has, &w, "longsword"));
        assert!(!weapon_proficient(has, &w, "rapier"));
        assert!(weapon_proficient(has, &w, "crossbow-light"));
        w.category = Some(WeaponCategory::Simple);
        assert!(weapon_proficient(has, &w, "club"));
        w.category = None;
        assert!(weapon_proficient(|_| false, &w, ""), "a weapon without a category");
        assert!(weapon_proficient(|p| p == "hand-crossbows", &Weapon { category: Some(WeaponCategory::Martial), properties: vec![WeaponProperty::Light], ..Weapon::melee(1, 1.0) }, "crossbow-hand"));
        assert!(armor_proficient(has, ArmorCategory::Heavy) && !armor_proficient(has, ArmorCategory::Shield) && armor_proficient(|_| false, ArmorCategory::Clothing));
    }

    #[test]
    fn coins_change_and_read() {
        let kinds = [1000, 100, 50, 10, 1];
        assert_eq!(change(1234, &kinds), vec![(100, 12), (10, 3), (1, 4)], "no platinum or electrum as change");
        assert_eq!(change(0, &kinds), vec![]);
        assert_eq!(change(70, &[50]), vec![(50, 1)], "only electrum: electrum");
        let names = ["gp", "sp", "cp"];
        assert_eq!((format_cp(1500, names), format_cp(150, names), format_cp(2, names), format_cp(0, names)), ("15 gp".into(), "1 gp 5 sp".into(), "2 cp".into(), "0 cp".into()));
    }
}
