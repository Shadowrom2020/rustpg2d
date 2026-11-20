//! Item categories for the editor's lists (the Database's item list, the asset library's browse
//! view and *Add from library* dialog): the SRD's equipment and magic item categories, worked out
//! from what an item is (no data of their own), and a filter picker.

use std::collections::HashSet;

use egui_macroquad::egui;
use rustpg2d_engine::defs::{ArmorCategory, Database, EquipSlot, ItemDef, WeaponCategory};

/// The SRD's mounts and vehicles (with tack, barding, feed and stabling), by index: the item
/// data has no category of its own for them.
const MOUNTS_AND_VEHICLES: [&str; 40] = [
    "camel",
    "donkey",
    "mule",
    "elephant",
    "horse-draft",
    "horse-riding",
    "mastiff",
    "pony",
    "warhorse",
    "barding-padded",
    "barding-leather",
    "barding-studded-leather",
    "barding-hide",
    "barding-chain-shirt",
    "barding-scale-mail",
    "barding-breastplate",
    "barding-half-plate",
    "barding-ring-mail",
    "barding-chain-mail",
    "barding-splint",
    "barding-plate",
    "bit-and-bridle",
    "carriage",
    "cart",
    "chariot",
    "animal-feed-1-day",
    "saddle-exotic",
    "saddle-military",
    "saddle-pack",
    "saddle-riding",
    "saddlebags",
    "sled",
    "stabling-1-day",
    "wagon",
    "galley",
    "keelboat",
    "longship",
    "rowboat",
    "sailing-ship",
    "warship",
];

/// The SRD's ammunition by index (also any item a ranged weapon of the project shoots).
const AMMUNITION: [&str; 4] = ["arrow", "crossbow-bolt", "sling-bullet", "blowgun-needle"];

/// An item's category: the SRD's equipment categories, its magic item categories, and *Other*
/// for a project's own things (food, ores, keys, furniture…).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemCat {
    SimpleMelee,
    SimpleRanged,
    MartialMelee,
    MartialRanged,
    LightArmor,
    MediumArmor,
    HeavyArmor,
    Shields,
    Ammunition,
    AdventuringGear,
    Tools,
    MountsVehicles,
    Money,
    MagicWeapons,
    MagicArmor,
    Potions,
    Rings,
    Rods,
    Scrolls,
    Staffs,
    Wands,
    Wondrous,
    Other,
}

/// A group of categories the picker offers as one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemGroup {
    Weapons,
    Armor,
    Magic,
}

/// What an item list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ItemFilter {
    #[default]
    All,
    Group(ItemGroup),
    Cat(ItemCat),
}

impl ItemCat {
    pub const ALL: [ItemCat; 23] = [
        ItemCat::SimpleMelee,
        ItemCat::SimpleRanged,
        ItemCat::MartialMelee,
        ItemCat::MartialRanged,
        ItemCat::LightArmor,
        ItemCat::MediumArmor,
        ItemCat::HeavyArmor,
        ItemCat::Shields,
        ItemCat::Ammunition,
        ItemCat::AdventuringGear,
        ItemCat::Tools,
        ItemCat::MountsVehicles,
        ItemCat::Money,
        ItemCat::MagicWeapons,
        ItemCat::MagicArmor,
        ItemCat::Potions,
        ItemCat::Rings,
        ItemCat::Rods,
        ItemCat::Scrolls,
        ItemCat::Staffs,
        ItemCat::Wands,
        ItemCat::Wondrous,
        ItemCat::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ItemCat::SimpleMelee => "Simple melee weapons",
            ItemCat::SimpleRanged => "Simple ranged weapons",
            ItemCat::MartialMelee => "Martial melee weapons",
            ItemCat::MartialRanged => "Martial ranged weapons",
            ItemCat::LightArmor => "Light armor",
            ItemCat::MediumArmor => "Medium armor",
            ItemCat::HeavyArmor => "Heavy armor",
            ItemCat::Shields => "Shields",
            ItemCat::Ammunition => "Ammunition",
            ItemCat::AdventuringGear => "Adventuring gear",
            ItemCat::Tools => "Tools",
            ItemCat::MountsVehicles => "Mounts and vehicles",
            ItemCat::Money => "Money",
            ItemCat::MagicWeapons => "Magic weapons and ammunition",
            ItemCat::MagicArmor => "Magic armor and shields",
            ItemCat::Potions => "Potions",
            ItemCat::Rings => "Rings",
            ItemCat::Rods => "Rods",
            ItemCat::Scrolls => "Scrolls and spell books",
            ItemCat::Staffs => "Staffs",
            ItemCat::Wands => "Wands",
            ItemCat::Wondrous => "Wondrous items",
            ItemCat::Other => "Other (the project's own)",
        }
    }

    /// The short id editor scripts use (`simple-melee`, `wondrous`).
    pub fn id(self) -> &'static str {
        match self {
            ItemCat::SimpleMelee => "simple-melee",
            ItemCat::SimpleRanged => "simple-ranged",
            ItemCat::MartialMelee => "martial-melee",
            ItemCat::MartialRanged => "martial-ranged",
            ItemCat::LightArmor => "light-armor",
            ItemCat::MediumArmor => "medium-armor",
            ItemCat::HeavyArmor => "heavy-armor",
            ItemCat::Shields => "shields",
            ItemCat::Ammunition => "ammunition",
            ItemCat::AdventuringGear => "gear",
            ItemCat::Tools => "tools",
            ItemCat::MountsVehicles => "mounts",
            ItemCat::Money => "money",
            ItemCat::MagicWeapons => "magic-weapons",
            ItemCat::MagicArmor => "magic-armor",
            ItemCat::Potions => "potions",
            ItemCat::Rings => "rings",
            ItemCat::Rods => "rods",
            ItemCat::Scrolls => "scrolls",
            ItemCat::Staffs => "staffs",
            ItemCat::Wands => "wands",
            ItemCat::Wondrous => "wondrous",
            ItemCat::Other => "other",
        }
    }

    pub fn group(self) -> Option<ItemGroup> {
        match self {
            ItemCat::SimpleMelee | ItemCat::SimpleRanged | ItemCat::MartialMelee | ItemCat::MartialRanged => Some(ItemGroup::Weapons),
            ItemCat::LightArmor | ItemCat::MediumArmor | ItemCat::HeavyArmor | ItemCat::Shields => Some(ItemGroup::Armor),
            ItemCat::MagicWeapons | ItemCat::MagicArmor | ItemCat::Potions | ItemCat::Rings | ItemCat::Rods | ItemCat::Scrolls | ItemCat::Staffs | ItemCat::Wands | ItemCat::Wondrous => Some(ItemGroup::Magic),
            _ => None,
        }
    }
}

impl ItemGroup {
    pub fn label(self) -> &'static str {
        match self {
            ItemGroup::Weapons => "Weapons",
            ItemGroup::Armor => "Armor",
            ItemGroup::Magic => "Magic items",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            ItemGroup::Weapons => "weapons",
            ItemGroup::Armor => "armor",
            ItemGroup::Magic => "magic",
        }
    }
}

impl ItemFilter {
    pub fn label(self) -> &'static str {
        match self {
            ItemFilter::All => "All items",
            ItemFilter::Group(g) => g.label(),
            ItemFilter::Cat(c) => c.label(),
        }
    }

    /// By the id editor scripts use: `all`, a group (`weapons`, `armor`, `magic`) or a category.
    pub fn parse(s: &str) -> Option<ItemFilter> {
        let s = s.trim().to_ascii_lowercase();
        if s.is_empty() || s == "all" {
            return Some(ItemFilter::All);
        }
        [ItemGroup::Weapons, ItemGroup::Armor, ItemGroup::Magic]
            .into_iter()
            .find(|g| g.id() == s)
            .map(ItemFilter::Group)
            .or_else(|| ItemCat::ALL.into_iter().find(|c| c.id() == s).map(ItemFilter::Cat))
    }

    pub fn id(self) -> &'static str {
        match self {
            ItemFilter::All => "all",
            ItemFilter::Group(g) => g.id(),
            ItemFilter::Cat(c) => c.id(),
        }
    }

    /// Whether an item of category `c` is shown.
    pub fn keeps(self, c: ItemCat) -> bool {
        match self {
            ItemFilter::All => true,
            ItemFilter::Group(g) => c.group() == Some(g),
            ItemFilter::Cat(x) => x == c,
        }
    }
}

/// The items the project's ranged weapons shoot (they count as ammunition).
pub fn ammo_of(db: &Database) -> HashSet<String> {
    db.items.iter().filter_map(|d| d.weapon.as_ref()?.ranged.as_ref()).map(|r| r.ammo.clone()).filter(|a| !a.is_empty()).collect()
}

/// The category of item `d` (`ammo`: [`ammo_of`]).
pub fn category(d: &ItemDef, ammo: &HashSet<String>) -> ItemCat {
    let srd = d.srd.as_str();
    let is_ammo = ammo.contains(&d.id) || AMMUNITION.contains(&srd);
    if d.coin_cp > 0 {
        return ItemCat::Money;
    }
    if let Some(m) = &d.magic {
        let id = d.id.as_str();
        let body_or_shield = d.armor.as_ref().is_some_and(|a| matches!(a.category, Some(ArmorCategory::Light | ArmorCategory::Medium | ArmorCategory::Heavy | ArmorCategory::Shield)));
        return if id.starts_with("staff") || (d.weapon.is_some() && !m.spells.is_empty()) {
            ItemCat::Staffs
        } else if id.starts_with("rod-") {
            ItemCat::Rods
        } else if d.weapon.is_some() || is_ammo {
            ItemCat::MagicWeapons
        } else if body_or_shield {
            ItemCat::MagicArmor
        } else if d.armor.as_ref().is_some_and(|a| matches!(a.slot, EquipSlot::Ring | EquipSlot::Ring2)) {
            ItemCat::Rings
        } else if id.starts_with("wand") || !m.spells.is_empty() {
            ItemCat::Wands
        } else if d.consumable && (!m.use_parts.is_empty() || ["potion", "oil", "philter", "elixir"].iter().any(|p| id.starts_with(p))) {
            ItemCat::Potions
        } else if !d.teaches.is_empty() {
            ItemCat::Scrolls
        } else {
            ItemCat::Wondrous
        };
    }
    if !d.teaches.is_empty() {
        return ItemCat::Scrolls;
    }
    if let Some(w) = &d.weapon {
        let ranged = w.ranged.is_some();
        return match (w.category, ranged) {
            (Some(WeaponCategory::Martial), false) => ItemCat::MartialMelee,
            (Some(WeaponCategory::Martial), true) => ItemCat::MartialRanged,
            (_, false) => ItemCat::SimpleMelee,
            (_, true) => ItemCat::SimpleRanged,
        };
    }
    if let Some(c) = d.armor.as_ref().and_then(|a| a.category) {
        match c {
            ArmorCategory::Light => return ItemCat::LightArmor,
            ArmorCategory::Medium => return ItemCat::MediumArmor,
            ArmorCategory::Heavy => return ItemCat::HeavyArmor,
            ArmorCategory::Shield => return ItemCat::Shields,
            ArmorCategory::Clothing => {}
        }
    }
    if is_ammo {
        ItemCat::Ammunition
    } else if !d.tool.is_empty() || rustpg2d_engine::srd::abilities::TOOLS.iter().any(|t| t.0 == srd) {
        ItemCat::Tools
    } else if MOUNTS_AND_VEHICLES.contains(&srd) {
        ItemCat::MountsVehicles
    } else if !srd.is_empty() {
        ItemCat::AdventuringGear
    } else {
        ItemCat::Other
    }
}

/// The filter picker: *All items*, the groups and their categories, with how many items each
/// has (`cats`: the categories of the items listed). Returns true when it changed.
pub fn picker(ui: &mut egui::Ui, id: &str, filter: &mut ItemFilter, cats: &[ItemCat]) -> bool {
    let count = |f: ItemFilter| cats.iter().filter(|c| f.keeps(**c)).count();
    let before = *filter;
    egui::ComboBox::from_id_salt(id).width(190.0).selected_text(format!("{} ({})", filter.label(), count(*filter))).show_ui(ui, |ui| {
        ui.selectable_value(filter, ItemFilter::All, format!("All items ({})", cats.len()));
        let mut last_group = None;
        for c in ItemCat::ALL {
            let n = count(ItemFilter::Cat(c));
            if c.group() != last_group {
                last_group = c.group();
                if let Some(g) = c.group() {
                    ui.separator();
                    ui.selectable_value(filter, ItemFilter::Group(g), egui::RichText::new(format!("{} ({})", g.label(), count(ItemFilter::Group(g)))).strong());
                } else {
                    ui.separator();
                }
            }
            if n == 0 {
                continue;
            }
            let indent = if c.group().is_some() { "    " } else { "" };
            ui.selectable_value(filter, ItemFilter::Cat(c), format!("{indent}{} ({n})", c.label()));
        }
    })
    .response
    .on_hover_text("The SRD's categories of equipment and magic items, worked out from what each item is; Other = the project's own things");
    *filter != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::defs::{Armor, Ranged, Weapon};
    use rustpg2d_engine::srd::magic_items::{ItemSpell, MagicItem};

    #[test]
    fn items_get_their_srd_category() {
        let none = HashSet::new();
        let mut sword = ItemDef::fresh("iron_sword");
        sword.weapon = Some(Weapon { category: Some(WeaponCategory::Martial), ..Weapon::melee(3, 0.4) });
        assert_eq!(category(&sword, &none), ItemCat::MartialMelee);
        let mut bow = ItemDef::fresh("bow");
        bow.weapon = Some(Weapon { category: Some(WeaponCategory::Simple), ..Weapon::ranged(2, 0.5, Ranged { projectile: Default::default(), speed: 300.0, range: 16.0, long_range: 0.0, ammo: "arrows".into() }) });
        assert_eq!(category(&bow, &none), ItemCat::SimpleRanged);
        let mut db = Database::default();
        db.items.push(bow);
        let ammo = ammo_of(&db);
        assert_eq!(category(&ItemDef::fresh("arrows"), &ammo), ItemCat::Ammunition);
        let mut mail = ItemDef::fresh("chainmail");
        mail.armor = Some(Armor { category: Some(ArmorCategory::Heavy), ..Armor::clothing(EquipSlot::Body) });
        assert_eq!(category(&mail, &none), ItemCat::HeavyArmor);
        // Magic: a ring, a wand, a potion, a +1 sword, a staff; scrolls teach.
        let magic = Some(MagicItem::default());
        let ring = ItemDef { armor: Some(Armor::clothing(EquipSlot::Ring)), magic: magic.clone(), ..ItemDef::fresh("ring-of-protection") };
        assert_eq!(category(&ring, &none), ItemCat::Rings);
        let wand = ItemDef { magic: Some(MagicItem { spells: vec![ItemSpell::default()], ..Default::default() }), ..ItemDef::fresh("wand-of-web") };
        assert_eq!(category(&wand, &none), ItemCat::Wands);
        let potion = ItemDef { consumable: true, magic: magic.clone(), ..ItemDef::fresh("potion-of-speed") };
        assert_eq!(category(&potion, &none), ItemCat::Potions);
        let plus = ItemDef { magic: magic.clone(), ..sword.clone() };
        assert_eq!(category(&plus, &none), ItemCat::MagicWeapons);
        let staff = ItemDef { id: "staff-of-fire".into(), ..plus.clone() };
        assert_eq!(category(&staff, &none), ItemCat::Staffs);
        assert_eq!(category(&ItemDef { magic, ..ItemDef::fresh("bag-of-holding") }, &none), ItemCat::Wondrous);
        assert_eq!(category(&ItemDef { teaches: "light".into(), ..ItemDef::fresh("scroll_light") }, &none), ItemCat::Scrolls);
        // Gear by the SRD index; the project's own things.
        assert_eq!(category(&ItemDef { srd: "thieves-tools".into(), ..ItemDef::fresh("tt") }, &none), ItemCat::Tools);
        assert_eq!(category(&ItemDef { srd: "warhorse".into(), ..ItemDef::fresh("w") }, &none), ItemCat::MountsVehicles);
        assert_eq!(category(&ItemDef { srd: "rope-hempen-50-feet".into(), ..ItemDef::fresh("r") }, &none), ItemCat::AdventuringGear);
        assert_eq!(category(&ItemDef { coin_cp: 100, ..ItemDef::fresh("coin") }, &none), ItemCat::Money);
        assert_eq!(category(&ItemDef::fresh("apple"), &none), ItemCat::Other);
    }

    #[test]
    fn filters_parse_and_keep_their_groups() {
        assert_eq!(ItemFilter::parse("magic"), Some(ItemFilter::Group(ItemGroup::Magic)));
        assert_eq!(ItemFilter::parse("wondrous"), Some(ItemFilter::Cat(ItemCat::Wondrous)));
        assert_eq!(ItemFilter::parse(""), Some(ItemFilter::All));
        assert!(ItemFilter::Group(ItemGroup::Weapons).keeps(ItemCat::MartialRanged) && !ItemFilter::Group(ItemGroup::Weapons).keeps(ItemCat::MagicWeapons));
        assert!(ItemCat::ALL.iter().all(|c| ItemFilter::parse(c.id()) == Some(ItemFilter::Cat(*c))));
    }
}
