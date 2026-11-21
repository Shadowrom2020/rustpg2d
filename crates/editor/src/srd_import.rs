//! The SRD 5.1 data import: races, subraces, their traits and choices, and backgrounds from a
//! checkout of 5e-bits/5e-database (`src/2014/en/5e-SRD-*.json`, the data behind the 5e-srd-api)
//! into a library's (or project's) `defs/races.ron` and `defs/backgrounds.ron`. The SRD's index
//! names are the ids (`hill-dwarf`), so running it again updates what it wrote; content fields
//! the SRD doesn't have (bodies, faction, size, colours, playable subraces) are kept. Run by
//! `cargo run -p rustpg2d-editor --example srd_import -- <checkout> [library]`.
//!
//! The SRD 5.1 is by Wizards of the Coast LLC, licensed under CC BY 4.0.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use rustpg2d_engine::backgrounds::BackgroundDef;
use rustpg2d_engine::classes::ClassDef;
use rustpg2d_engine::defs::CharacterDef;
use rustpg2d_engine::races::{ChoiceKind, RaceChoice, RaceDef, RaceSkill, RaceTrait};
use serde_json::Value;

/// The 5e-database commit the data is pinned to (updating it is a deliberate step with a diff).
pub const SRD_COMMIT: &str = "a6212beb4b278917c2cff41c2b04c4ef4f3e0d6f";

/// Traits that give hit points per level (they say so only in their text).
const HP_PER_LEVEL: [(&str, i32); 1] = [("dwarven-toughness", 1)];

/// Damage types the SRD's traits give resistance to (they say so only in their text).
const RESISTANCES: [(&str, &str); 2] = [("dwarven-resilience", "poison"), ("hellish-resistance", "fire")];

/// The JSON files of a checkout, by name (`Races`, `Traits`, …).
pub struct SrdData {
    races: Vec<Value>,
    subraces: Vec<Value>,
    traits: BTreeMap<String, Value>,
    backgrounds: Vec<Value>,
    monsters: Vec<Value>,
    classes: Vec<Value>,
    subclasses: Vec<Value>,
    levels: Vec<Value>,
    features: Vec<Value>,
    /// The spells (`5e-SRD-Spells.json`; empty when a checkout has none).
    spells: Vec<Value>,
    /// The equipment (`5e-SRD-Equipment.json`; empty when a checkout has none).
    equipment: Vec<Value>,
    /// The magic items (`5e-SRD-Magic-Items.json`; empty when a checkout has none).
    magic_items: Vec<Value>,
}

fn list(v: &Value, key: &str) -> Vec<Value> {
    v.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// `desc` paragraphs as one text.
fn desc(v: &Value) -> String {
    match v.get("desc") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "),
        Some(Value::String(t)) => t.clone(),
        _ => String::new(),
    }
}

/// A proficiency's id as the engine names it: skills without their `skill-` prefix.
fn prof_id(index: &str) -> String {
    index.strip_prefix("skill-").unwrap_or(index).to_string()
}

/// The option ids (and names) of an SRD option set (`{choose, from: {options: [{item}]}}`).
fn options(set: &Value) -> (u32, Vec<(String, String)>) {
    let choose = set.get("choose").and_then(Value::as_u64).unwrap_or(1) as u32;
    let opts = set
        .pointer("/from/options")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|o| o.get("item").or_else(|| o.get("ability_score"))).map(|i| (s(i, "index").to_string(), s(i, "name").to_string())).collect())
        .unwrap_or_default();
    (choose, opts)
}

impl SrdData {
    /// Reads the files of a checkout (`<dir>/src/2014/en/`).
    pub fn read(checkout: &Path) -> Result<SrdData> {
        let en = checkout.join("src/2014/en");
        let read = |name: &str| -> Result<Vec<Value>> {
            let p = en.join(format!("5e-SRD-{name}.json"));
            let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
            serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))
        };
        let traits = read("Traits")?.into_iter().map(|t| (s(&t, "index").to_string(), t)).collect();
        Ok(SrdData {
            races: read("Races")?,
            subraces: read("Subraces")?,
            traits,
            backgrounds: read("Backgrounds")?,
            monsters: read("Monsters")?,
            classes: read("Classes")?,
            subclasses: read("Subclasses")?,
            levels: read("Levels")?,
            features: read("Features")?,
            spells: read("Spells").unwrap_or_default(),
            equipment: read("Equipment").unwrap_or_default(),
            magic_items: read("Magic-Items").unwrap_or_default(),
        })
    }

    /// Traits, fixed proficiencies, resistances, darkvision and the choices of a list of trait
    /// references.
    fn apply_traits(&self, refs: &[Value], r: &mut RaceDef) {
        r.traits.clear();
        r.skills.clear();
        r.resistances.clear();
        r.darkvision_ft = 0;
        r.hp_per_level = 0;
        let mut choices = Vec::new();
        for t in refs.iter().filter_map(|t| self.traits.get(s(t, "index"))) {
            let id = s(t, "index");
            r.traits.push(RaceTrait { id: id.into(), name: s(t, "name").into(), description: desc(t), effects: trait_effects(id) });
            for p in list(t, "proficiencies") {
                let id = prof_id(s(&p, "index"));
                if !r.skills.iter().any(|x| x.skill == id) {
                    r.skills.push(RaceSkill { skill: id, level: 1 });
                }
            }
            if let Some((_, kind)) = RESISTANCES.iter().find(|(t, _)| *t == id) {
                r.resistances.push(kind.to_string());
            }
            if id == "darkvision" {
                r.darkvision_ft = 60;
            }
            if let Some((_, n)) = HP_PER_LEVEL.iter().find(|(t, _)| *t == id) {
                r.hp_per_level += n;
            }
            if let Some(set) = t.get("proficiency_choices") {
                let (choose, opts) = options(set);
                let kind = if opts.iter().all(|(o, _)| o.starts_with("skill-")) { ChoiceKind::Skill } else { ChoiceKind::Tool };
                choices.push(RaceChoice { id: id.into(), label: s(t, "name").into(), kind, choose, options: opts.iter().map(|(o, _)| prof_id(o)).collect(), ..Default::default() });
            }
            if let Some(set) = t.get("language_options") {
                let (choose, opts) = options(set);
                choices.push(RaceChoice { id: id.into(), label: s(t, "name").into(), kind: ChoiceKind::Language, choose, options: opts.into_iter().map(|(o, _)| o).collect(), ..Default::default() });
            }
            if let Some(set) = t.pointer("/trait_specific/subtrait_options") {
                let (choose, opts) = options(set);
                let mut c = RaceChoice { id: id.into(), label: s(t, "name").into(), kind: ChoiceKind::Ancestry, choose, ..Default::default() };
                for (o, name) in opts {
                    if let Some(dt) = self.traits.get(&o).and_then(|st| st.pointer("/trait_specific/damage_type/index")).and_then(Value::as_str) {
                        c.grants.insert(o.clone(), dt.to_string());
                    }
                    c.names.insert(o.clone(), name);
                    c.options.push(o);
                }
                choices.push(c);
            }
            if let Some(set) = t.pointer("/trait_specific/spell_options") {
                let (choose, opts) = options(set);
                let mut c = RaceChoice { id: id.into(), label: s(t, "name").into(), kind: ChoiceKind::Cantrip, choose, ..Default::default() };
                for (o, name) in opts {
                    c.names.insert(o.clone(), name);
                    c.options.push(o);
                }
                choices.push(c);
            }
        }
        // Choices of the race itself (ability increases, languages) come first.
        let old: Vec<RaceChoice> = std::mem::take(&mut r.choices);
        let own = old.iter().filter(|c| c.id.ends_with("-ability-increase") || c.id.ends_with("-language-choice")).cloned();
        // The colours a project gave a choice's options (a dragonborn's ancestry) stay.
        let choices = choices.into_iter().map(|mut c| {
            if let Some(o) = old.iter().find(|o| o.id == c.id) {
                c.colors = o.colors.clone();
            }
            c
        });
        r.choices = own.chain(choices).collect();
    }

    /// Writes the SRD's races, subraces and backgrounds into `races` / `backgrounds`: existing
    /// ones are updated (their content fields kept), new ones added.
    pub fn import(&self, races: &mut Vec<RaceDef>, backgrounds: &mut Vec<BackgroundDef>) -> (usize, usize) {
        let (mut added, mut updated) = (0, 0);
        let mut upsert = |races: &mut Vec<RaceDef>, id: &str| -> usize {
            match races.iter().position(|r| r.id == id) {
                Some(k) => {
                    updated += 1;
                    k
                }
                None => {
                    added += 1;
                    races.push(RaceDef { id: id.into(), ..Default::default() });
                    races.len() - 1
                }
            }
        };
        for j in &self.races {
            let id = s(j, "index");
            let k = upsert(races, id);
            let new = races[k].name.is_empty();
            let r = &mut races[k];
            r.name = s(j, "name").into();
            r.parent.clear();
            r.ability_bonuses = list(j, "ability_bonuses").iter().map(|b| (b.pointer("/ability_score/index").and_then(Value::as_str).unwrap_or("").to_string(), b.get("bonus").and_then(Value::as_i64).unwrap_or(0) as i32)).collect();
            r.walk_ft = j.get("speed").and_then(Value::as_u64).unwrap_or(30) as u32;
            r.speed = 0.0;
            // Hit points and armor class follow the SRD rules (0.27's race bonuses would count twice).
            r.max_hp = 0;
            r.defense = 0;
            r.size_category = s(j, "size").into();
            r.languages = list(j, "languages").iter().map(|l| s(l, "index").to_string()).collect();
            let subraces = !list(j, "subraces").is_empty();
            // A race with subraces is chosen as one of them.
            if subraces {
                r.playable = false;
            } else if new {
                r.playable = true;
            }
            if new && r.size_category == "Small" {
                r.size = 0.75;
            }
            let mut own = Vec::new();
            if let Some(set) = j.get("ability_bonus_options") {
                let (choose, opts) = options(set);
                own.push(RaceChoice { id: format!("{id}-ability-increase"), label: "Ability Score Increase".into(), kind: ChoiceKind::Ability, choose, options: opts.into_iter().map(|(o, _)| o).collect(), ..Default::default() });
            }
            if let Some(set) = j.get("language_options") {
                let (choose, opts) = options(set);
                own.push(RaceChoice { id: format!("{id}-language-choice"), label: "Extra Language".into(), kind: ChoiceKind::Language, choose, options: opts.into_iter().map(|(o, _)| o).collect(), ..Default::default() });
            }
            // (Choices the project gave colours to are handed on: apply_traits keeps their colours.)
            own.extend(r.choices.drain(..).filter(|c| !c.colors.is_empty()));
            r.choices = own;
            let traits = list(j, "traits");
            self.apply_traits(&traits, r);
        }
        for j in &self.subraces {
            let id = s(j, "index");
            let k = upsert(races, id);
            let new = races[k].name.is_empty();
            let r = &mut races[k];
            r.name = s(j, "name").into();
            r.parent = j.pointer("/race/index").and_then(Value::as_str).unwrap_or("").into();
            r.ability_bonuses = list(j, "ability_bonuses").iter().map(|b| (b.pointer("/ability_score/index").and_then(Value::as_str).unwrap_or("").to_string(), b.get("bonus").and_then(Value::as_i64).unwrap_or(0) as i32)).collect();
            r.description = desc(j);
            r.walk_ft = 0;
            r.speed = 0.0;
            r.max_hp = 0;
            r.defense = 0;
            r.languages.clear();
            if new {
                r.playable = true;
            }
            r.choices.retain(|c| !c.colors.is_empty());
            let traits = list(j, "racial_traits");
            self.apply_traits(&traits, r);
        }
        for j in &self.backgrounds {
            let id = s(j, "index");
            let b = match backgrounds.iter().position(|b| b.id == id) {
                Some(k) => {
                    updated += 1;
                    &mut backgrounds[k]
                }
                None => {
                    added += 1;
                    backgrounds.push(BackgroundDef::fresh(id));
                    backgrounds.last_mut().unwrap()
                }
            };
            b.name = s(j, "name").into();
            b.proficiencies = list(j, "starting_proficiencies").iter().map(|p| prof_id(s(p, "index"))).collect();
            b.language_choices = j.pointer("/language_options/choose").and_then(Value::as_u64).unwrap_or(0) as u32;
            b.feature = j.pointer("/feature/name").and_then(Value::as_str).unwrap_or("").into();
            b.feature_text = j.get("feature").map(desc).unwrap_or_default();
            // (Its equipment is the SRD's, which arrives with 0.31: the starting items stay the
            // project's.)
        }
        (added, updated)
    }
}

impl SrdData {
    /// The item id an SRD item or equipment category (`martial-weapons`) is given as: the library
    /// item standing for it, else its index; None when the data hasn't the item.
    fn item_id(&self, index: &str) -> Option<String> {
        let known = self.equipment.is_empty() || self.equipment.iter().any(|v| s(v, "index") == index);
        if let Some(id) = library_item(index).filter(|_| known) {
            return Some(id.to_string());
        }
        if let Some((_, item)) = CATEGORY_ITEM.iter().find(|(c, _)| *c == index) {
            return self.item_id(item).or_else(|| library_item(item).map(str::to_string));
        }
        self.equipment.iter().any(|v| s(v, "index") == index).then(|| index.to_string())
    }

    /// An SRD item as an item definition (its id: the index).
    fn equipment_def(&self, v: &Value) -> rustpg2d_engine::defs::ItemDef {
        use rustpg2d_engine::defs::*;
        let index = s(v, "index");
        let mut d = ItemDef::fresh(index);
        d.name = s(v, "name").into();
        d.icon = SpriteRef::new(gear::SHEET, gear_icon(v));
        d.srd = index.into();
        d.cost_cp = cost_cp(v);
        d.value = if d.cost_cp == 0 { 0 } else { (d.cost_cp.div_ceil(100)).max(1) };
        d.weight = v.get("weight").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let mut text = vec![desc(v)];
        let ft = |key: &str, k: &str| v.pointer(&format!("/{key}/{k}")).and_then(Value::as_u64).map(|n| n as u32);
        let cat = s(v.get("equipment_category").unwrap_or(&Value::Null), "index");
        d.max_stack = 1;
        match cat {
            "weapon" => {
                let props: Vec<String> = list(v, "properties").iter().map(|p| s(p, "index").to_string()).collect();
                let mut properties: Vec<WeaponProperty> = props.iter().filter_map(|p| WeaponProperty::parse(p)).collect();
                // (A lance needs both hands unless mounted: there are no mounts.)
                if index == "lance" && !properties.contains(&WeaponProperty::TwoHanded) {
                    properties.push(WeaponProperty::TwoHanded);
                }
                let dice = v.pointer("/damage/damage_dice").and_then(Value::as_str).unwrap_or("0").to_string();
                let sides = dice.split_once('d').and_then(|(_, n)| n.parse::<i32>().ok()).unwrap_or(4);
                let mut w = Weapon::melee(((sides - 2) / 2).max(1), 0.5);
                w.dice = dice;
                w.damage_type = v.pointer("/damage/damage_type/index").and_then(Value::as_str).and_then(rustpg2d_engine::srd::DamageType::parse);
                w.category = Some(if s(v, "weapon_category") == "Martial" { WeaponCategory::Martial } else { WeaponCategory::Simple });
                w.two_handed = properties.contains(&WeaponProperty::TwoHanded);
                w.versatile = v.pointer("/two_handed_damage/damage_dice").and_then(Value::as_str).unwrap_or("").into();
                if properties.contains(&WeaponProperty::Thrown) {
                    w.thrown_ft = ft("throw_range", "normal").map(|n| (n, ft("throw_range", "long").unwrap_or(n * 3)));
                }
                if properties.contains(&WeaponProperty::Reach) {
                    w.reach = 1.9;
                }
                if s(v, "weapon_range") == "Ranged" && index != "net" || properties.contains(&WeaponProperty::Ammunition) {
                    let ammo = match index {
                        _ if index.contains("crossbow") => "crossbow-bolt",
                        _ if index.contains("bow") => "arrow",
                        "sling" => "sling-bullet",
                        "blowgun" => "blowgun-needle",
                        _ => "",
                    };
                    let projectile = match ammo {
                        "arrow" => SpriteRef::new("items", 9),
                        "sling-bullet" => SpriteRef::new(gear::SHEET, gear::BULLET),
                        _ if ammo.is_empty() => d.icon.clone(),
                        _ => SpriteRef::new("items", 10),
                    };
                    let (normal, long) = (ft("range", "normal").unwrap_or(30), ft("range", "long").unwrap_or(0));
                    w.ranged = Some(Ranged {
                        projectile,
                        speed: 480.0,
                        range: normal as f32 / 5.0,
                        long_range: long as f32 / 5.0,
                        ammo: if ammo.is_empty() { String::new() } else { self.item_id(ammo).unwrap_or_else(|| ammo.to_string()) },
                    });
                    w.cooldown = 0.6;
                }
                w.properties = properties;
                text.push(list(v, "special").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "));
                d.weapon = Some(w);
            }
            "armor" => {
                let category = match s(v, "armor_category") {
                    "Light" => ArmorCategory::Light,
                    "Medium" => ArmorCategory::Medium,
                    "Heavy" => ArmorCategory::Heavy,
                    _ => ArmorCategory::Shield,
                };
                let dex = v.pointer("/armor_class/dex_bonus").and_then(Value::as_bool).unwrap_or(false);
                let max = v.pointer("/armor_class/max_bonus").and_then(Value::as_i64).map(|n| n as i32);
                d.armor = Some(Armor {
                    category: Some(category),
                    base_ac: v.pointer("/armor_class/base").and_then(Value::as_i64).unwrap_or(10) as i32,
                    dex_max: match (category, dex) {
                        (ArmorCategory::Shield, _) => None,
                        (_, false) => Some(0),
                        (_, true) => max,
                    },
                    str_min: v.get("str_minimum").and_then(Value::as_i64).unwrap_or(0) as i32,
                    stealth_disadvantage: v.get("stealth_disadvantage").and_then(Value::as_bool).unwrap_or(false),
                    ..Armor::clothing(if category == ArmorCategory::Shield { EquipSlot::Offhand } else { EquipSlot::Body })
                });
            }
            "mounts-and-vehicles" => {
                let mut extra = Vec::new();
                if let (Some(n), Some(u)) = (v.pointer("/speed/quantity").and_then(Value::as_f64), v.pointer("/speed/unit").and_then(Value::as_str)) {
                    extra.push(format!("Speed {n} {u}."));
                }
                if let Some(c) = v.get("capacity").and_then(Value::as_str) {
                    extra.push(format!("Carrying capacity {c}."));
                }
                text.push(extra.join(" "));
            }
            _ => {
                let gear_cat = v.pointer("/gear_category/index").and_then(Value::as_str).unwrap_or("");
                d.max_stack = match gear_cat {
                    "ammunition" => 50,
                    "equipment-packs" => 1,
                    _ if cat == "tools" => 1,
                    _ => 10,
                };
                // Ammunition is sold in bundles (20 arrows for 1 gp): the cost is for one.
                if let Some(q) = v.get("quantity").and_then(Value::as_u64).filter(|q| *q > 1) {
                    d.cost_cp = (d.cost_cp / q as u32).max(1);
                    d.weight /= q as f32;
                    d.value = 1;
                }
                let light = match index {
                    "torch" => Some(LightSpec::new(4.0, [255, 190, 120], true)),
                    "candle" => Some(LightSpec::new(2.0, [255, 200, 140], true)),
                    "lamp" => Some(LightSpec::new(5.0, [255, 210, 150], true)),
                    "lantern-hooded" => Some(LightSpec::new(5.5, [255, 220, 170], false)),
                    "lantern-bullseye" => Some(LightSpec::new(6.5, [255, 225, 180], false)),
                    _ => None,
                };
                if light.is_some() {
                    d.light = light;
                    d.armor = Some(Armor::clothing(EquipSlot::Offhand));
                }
                // A healer's kit's ten uses (first aid at 0 HP).
                if index == "healers-kit" {
                    (d.uses, d.max_stack) = (10, 1);
                }
                if index == "backpack" {
                    d.armor = Some(Armor::clothing(EquipSlot::Back));
                    d.bag_slots = 6;
                }
                // A pack: using it unpacks what it holds.
                let contents = list(v, "contents");
                if !contents.is_empty() {
                    let mut actions = Vec::new();
                    let mut names = Vec::new();
                    for c in &contents {
                        let i = c.pointer("/item/index").and_then(Value::as_str).unwrap_or("");
                        let n = c.get("quantity").and_then(Value::as_u64).unwrap_or(1) as u32;
                        let Some(item) = self.item_id(i) else { continue };
                        let name = c.pointer("/item/name").and_then(Value::as_str).unwrap_or(i);
                        names.push(if n > 1 { format!("{n} × {name}") } else { name.to_string() });
                        actions.push(rustpg2d_engine::events::Action::GiveItem { item, count: n });
                    }
                    d.consumable = true;
                    d.on_use = vec![rustpg2d_engine::events::Trigger { actions, ..rustpg2d_engine::events::Trigger::new(rustpg2d_engine::events::TriggerOn::UseItem { item: String::new() }) }];
                    text.push(format!("Use it to unpack: {}.", names.join(", ")));
                }
            }
        }
        d.description = text.into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        d
    }

    /// The SRD's equipment and the five coins into `items`. Library items standing for an SRD
    /// item (`STANDS_FOR`) and items of an earlier run (`srd` set) get its numbers and keep their
    /// own name, text and look; the `coin` becomes the gold piece. New items are added when `add`
    /// (a library), else only the coins, those in `wanted` (a project's class equipment) and what
    /// packs among them hold. Returns (added, updated).
    pub fn import_equipment(&self, items: &mut Vec<rustpg2d_engine::defs::ItemDef>, add: bool, wanted: &[String]) -> (usize, usize) {
        use rustpg2d_engine::defs::*;
        let mut defs: Vec<ItemDef> = self.equipment.iter().map(|v| self.equipment_def(v)).collect();
        // References to a standing library item the project lacks name the SRD item itself.
        let absent: BTreeMap<&str, &str> = STANDS_FOR.iter().filter(|(_, ids)| !items.iter().any(|d| d.id == ids[0])).map(|(i, ids)| (ids[0], *i)).collect();
        for d in &mut defs {
            if let Some(r) = d.weapon.as_mut().and_then(|w| w.ranged.as_mut()) {
                if let Some(i) = absent.get(r.ammo.as_str()) {
                    r.ammo = i.to_string();
                }
            }
            for a in d.on_use.iter_mut().flat_map(|t| t.actions.iter_mut()) {
                if let rustpg2d_engine::events::Action::GiveItem { item, .. } = a {
                    if let Some(i) = absent.get(item.as_str()) {
                        *item = i.to_string();
                    }
                }
            }
        }
        for (id, name, text, worth, icon) in COINS {
            let mut d = ItemDef::fresh(id);
            (d.name, d.description) = (name.into(), text.into());
            d.icon = SpriteRef::new(gear::SHEET, icon);
            (d.coin_cp, d.max_stack, d.weight) = (worth, 999, 0.02);
            defs.push(d);
        }
        // What a project needs: the wanted items and the packs' contents among them.
        let mut need: std::collections::BTreeSet<String> = wanted.iter().cloned().collect();
        loop {
            let more: Vec<String> = defs.iter().filter(|d| need.contains(&d.id)).flat_map(|d| d.on_use.iter().flat_map(|t| t.actions.iter())).filter_map(|a| match a {
                rustpg2d_engine::events::Action::GiveItem { item, .. } => Some(item.clone()),
                _ => None,
            }).filter(|i| !need.contains(i)).collect();
            if more.is_empty() {
                break;
            }
            need.extend(more);
        }
        let (mut added, mut updated) = (0, 0);
        for new in defs {
            let standing: &[&str] = STANDS_FOR.iter().find(|(i, _)| *i == new.srd).map_or(&[], |(_, ids)| *ids);
            let mut found = false;
            for old in items.iter_mut().filter(|d| d.id == new.id || standing.contains(&d.id.as_str()) || (!new.srd.is_empty() && d.srd == new.srd && d.magic.is_none() && !d.silvered)) {
                found = true;
                let before = old.clone();
                merge_item(old, &new);
                if *old != before {
                    updated += 1;
                }
            }
            // (A standing item's id is what references use: it is never added under the index.)
            let id_taken = standing.iter().any(|s| items.iter().any(|d| d.id == *s));
            if !found && !id_taken && (add || new.coin_cp > 0 || need.contains(&new.id)) {
                items.push(new);
                added += 1;
            }
        }
        (added, updated)
    }

    /// The SRD's NPC humanoids as stat block templates (not the lycanthropes' forms).
    pub fn npc_templates(&self) -> Vec<rustpg2d_engine::srd::StatBlock> {
        self.monsters.iter().filter(|m| s(m, "type") == "humanoid" && !s(m, "index").starts_with("were")).map(monster_block).collect()
    }

    /// The ids of every SRD monster (`rules.ron`).
    pub fn monster_ids(&self) -> Vec<String> {
        self.monsters.iter().map(|m| s(m, "index").to_string()).collect()
    }

    /// Every SRD monster as a character definition in `chars`: a new one per monster (`add`), and
    /// the SRD's data for those already there: a definition of the monster's id without a block
    /// of the SRD's (a hand-made wolf) takes the whole block, its HP and XP; any character with
    /// an SRD monster's block keeps its numbers (a tougher orc chief) and only gains what it lacks
    /// (size, type, speeds, senses, features, its attacks' other damage and riders). Returns
    /// (added, updated).
    pub fn import_monsters(&self, chars: &mut Vec<CharacterDef>, add: bool) -> (usize, usize) {
        let blocks: BTreeMap<String, rustpg2d_engine::srd::StatBlock> = self.monsters.iter().map(|m| (s(m, "index").to_string(), monster_block(m))).collect();
        let (mut added, mut updated) = (0, 0);
        for c in chars.iter_mut() {
            let before = c.clone();
            let own = c.stats.as_ref().map(|b| b.template.clone()).filter(|t| blocks.contains_key(t));
            match (own, blocks.get(&c.id)) {
                (Some(t), _) => {
                    let src = &blocks[&t];
                    let b = c.stats.as_mut().unwrap();
                    b.size = src.size;
                    b.creature_type = src.creature_type.clone();
                    b.speeds = src.speeds;
                    b.senses = src.senses.clone();
                    b.features = src.features.clone();
                    if b.attacks.is_empty() {
                        b.attacks = src.attacks.clone();
                        b.multiattack = src.multiattack;
                    }
                    // Its spells (0.32's AI casts them).
                    b.spellcasting = src.spellcasting.clone();
                    // Its save actions, legendary actions and traits that act (0.32).
                    (b.actions, b.legendary, b.legendary_points, b.legendary_resistance) = (src.actions.clone(), src.legendary.clone(), src.legendary_points, src.legendary_resistance);
                    (b.regeneration, b.pack_tactics) = (src.regeneration.clone(), src.pack_tactics);
                    // Its attacks keep their numbers and gain the SRD attack's other damage and
                    // what a hit does (0.32), unless they have their own (a goblin poisoner's arrows).
                    for a in &mut b.attacks {
                        if let Some(x) = src.attacks.iter().find(|x| x.name == a.name) {
                            if a.extra.is_empty() {
                                a.extra = x.extra.clone();
                            }
                            if a.on_hit.is_empty() {
                                a.on_hit = x.on_hit.clone();
                            }
                            a.versatile = x.versatile.clone();
                        }
                    }
                }
                (None, Some(src)) => {
                    c.stats = Some(src.clone());
                    if c.max_hp > 0 {
                        c.max_hp = src.hp;
                        c.xp = src.xp;
                    }
                }
                (None, None) => {}
            }
            if *c != before {
                updated += 1;
            }
        }
        if !add {
            return (added, updated);
        }
        for m in &self.monsters {
            let id = s(m, "index");
            if chars.iter().any(|c| c.id == id) {
                continue;
            }
            chars.push(monster_character(m, &blocks[id]));
            added += 1;
        }
        (added, updated)
    }
}

impl SrdData {
    /// The SRD's classes with their subclasses, levels and features into `classes`: existing ones
    /// (by id) are replaced but keep the content fields the SRD doesn't have (description,
    /// equipment a project mapped by hand); new ones added. Returns (added, updated) and the SRD
    /// items the starting equipment had that no library item stands for.
    pub fn import_classes(&self, classes: &mut Vec<ClassDef>) -> (usize, usize, Vec<String>) {
        let (mut added, mut updated, mut unmapped) = (0, 0, Vec::new());
        for c in &self.classes {
            let id = s(c, "index").to_string();
            let mut def = self.class_def(c, &mut unmapped);
            match classes.iter_mut().find(|d| d.id == id) {
                Some(old) => {
                    if !old.description.is_empty() {
                        def.description = old.description.clone();
                    }
                    if *old != def {
                        *old = def;
                        updated += 1;
                    }
                }
                None => {
                    classes.push(def);
                    added += 1;
                }
            }
        }
        unmapped.sort();
        unmapped.dedup();
        (added, updated, unmapped)
    }

    fn class_def(&self, c: &Value, unmapped: &mut Vec<String>) -> ClassDef {
        use rustpg2d_engine::classes::{ClassLevel, SubclassDef};
        let id = s(c, "index");
        let profs = list(c, "proficiencies");
        let (choose, options) = list(c, "proficiency_choices").first().map(options).unwrap_or((0, Vec::new()));
        let saves = list(c, "saving_throws").iter().map(|a| s(a, "index").to_string()).collect();
        let proficiencies = profs.iter().map(|p| s(p, "index")).filter(|p| !p.starts_with("saving-throw-")).map(prof_id).collect();
        // Levels 1-20 of the class (not a subclass's), with the class's numbers flattened.
        let mut levels: Vec<ClassLevel> = (0..20).map(|_| ClassLevel::default()).collect();
        for l in self.levels.iter().filter(|l| l.pointer("/class/index").and_then(Value::as_str) == Some(id) && l.get("subclass").is_none()) {
            let n = l.get("level").and_then(Value::as_u64).unwrap_or(1).clamp(1, 20) as usize;
            let lv = &mut levels[n - 1];
            lv.features = list(l, "features").iter().map(|f| s(f, "index").to_string()).collect();
            for (key, prefix) in [("class_specific", ""), ("spellcasting", "spellcasting")] {
                if let Some(o) = l.get(key) {
                    flatten_numbers(prefix, o, &mut lv.numbers);
                }
            }
        }
        let mut subclasses = Vec::new();
        let mut subclass_level = 0;
        let mut subclass_title = String::new();
        for sc in self.subclasses.iter().filter(|x| x.pointer("/class/index").and_then(Value::as_str) == Some(id)) {
            let sid = s(sc, "index");
            let mut lv: Vec<(u32, Vec<String>)> = self
                .levels
                .iter()
                .filter(|l| l.pointer("/subclass/index").and_then(Value::as_str) == Some(sid))
                .map(|l| (l.get("level").and_then(Value::as_u64).unwrap_or(1) as u32, list(l, "features").iter().map(|f| s(f, "index").to_string()).collect()))
                .collect();
            lv.sort_by_key(|(l, _)| *l);
            subclass_title = s(sc, "subclass_flavor").to_string();
            subclasses.push(SubclassDef { id: sid.into(), name: s(sc, "name").into(), description: desc(sc), levels: lv });
        }
        // The level the subclass is chosen at: the class feature that names it ("primal-path").
        let flavor = subclass_title.to_ascii_lowercase().replace(' ', "-");
        if let Some((k, _)) = levels.iter().enumerate().find(|(_, l)| l.features.iter().any(|f| f.ends_with(&flavor) || *f == flavor)) {
            subclass_level = k as u32 + 1;
        }
        if subclass_level == 0 {
            subclass_level = subclasses.iter().flat_map(|s| s.levels.iter().map(|(l, _)| *l)).min().unwrap_or(3);
        }
        let features = self.features.iter().filter(|f| f.pointer("/class/index").and_then(Value::as_str) == Some(id)).map(class_feature).collect();
        // Starting equipment: the fixed items and the first option of every choice.
        let mut equipment = Vec::new();
        let mut add = |index: &str, count: u32, equipment: &mut Vec<rustpg2d_engine::inventory::ItemStack>| match self.item_id(index) {
            Some(item) => match equipment.iter_mut().find(|i: &&mut rustpg2d_engine::inventory::ItemStack| i.item == item) {
                Some(st) => st.count += count,
                None => equipment.push(rustpg2d_engine::inventory::ItemStack::new(&item, count)),
            },
            None => unmapped.push(index.to_string()),
        };
        for e in list(c, "starting_equipment") {
            add(e.pointer("/equipment/index").and_then(Value::as_str).unwrap_or(""), e.get("quantity").and_then(Value::as_u64).unwrap_or(1) as u32, &mut equipment);
        }
        for set in list(c, "starting_equipment_options") {
            if let Some(first) = set.pointer("/from/options/0") {
                for (index, count) in option_items(first) {
                    add(&index, count, &mut equipment);
                }
            }
        }
        // A caster without a focus among its gear gets one (the SRD offers it in a choice whose
        // first option is something else): its material components need it.
        if c.get("spellcasting").is_some() && !equipment.iter().any(|i| rustpg2d_engine::srd::equipment::FOCUS.contains(&i.item.as_str())) {
            let focus = match id {
                "cleric" | "paladin" => "amulet",
                "druid" => "sprig-of-mistletoe",
                _ => "component-pouch",
            };
            add(focus, 1, &mut equipment);
        }
        let multiclass = c
            .pointer("/multi_classing/prerequisites")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|p| (p.pointer("/ability_score/index").and_then(Value::as_str).unwrap_or("").to_string(), p.get("minimum_score").and_then(Value::as_i64).unwrap_or(13) as i32)).collect())
            .unwrap_or_default();
        ClassDef {
            id: id.into(),
            name: s(c, "name").into(),
            description: String::new(),
            hit_die: c.get("hit_die").and_then(Value::as_u64).unwrap_or(8) as u32,
            saves,
            proficiencies,
            skill_choices: choose,
            skill_options: options.iter().map(|(o, _)| prof_id(o)).collect(),
            equipment,
            multiclass,
            subclass_level,
            subclass_title,
            levels,
            subclasses,
            features,
            spellcasting: class_spellcasting(c),
        }
    }

    /// Imports the SRD's spells into `spells`: new ones added, existing ones (by id) updated but
    /// keeping their icon and look. Returns (added, updated, the story-only ids).
    pub fn import_spells(&self, spells: &mut Vec<rustpg2d_engine::skills::SpellDef>) -> (usize, usize, Vec<String>) {
        let (mut added, mut updated, mut story) = (0, 0, Vec::new());
        for v in &self.spells {
            let mut def = spell_def(v);
            if def.story_only {
                story.push(def.id.clone());
            }
            match spells.iter_mut().find(|d| d.id == def.id) {
                Some(old) => {
                    def.icon = old.icon.clone();
                    def.visual = old.visual.clone();
                    if *old != def {
                        *old = def;
                        updated += 1;
                    }
                }
                None => {
                    spells.push(def);
                    added += 1;
                }
            }
        }
        (added, updated, story)
    }
}

/// A class's spellcasting: its ability from the data, how it has its spells by the SRD's rules.
fn class_spellcasting(c: &Value) -> Option<rustpg2d_engine::classes::Spellcasting> {
    use rustpg2d_engine::classes::{CasterKind as K, Spellcasting};
    let ability = rustpg2d_engine::srd::Ability::parse(c.pointer("/spellcasting/spellcasting_ability/index").and_then(Value::as_str)?)?;
    let (kind, half, ritual) = match s(c, "index") {
        "cleric" | "druid" => (K::Prepared, false, true),
        "paladin" => (K::Prepared, true, false),
        "wizard" => (K::Spellbook, false, true),
        "warlock" => (K::Pact, false, false),
        "bard" => (K::Known, false, true),
        _ => (K::Known, false, false),
    };
    Some(Spellcasting { ability, kind, half, ritual })
}

/// A duration as rounds of 6 seconds ("Up to 1 minute" = 10; instantaneous and permanent = 0).
fn duration_rounds(text: &str) -> u32 {
    let t = text.to_ascii_lowercase();
    let n: u32 = t.split_whitespace().find_map(|w| w.parse().ok()).unwrap_or(1);
    let per = if t.contains("round") {
        1
    } else if t.contains("minute") {
        10
    } else if t.contains("hour") {
        600
    } else if t.contains("day") {
        14_400
    } else {
        0
    };
    n * per
}

/// An SRD spell as a definition: its fields, its mechanics from the data and [`spell_extras`].
fn spell_def(v: &Value) -> rustpg2d_engine::skills::SpellDef {
    use rustpg2d_engine::skills::{Area, Shape, SpellCast, SpellDef, SpellPart as P, SpellRange};
    let mut d = SpellDef::fresh(s(v, "index"));
    d.name = s(v, "name").into();
    let mut text = desc(v);
    if let Some(h) = v.get("higher_level").and_then(Value::as_array).filter(|a| !a.is_empty()) {
        text += &format!(" At Higher Levels. {}", h.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "));
    }
    d.description = text;
    d.level = v.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
    d.school = v.pointer("/school/index").and_then(Value::as_str).unwrap_or("").into();
    d.casting = match s(v, "casting_time") {
        "1 action" => SpellCast::Action,
        "1 bonus action" => SpellCast::Bonus,
        "1 reaction" => SpellCast::Reaction,
        t => SpellCast::Minutes((duration_rounds(t) / 10).max(1)),
    };
    let range = s(v, "range");
    d.range = match range {
        "Self" => SpellRange::Caster,
        "Touch" => SpellRange::Touch,
        r if r.ends_with(" feet") => SpellRange::Feet(r.trim_end_matches(" feet").trim().parse().unwrap_or(30)),
        _ => SpellRange::Sight,
    };
    d.area = v.get("area_of_effect").map(|a| {
        let shape = match s(a, "type") {
            "cube" => Shape::Cube,
            "cone" => Shape::Cone,
            "line" => Shape::Line,
            "cylinder" => Shape::Cylinder,
            _ => Shape::Sphere,
        };
        Area { shape, feet: a.get("size").and_then(Value::as_u64).unwrap_or(5) as u32 }
    });
    d.components = list(v, "components").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ");
    d.material = s(v, "material").into();
    d.ritual = v.get("ritual").and_then(Value::as_bool).unwrap_or(false);
    d.concentration = v.get("concentration").and_then(Value::as_bool).unwrap_or(false);
    d.duration = s(v, "duration").into();
    d.duration_rounds = duration_rounds(&d.duration);
    d.classes = list(v, "classes").iter().map(|c| s(c, "index").to_string()).collect();
    // The mechanics the data gives: an attack or a save, damage and healing by level.
    let by_level = |o: Option<&Value>| -> Vec<(u32, String)> {
        let mut out: Vec<(u32, String)> = o.and_then(Value::as_object).map(|m| m.iter().filter_map(|(k, x)| Some((k.parse().ok()?, x.as_str()?.to_string()))).collect()).unwrap_or_default();
        out.sort();
        out
    };
    let mut parts = Vec::new();
    match s(v, "attack_type") {
        "ranged" => parts.push(P::Attack { ranged: true }),
        "melee" => parts.push(P::Attack { ranged: false }),
        _ => {}
    }
    if let Some(dc) = v.get("dc") {
        if let Some(a) = dc.pointer("/dc_type/index").and_then(Value::as_str).and_then(rustpg2d_engine::srd::Ability::parse) {
            parts.push(P::Save { ability: a, half: s(dc, "dc_success") == "half" });
        }
    }
    if let Some(dmg) = list(v, "damage").first() {
        let by = by_level(dmg.get("damage_at_slot_level").or_else(|| dmg.get("damage_at_character_level")));
        if !by.is_empty() {
            let damage_type = dmg.pointer("/damage_type/index").and_then(Value::as_str).and_then(rustpg2d_engine::srd::DamageType::parse);
            parts.push(P::Damage { by_level: by, damage_type });
        }
    }
    let heal = by_level(v.get("heal_at_slot_level"));
    if !heal.is_empty() {
        parts.push(P::Heal { by_level: heal });
    }
    parts.extend(spell_extras(s(v, "index")));
    // Damage without an attack or a save in the data: Magic Missile's darts hit; Call Lightning and
    // Flaming Sphere are DEX saves for half, Scorching Ray a ranged attack (one ray); the rest is a
    // weapon's rider, a smite or Sleep's hit point pool, not damage (left out).
    let index = s(v, "index");
    let damaging = parts.iter().any(|p| matches!(p, P::Damage { .. }));
    if damaging && !parts.iter().any(|p| matches!(p, P::Attack { .. } | P::Save { .. })) {
        match index {
            "magic-missile" | "spike-growth" => {}
            "call-lightning" | "flaming-sphere" => parts.insert(0, P::Save { ability: rustpg2d_engine::srd::Ability::Dex, half: true }),
            "scorching-ray" => parts.insert(0, P::Attack { ranged: true }),
            _ => parts.retain(|p| !matches!(p, P::Damage { .. })),
        }
    }
    // Costly components (10 gp or more) of the spells that act: library items, used up.
    let costly = |item: &str, count: u32| Some(rustpg2d_engine::skills::Costly { item: item.into(), count });
    d.costly = match index {
        "revivify" => costly("diamond", 3),
        "raise-dead" => costly("diamond", 5),
        "resurrection" => costly("diamond", 10),
        "greater-restoration" | "stoneskin" => costly("diamond-dust", 4),
        "continual-flame" => costly("ruby-dust", 1),
        "warding-bond" => costly("platinum-ring", 2),
        "find-familiar" => costly("incense", 1),
        "true-seeing" => costly("eye-ointment", 1),
        "identify" => costly("pearl", 1),
        _ => d.costly.take(),
    };
    // (Misty Step's 30 feet are the teleport's, the data's range is the caster.)
    if index == "misty-step" {
        d.range = rustpg2d_engine::skills::SpellRange::Feet(30);
    }
    // (Light's save is for an object a foe holds: lit by the caster, there is none.)
    if s(v, "index") == "light" {
        parts.retain(|p| !matches!(p, P::Save { .. }));
    }
    // (A save alone, without anything it keeps off, does nothing: the spell is the story's.)
    let acts = parts.iter().any(|p| !matches!(p, P::Save { .. } | P::Targets { .. }));
    d.story_only = !acts;
    if d.story_only {
        parts.clear();
    }
    d.parts = parts;
    d
}

/// What a racial trait does (0.32), by trait index; empty = its text only.
fn trait_effects(index: &str) -> Vec<rustpg2d_engine::classes::FeatureEffect> {
    use rustpg2d_engine::classes::{ActCost, ActEffect, FeatureEffect as F, Recharge};
    use rustpg2d_engine::srd::conditions::Condition as K;
    use rustpg2d_engine::srd::Ability;
    let resource = |id: &str, recharge: Recharge| F::Resource { id: id.into(), count: 1, number: String::new(), ability: None, per_level: 0, recharge };
    match index {
        "lucky" => vec![F::Lucky],
        "brave" => vec![F::SaveAdvantageAgainst { conditions: vec![K::Frightened] }],
        "fey-ancestry" => vec![F::SaveAdvantageAgainst { conditions: vec![K::Charmed] }, F::NoMagicSleep],
        "gnome-cunning" => vec![F::MagicSaves { abilities: vec![Ability::Int, Ability::Wis, Ability::Cha] }],
        "halfling-nimbleness" => vec![F::Nimble],
        "relentless-endurance" => vec![resource("relentless-endurance", Recharge::LongRest), F::Endurance { resource: "relentless-endurance".into() }],
        "savage-attacks" => vec![F::BrutalCritical { dice: 1 }],
        "breath-weapon" => vec![
            resource("breath-weapon", Recharge::ShortRest),
            F::Act {
                id: "breath-weapon".into(),
                name: "Breath Weapon".into(),
                cost: ActCost::Action,
                resource: "breath-weapon".into(),
                effect: ActEffect::Breath { by_level: vec![(1, "2d6".into()), (6, "3d6".into()), (11, "4d6".into()), (16, "5d6".into())] },
            },
        ],
        _ => Vec::new(),
    }
}

/// What the SRD data doesn't say as numbers: conditions, buffs, targets, light, unlocking and
/// temporary hit points of the spells the engine can do (by spell index).
fn spell_extras(index: &str) -> Vec<rustpg2d_engine::skills::SpellPart> {
    use rustpg2d_engine::skills::SpellPart as P;
    use rustpg2d_engine::srd::conditions::{Buff as B, Condition as K};
    let cond = |c: K| P::Condition { condition: c, on_fail: true };
    let always = |c: K| P::Condition { condition: c, on_fail: false };
    let buff = |b: B| P::Buff { buff: b };
    use rustpg2d_engine::skills::Lasting as L;
    let lasting = |effect: L| P::Lasting { effect };
    match index {
        // (Magic Missile's darts: the data's dice are all of them together, at one target.)
        "bless" => vec![P::Targets { count: 3, per_slot: 1 }, buff(B::AttackDie { dice: "1d4".into(), minus: false }), buff(B::SaveDie { dice: "1d4".into(), minus: false })],
        "bane" => vec![P::Targets { count: 3, per_slot: 1 }, buff(B::AttackDie { dice: "1d4".into(), minus: true }), buff(B::SaveDie { dice: "1d4".into(), minus: true })],
        "resistance" => vec![buff(B::SaveDie { dice: "1d4".into(), minus: false })],
        "shield-of-faith" => vec![buff(B::Ac { bonus: 2 })],
        "shield" => vec![buff(B::Ac { bonus: 5 })],
        "mage-armor" => vec![buff(B::AcBase { base: 13 })],
        "longstrider" => vec![buff(B::Speed { feet: 10 })],
        "haste" => vec![buff(B::Haste)],
        "slow" => vec![buff(B::Speed { feet: -15 })],
        "stoneskin" => vec![buff(B::Resistance { types: vec!["bludgeoning".into(), "piercing".into(), "slashing".into()] })],
        "ray-of-frost" => vec![buff(B::Speed { feet: -10 })],
        "hold-person" | "hold-monster" => vec![cond(K::Paralyzed)],
        "command" => vec![cond(K::Prone)],
        "hideous-laughter" => vec![cond(K::Prone), cond(K::Incapacitated)],
        "blindness-deafness" => vec![cond(K::Blinded)],
        "entangle" | "web" => vec![cond(K::Restrained), lasting(L { repeat: true, difficult: true, ..Default::default() })],
        "fear" => vec![cond(K::Frightened)],
        "charm-person" => vec![cond(K::Charmed)],
        "power-word-stun" => vec![always(K::Stunned)],
        "animal-friendship" | "dominate-beast" | "dominate-person" | "suggestion" => vec![cond(K::Charmed)],
        "hypnotic-pattern" => vec![cond(K::Charmed), cond(K::Incapacitated)],
        "banishment" | "confusion" => vec![cond(K::Incapacitated)],
        "stinking-cloud" => vec![cond(K::Incapacitated), lasting(L { repeat: true, obscures: true, ..Default::default() })],
        "grease" => vec![cond(K::Prone), lasting(L { repeat: true, difficult: true, ..Default::default() })],
        "sleet-storm" => vec![cond(K::Prone), lasting(L { repeat: true, obscures: true, difficult: true, ..Default::default() })],
        "protection-from-poison" => vec![buff(B::Resistance { types: vec!["poison".into()] })],
        "magic-weapon" => vec![buff(B::AttackDie { dice: "1".into(), minus: false })],
        "invisibility" | "greater-invisibility" => vec![always(K::Invisible)],
        "mass-cure-wounds" | "mass-healing-word" | "prayer-of-healing" => vec![P::Targets { count: 6, per_slot: 0 }],
        "light" | "continual-flame" => vec![P::Light { radius: 4.0 }],
        "dancing-lights" => vec![P::Light { radius: 2.0 }],
        "daylight" => vec![P::Light { radius: 12.0 }],
        "knock" => vec![P::Unlock],
        "spare-the-dying" => vec![P::Stabilize],
        "false-life" => vec![P::TempHp { by_level: (1..=9).map(|l| (l, format!("1d4+{}", 4 + 5 * (l - 1)))).collect() }],
        "aid" => vec![P::TempHp { by_level: (2..=9).map(|l| (l, format!("{}", 5 * (l - 1)))).collect() }],
        "heroism" => vec![P::TempHp { by_level: vec![(1, "MOD".into())] }],
        // (0.32: teleports, hit point pools, damage on hits, checks and AC.)
        "misty-step" | "dimension-door" => vec![P::Teleport],
        "sleep" => vec![P::HpPool { by_level: (1..=9).map(|l| (l, format!("{}d8", 5 + 2 * (l - 1)))).collect(), condition: K::Unconscious }],
        "color-spray" => vec![P::HpPool { by_level: (1..=9).map(|l| (l, format!("{}d10", 6 + 2 * (l - 1)))).collect(), condition: K::Blinded }],
        "hunters-mark" => vec![P::Mark { buff: B::HitDamage { dice: "1d6".into(), damage_type: None, mark: String::new(), once: false } }],
        "divine-favor" => vec![buff(B::HitDamage { dice: "1d4".into(), damage_type: Some(rustpg2d_engine::srd::DamageType::Radiant), mark: String::new(), once: false })],
        "branding-smite" => vec![buff(B::HitDamage { dice: "2d6".into(), damage_type: Some(rustpg2d_engine::srd::DamageType::Radiant), mark: String::new(), once: true })],
        "shillelagh" => vec![buff(B::WeaponDie { dice: "1d8".into() })],
        "guidance" => vec![buff(B::CheckDie { dice: "1d4".into() })],
        "enhance-ability" => vec![buff(B::CheckAdvantage { ability: None })],
        "true-strike" => vec![P::Mark { buff: B::NextAttack { mark: String::new() } }],
        "beacon-of-hope" => vec![P::Targets { count: 6, per_slot: 0 }, buff(B::Hope)],
        "barkskin" => vec![buff(B::AcMin { ac: 16 })],
        // (Defences and ending spells.)
        "blur" => vec![buff(B::Blur)],
        "mirror-image" => vec![buff(B::Images { count: 3 })],
        "sanctuary" => vec![buff(B::Sanctuary { dc: 0 })],
        "protection-from-evil-and-good" => vec![buff(B::Warded { types: ["aberration", "celestial", "elemental", "fey", "fiend", "undead"].map(String::from).to_vec() })],
        "protection-from-energy" => vec![buff(B::Resistance { types: vec!["fire".into()] })],
        "death-ward" => vec![buff(B::DeathWard)],
        "warding-bond" => vec![buff(B::Bond { mark: String::new() })],
        "freedom-of-movement" => vec![buff(B::ConditionImmunity { conditions: vec![K::Paralyzed, K::Restrained] })],
        "mind-blank" => vec![buff(B::ConditionImmunity { conditions: vec![K::Charmed] }), buff(B::Resistance { types: vec!["psychic".into()] })],
        "greater-restoration" => vec![P::Cure { conditions: vec![K::Charmed, K::Petrified], disease: false, exhaustion: true }],
        "dispel-magic" => vec![P::Dispel],
        "counterspell" => vec![P::Counter],
        // (Exploring.)
        "feather-fall" => vec![buff(B::FeatherFall)],
        "jump" => vec![buff(B::Jump { times: 3.0 })],
        "spider-climb" => vec![buff(B::SpiderClimb)],
        "fly" => vec![buff(B::Fly { feet: 60 })],
        "comprehend-languages" | "tongues" => vec![buff(B::Tongues)],
        "pass-without-trace" => vec![P::Targets { count: 6, per_slot: 0 }, buff(B::SkillBonus { skill: "stealth".into(), bonus: 10 })],
        "detect-magic" => vec![buff(B::Detect { what: vec!["magic".into()] })],
        "detect-poison-and-disease" => vec![buff(B::Detect { what: vec!["poison".into()] })],
        "detect-evil-and-good" => vec![buff(B::Detect { what: ["aberration", "celestial", "elemental", "fey", "fiend", "undead"].map(String::from).to_vec() })],
        "find-traps" => vec![P::RevealTraps],
        // (Bringing back the dead: within a minute, ten days, a century.)
        "revivify" => vec![P::Revive { within_hours: 0.017, full: false }],
        "raise-dead" => vec![P::Revive { within_hours: 240.0, full: false }],
        "resurrection" => vec![P::Revive { within_hours: 876_000.0, full: true }],
        // (Lasting areas: the area stays; `repeat` = the parts act again.)
        "fog-cloud" => vec![lasting(L { obscures: true, ..Default::default() })],
        "darkness" => vec![lasting(L { dark: true, ..Default::default() })],
        "silence" => vec![lasting(L { silence: true, ..Default::default() })],
        "wall-of-stone" | "wall-of-force" => vec![lasting(L { wall: true, feet: 50, ..Default::default() })],
        "spike-growth" => vec![P::Damage { by_level: vec![(2, "2d4".into())], damage_type: Some(rustpg2d_engine::srd::DamageType::Piercing) }, lasting(L { difficult: true, repeat: true, per_tile: true, later: true, ..Default::default() })],
        "spirit-guardians" => vec![
            P::Save { ability: rustpg2d_engine::srd::Ability::Wis, half: true },
            P::Damage { by_level: (3..=9).map(|l| (l, format!("{l}d8"))).collect(), damage_type: Some(rustpg2d_engine::srd::DamageType::Radiant) },
            lasting(L { repeat: true, later: true, follows: true, foes_only: true, difficult: true, feet: 15, ..Default::default() }),
        ],
        "cloudkill" | "incendiary-cloud" => vec![lasting(L { repeat: true, obscures: true, ..Default::default() })],
        "moonbeam" | "wall-of-fire" => vec![lasting(L { repeat: true, ..Default::default() })],
        "insect-plague" | "black-tentacles" | "wall-of-thorns" | "blade-barrier" => vec![lasting(L { repeat: true, difficult: true, ..Default::default() })],
        _ => Vec::new(),
    }
}

/// `{"sneak_attack": {"dice_count": 3}}` as `sneak_attack.dice_count = 3` (numbers only).
fn flatten_numbers(prefix: &str, v: &Value, out: &mut BTreeMap<String, i64>) {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten_numbers(&key, x, out);
            }
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.insert(prefix.to_string(), i);
            }
        }
        Value::Bool(b) => {
            out.insert(prefix.to_string(), *b as i64);
        }
        _ => {}
    }
}

/// The SRD equipment (and categories) a starting option names, with counts.
fn option_items(o: &Value) -> Vec<(String, u32)> {
    match s(o, "option_type") {
        "counted_reference" => vec![(o.pointer("/of/index").and_then(Value::as_str).unwrap_or("").to_string(), o.get("count").and_then(Value::as_u64).unwrap_or(1) as u32)],
        "reference" => vec![(o.pointer("/item/index").and_then(Value::as_str).unwrap_or("").to_string(), 1)],
        "multiple" => list(o, "items").iter().flat_map(option_items).collect(),
        "choice" => {
            let n = o.pointer("/choice/choose").and_then(Value::as_u64).unwrap_or(1) as u32;
            match o.pointer("/choice/from/equipment_category/index").and_then(Value::as_str) {
                Some(cat) => vec![(cat.to_string(), n)],
                None => o.pointer("/choice/from/options/0").map(option_items).unwrap_or_default(),
            }
        }
        _ => Vec::new(),
    }
}

/// Library items that stand for an SRD item (the first one is what class equipment and packs
/// give): they get its numbers and keep their own name, text and look.
const STANDS_FOR: [(&str, &[&str]); 18] = [
    ("longsword", &["iron_sword", "fine_iron_sword"]),
    // (Its own 3D model, carried like a spear: a long swing.)
    ("quarterstaff", &["quarterstaff"]),
    ("shortsword", &["sword"]),
    ("spear", &["spear"]),
    ("battleaxe", &["axe"]),
    ("shortbow", &["bow", "crude_bow"]),
    ("crossbow-light", &["crossbow"]),
    ("leather-armor", &["leather_armor"]),
    ("chain-mail", &["chainmail"]),
    ("shield", &["wooden_shield"]),
    ("torch", &["torch"]),
    ("lantern-hooded", &["lantern"]),
    ("arrow", &["arrows"]),
    ("crossbow-bolt", &["bolts"]),
    ("backpack", &["small_backpack"]),
    ("hammer", &["hammer"]),
    ("pick-miners", &["pickaxe"]),
    ("fishing-tackle", &["fishing_rod"]),
];

/// An equipment category class equipment offers ("any martial weapon"): the item given.
const CATEGORY_ITEM: [(&str, &str); 9] = [
    ("martial-weapons", "longsword"),
    ("martial-melee-weapons", "longsword"),
    ("simple-weapons", "spear"),
    ("simple-melee-weapons", "spear"),
    ("arcane-foci", "crystal"),
    ("holy-symbols", "amulet"),
    ("druidic-foci", "sprig-of-mistletoe"),
    ("musical-instruments", "lute"),
    ("artisans-tools", "smiths-tools"),
];

/// The coins (id, name, text, worth in copper pieces, icon); the gold piece keeps the old `coin` id.
const COINS: [(&str, &str, &str, u32, u32); 5] = [
    ("copper-piece", "Copper Piece", "The smallest coin: ten make a silver piece.", 1, gear::COPPER),
    ("silver-piece", "Silver Piece", "A silver coin worth ten copper pieces.", 10, gear::SILVER),
    ("electrum-piece", "Electrum Piece", "An electrum coin worth five silver pieces.", 50, gear::ELECTRUM),
    ("coin", "Gold Piece", "A gold coin worth ten silver or a hundred copper pieces.", 100, gear::GOLD),
    ("platinum-piece", "Platinum Piece", "A platinum coin worth ten gold pieces.", 1000, gear::PLATINUM),
];

/// The frames of the generic equipment icons (`sprites/srd_gear.png`, sheet `srd_gear`).
mod gear {
    pub const SHEET: &str = "srd_gear";
    pub const KNIFE: u32 = 0;
    pub const SWORD: u32 = 1;
    pub const AXE: u32 = 2;
    pub const POLEARM: u32 = 3;
    pub const CLUB: u32 = 4;
    pub const BOW: u32 = 5;
    pub const CROSSBOW: u32 = 6;
    pub const SLING: u32 = 7;
    pub const LIGHT_ARMOR: u32 = 8;
    pub const MEDIUM_ARMOR: u32 = 9;
    pub const HEAVY_ARMOR: u32 = 10;
    pub const SHIELD: u32 = 11;
    pub const ARTISAN: u32 = 12;
    pub const INSTRUMENT: u32 = 13;
    pub const GAMING: u32 = 14;
    pub const GEAR: u32 = 15;
    pub const AMMO: u32 = 16;
    pub const PACK: u32 = 17;
    pub const MOUNT: u32 = 18;
    pub const VEHICLE: u32 = 19;
    pub const SHIP: u32 = 20;
    pub const ARCANE: u32 = 21;
    pub const HOLY: u32 = 22;
    pub const DRUIDIC: u32 = 23;
    pub const KIT: u32 = 24;
    pub const LIGHT: u32 = 25;
    pub const COPPER: u32 = 26;
    pub const SILVER: u32 = 27;
    pub const ELECTRUM: u32 = 28;
    pub const GOLD: u32 = 29;
    pub const PLATINUM: u32 = 30;
    pub const LOCKPICKS: u32 = 31;
    pub const CLOTHES: u32 = 32;
    pub const VIAL: u32 = 33;
    pub const ROPE: u32 = 34;
    pub const BOOK: u32 = 35;
    pub const FOOD: u32 = 36;
    pub const CONTAINER: u32 = 37;
    /// A sling bullet in flight.
    pub const BULLET: u32 = 38;
}

/// The library item standing for an SRD item (`STANDS_FOR`), without any data.
fn library_item(index: &str) -> Option<&'static str> {
    STANDS_FOR.iter().find(|(i, _)| *i == index).map(|(_, ids)| ids[0])
}

/// The generic icon of an SRD item.
fn gear_icon(v: &Value) -> u32 {
    let id = s(v, "index");
    let has = |w: &str| id.contains(w);
    let any = |ws: &[&str]| ws.iter().any(|w| id.contains(w));
    match s(v.get("equipment_category").unwrap_or(&Value::Null), "index") {
        "weapon" => match id {
            _ if has("crossbow") => gear::CROSSBOW,
            _ if has("bow") => gear::BOW,
            _ if any(&["dagger", "sickle"]) => gear::KNIFE,
            _ if any(&["sword", "rapier", "scimitar"]) => gear::SWORD,
            _ if has("axe") => gear::AXE,
            _ if any(&["spear", "javelin", "pike", "halberd", "glaive", "lance", "trident", "quarterstaff"]) => gear::POLEARM,
            _ if any(&["sling", "dart", "net", "blowgun", "whip"]) => gear::SLING,
            _ => gear::CLUB,
        },
        "armor" => match s(v, "armor_category") {
            "Light" => gear::LIGHT_ARMOR,
            "Medium" => gear::MEDIUM_ARMOR,
            "Heavy" => gear::HEAVY_ARMOR,
            _ => gear::SHIELD,
        },
        "tools" => match s(v, "tool_category") {
            "Musical Instrument" => gear::INSTRUMENT,
            "Gaming Sets" => gear::GAMING,
            _ if id == "thieves-tools" => gear::LOCKPICKS,
            _ if any(&["kit", "navigator"]) => gear::KIT,
            _ => gear::ARTISAN,
        },
        "mounts-and-vehicles" => match s(v, "vehicle_category") {
            "Mounts and Other Animals" => gear::MOUNT,
            "Waterborne Vehicles" => gear::SHIP,
            _ => gear::VEHICLE,
        },
        _ => match v.pointer("/gear_category/index").and_then(Value::as_str).unwrap_or("") {
            "ammunition" => gear::AMMO,
            "equipment-packs" => gear::PACK,
            "arcane-foci" => gear::ARCANE,
            "holy-symbols" => gear::HOLY,
            "druidic-foci" => gear::DRUIDIC,
            "kits" => gear::KIT,
            _ if any(&["lantern", "lamp", "candle", "torch"]) => gear::LIGHT,
            _ if any(&["clothes", "robes", "vestments"]) => gear::CLOTHES,
            _ if any(&["vial", "flask", "bottle", "ink-1", "antitoxin"]) => gear::VIAL,
            _ if any(&["rope", "chain", "string", "manacles"]) => gear::ROPE,
            _ if any(&["book", "paper", "parchment", "case-map"]) => gear::BOOK,
            _ if has("rations") => gear::FOOD,
            _ if any(&["chest", "barrel", "basket", "bucket", "pot-", "jug", "case-crossbow"]) => gear::CONTAINER,
            _ => gear::GEAR,
        },
    }
}

/// A cost of the data in copper pieces.
fn cost_cp(v: &Value) -> u32 {
    let n = v.pointer("/cost/quantity").and_then(Value::as_u64).unwrap_or(0) as u32;
    n * match v.pointer("/cost/unit").and_then(Value::as_str).unwrap_or("gp") {
        "cp" => 1,
        "sp" => 10,
        "ep" => 50,
        "pp" => 1000,
        _ => 100,
    }
}

/// An SRD item's data on an item of the project: an item the import made (its id is the index)
/// takes all of it but its icon; a library item standing for it (and a coin) keeps its
/// name, text and look.
fn merge_item(old: &mut rustpg2d_engine::defs::ItemDef, new: &rustpg2d_engine::defs::ItemDef) {
    if new.coin_cp > 0 {
        // A coin keeps what it does when used (a story's script); an old one becomes the coin.
        if old.coin_cp == 0 {
            (old.name, old.description) = (new.name.clone(), new.description.clone());
        }
        (old.coin_cp, old.weight, old.max_stack) = (new.coin_cp, new.weight, old.max_stack.max(new.max_stack));
        return;
    }
    let standing = STANDS_FOR.iter().any(|(_, ids)| ids.contains(&old.id.as_str()));
    if old.id == new.id && !standing {
        let icon = old.icon.clone();
        *old = new.clone();
        old.icon = icon;
        return;
    }
    (old.srd, old.cost_cp, old.weight) = (new.srd.clone(), new.cost_cp, new.weight);
    match (&mut old.weapon, &new.weapon) {
        (Some(o), Some(n)) => {
            (o.category, o.properties, o.dice, o.versatile, o.thrown_ft, o.two_handed) = (n.category, n.properties.clone(), n.dice.clone(), n.versatile.clone(), n.thrown_ft, n.two_handed);
            if n.damage_type.is_some() {
                o.damage_type = n.damage_type;
            }
            if let (Some(or), Some(nr)) = (&mut o.ranged, &n.ranged) {
                (or.range, or.long_range) = (nr.range, nr.long_range);
            }
        }
        (None, Some(n)) => old.weapon = Some(n.clone()),
        _ => {}
    }
    match (&mut old.armor, &new.armor) {
        (Some(o), Some(n)) => {
            (o.category, o.base_ac, o.dex_max, o.str_min, o.stealth_disadvantage) = (n.category, n.base_ac, n.dex_max, n.str_min, n.stealth_disadvantage);
            (o.defense, o.speed) = (0, 0.0);
        }
        (None, Some(n)) => old.armor = Some(n.clone()),
        _ => {}
    }
}

/// An SRD feature: its text, the choice it asks for, and what of it acts ([`feature_effects`]).
fn class_feature(f: &Value) -> rustpg2d_engine::classes::ClassFeature {
    use rustpg2d_engine::classes::{ClassFeature, FeatureChoice};
    let id = s(f, "index");
    let choice = if id.contains("ability-score-improvement") {
        Some(FeatureChoice::AbilityScores)
    } else if f.pointer("/feature_specific/expertise_options").is_some() {
        Some(FeatureChoice::Expertise { count: 2 })
    } else if let Some(set) = f.pointer("/feature_specific/subfeature_options") {
        let choose = set.get("choose").and_then(Value::as_u64).unwrap_or(1) as u32;
        let options = set.pointer("/from/options").and_then(Value::as_array).map(|a| a.iter().filter_map(|o| o.pointer("/item/index").and_then(Value::as_str)).map(str::to_string).collect()).unwrap_or_default();
        Some(FeatureChoice::Options { choose, options })
    } else if id == "bonus-proficiencies" {
        Some(FeatureChoice::Skills { count: 3 })
    } else {
        None
    };
    ClassFeature { id: id.into(), name: s(f, "name").into(), level: f.get("level").and_then(Value::as_u64).unwrap_or(1) as u32, text: desc(f), effects: feature_effects(id), choice }
}

/// What of an SRD feature acts (the rest stays its text): the engine's vocabulary by feature.
fn feature_effects(id: &str) -> Vec<rustpg2d_engine::classes::FeatureEffect> {
    use rustpg2d_engine::classes::{FeatureEffect as E, FightingStyle as F};
    use rustpg2d_engine::srd::Ability as A;
    use rustpg2d_engine::classes::{ActCost as C, ActEffect as X, Recharge as R};
    use rustpg2d_engine::srd::conditions::Condition as K;
    let style = |st| vec![E::FightingStyle { style: st }];
    let saves = |a: &[&str]| a.iter().map(|x| E::Proficient { what: format!("save:{x}") }).collect::<Vec<_>>();
    // The features that act: resources (uses back after a rest), class actions, riders.
    let pool = |id: &str, count: i32, number: &str, ability: Option<A>, per_level: i32, recharge: R| E::Resource { id: id.into(), count, number: number.into(), ability, per_level, recharge };
    let act = |id: &str, name: &str, cost: C, resource: &str, effect: X| E::Act { id: id.into(), name: name.into(), cost, resource: resource.into(), effect };
    let basic = |id: &str, name: &str, cost: C, resource: &str, what: &str| act(id, name, cost, resource, X::Basic { act: what.into() });
    match id {
        "rage" => vec![pool("rage", 0, "rage_count", None, 0, R::LongRest), act("rage", "Rage", C::Bonus, "rage", X::Rage)],
        "reckless-attack" => vec![act("reckless-attack", "Reckless Attack", C::Free, "", X::Reckless)],
        "frenzy" => vec![act("frenzy", "Frenzy attack", C::Bonus, "", X::FrenzyAttack)],
        "intimidating-presence" => vec![act("intimidating-presence", "Intimidating Presence", C::Action, "", X::AreaSave { feet: 30, save: A::Wis, ability: A::Cha, condition: K::Frightened, rounds: 1, types: Vec::new(), single: true, destroy: false })],
        "relentless-rage" => vec![E::DropTo1 { dc: 10 }],
        "retaliation" => vec![E::ReactionAttack],
        "bardic-inspiration-d6" => vec![pool("bardic-inspiration", 0, "", Some(A::Cha), 0, R::LongRest), act("bardic-inspiration", "Bardic Inspiration", C::Bonus, "bardic-inspiration", X::GiveDie { feet: 60, number: "bardic_inspiration_die".into() })],
        "font-of-inspiration" => vec![pool("bardic-inspiration", 0, "", Some(A::Cha), 0, R::ShortRest)],
        "channel-divinity-1-rest" => vec![pool("channel-divinity", 0, "channel_divinity_charges", None, 0, R::ShortRest)],
        "channel-divinity-turn-undead" => vec![act("turn-undead", "Turn Undead", C::Action, "channel-divinity", X::AreaSave { feet: 30, save: A::Wis, ability: A::Wis, condition: K::Frightened, rounds: 10, types: vec!["undead".into()], single: false, destroy: true })],
        "destroy-undead-cr-1-2-or-below" => vec![E::DestroyUndead { cr: 0.5 }],
        "destroy-undead-cr-1-or-below" => vec![E::DestroyUndead { cr: 1.0 }],
        "destroy-undead-cr-2-or-below" => vec![E::DestroyUndead { cr: 2.0 }],
        "destroy-undead-cr-3-or-below" => vec![E::DestroyUndead { cr: 3.0 }],
        "destroy-undead-cr-4-or-below" => vec![E::DestroyUndead { cr: 4.0 }],
        "channel-divinity-preserve-life" => vec![act("preserve-life", "Preserve Life", C::Action, "channel-divinity", X::HealAllies { feet: 30, per_level: 5 })],
        "divine-strike" => vec![E::ExtraDamage { dice: vec![(8, "1d8".into()), (14, "2d8".into())], damage_type: Some(rustpg2d_engine::srd::DamageType::Radiant), hurt_target: false }],
        "second-wind" => vec![pool("second-wind", 1, "", None, 0, R::ShortRest), act("second-wind", "Second Wind", C::Bonus, "second-wind", X::Heal { dice: "1d10".into(), per_level: 1 })],
        "action-surge-1-use" => vec![pool("action-surge", 0, "action_surges", None, 0, R::ShortRest), act("action-surge", "Action Surge", C::Free, "action-surge", X::ExtraAction)],
        "indomitable-1-use" => vec![pool("indomitable", 0, "indomitable_uses", None, 0, R::LongRest), E::RerollSave { resource: "indomitable".into() }],
        "survivor" => vec![E::StartOfTurnHeal { base: 5, ability: A::Con }],
        "ki" => vec![pool("ki", 0, "ki_points", None, 0, R::ShortRest)],
        "flurry-of-blows" => vec![act("flurry-of-blows", "Flurry of Blows", C::Bonus, "ki", X::UnarmedStrikes { count: 2 })],
        "patient-defense" => vec![basic("patient-defense", "Patient Defense", C::Bonus, "ki", "dodge")],
        "step-of-the-wind" => vec![basic("step-of-the-wind-dash", "Step of the Wind: Dash", C::Bonus, "ki", "dash"), basic("step-of-the-wind-disengage", "Step of the Wind: Disengage", C::Bonus, "ki", "disengage")],
        "deflect-missiles" => vec![E::ReactionReduceRanged { dice: "1d10".into(), ability: A::Dex, per_level: 1 }],
        "open-hand-technique" => vec![E::OnHitSave { resource: String::new(), save: A::Dex, ability: A::Wis, condition: K::Prone, rounds: 0, melee: false, flurry: true }],
        "stunning-strike" => vec![E::OnHitSave { resource: "ki".into(), save: A::Con, ability: A::Wis, condition: K::Stunned, rounds: 1, melee: true, flurry: false }],
        "wholeness-of-body" => vec![pool("wholeness-of-body", 1, "", None, 0, R::LongRest), act("wholeness-of-body", "Wholeness of Body", C::Action, "wholeness-of-body", X::Heal { dice: String::new(), per_level: 3 })],
        "lay-on-hands" => vec![pool("lay-on-hands", 0, "", None, 5, R::LongRest), act("lay-on-hands", "Lay on Hands", C::Action, "lay-on-hands", X::HealPool)],
        "channel-divinity" => vec![pool("channel-divinity", 1, "", None, 0, R::ShortRest)],
        "channel-divinity-sacred-weapon" => vec![act("sacred-weapon", "Sacred Weapon", C::Action, "channel-divinity", X::WeaponBuff { ability: A::Cha, rounds: 10 })],
        "channel-divinity-turn-the-unholy" => vec![act("turn-the-unholy", "Turn the Unholy", C::Action, "channel-divinity", X::AreaSave { feet: 30, save: A::Wis, ability: A::Cha, condition: K::Frightened, rounds: 10, types: vec!["fiend".into(), "undead".into()], single: false, destroy: false })],
        "aura-of-devotion" => vec![E::Aura { feet: 10, save_bonus: false, immunity: "charmed".into() }],
        "hunters-prey-colossus-slayer" => vec![E::ExtraDamage { dice: vec![(1, "1d8".into())], damage_type: None, hurt_target: true }],
        "superior-hunters-defense-evasion" => vec![E::Evasion],
        "uncanny-dodge" | "superior-hunters-defense-uncanny-dodge" => vec![E::ReactionHalveDamage],
        "sneak-attack" => vec![E::SneakAttack],
        "cunning-action" => vec![basic("cunning-action-dash", "Cunning Action: Dash", C::Bonus, "", "dash"), basic("cunning-action-disengage", "Cunning Action: Disengage", C::Bonus, "", "disengage"), basic("cunning-action-hide", "Cunning Action: Hide", C::Bonus, "", "hide")],
        "dark-ones-blessing" => vec![E::OnKillTempHp { ability: A::Cha, per_level: 1 }],
        "barbarian-unarmored-defense" => vec![E::ArmorClass { base: 10, abilities: vec![A::Dex, A::Con], shield: true }],
        "monk-unarmored-defense" => vec![E::ArmorClass { base: 10, abilities: vec![A::Dex, A::Wis], shield: false }],
        "draconic-resilience" => vec![E::ArmorClass { base: 13, abilities: vec![A::Dex], shield: true }, E::HpPerLevel { hp: 1 }],
        "fast-movement" => vec![E::Speed { feet: 10, unarmored: false }],
        // (The monk's bonus grows with the class's `unarmored_movement` number: 0 = that number.)
        "unarmored-movement-1" => vec![E::Speed { feet: 0, unarmored: true }],
        "jack-of-all-trades" => vec![E::HalfProficiency { abilities: Vec::new() }],
        "remarkable-athlete" => vec![E::HalfProficiency { abilities: vec![A::Str, A::Dex, A::Con] }],
        "reliable-talent" => vec![E::ReliableTalent],
        "improved-critical" => vec![E::CritRange { from: 19 }],
        "superior-critical" => vec![E::CritRange { from: 18 }],
        "extra-attack-1" | "barbarian-extra-attack" | "monk-extra-attack" | "paladin-extra-attack" | "ranger-extra-attack" => vec![E::ExtraAttacks { attacks: 2 }],
        "extra-attack-2" => vec![E::ExtraAttacks { attacks: 3 }],
        "extra-attack-3" => vec![E::ExtraAttacks { attacks: 4 }],
        "brutal-critical-1-die" => vec![E::BrutalCritical { dice: 1 }],
        "brutal-critical-2-dice" => vec![E::BrutalCritical { dice: 2 }],
        "brutal-critical-3-dice" => vec![E::BrutalCritical { dice: 3 }],
        x if x.ends_with("fighting-style-archery") => style(F::Archery),
        x if x.ends_with("fighting-style-defense") => style(F::Defense),
        x if x.ends_with("fighting-style-dueling") => style(F::Dueling),
        x if x.ends_with("fighting-style-great-weapon-fighting") => style(F::GreatWeaponFighting),
        x if x.ends_with("fighting-style-protection") => style(F::Protection),
        x if x.ends_with("fighting-style-two-weapon-fighting") => style(F::TwoWeaponFighting),
        "monk-evasion" | "rogue-evasion" => vec![E::Evasion],
        "danger-sense" => vec![E::SaveAdvantage { ability: A::Dex }],
        "feral-instinct" => vec![E::InitiativeAdvantage],
        "purity-of-body" => vec![E::ConditionImmunity { conditions: vec!["poisoned".into()] }, E::Immunity { types: vec!["poison".into()] }],
        "aura-of-protection" => vec![E::Aura { feet: 10, save_bonus: true, immunity: String::new() }],
        "aura-of-courage" => vec![E::Aura { feet: 10, save_bonus: false, immunity: "frightened".into() }],
        "aura-improvements" => vec![E::Aura { feet: 30, save_bonus: false, immunity: String::new() }],
        "primal-champion" => vec![E::AbilityBonus { ability: A::Str, amount: 4, max: 24 }, E::AbilityBonus { ability: A::Con, amount: 4, max: 24 }],
        "diamond-soul" => saves(&["str", "dex", "con", "int", "wis", "cha"]),
        "slippery-mind" => saves(&["wis"]),
        "bonus-proficiency" => vec![E::Proficient { what: "heavy-armor".into() }],
        "martial-arts" => vec![E::MartialArts, act("martial-arts-strike", "Martial Arts: bonus strike", C::Bonus, "", X::UnarmedStrikes { count: 1 })],
        _ => Vec::new(),
    }
}

/// The look of a monster without one of its own: a library model where one fits, else the
/// mannequin (the library's `mannequin` model) until models for the monsters come.
fn monster_model(m: &Value) -> &'static str {
    const BY_ID: [(&str, &str); 28] = [
        ("wolf", "wolf"),
        ("dire-wolf", "wolf"),
        ("bat", "bat"),
        ("giant-bat", "bat"),
        ("cat", "cat"),
        ("mastiff", "dog"),
        ("jackal", "dog"),
        ("skeleton", "skeleton"),
        ("minotaur-skeleton", "skeleton"),
        ("warhorse-skeleton", "skeleton"),
        ("zombie", "skeleton"),
        ("ghoul", "skeleton"),
        ("ghast", "skeleton"),
        ("ghost", "ghost"),
        ("specter", "ghost"),
        ("wraith", "ghost"),
        ("troll", "troll"),
        ("ogre", "troll"),
        ("bandit", "bandit"),
        ("bandit-captain", "bandit_chief"),
        ("guard", "guard"),
        ("knight", "guard"),
        ("veteran", "guard"),
        ("noble", "noble"),
        ("priest", "priest"),
        ("acolyte", "priest"),
        ("mage", "wizard"),
        ("archmage", "wizard"),
    ];
    let id = s(m, "index");
    if let Some((_, model)) = BY_ID.iter().find(|(i, _)| *i == id) {
        return model;
    }
    match s(m, "type") {
        "ooze" => "slime",
        "humanoid" if id == "commoner" || id == "cultist" || id == "cult-fanatic" || id == "spy" || id == "thug" => "villager",
        "humanoid" if id.contains("goblin") => "goblin",
        "humanoid" if id.contains("orc") => "orc",
        _ => "mannequin",
    }
}

/// A new character definition for an SRD monster.
fn monster_character(m: &Value, block: &rustpg2d_engine::srd::StatBlock) -> CharacterDef {
    use rustpg2d_engine::defs::{Appearance, Behavior};
    let humanoid = s(m, "type") == "humanoid";
    let evil = s(m, "alignment").contains("evil");
    // Humanoids live among others (unless evil); beasts and monsters that attack hunt the player.
    let hostile = !block.attacks.is_empty() && (!humanoid || evil);
    let behavior = if hostile { Behavior::Chase { sight: 8.0, leash: 20.0, flee_below: 0.0, keep_distance: 0.0, group: true, alert: 6.0 } } else { Behavior::Wander { radius: 3.0 } };
    let size = (block.size.scale() * 100.0).round() as u16;
    let description = format!("{} {}, challenge {} ({} XP){}.", block.size.name(), block.creature_type, block.cr, block.xp, if block.alignment.is_empty() { String::new() } else { format!(", {}", block.alignment) });
    let fresh = CharacterDef::fresh(s(m, "index"));
    CharacterDef {
        name: s(m, "name").into(),
        description,
        behavior,
        max_hp: block.hp,
        hostile,
        xp: block.xp,
        // (Faster walkers walk faster: the default speed is for 30 ft.)
        speed: fresh.speed * (block.speed_ft.max(5) as f32 / 30.0).clamp(0.3, 2.5),
        appearance: Appearance { model: monster_model(m).into(), size: if size == 100 { 0 } else { size }, ..Default::default() },
        stats: Some(block.clone()),
        ..fresh
    }
}

/// An SRD monster's stat block: the template / monster id, its numbers, attacks, Multiattack,
/// size, type, speeds, senses and the texts of its traits and actions.
fn monster_block(m: &Value) -> rustpg2d_engine::srd::StatBlock {
    use rustpg2d_engine::srd::{FeatureKind, Size, Speeds, StatFeature};
    let feet = |v: Option<&str>| v.and_then(|s| s.split_whitespace().next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
    let score = |k: &str| m.get(k).and_then(Value::as_i64).unwrap_or(10) as i32;
    let cr = m.get("challenge_rating").and_then(Value::as_f64).unwrap_or(0.0);
    let cr = match cr {
        c if (c - 0.125).abs() < 1e-6 => "1/8".to_string(),
        c if (c - 0.25).abs() < 1e-6 => "1/4".to_string(),
        c if (c - 0.5).abs() < 1e-6 => "1/2".to_string(),
        c => format!("{}", c as i64),
    };
    let proficiencies = list(m, "proficiencies")
        .iter()
        .filter_map(|p| p.pointer("/proficiency/index").and_then(Value::as_str))
        .map(|p| match p.strip_prefix("saving-throw-") {
            Some(a) => format!("save:{a}"),
            None => prof_id(p),
        })
        .collect();
    let speed = |k: &str| feet(m.pointer(&format!("/speed/{k}")).and_then(Value::as_str));
    // "blindsight 60 ft., darkvision 120 ft., passive Perception 23"
    let senses = m
        .get("senses")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, v)| match k.as_str() {
                    "passive_perception" => format!("passive Perception {}", v.as_i64().unwrap_or(10)),
                    k => format!("{} {}", k.replace('_', " "), v.as_str().map_or_else(|| v.to_string(), str::to_string)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let feature = |kind: FeatureKind| move |a: &Value| StatFeature { kind, name: s(a, "name").into(), text: desc(a) };
    let mut features: Vec<StatFeature> = list(m, "special_abilities").iter().map(feature(FeatureKind::Trait)).collect();
    features.extend(list(m, "actions").iter().map(feature(FeatureKind::Action)));
    features.extend(list(m, "reactions").iter().map(feature(FeatureKind::Reaction)));
    features.extend(list(m, "legendary_actions").iter().map(feature(FeatureKind::Legendary)));
    let creature_type = match s(m, "subtype") {
        "" => s(m, "type").to_string(),
        sub => format!("{} ({sub})", s(m, "type")),
    };
    rustpg2d_engine::srd::StatBlock {
        template: s(m, "index").into(),
        name: s(m, "name").into(),
        abilities: rustpg2d_engine::srd::Abilities([score("strength"), score("dexterity"), score("constitution"), score("intelligence"), score("wisdom"), score("charisma")]),
        ac: m.pointer("/armor_class/0/value").and_then(Value::as_i64).unwrap_or(10) as i32,
        hp: m.get("hit_points").and_then(Value::as_i64).unwrap_or(1) as i32,
        hit_dice: m.get("hit_points_roll").and_then(Value::as_str).unwrap_or(s(m, "hit_dice")).to_string(),
        speed_ft: speed("walk"),
        darkvision_ft: feet(m.pointer("/senses/darkvision").and_then(Value::as_str)),
        proficiencies,
        languages: s(m, "languages").into(),
        alignment: s(m, "alignment").into(),
        cr,
        xp: m.get("xp").and_then(Value::as_u64).unwrap_or(0) as u32,
        resistances: strings(m, "damage_resistances"),
        immunities: strings(m, "damage_immunities"),
        vulnerabilities: strings(m, "damage_vulnerabilities"),
        condition_immunities: list(m, "condition_immunities").iter().map(|c| s(c, "index").to_string()).collect(),
        attacks: list(m, "actions").iter().filter_map(stat_attack).collect(),
        multiattack: list(m, "actions").iter().find(|a| s(a, "name") == "Multiattack").map_or(0, |a| multiattack_count(&desc(a))),
        size: Size::parse(s(m, "size")).unwrap_or_default(),
        creature_type,
        speeds: Speeds { fly: speed("fly"), swim: speed("swim"), climb: speed("climb"), burrow: speed("burrow") },
        senses,
        features,
        spellcasting: monster_spellcasting(m),
        actions: monster_actions(m),
        legendary: monster_legendary(m),
        legendary_points: if list(m, "legendary_actions").is_empty() { 0 } else { 3 },
        legendary_resistance: list(m, "special_abilities").iter().find(|a| s(a, "name").starts_with("Legendary Resistance")).map_or(0, |a| a.pointer("/usage/times").and_then(Value::as_u64).unwrap_or(3) as u32),
        regeneration: list(m, "special_abilities").iter().find(|a| s(a, "name") == "Regeneration").and_then(|a| regeneration(&desc(a))),
        pack_tactics: list(m, "special_abilities").iter().any(|a| s(a, "name") == "Pack Tactics"),
    }
}

/// The number just before `pat` in `text` ("in a 60-foot cone" with "-foot cone" -> 60).
fn number_before(text: &str, pat: &str) -> Option<u32> {
    let i = text.find(pat)?;
    let digits: String = text[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().ok()
}

/// The number just after `word` ("within 120 feet" with "within " -> 120).
fn number_after_word(text: &str, word: &str) -> Option<u32> {
    let rest = &text[text.find(word)? + word.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// A save action's area from its text: a cone or line from the creature, a sphere at a point or
/// around the creature, a cube; `None` with the range for one creature. Returns (area, around
/// the creature, range in feet).
pub fn action_area(text: &str) -> (Option<rustpg2d_engine::skills::Area>, bool, u32) {
    use rustpg2d_engine::skills::{Area, Shape};
    let l = text.to_ascii_lowercase();
    let range = number_after_word(&l, "within ").unwrap_or(5);
    if let Some(n) = number_before(&l, "-foot cone") {
        return (Some(Area { shape: Shape::Cone, feet: n }), false, n);
    }
    if let Some(n) = number_before(&l, "-foot line") {
        return (Some(Area { shape: Shape::Line, feet: n }), false, n);
    }
    if let Some(n) = number_before(&l, "-foot-radius sphere").or_else(|| number_before(&l, "-foot radius sphere")) {
        return (Some(Area { shape: Shape::Sphere, feet: n }), false, range);
    }
    if let Some(n) = number_before(&l, "-foot cube") {
        return (Some(Area { shape: Shape::Cube, feet: n }), false, range);
    }
    // "each creature within 10 feet of the dragon", "within 120 feet of it".
    if l.contains("each creature") || l.contains("each other creature") || l.contains("creature of the") {
        if let Some(n) = number_after_word(&l, "within ").filter(|_| l.contains(" of the ") || l.contains(" of it")) {
            return (Some(Area { shape: Shape::Sphere, feet: n }), true, 0);
        }
    }
    (None, false, range)
}

/// One of a monster's actions with a saving throw and no attack roll (a breath weapon, a wing
/// beat, a gaze) from the data's `dc`, `damage` and `usage` and its text's area and condition.
pub fn save_action(a: &Value, legendary_only: bool) -> Option<rustpg2d_engine::srd::StatAction> {
    use rustpg2d_engine::srd::{Ability, Condition, DamageType, StatAction};
    if a.get("attack_bonus").is_some() {
        return None;
    }
    let dc = a.get("dc")?;
    let text = desc(a);
    let l = text.to_ascii_lowercase();
    let (area, around, range_ft) = action_area(&text);
    let damage = a.pointer("/damage/0");
    let conditions = [
        ("knocked prone", Condition::Prone),
        ("restrained", Condition::Restrained),
        ("poisoned", Condition::Poisoned),
        ("paralyzed", Condition::Paralyzed),
        ("frightened", Condition::Frightened),
        ("stunned", Condition::Stunned),
        ("blinded", Condition::Blinded),
        ("charmed", Condition::Charmed),
        ("incapacitated", Condition::Incapacitated),
        ("petrified", Condition::Petrified),
    ];
    let given = |w: &str| l.contains(&format!("be {w}")) || l.contains(&format!("become {w}")) || l.contains(&format!("is {w}")) || l.contains(&format!("and {w}"));
    let condition = conditions.iter().find(|(w, _)| given(w)).map(|c| c.1);
    let rounds = if l.contains("for 1 minute") {
        10
    } else if l.contains("for 1 hour") {
        600
    } else if l.contains("until the end of") {
        1
    } else {
        0
    };
    let (recharge, per_day) = match a.pointer("/usage/type").and_then(Value::as_str) {
        Some("recharge on roll") => (a.pointer("/usage/min_value").and_then(Value::as_u64).unwrap_or(6) as u32, 0),
        Some("per day") => (0, a.pointer("/usage/times").and_then(Value::as_u64).unwrap_or(1) as u32),
        Some("recharge after rest") => (0, 1),
        _ => (0, 0),
    };
    let name = s(a, "name");
    let name = name.split(" (").next().unwrap_or(name).to_string();
    Some(StatAction {
        name,
        area,
        around,
        range_ft,
        save: Ability::parse(dc.pointer("/dc_type/index").and_then(Value::as_str).unwrap_or("dex")).unwrap_or(Ability::Dex),
        dc: dc.get("dc_value").and_then(Value::as_i64).unwrap_or(10) as i32,
        dice: damage.map_or(String::new(), |d| s(d, "damage_dice").replace(' ', "")),
        damage_type: damage.and_then(|d| d.pointer("/damage_type/index")).and_then(Value::as_str).and_then(DamageType::parse),
        // (The data says "none" for some whose text says "half as much": the text wins.)
        half: dc.get("success_type").and_then(Value::as_str) == Some("half") || l.contains("half as much"),
        condition,
        rounds,
        recharge,
        per_day,
        legendary_only,
    })
}

/// A monster's save actions: its actions' and its legendary actions' (those only legendary).
fn monster_actions(m: &Value) -> Vec<rustpg2d_engine::srd::StatAction> {
    let mut out: Vec<rustpg2d_engine::srd::StatAction> = list(m, "actions").iter().filter_map(|a| save_action(a, false)).collect();
    for a in list(m, "legendary_actions") {
        if let Some(x) = save_action(&a, true).filter(|x| !out.iter().any(|o| o.name == x.name)) {
            out.push(x);
        }
    }
    out
}

/// A monster's legendary actions that act: an attack it names ("The dragon makes a tail
/// attack") or one of its save actions, with the cost ("Costs 2 Actions").
fn monster_legendary(m: &Value) -> Vec<rustpg2d_engine::srd::StatLegendary> {
    let attacks: Vec<String> = list(m, "actions").iter().filter(|a| a.get("attack_bonus").is_some()).map(|a| s(a, "name").to_string()).collect();
    list(m, "legendary_actions")
        .iter()
        .filter_map(|a| {
            let full = s(a, "name");
            let cost = number_after_word(&full.to_ascii_lowercase(), "(costs ").unwrap_or(1);
            let name = full.split(" (").next().unwrap_or(full).to_string();
            let text = desc(a).to_ascii_lowercase();
            let action = if a.get("dc").is_some() && a.get("attack_bonus").is_none() { name.clone() } else { String::new() };
            // "makes a tail attack", "makes one claw attack", "makes a bite attack".
            let attack = if action.is_empty() {
                attacks.iter().find(|n| text.contains(&format!(" {} attack", n.to_ascii_lowercase()))).cloned().unwrap_or_default()
            } else {
                String::new()
            };
            (!attack.is_empty() || !action.is_empty()).then_some(rustpg2d_engine::srd::StatLegendary { name, attack, action, cost })
        })
        .collect()
}

/// Regeneration's numbers from its text: "The troll regains 10 hit points at the start of its
/// turn. If the troll takes acid or fire damage, this trait doesn't function at the start of the
/// troll's next turn. The troll dies only if it starts its turn with 0 hit points and doesn't
/// regenerate."
pub fn regeneration(text: &str) -> Option<rustpg2d_engine::srd::Regeneration> {
    use rustpg2d_engine::srd::DamageType;
    let l = text.to_ascii_lowercase();
    let hp = number_after_word(&l, "regains ")? as i32;
    let stopped_by = l
        .split(". ")
        .find(|s| s.contains("doesn't function") || s.contains("does not function"))
        .and_then(|s| s.split("takes ").nth(1))
        .map(|s| s.split(" damage").next().unwrap_or("").split(|c: char| !c.is_ascii_alphabetic()).filter_map(DamageType::parse).collect())
        .unwrap_or_default();
    Some(rustpg2d_engine::srd::Regeneration { hp, stopped_by, revives: l.contains("dies only if it starts its turn with 0 hit points") })
}

/// A monster's *Spellcasting* and *Innate Spellcasting* from the data's structured part: the DC,
/// the spell attack bonus (the data's modifier, else DC − 8), the slots and every spell (innate
/// ones at will or so many times a day). None without either.
fn monster_spellcasting(m: &Value) -> Option<rustpg2d_engine::srd::StatSpellcasting> {
    use rustpg2d_engine::srd::{Ability, StatSpell, StatSpellcasting};
    let mut out: Option<StatSpellcasting> = None;
    for a in &list(m, "special_abilities") {
        let Some(sc) = a.get("spellcasting") else { continue };
        let innate = s(a, "name").to_ascii_lowercase().contains("innate");
        let dc = sc.get("dc").and_then(Value::as_i64).unwrap_or(10) as i32;
        let modifier = sc.get("modifier").and_then(Value::as_i64).unwrap_or(0) as i32;
        let ability = sc.pointer("/ability/index").and_then(Value::as_str).and_then(Ability::parse).unwrap_or(Ability::Int);
        let o = out.get_or_insert_with(|| StatSpellcasting { ability, dc, attack: if modifier > 0 { modifier } else { dc - 8 }, ..Default::default() });
        // (The leveled spellcasting's DC wins over an innate one's.)
        if !innate {
            (o.ability, o.dc, o.attack) = (ability, dc, if modifier > 0 { modifier } else { dc - 8 });
            o.level = sc.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
            if let Some(slots) = sc.get("slots").and_then(Value::as_object) {
                let mut v = vec![0u32; 9];
                for (lvl, n) in slots {
                    if let (Ok(l), Some(n)) = (lvl.parse::<usize>(), n.as_u64()) {
                        if (1..=9).contains(&l) {
                            v[l - 1] = n as u32;
                        }
                    }
                }
                while v.last() == Some(&0) {
                    v.pop();
                }
                o.slots = v;
            }
        }
        for sp in &list(sc, "spells") {
            let id = s(sp, "url").rsplit('/').next().unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            let level = sp.get("level").and_then(Value::as_u64).unwrap_or(0) as u32;
            let per_day = match sp.pointer("/usage/type").and_then(Value::as_str) {
                Some("per day") => sp.pointer("/usage/times").and_then(Value::as_u64).unwrap_or(1) as u32,
                _ => 0,
            };
            o.spells.push(StatSpell { spell: id, level, innate, per_day });
        }
    }
    out
}

/// An SRD action with an attack roll as a stat block attack: its bonus, the first damage (of a
/// choice, the first option: a spear's one-handed damage), reach and range from its text.
fn stat_attack(a: &Value) -> Option<rustpg2d_engine::srd::StatAttack> {
    let bonus = a.get("attack_bonus")?.as_i64()? as i32;
    let dmg = list(a, "damage");
    let first = dmg.first()?;
    // A choice ("choose": 1, "from": {"options": [{damage_dice…}]}) or the damage itself.
    let d = first.pointer("/from/options/0").unwrap_or(first);
    let dice = s(d, "damage_dice").to_string();
    if dice.is_empty() {
        return None;
    }
    let damage_type = d.pointer("/damage_type/index").and_then(Value::as_str).and_then(rustpg2d_engine::srd::DamageType::parse);
    // More parts of other types ("plus 7 (2d6) fire damage"; a choice's first option).
    let extra = dmg
        .iter()
        .skip(1)
        .map(|p| p.pointer("/from/options/0").unwrap_or(p))
        .filter_map(|p| {
            let dice = s(p, "damage_dice").replace(' ', "");
            let t = p.pointer("/damage_type/index").and_then(Value::as_str).and_then(rustpg2d_engine::srd::DamageType::parse);
            (!dice.is_empty()).then_some(rustpg2d_engine::srd::DamagePart { dice, damage_type: t })
        })
        .collect();
    let text = desc(a);
    let feet_after = |word: &str| -> Option<&str> { text.split(word).nth(1).map(|t| t.trim_start()) };
    let reach_ft = feet_after("reach ").and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    let (range_ft, long_ft) = feet_after("range ")
        .and_then(|t| t.split(' ').next())
        .and_then(|r| r.split_once('/').map(|(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(0))))
        .unwrap_or((0, 0));
    let on_hit = hit_riders(&text);
    let versatile = two_handed_dice(&text);
    Some(rustpg2d_engine::srd::StatAttack { name: s(a, "name").into(), bonus, dice: dice.replace(' ', ""), damage_type, reach_ft, range_ft, long_ft, versatile, extra, on_hit })
}

/// A versatile weapon's damage with two hands from an attack's text: "…, or 6 (1d8 + 2) piercing
/// damage if used with two hands to make a melee attack" → `1d8+2`; "" when it says nothing.
pub fn two_handed_dice(text: &str) -> String {
    let l = text.to_ascii_lowercase();
    let Some(at) = l.find("if used with two hands") else { return String::new() };
    let before = &l[..at];
    let Some(open) = before.rfind('(') else { return String::new() };
    let inner = before[open + 1..].split(')').next().unwrap_or("");
    let dice: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
    if rustpg2d_engine::srd::Expr::parse(&dice).is_ok() { dice } else { String::new() }
}

/// What an attack's text says a hit does besides its damage: "must succeed on a DC 11 Strength
/// saving throw or be knocked prone", "must make a DC 11 Constitution saving throw, taking 9 (2d8)
/// poison damage on a failed save, or half as much damage on a successful one", "the target is
/// grappled (escape DC 13)", "restrained", "poisoned for 1 minute" (rounds: 1 minute = 10).
pub fn hit_riders(text: &str) -> Vec<rustpg2d_engine::srd::HitRider> {
    use rustpg2d_engine::srd::{Ability, Condition, DamageType, HitRider};
    let l = text.to_ascii_lowercase();
    // Only what follows the hit ("Hit: 7 (2d4 + 2) piercing damage. If the target…").
    let Some(after) = l.split_once("hit:").map(|(_, r)| r) else { return Vec::new() };
    let number_after = |s: &str, word: &str| -> Option<i32> { s.split(word).nth(1)?.split(|c: char| !c.is_ascii_digit()).find(|w| !w.is_empty())?.parse().ok() };
    let conditions = [
        ("knocked prone", Condition::Prone),
        ("grappled", Condition::Grappled),
        ("restrained", Condition::Restrained),
        ("poisoned", Condition::Poisoned),
        ("paralyzed", Condition::Paralyzed),
        ("frightened", Condition::Frightened),
        ("stunned", Condition::Stunned),
        ("blinded", Condition::Blinded),
        ("charmed", Condition::Charmed),
        ("incapacitated", Condition::Incapacitated),
    ];
    let mut out = Vec::new();
    // Each sentence on its own: a save with its condition or damage, or a condition outright.
    for sentence in after.split(". ") {
        let save = sentence.find("saving throw").and_then(|_| {
            let dc = number_after(sentence, "dc ")?;
            let ability = ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"].iter().position(|a| sentence.contains(&format!("{a} saving throw")))?;
            Some((Ability::ALL[ability], dc))
        });
        // (A condition only when the sentence gives it: "or be knocked prone", "is grappled".)
        let given = |w: &str| sentence.contains(&format!("be {w}")) || sentence.contains(&format!("is {w}")) || sentence.contains(&format!("becomes {w}")) || sentence.contains(&format!("or {w}"));
        let condition = conditions.iter().find(|(w, _)| given(w) && !sentence.contains(&format!("while {w}")) && !sentence.contains(&format!("isn't {w}"))).map(|c| c.1);
        // Damage on a failed save: "taking 9 (2d8) poison damage on a failed save".
        let damage = save.and_then(|_| {
            let rest = sentence.split("taking ").nth(1)?;
            let dice: String = rest.split('(').nth(1)?.split(')').next()?.chars().filter(|c| !c.is_whitespace()).collect();
            let word = rest.split(')').nth(1)?.split_whitespace().next()?;
            Some((dice, DamageType::parse(word)))
        });
        // "or contract a disease" (the diseased giant rat's bite): the library's rat-bite fever.
        let disease = if sentence.contains("contract a disease") { RAT_DISEASE.to_string() } else { String::new() };
        if save.is_none() && condition.is_none() {
            continue;
        }
        if save.is_some() && condition.is_none() && damage.is_none() && disease.is_empty() {
            continue;
        }
        let rounds = if sentence.contains("for 1 minute") {
            10
        } else if sentence.contains("for 1 hour") {
            600
        } else if sentence.contains("until the end of") {
            1
        } else {
            0
        };
        let escape_dc = number_after(sentence, "escape dc").unwrap_or(0);
        out.push(HitRider {
            save: save.map(|s| s.0),
            dc: save.map_or(0, |s| s.1),
            condition,
            rounds,
            dice: damage.as_ref().map_or(String::new(), |d| d.0.clone()),
            damage_type: damage.and_then(|d| d.1),
            half: sentence.contains("half as much"),
            escape_dc,
            disease,
        });
    }
    out
}

/// The disease of a bite that says "contract a disease" (the starter library's `diseases.ron`).
pub const RAT_DISEASE: &str = "rat_bite_fever";

/// "The captain makes three melee attacks" -> 3.
fn multiattack_count(text: &str) -> u32 {
    const WORDS: [(&str, u32); 5] = [("two", 2), ("three", 3), ("four", 4), ("five", 5), ("six", 6)];
    let t = text.to_ascii_lowercase();
    t.split(|c: char| !c.is_ascii_alphanumeric()).find_map(|w| WORDS.iter().find(|(n, _)| *n == w).map(|(_, k)| *k)).unwrap_or(2)
}

/// A list of texts of a JSON object ("damage_resistances": ["poison"]).
fn strings(v: &Value, key: &str) -> Vec<String> {
    list(v, key).iter().filter_map(Value::as_str).map(str::to_string).collect()
}

/// Writes the NPC templates the engine builds in (`crates/engine/src/srd/npcs.ron`).
pub fn write_npcs(checkout: &Path, out: &Path) -> Result<usize> {
    let t = SrdData::read(checkout)?.npc_templates();
    std::fs::write(out, rustpg2d_engine::project::ron_text(&t)?).with_context(|| format!("writing {}", out.display()))?;
    Ok(t.len())
}

/// The SRD's own words for the engine's constants (`crates/engine/src/srd/rules.ron`):
/// conditions, damage types, languages, alignments, abilities, skills, and the ids of its races
/// and backgrounds.
pub fn rules_text(checkout: &Path) -> Result<rustpg2d_engine::srd::rules::RulesText> {
    use rustpg2d_engine::srd::rules::{Alignment, Entry, Language, RulesText};
    let en = checkout.join("src/2014/en");
    let read = |name: &str| -> Result<Vec<Value>> {
        let p = en.join(format!("5e-SRD-{name}.json"));
        let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))
    };
    let entries = |l: Vec<Value>| l.iter().map(|v| Entry { id: s(v, "index").into(), name: s(v, "name").into(), text: desc(v) }).collect::<Vec<_>>();
    let skills = read("Skills")?;
    let data = SrdData::read(checkout)?;
    Ok(RulesText {
        conditions: entries(read("Conditions")?),
        damage_types: entries(read("Damage-Types")?),
        languages: read("Languages")?
            .iter()
            .map(|v| Language { id: s(v, "index").into(), name: s(v, "name").into(), kind: s(v, "type").into(), speakers: list(v, "typical_speakers").iter().filter_map(Value::as_str).map(str::to_string).collect(), script: s(v, "script").into() })
            .collect(),
        alignments: read("Alignments")?.iter().map(|v| Alignment { id: s(v, "index").into(), name: s(v, "name").into(), abbreviation: s(v, "abbreviation").into(), text: desc(v) }).collect(),
        // (The abilities by their full name: "Strength".)
        abilities: read("Ability-Scores")?.iter().map(|v| Entry { id: s(v, "index").into(), name: s(v, "full_name").into(), text: desc(v) }).collect(),
        skill_abilities: skills.iter().map(|v| (s(v, "index").to_string(), v.pointer("/ability_score/index").and_then(Value::as_str).unwrap_or("").to_string())).collect(),
        skills: entries(skills),
        races: data.races.iter().chain(&data.subraces).map(|v| s(v, "index").to_string()).collect(),
        backgrounds: data.backgrounds.iter().map(|v| s(v, "index").to_string()).collect(),
        monsters: data.monster_ids(),
    })
}

/// Writes [`rules_text`] (`crates/engine/src/srd/rules.ron`).
pub fn write_rules(checkout: &Path, out: &Path) -> Result<()> {
    let t = rules_text(checkout)?;
    std::fs::write(out, rustpg2d_engine::project::ron_text(&t)?).with_context(|| format!("writing {}", out.display()))
}

/// Gives the characters of a project (the example) the SRD's monster data, without adding
/// monsters (`SrdData::import_monsters`), and the SRD's classes; returns how many characters
/// changed.
pub fn update_monsters_in(checkout: &Path, root: &Path) -> Result<usize> {
    let data = SrdData::read(checkout)?;
    let mut p = rustpg2d_engine::project::Project::load(root)?;
    let (_, updated) = data.import_monsters(&mut p.db.characters, false);
    if updated > 0 {
        std::fs::write(root.join("defs/characters.ron"), rustpg2d_engine::project::ron_text(&p.db.characters)?)?;
    }
    data.import_classes(&mut p.db.classes);
    std::fs::write(root.join("defs/classes.ron"), rustpg2d_engine::project::ron_text(&p.db.classes)?)?;
    // The equipment its classes start with, the coins, and the SRD's numbers on its weapons and armor.
    let wanted: Vec<String> = p.db.classes.iter().flat_map(|c| c.equipment.iter().map(|i| i.item.clone())).collect();
    data.import_equipment(&mut p.db.items, false, &wanted);
    // The magic items it shows (and its potion as the Potion of Healing).
    let wanted: Vec<String> = EXAMPLE_MAGIC.iter().map(|x| x.to_string()).collect();
    data.import_magic_items(&mut p.db.items, &p.db.spells, false, &wanted);
    write_items(&mut p, root)?;
    Ok(updated)
}

/// Writes `defs/items.ron` and, when an item shows a generic equipment icon, adds the
/// `srd_gear` sheet (its picture `sprites/srd_gear.png` comes with the starter library).
fn write_items(p: &mut rustpg2d_engine::project::Project, root: &Path) -> Result<()> {
    let mut sheets = false;
    for (sheet, file) in [(gear::SHEET, "sprites/srd_gear.png"), (magic_icon::SHEET, "sprites/srd_magic.png")] {
        if p.db.items.iter().any(|i| i.icon.sheet == sheet) && !p.db.sheets.iter().any(|s| s.id == sheet) {
            p.db.sheets.push(rustpg2d_engine::defs::SheetDef::new(sheet, file, 32, 32));
            sheets = true;
        }
    }
    if sheets {
        std::fs::write(root.join("defs/sheets.ron"), rustpg2d_engine::project::ron_text(&p.db.sheets)?)?;
    }
    std::fs::write(root.join("defs/items.ron"), rustpg2d_engine::project::ron_text(&p.db.items)?)?;
    Ok(())
}

/// Imports into the `defs/` of a library or project folder; returns a one-line report.
pub fn import_into(checkout: &Path, root: &Path) -> Result<String> {
    let data = SrdData::read(checkout)?;
    let mut p = rustpg2d_engine::project::Project::load(root)?;
    let (added, updated) = data.import(&mut p.db.races, &mut p.db.backgrounds);
    let (m_added, m_updated) = data.import_monsters(&mut p.db.characters, true);
    let (c_added, c_updated, unmapped) = data.import_classes(&mut p.db.classes);
    let (s_added, s_updated, story) = data.import_spells(&mut p.db.spells);
    let (e_added, e_updated) = data.import_equipment(&mut p.db.items, true, &[]);
    let (mi_added, mi_updated, mi_story) = data.import_magic_items(&mut p.db.items, &p.db.spells, true, &[]);
    if e_added + e_updated + mi_added + mi_updated > 0 {
        write_items(&mut p, root)?;
    }
    let defs = root.join("defs");
    if s_added + s_updated > 0 {
        std::fs::write(defs.join("spells.ron"), rustpg2d_engine::project::ron_text(&p.db.spells)?)?;
    }
    std::fs::write(defs.join("races.ron"), rustpg2d_engine::project::ron_text(&p.db.races)?)?;
    std::fs::write(defs.join("backgrounds.ron"), rustpg2d_engine::project::ron_text(&p.db.backgrounds)?)?;
    if m_added + m_updated > 0 {
        std::fs::write(defs.join("characters.ron"), rustpg2d_engine::project::ron_text(&p.db.characters)?)?;
    }
    std::fs::write(defs.join("classes.ron"), rustpg2d_engine::project::ron_text(&p.db.classes)?)?;
    Ok(format!(
        "{}: {added} added, {updated} updated ({} races, {} backgrounds); monsters: {m_added} added, {m_updated} characters updated ({} characters); classes: {c_added} added, {c_updated} updated (starting equipment without a library item: {}); spells: {s_added} added, {s_updated} updated, {} of {} story only; equipment: {e_added} added, {e_updated} updated; magic items: {mi_added} added, {mi_updated} updated, {mi_story} story only",
        root.display(),
        p.db.races.len(),
        p.db.backgrounds.len(),
        p.db.characters.len(),
        if unmapped.is_empty() { "-".to_string() } else { unmapped.join(", ") },
        story.len(),
        data.spells.len()
    ))
}

/// The frames of the generic magic item icons (`sprites/srd_magic.png`, sheet `srd_magic`).
mod magic_icon {
    pub const SHEET: &str = "srd_magic";
    pub const POTION: u32 = 0;
    pub const RING: u32 = 1;
    pub const WAND: u32 = 2;
    pub const STAFF: u32 = 3;
    pub const ROD: u32 = 4;
    pub const SCROLL: u32 = 5;
    pub const AMULET: u32 = 6;
    pub const CLOAK: u32 = 7;
    pub const BELT: u32 = 8;
    pub const BOOTS: u32 = 9;
    pub const GLOVES: u32 = 10;
    pub const BRACERS: u32 = 11;
    pub const HELM: u32 = 12;
    pub const WONDROUS: u32 = 13;
    pub const BAG: u32 = 14;
    pub const FIGURINE: u32 = 15;
    pub const STONE: u32 = 16;
    /// How many frames the sheet has.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const FRAMES: u32 = 17;
}

/// The generic +1 items the library gets ready-made (the SRD's +1 / +2 / +3 weapons, armor and
/// ammunition are a bonus any such item can have): (id, name, the SRD item it is a copy of).
const PLUS_ONE: [(&str, &str, &str); 6] = [
    ("longsword-plus-1", "+1 Longsword", "longsword"),
    ("dagger-plus-1", "+1 Dagger", "dagger"),
    ("shortbow-plus-1", "+1 Shortbow", "shortbow"),
    ("arrow-plus-1", "+1 Arrows", "arrow"),
    ("chain-mail-plus-1", "+1 Chain Mail", "chain-mail"),
    ("shield-plus-1", "+1 Shield", "shield"),
];

/// The silvered items the library gets ready-made (the SRD's 100 gp silvering of a weapon or ten
/// pieces of ammunition): (id, name, the SRD item it is a copy of).
const SILVERED: [(&str, &str, &str); 2] = [("dagger-silvered", "Silvered Dagger", "dagger"), ("arrow-silvered", "Silvered Arrows", "arrow")];

/// Library items that are an SRD magic item (they keep their id, name, look, triggers and price
/// and get its rules): (magic item index, library item id).
const MAGIC_STANDS_FOR: [(&str, &str); 2] = [("potion-of-healing-common", "potion"), ("potion-of-giant-strength-hill", "elixir_strength")];

/// The magic items the example project shows (added to it by the import).
pub const EXAMPLE_MAGIC: [&str; 3] = ["ring-of-protection", "wand-of-magic-missiles", "longsword-plus-1"];

/// The SRD's generic magic items that are a bonus on any item: not imported (see [`PLUS_ONE`]),
/// nor the spell scrolls (an item that *teaches* a spell is a scroll of it).
fn generic_magic(index: &str) -> bool {
    ["weapon-1", "weapon-2", "weapon-3", "armor-1", "armor-2", "armor-3", "ammunition-1", "ammunition-2", "ammunition-3"].contains(&index) || index.starts_with("spell-scroll")
}

/// What a magic item does by the rules (the hand-made part of the import): its bonus, passive
/// effects, spells cast from it, what using it does.
#[derive(Default)]
struct MagicRules {
    bonus: i32,
    effects: Vec<rustpg2d_engine::classes::FeatureEffect>,
    /// (spell, charges, slot (0 = its own), up to slot, DC)
    spells: Vec<(&'static str, u32, u32, u32, u32)>,
    use_parts: Vec<rustpg2d_engine::skills::SpellPart>,
    use_seconds: f32,
    /// A worn item's slot when its name doesn't tell.
    slot: Option<rustpg2d_engine::defs::EquipSlot>,
    /// A backpack's extra inventory slots.
    bag_slots: u32,
}

/// The rules of SRD magic item `index` (None: a story item, its effect is up to scripts).
fn magic_rules(index: &str) -> Option<MagicRules> {
    use rustpg2d_engine::classes::{BonusTo, FeatureEffect as F};
    use rustpg2d_engine::skills::SpellPart as P;
    use rustpg2d_engine::srd::conditions::{Buff, Condition};
    use rustpg2d_engine::srd::Ability as A;
    let res = |t: &str| F::Resistance { types: vec![t.to_string()] };
    let score = |a: A, n: i32| F::AbilityScore { ability: a, score: n };
    let bonus = |to: BonusTo, n: i32| F::Bonus { to, amount: n };
    let hour = 3600.0;
    let mut r = MagicRules::default();
    // The ten damage types of rings, potions and armor of resistance, and the dragons' colours.
    const TYPES: [&str; 10] = ["acid", "cold", "fire", "force", "lightning", "necrotic", "poison", "psychic", "radiant", "thunder"];
    const DRAGONS: [(&str, &str); 10] = [("black", "acid"), ("blue", "lightning"), ("brass", "fire"), ("bronze", "lightning"), ("copper", "acid"), ("gold", "fire"), ("green", "poison"), ("red", "fire"), ("silver", "cold"), ("white", "cold")];
    const GIANTS: [(&str, i32); 6] = [("hill", 21), ("stone", 23), ("frost", 23), ("fire", 25), ("cloud", 27), ("storm", 29)];
    if let Some(t) = index.strip_prefix("ring-of-resistance-") {
        r.effects = vec![res(t)];
        return TYPES.contains(&t).then_some(r);
    }
    if let Some(t) = index.strip_prefix("potion-of-resistance-") {
        r.use_parts = vec![P::Buff { buff: Buff::Resistance { types: vec![t.to_string()] } }];
        r.use_seconds = hour;
        return TYPES.contains(&t).then_some(r);
    }
    if let Some(c) = index.strip_prefix("dragon-scale-mail-") {
        let (_, t) = DRAGONS.iter().find(|(d, _)| *d == c)?;
        (r.bonus, r.effects) = (1, vec![res(t), F::SaveAdvantage { ability: A::Dex }]);
        return Some(r);
    }
    if let Some(g) = index.strip_prefix("belt-of-giant-strength-") {
        r.effects = vec![score(A::Str, GIANTS.iter().find(|(x, _)| *x == g)?.1)];
        return Some(r);
    }
    if let Some(g) = index.strip_prefix("potion-of-giant-strength-") {
        r.use_parts = vec![P::Buff { buff: Buff::AbilityScore { ability: A::Str, score: GIANTS.iter().find(|(x, _)| *x == g)?.1 } }];
        r.use_seconds = hour;
        return Some(r);
    }
    if let Some(n) = index.strip_prefix("wand-of-the-war-mage-") {
        r.effects = vec![bonus(BonusTo::SpellAttack, n.parse().ok()?)];
        return Some(r);
    }
    match index {
        // Potions.
        "potion-of-healing-common" => r.use_parts = vec![P::Heal { by_level: vec![(0, "2d4 + 2".into())] }],
        "potion-of-healing-greater" => r.use_parts = vec![P::Heal { by_level: vec![(0, "4d4 + 4".into())] }],
        "potion-of-healing-superior" => r.use_parts = vec![P::Heal { by_level: vec![(0, "8d4 + 8".into())] }],
        "potion-of-healing-supreme" => r.use_parts = vec![P::Heal { by_level: vec![(0, "10d4 + 20".into())] }],
        "potion-of-heroism" => {
            r.use_parts = vec![P::TempHp { by_level: vec![(0, "10".into())] }, P::Buff { buff: Buff::AttackDie { dice: "1d4".into(), minus: false } }, P::Buff { buff: Buff::SaveDie { dice: "1d4".into(), minus: false } }];
            r.use_seconds = hour;
        }
        "potion-of-speed" => {
            r.use_parts = vec![P::Buff { buff: Buff::Haste }];
            r.use_seconds = 60.0;
        }
        "potion-of-invisibility" => {
            r.use_parts = vec![P::Condition { condition: Condition::Invisible, on_fail: false }];
            r.use_seconds = hour;
        }
        // Rings.
        "ring-of-protection" => r.effects = vec![bonus(BonusTo::Ac, 1), bonus(BonusTo::Saves, 1)],
        "ring-of-evasion" => r.effects = vec![F::Evasion],
        "ring-of-free-action" => r.effects = vec![F::ConditionImmunity { conditions: vec!["paralyzed".into(), "restrained".into()] }],
        "ring-of-warmth" => r.effects = vec![res("cold")],
        "ring-of-invisibility" => r.spells = vec![("invisibility", 0, 0, 0, 0)],
        "ring-of-animal-influence" => r.spells = vec![("animal-friendship", 1, 0, 0, 13), ("fear", 1, 0, 0, 13)],
        // Wands.
        "wand-of-magic-missiles" => r.spells = vec![("magic-missile", 1, 1, 7, 0)],
        "wand-of-fireballs" => r.spells = vec![("fireball", 1, 3, 9, 15)],
        "wand-of-lightning-bolts" => r.spells = vec![("lightning-bolt", 1, 3, 9, 15)],
        "wand-of-web" => r.spells = vec![("web", 1, 0, 0, 15)],
        "wand-of-fear" => r.spells = vec![("command", 1, 0, 0, 15), ("fear", 2, 0, 0, 15)],
        "wand-of-binding" => r.spells = vec![("hold-person", 2, 0, 0, 17), ("hold-monster", 5, 0, 0, 17)],
        // Staffs (also quarterstaffs) and rods.
        "staff-of-healing" => r.spells = vec![("cure-wounds", 1, 1, 4, 0), ("mass-cure-wounds", 5, 0, 0, 0)],
        "staff-of-fire" => {
            r.effects = vec![res("fire")];
            r.spells = vec![("burning-hands", 1, 0, 0, 0), ("fireball", 3, 0, 0, 0), ("wall-of-fire", 4, 0, 0, 0)];
        }
        "staff-of-frost" => {
            r.effects = vec![res("cold")];
            r.spells = vec![("ice-storm", 4, 0, 0, 0), ("wall-of-ice", 4, 0, 0, 0), ("cone-of-cold", 5, 0, 0, 0)];
        }
        "staff-of-striking" => r.bonus = 3,
        "staff-of-thunder-and-lightning" => r.bonus = 2,
        "staff-of-power" => {
            r.bonus = 2;
            r.effects = vec![bonus(BonusTo::Ac, 2), bonus(BonusTo::Saves, 2), bonus(BonusTo::SpellAttack, 2)];
            r.spells = vec![("magic-missile", 1, 1, 0, 0), ("lightning-bolt", 5, 5, 0, 0), ("fireball", 5, 5, 0, 0), ("cone-of-cold", 5, 0, 0, 0), ("hold-monster", 5, 0, 0, 0)];
        }
        "staff-of-the-magi" => {
            r.bonus = 2;
            r.effects = vec![bonus(BonusTo::SpellAttack, 2)];
            r.spells = vec![("fireball", 7, 7, 0, 0), ("lightning-bolt", 7, 7, 0, 0), ("ice-storm", 4, 0, 0, 0), ("wall-of-fire", 4, 0, 0, 0), ("web", 2, 0, 0, 0), ("light", 0, 0, 0, 0)];
        }
        "staff-of-the-woodlands" => {
            r.bonus = 2;
            r.effects = vec![bonus(BonusTo::SpellAttack, 2)];
        }
        "staff-of-the-python" => r.bonus = 2,
        "rod-of-lordly-might" => r.bonus = 3,
        // Weapons.
        "flame-tongue" => r.effects = vec![F::ExtraDamage { dice: vec![(1, "2d6".into())], damage_type: Some(rustpg2d_engine::srd::DamageType::Fire), hurt_target: false }],
        "frost-brand" => r.effects = vec![F::ExtraDamage { dice: vec![(1, "1d6".into())], damage_type: Some(rustpg2d_engine::srd::DamageType::Cold), hurt_target: false }, res("fire")],
        "dragon-slayer" | "giant-slayer" | "mace-of-smiting" | "dagger-of-venom" => r.bonus = 1,
        "luck-blade" => (r.bonus, r.effects) = (1, vec![bonus(BonusTo::Saves, 1)]),
        "berserker-axe" => (r.bonus, r.effects) = (1, vec![F::HpPerLevel { hp: 1 }]),
        "hammer-of-thunderbolts" => r.bonus = 1,
        "sun-blade" | "nine-lives-stealer" | "scimitar-of-speed" => r.bonus = 2,
        "holy-avenger" | "defender" | "vorpal-sword" | "dwarven-thrower" => r.bonus = 3,
        "defender-1" => r.bonus = 3,
        // Armor and shields.
        "dwarven-plate" => r.bonus = 2,
        "elven-chain" => (r.bonus, r.effects) = (1, vec![F::Proficient { what: "medium-armor".into() }]),
        "glamoured-studded-leather" | "demon-armor" => r.bonus = 1,
        "armor-of-invulnerability" => r.effects = vec![F::Resistance { types: vec!["bludgeoning".into(), "piercing".into(), "slashing".into()] }],
        "mithral-armor" => {}
        // Wondrous items.
        "amulet-of-health" => r.effects = vec![score(A::Con, 19)],
        "gauntlets-of-ogre-power" => r.effects = vec![score(A::Str, 19)],
        "headband-of-intellect" => r.effects = vec![score(A::Int, 19)],
        "cloak-of-protection" => r.effects = vec![bonus(BonusTo::Ac, 1), bonus(BonusTo::Saves, 1)],
        // (+2 AC without armor or shield: an unarmored formula of 12 + DEX.)
        "bracers-of-defense" => r.effects = vec![F::ArmorClass { base: 12, abilities: vec![A::Dex], shield: false }],
        "robe-of-the-archmagi" => {
            r.effects = vec![F::ArmorClass { base: 15, abilities: vec![A::Dex], shield: false }, bonus(BonusTo::SpellDc, 2), bonus(BonusTo::SpellAttack, 2)];
            r.slot = Some(rustpg2d_engine::defs::EquipSlot::Cloak);
        }
        "boots-of-elvenkind" | "cloak-of-elvenkind" => r.effects = vec![F::CheckAdvantage { skill: "stealth".into() }],
        "boots-of-the-winterlands" => r.effects = vec![res("cold")],
        "goggles-of-night" => r.effects = vec![F::Darkvision { feet: 60 }],
        "eyes-of-the-eagle" => r.effects = vec![F::CheckAdvantage { skill: "perception".into() }],
        "eyes-of-minute-seeing" => r.effects = vec![F::CheckAdvantage { skill: "investigation".into() }],
        "periapt-of-proof-against-poison" => r.effects = vec![F::Immunity { types: vec!["poison".into()] }, F::ConditionImmunity { conditions: vec!["poisoned".into()] }],
        "brooch-of-shielding" => r.effects = vec![res("force")],
        "cloak-of-arachnida" => r.effects = vec![res("poison")],
        "stone-of-good-luck-luckstone" => {
            r.effects = vec![bonus(BonusTo::Saves, 1)];
            r.slot = Some(rustpg2d_engine::defs::EquipSlot::Neck);
        }
        "ioun-stone-protection" => r.effects = vec![bonus(BonusTo::Ac, 1)],
        "ioun-stone-agility" => r.effects = vec![F::AbilityBonus { ability: A::Dex, amount: 2, max: 20 }],
        "ioun-stone-fortitude" => r.effects = vec![F::AbilityBonus { ability: A::Con, amount: 2, max: 20 }],
        "ioun-stone-insight" => r.effects = vec![F::AbilityBonus { ability: A::Wis, amount: 2, max: 20 }],
        "ioun-stone-intellect" => r.effects = vec![F::AbilityBonus { ability: A::Int, amount: 2, max: 20 }],
        "ioun-stone-leadership" => r.effects = vec![F::AbilityBonus { ability: A::Cha, amount: 2, max: 20 }],
        "ioun-stone-strength" => r.effects = vec![F::AbilityBonus { ability: A::Str, amount: 2, max: 20 }],
        "bag-of-holding" => r.bag_slots = 12,
        "handy-haversack" => r.bag_slots = 8,
        _ => return None,
    }
    Some(r)
}

/// The SRD weapon or armor a magic weapon / armor is made of, by its first line ("Weapon (any
/// sword), rare", "Armor (plate), legendary"): an SRD equipment index.
fn magic_base(index: &str, line: &str) -> &'static str {
    let l = line.to_ascii_lowercase();
    let what = l.split_once('(').and_then(|(_, r)| r.split(')').next()).unwrap_or("");
    match index {
        "staff-of-the-woodlands" | "staff-of-power" | "staff-of-the-magi" | "staff-of-striking" | "staff-of-thunder-and-lightning" | "staff-of-the-python" | "staff-of-fire" | "staff-of-frost" | "staff-of-healing" | "staff-of-charming" | "staff-of-swarming-insects" | "staff-of-withering" => return "quarterstaff",
        "rod-of-lordly-might" => return "mace",
        "elven-chain" => return "chain-shirt",
        "mithral-armor" | "adamantine-armor" => return "chain-mail",
        "demon-armor" | "dwarven-plate" | "plate-armor-of-etherealness" | "armor-of-invulnerability" => return "plate-armor",
        _ => {}
    }
    for (word, base) in [
        ("greatsword", "greatsword"),
        ("longsword", "longsword"),
        ("scimitar", "scimitar"),
        ("dagger", "dagger"),
        ("javelin", "javelin"),
        ("warhammer", "warhammer"),
        ("maul", "maul"),
        ("trident", "trident"),
        ("mace", "mace"),
        ("longbow", "longbow"),
        ("arrow", "arrow"),
        ("axe", "battleaxe"),
        ("sword", "longsword"),
        ("scale mail", "scale-mail"),
        ("studded leather", "studded-leather-armor"),
        ("chain shirt", "chain-shirt"),
        ("chain mail", "chain-mail"),
        ("plate", "plate-armor"),
        ("shield", "shield"),
        ("ammunition", "arrow"),
    ] {
        if what.contains(word) {
            return base;
        }
    }
    if l.starts_with("armor") {
        "chain-mail"
    } else if l.starts_with("weapon") {
        "longsword"
    } else {
        ""
    }
}

/// A worn magic item's slot and icon by its name.
fn magic_kind(index: &str, category: &str) -> (Option<rustpg2d_engine::defs::EquipSlot>, u32) {
    use rustpg2d_engine::defs::EquipSlot as S;
    use magic_icon as I;
    match category {
        "potion" => return (None, I::POTION),
        "ring" => return (Some(S::Ring), I::RING),
        "wand" => return (None, I::WAND),
        "staff" => return (None, I::STAFF),
        "rod" => return (None, I::ROD),
        "scroll" => return (None, I::SCROLL),
        _ => {}
    }
    let has = |w: &[&str]| w.iter().any(|x| index.split('-').any(|p| p == *x));
    if has(&["amulet", "necklace", "periapt", "medallion", "brooch", "scarab"]) {
        (Some(S::Neck), I::AMULET)
    } else if has(&["cloak", "cape", "mantle", "robe"]) {
        (Some(S::Cloak), I::CLOAK)
    } else if has(&["belt", "girdle"]) {
        (Some(S::Belt), I::BELT)
    } else if has(&["boots", "slippers"]) {
        (Some(S::Feet), I::BOOTS)
    } else if has(&["gloves", "gauntlets"]) {
        (Some(S::Hands), I::GLOVES)
    } else if has(&["bracers"]) {
        (Some(S::Hands), I::BRACERS)
    } else if has(&["helm", "hat", "headband", "circlet", "goggles", "eyes", "ioun"]) {
        (Some(S::Head), I::HELM)
    } else if has(&["bag", "haversack", "portable"]) {
        (None, I::BAG)
    } else if has(&["figurine"]) {
        (None, I::FIGURINE)
    } else if has(&["stone", "gem", "pearl", "bead"]) {
        (None, I::STONE)
    } else {
        (None, I::WONDROUS)
    }
}

impl SrdData {
    /// The SRD's magic items into `items` (after the equipment: magic weapons and armor copy
    /// their base item, its look included), and the six ready-made +1 items. Items of an earlier
    /// run (`srd` = the magic item's index) are replaced, keeping their own look; new ones are
    /// added when `add` (a library), else only those in `wanted`. Spells cast from an item are
    /// left out when `spells` lacks them. Returns (added, updated, story items).
    pub fn import_magic_items(&self, items: &mut Vec<rustpg2d_engine::defs::ItemDef>, spells: &[rustpg2d_engine::skills::SpellDef], add: bool, wanted: &[String]) -> (usize, usize, usize) {
        use rustpg2d_engine::defs::*;
        use rustpg2d_engine::srd::magic_items::{parse_attunement, parse_charges, ItemSpell, MagicItem, Rarity};
        // The item an SRD equipment index is in `items` (a library item standing for it).
        let base_of = |items: &[ItemDef], index: &str| -> Option<ItemDef> {
            let standing = library_item(index);
            items.iter().find(|d| d.magic.is_none() && (Some(d.id.as_str()) == standing || d.id == index || d.srd == index)).cloned()
        };
        let mut defs: Vec<ItemDef> = Vec::new();
        let mut story = 0;
        for v in &self.magic_items {
            let index = s(v, "index");
            let has_variants = v.get("variants").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
            if has_variants || generic_magic(index) {
                continue;
            }
            let lines: Vec<&str> = v.get("desc").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let first = lines.first().copied().unwrap_or("");
            let text = lines.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
            let category = v.pointer("/equipment_category/index").and_then(Value::as_str).unwrap_or("");
            let rarity = Rarity::parse(v.pointer("/rarity/name").and_then(Value::as_str).unwrap_or("")).unwrap_or(Rarity::Uncommon);
            let (attunement, attune_by) = parse_attunement(first);
            let (charges, recharge) = parse_charges(&text);
            let rules = magic_rules(index);
            if rules.is_none() {
                story += 1;
            }
            let rules = rules.unwrap_or_default();
            // Weapons, armor, staffs and rods are their base item made magic.
            let base = if matches!(category, "weapon" | "armor" | "ammunition" | "staff") || index == "rod-of-lordly-might" { magic_base(index, first) } else { "" };
            let mut d = match (!base.is_empty()).then(|| base_of(items, base)).flatten() {
                Some(b) => ItemDef { id: index.into(), magic: None, cost_cp: 0, ..b },
                None => ItemDef::fresh(index),
            };
            d.name = s(v, "name").into();
            d.description = text.clone();
            d.srd = if base.is_empty() { index.into() } else { base.into() };
            let (slot, icon) = magic_kind(index, category);
            if base.is_empty() || d.icon.sheet.is_empty() {
                d.icon = SpriteRef::new(magic_icon::SHEET, icon);
            }
            if let Some(slot) = rules.slot.or(slot).filter(|_| d.weapon.is_none() && d.armor.is_none()) {
                d.armor = Some(Armor { category: Some(ArmorCategory::Clothing), ..Armor::clothing(slot) });
                d.max_stack = 1;
            }
            if rules.bag_slots > 0 {
                d.armor = Some(Armor { category: Some(ArmorCategory::Clothing), ..Armor::clothing(EquipSlot::Back) });
                (d.bag_slots, d.max_stack, d.icon) = (rules.bag_slots, 1, SpriteRef::new(magic_icon::SHEET, magic_icon::BAG));
            }
            if index == "mithral-armor" {
                if let Some(a) = &mut d.armor {
                    (a.str_min, a.stealth_disadvantage) = (0, false);
                }
            }
            if category == "potion" {
                d.consumable = true;
                d.max_stack = 10;
                if rules.use_parts.iter().any(|p| matches!(p, rustpg2d_engine::skills::SpellPart::Heal { .. })) {
                    d.use_on = UseTarget::Choose;
                }
            }
            if charges > 0 {
                d.max_stack = 1;
            }
            let spells_of: Vec<ItemSpell> = rules
                .spells
                .iter()
                .filter(|(sp, ..)| spells.iter().any(|x| x.id == *sp && !x.story_only))
                .map(|(sp, ch, slot, up, dc)| ItemSpell { spell: sp.to_string(), charges: *ch, slot: *slot, upcast_to: *up, dc: *dc })
                .collect();
            d.magic = Some(MagicItem {
                rarity,
                attunement,
                attune_by,
                bonus: rules.bonus,
                effects: rules.effects,
                charges,
                recharge,
                destroy_on_last: text.contains("last charge") && (text.contains("crumbles") || text.contains("destroyed")),
                spells: spells_of,
                use_parts: rules.use_parts,
                use_seconds: rules.use_seconds,
            });
            // The SRD's prices: potions of healing its own, the others by rarity.
            d.cost_cp = match index {
                "potion-of-healing-common" => 5_000,
                "potion-of-healing-greater" => 15_000,
                "potion-of-healing-superior" => 45_000,
                "potion-of-healing-supreme" => 135_000,
                _ => rarity.price_cp(),
            };
            d.value = d.cost_cp / 100;
            defs.push(d);
        }
        // The ready-made +1 items: copies of their SRD item.
        for (id, name, base) in PLUS_ONE {
            let Some(b) = base_of(items, base) else { continue };
            let mut d = ItemDef { id: id.into(), name: name.into(), ..b };
            d.description = format!("{} A magic one: +1 to {}.", d.description.trim(), if d.armor.is_some() { "armor class" } else { "attack and damage rolls" }).trim().to_string();
            d.srd = base.into();
            d.magic = Some(MagicItem { rarity: Rarity::Uncommon, bonus: 1, ..Default::default() });
            d.cost_cp = Rarity::Uncommon.price_cp();
            d.value = d.cost_cp / 100;
            // (The arrows' +1 counts when they are shot.)
            defs.push(d);
        }
        // The ready-made silvered items: their SRD item with the silvering's price.
        for (id, name, base) in SILVERED {
            let Some(b) = base_of(items, base) else { continue };
            let ammo = b.max_stack > 1;
            let mut d = ItemDef { id: id.into(), name: name.into(), ..b };
            d.description = format!("{} Silvered: it harms creatures that only silver hurts (a werewolf's resistance to nonmagical attacks that aren't silvered).", d.description.trim()).trim().to_string();
            d.srd = base.into();
            d.silvered = true;
            // (100 gp for a weapon or ten pieces of ammunition.)
            d.cost_cp += if ammo { 1_000 } else { 10_000 };
            d.value = (d.cost_cp / 100).max(1);
            defs.push(d);
        }
        let (mut added, mut updated) = (0, 0);
        for new in defs {
            // A library item standing for it: only the rules (its id, name, look and triggers stay).
            if let Some((_, id)) = MAGIC_STANDS_FOR.iter().find(|(i, _)| *i == new.id) {
                if let Some(old) = items.iter_mut().find(|d| d.id == *id) {
                    let before = old.clone();
                    // (Its price stays: the project's economy.)
                    (old.magic, old.srd, old.use_on) = (new.magic.clone(), new.id.clone(), new.use_on);
                    if *old != before {
                        updated += 1;
                    }
                    continue;
                }
            }
            match items.iter_mut().find(|d| d.id == new.id) {
                Some(old) => {
                    // An earlier run's item: the SRD's data, its own look kept.
                    let (icon, model, held) = (old.icon.clone(), old.model.clone(), old.held.clone());
                    let before = old.clone();
                    *old = new;
                    (old.icon, old.model, old.held) = (icon, model, held);
                    if *old != before {
                        updated += 1;
                    }
                }
                None if add || wanted.contains(&new.id) => {
                    items.push(new);
                    added += 1;
                }
                None => {}
            }
        }
        (added, updated, story)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Save actions, legendary actions and traits that act come from the data and the texts.
    #[test]
    fn monster_abilities_come_from_the_data() {
        use rustpg2d_engine::skills::Shape;
        use rustpg2d_engine::srd::{Ability, Condition, DamageType};
        assert_eq!(action_area("exhales fire in a 60-foot cone").0.map(|a| (a.shape, a.feet)), Some((Shape::Cone, 60)));
        assert_eq!(action_area("acid in a 30-foot line that is 5 feet wide").0.map(|a| (a.shape, a.feet)), Some((Shape::Line, 30)));
        let around = action_area("Each creature within 10 ft. of the dragon must succeed");
        assert_eq!((around.0.map(|a| (a.shape, a.feet)), around.1), (Some((Shape::Sphere, 10)), true));
        assert_eq!(action_area("one creature it can see within 30 feet of it").2, 30);
        let m: Value = serde_json::json!({
            "actions": [
                {"name": "Tail", "attack_bonus": 14, "desc": "Melee Weapon Attack: +14 to hit, reach 15 ft."},
                {"name": "Fire Breath", "desc": "The dragon exhales fire in a 60-foot cone. Each creature in that area must make a DC 21 Dexterity saving throw, taking 63 (18d6) fire damage on a failed save, or half as much damage on a successful one.",
                 "usage": {"type": "recharge on roll", "dice": "1d6", "min_value": 5},
                 "dc": {"dc_type": {"index": "dex"}, "dc_value": 21, "success_type": "none"},
                 "damage": [{"damage_type": {"index": "fire"}, "damage_dice": "18d6"}]}],
            "legendary_actions": [
                {"name": "Detect", "desc": "The dragon makes a Wisdom (Perception) check."},
                {"name": "Tail Attack", "desc": "The dragon makes a tail attack."},
                {"name": "Wing Attack (Costs 2 Actions)", "desc": "Each creature within 10 ft. of the dragon must succeed on a DC 22 Dexterity saving throw or take 15 (2d6 + 8) bludgeoning damage and be knocked prone.",
                 "dc": {"dc_type": {"index": "dex"}, "dc_value": 22, "success_type": "none"}, "damage": [{"damage_type": {"index": "bludgeoning"}, "damage_dice": "2d6+8"}]}]});
        let acts = monster_actions(&m);
        let breath = &acts[0];
        // (The data says "none"; the text's "half as much" wins.)
        assert_eq!((breath.name.as_str(), breath.save, breath.dc, breath.dice.as_str(), breath.damage_type, breath.half, breath.recharge), ("Fire Breath", Ability::Dex, 21, "18d6", Some(DamageType::Fire), true, 5));
        let wing = &acts[1];
        assert!(wing.legendary_only && wing.around && wing.condition == Some(Condition::Prone));
        let leg = monster_legendary(&m);
        assert_eq!(leg.iter().map(|l| (l.name.as_str(), l.attack.as_str(), l.action.as_str(), l.cost)).collect::<Vec<_>>(), [("Tail Attack", "Tail", "", 1), ("Wing Attack", "", "Wing Attack", 2)]);
        let r = regeneration("The troll regains 10 hit points at the start of its turn. If the troll takes acid or fire damage, this trait doesn't function at the start of the troll's next turn. The troll dies only if it starts its turn with 0 hit points and doesn't regenerate.").unwrap();
        assert_eq!((r.hp, r.stopped_by, r.revives), (10, vec![DamageType::Acid, DamageType::Fire], true));
    }

    #[test]
    fn spellcasting_comes_from_the_structured_data() {
        let m: Value = serde_json::json!({"special_abilities": [
            {"name": "Spellcasting", "spellcasting": {"level": 1, "ability": {"index": "wis"}, "dc": 12, "modifier": 4, "slots": {"1": 3}, "spells": [
                {"name": "Sacred Flame", "level": 0, "url": "/api/2014/spells/sacred-flame"},
                {"name": "Cure Wounds", "level": 1, "url": "/api/2014/spells/cure-wounds"}]}},
            {"name": "Innate Spellcasting", "spellcasting": {"ability": {"index": "cha"}, "dc": 11, "modifier": 0, "spells": [
                {"name": "Darkness", "level": 2, "usage": {"type": "per day", "times": 1}, "url": "/api/2014/spells/darkness"}]}}]});
        let sc = monster_spellcasting(&m).unwrap();
        assert_eq!((sc.ability, sc.dc, sc.attack, sc.slots.clone()), (rustpg2d_engine::srd::Ability::Wis, 12, 4, vec![3]));
        assert_eq!(sc.spells.iter().map(|s| (s.spell.as_str(), s.level, s.innate, s.per_day)).collect::<Vec<_>>(), vec![("sacred-flame", 0, false, 0), ("cure-wounds", 1, false, 0), ("darkness", 2, true, 1)]);
        assert!(monster_spellcasting(&serde_json::json!({"special_abilities": []})).is_none());
    }

    #[test]
    fn versatile_damage_comes_from_the_attack_text() {
        assert_eq!(two_handed_dice("Melee or Ranged Weapon Attack: +3 to hit, reach 5 ft. or range 20/60 ft., one target. Hit: 4 (1d6 + 1) piercing damage, or 5 (1d8 + 1) piercing damage if used with two hands to make a melee attack."), "1d8+1");
        assert_eq!(two_handed_dice("Melee Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 5 (1d6 + 2) slashing damage."), "");
    }

    #[test]
    fn hit_riders_come_from_the_attack_text() {
        use rustpg2d_engine::srd::{Ability, Condition, DamageType};
        let wolf = hit_riders("Melee Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 7 (2d4 + 2) piercing damage. If the target is a creature, it must succeed on a DC 11 Strength saving throw or be knocked prone.");
        assert_eq!(wolf.len(), 1);
        assert_eq!((wolf[0].save, wolf[0].dc, wolf[0].condition), (Some(Ability::Str), 11, Some(Condition::Prone)));
        let spider = hit_riders("Melee Weapon Attack: +5 to hit, reach 5 ft., one creature. Hit: 7 (1d8 + 3) piercing damage, and the target must make a DC 11 Constitution saving throw, taking 9 (2d8) poison damage on a failed save, or half as much damage on a successful one. If the poison damage reduces the target to 0 hit points, the target is stable but poisoned for 1 hour, even after regaining hit points, and is paralyzed while poisoned in this way.");
        assert_eq!((spider[0].save, spider[0].dc, spider[0].dice.as_str(), spider[0].damage_type, spider[0].half), (Some(Ability::Con), 11, "2d8", Some(DamageType::Poison), true));
        assert!(spider[0].condition.is_none(), "the poison is damage: {spider:?}");
        let grab = hit_riders("Melee Weapon Attack: +6 to hit, reach 5 ft., one target. Hit: 11 (2d6 + 4) bludgeoning damage, and the target is grappled (escape DC 13). Until this grapple ends, the creature is restrained, and the snake can't constrict another target.");
        assert_eq!((grab[0].save, grab[0].condition, grab[0].escape_dc), (None, Some(Condition::Grappled), 13));
        let ghoul = hit_riders("Melee Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 7 (2d4 + 2) slashing damage. If the target is a creature other than an elf or undead, it must succeed on a DC 10 Constitution saving throw or be paralyzed for 1 minute. The target can repeat the saving throw at the end of each of its turns, ending the effect on itself on a success.");
        assert_eq!((ghoul[0].condition, ghoul[0].rounds, ghoul[0].dc), (Some(Condition::Paralyzed), 10, 10));
        assert!(hit_riders("Melee Weapon Attack: +14 to hit, reach 10 ft., one target. Hit: 19 (2d10 + 8) piercing damage plus 7 (2d6) fire damage.").is_empty());
    }

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/srd")
    }

    #[test]
    fn races_and_backgrounds_import_and_a_second_run_changes_nothing() {
        let data = SrdData::read(&fixture()).unwrap();
        // An existing dwarf keeps its looks and faction; its old speed percent, HP and defense go.
        let mut races = vec![RaceDef { id: "dwarf".into(), name: "Dwarf".into(), playable: true, bodies_male: vec!["dwarf".into()], faction: "dwarves".into(), speed: -10.0, max_hp: 3, defense: 1, ..Default::default() }];
        let mut bgs = Vec::new();
        let (added, updated) = data.import(&mut races, &mut bgs);
        assert_eq!((added, updated), (4, 1), "{:?}", races.iter().map(|r| &r.id).collect::<Vec<_>>());
        let dwarf = races.iter().find(|r| r.id == "dwarf").unwrap();
        assert_eq!((dwarf.bodies_male.len(), dwarf.faction.as_str(), dwarf.speed, dwarf.walk_ft, dwarf.playable), (1, "dwarves", 0.0, 25, false));
        assert_eq!((dwarf.max_hp, dwarf.defense), (0, 0));
        assert_eq!(dwarf.ability_bonuses.get("con"), Some(&2));
        assert_eq!((dwarf.darkvision_ft, dwarf.resistances.clone()), (60, vec!["poison".to_string()]));
        assert!(dwarf.skills.iter().any(|s| s.skill == "battleaxes"));
        let tool = dwarf.choices.iter().find(|c| c.kind == ChoiceKind::Tool).expect("the dwarf's tool");
        assert_eq!(tool.options, vec!["smiths-tools", "brewers-supplies", "masons-tools"]);
        let hill = races.iter().find(|r| r.id == "hill-dwarf").unwrap();
        assert_eq!((hill.parent.as_str(), hill.playable), ("dwarf", true));
        let merged = hill.merged(Some(dwarf));
        assert_eq!((merged.ability_bonuses.get("con"), merged.ability_bonuses.get("wis")), (Some(&2), Some(&1)));
        assert_eq!(merged.hp_per_level, 1, "Dwarven Toughness");
        let half_elf = races.iter().find(|r| r.id == "half-elf").unwrap();
        let kinds: Vec<(ChoiceKind, u32)> = half_elf.choices.iter().map(|c| (c.kind, c.choose)).collect();
        assert_eq!(kinds, vec![(ChoiceKind::Ability, 2), (ChoiceKind::Language, 1), (ChoiceKind::Skill, 2)]);
        let dragonborn = races.iter().find(|r| r.id == "dragonborn").unwrap();
        let anc = dragonborn.choices.iter().find(|c| c.kind == ChoiceKind::Ancestry).unwrap();
        assert_eq!(anc.grants.get("draconic-ancestry-red").map(String::as_str), Some("fire"));
        assert_eq!(bgs[0].proficiencies, vec!["insight", "religion"]);
        assert_eq!((bgs[0].language_choices, bgs[0].feature.as_str()), (2, "Shelter of the Faithful"));
        // A project's colours for the ancestry's options stay through another run.
        let anc_k = races.iter().position(|r| r.id == "dragonborn").unwrap();
        let ci = races[anc_k].choices.iter().position(|c| c.kind == ChoiceKind::Ancestry).unwrap();
        races[anc_k].choices[ci].colors.insert("draconic-ancestry-red".into(), [("skin".to_string(), [186, 50, 42])].into());
        // A second run: nothing changes.
        let (r1, b1) = (races.clone(), bgs.clone());
        data.import(&mut races, &mut bgs);
        assert_eq!((races, bgs), (r1, b1));
    }

    #[test]
    fn a_folder_gets_its_races_and_backgrounds_files() {
        assert_eq!(SRD_COMMIT.len(), 40);
        let root = std::env::temp_dir().join(format!("rustpg2d-srd-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("defs")).unwrap();
        let p = rustpg2d_engine::project::Project { root: root.clone(), config: rustpg2d_engine::project::ProjectConfig::new("Library"), db: Default::default() };
        p.save_config().unwrap();
        let first = import_into(&fixture(), &root).unwrap();
        assert!(first.contains("5 added"), "{first}");
        let read = |f: &str| std::fs::read(root.join(f)).unwrap();
        let (races, bgs) = (read("defs/races.ron"), read("defs/backgrounds.ron"));
        write_rules(&fixture(), &root.join("rules.ron")).unwrap();
        let rules = read("rules.ron");
        import_into(&fixture(), &root).unwrap();
        write_rules(&fixture(), &root.join("rules.ron")).unwrap();
        // A second run writes the same files, byte for byte.
        assert_eq!((read("defs/races.ron"), read("defs/backgrounds.ron"), read("rules.ron")), (races, bgs, rules));
        let chars = read("defs/characters.ron");
        let loaded = rustpg2d_engine::project::Project::load(&root).unwrap();
        assert_eq!((loaded.db.races.len(), loaded.db.backgrounds.len(), loaded.db.characters.len()), (4, 1, 5), "the fixture's five monsters");
        // A project's characters only get the data (the monsters are there already: nothing changes).
        assert_eq!(update_monsters_in(&fixture(), &root).unwrap(), 0);
        assert_eq!(read("defs/characters.ron"), chars);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_rules_text_comes_from_the_data() {
        let t = rules_text(&fixture()).unwrap();
        let ids = |l: &[rustpg2d_engine::srd::rules::Entry]| l.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&t.conditions), ["poisoned", "prone"]);
        assert!(t.conditions[0].text.contains("disadvantage on attack rolls"));
        assert_eq!(ids(&t.damage_types), ["fire", "poison"]);
        assert_eq!((t.languages[1].id.as_str(), t.languages[1].kind.as_str(), t.languages[1].script.as_str()), ("draconic", "Exotic", "Draconic"));
        assert_eq!((t.alignments[0].abbreviation.as_str(), t.abilities[0].name.as_str()), ("LG", "Strength"));
        assert_eq!(t.skill_abilities.get("stealth").map(String::as_str), Some("dex"));
        assert!(t.races.contains(&"hill-dwarf".to_string()) && t.backgrounds == ["acolyte"]);
        // The NPC templates carry their alignment.
        assert_eq!(SrdData::read(&fixture()).unwrap().npc_templates()[1].alignment, "any alignment");
    }

    #[test]
    fn npc_templates_come_from_the_humanoids() {
        let t = SrdData::read(&fixture()).unwrap().npc_templates();
        let names: Vec<&str> = t.iter().map(|s| s.template.as_str()).collect();
        assert_eq!(names, ["bandit", "guard"], "no lycanthropes, no beasts");
        let g = &t[1];
        assert_eq!((g.ac, g.hp, g.hit_dice.as_str(), g.speed_ft, g.cr.as_str(), g.xp), (16, 11, "2d8+2", 30, "1/8", 25));
        assert_eq!(g.proficiencies, vec!["perception"]);
        // Its attack: the spear, one-handed (the first damage option), melee and thrown.
        let spear = &g.attacks[0];
        assert_eq!((spear.name.as_str(), spear.bonus, spear.dice.as_str(), spear.reach_ft, spear.range_ft, spear.long_ft), ("Spear", 3, "1d6+1", 5, 20, 60));
        assert_eq!(spear.damage_type, Some(rustpg2d_engine::srd::DamageType::Piercing));
        assert_eq!((g.multiattack, multiattack_count("The captain makes three melee attacks")), (0, 3));
        // Written as the engine reads them, and the same as the built-in guard.
        let out = std::env::temp_dir().join(format!("rustpg2d_npcs_{}.ron", std::process::id()));
        assert_eq!(write_npcs(&fixture(), &out).unwrap(), 2);
        let back: Vec<rustpg2d_engine::srd::StatBlock> = ron::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let _ = std::fs::remove_file(&out);
        assert_eq!(back, t);
        assert_eq!(Some(&back[1]), rustpg2d_engine::srd::npc_template("guard"));
    }

    #[test]
    fn monsters_import_as_characters_and_a_second_run_changes_nothing() {
        use rustpg2d_engine::srd::{FeatureKind, Size};
        let data = SrdData::read(&fixture()).unwrap();
        // A hand-made wolf of the library (its look stays) and a templated guard.
        let mut wolf = CharacterDef::fresh("wolf");
        (wolf.max_hp, wolf.appearance.model) = (6, "wolf".into());
        let mut guard = CharacterDef::fresh("town_guard");
        guard.stats = Some(rustpg2d_engine::srd::StatBlock { template: "guard".into(), hp: 20, ..Default::default() });
        let mut chars = vec![wolf, guard];
        let (added, updated) = data.import_monsters(&mut chars, true);
        assert_eq!((added, updated), (4, 2), "bandit, guard, werewolf, dragon added; wolf and town_guard updated");
        let w = &chars[0];
        let b = w.stats.as_ref().unwrap();
        assert_eq!((w.max_hp, w.xp, b.template.as_str(), b.ac, w.appearance.model.as_str()), (11, 50, "wolf", 13, "wolf"));
        assert!(b.features.iter().any(|f| f.kind == FeatureKind::Trait && f.name == "Pack Tactics"));
        assert!(b.senses.contains("passive Perception 13"), "{}", b.senses);
        // The guard keeps its own hit points and gains the SRD's size, type and texts.
        let g = chars[1].stats.as_ref().unwrap();
        assert_eq!((g.hp, g.creature_type.as_str(), g.attacks.is_empty()), (20, "humanoid (any race)", false));
        // A new monster: the dragon, Huge, flying, three attacks, legendary actions, hostile.
        let d = chars.iter().find(|c| c.id == "adult-red-dragon").unwrap();
        let db = d.stats.as_ref().unwrap();
        assert_eq!((d.name.as_str(), d.hostile, d.appearance.size, db.size, db.speeds.fly, db.speeds.climb, db.multiattack), ("Adult Red Dragon", true, 200, Size::Huge, 80, 40, 3));
        assert_eq!(db.attacks.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Bite", "Claw", "Tail"]);
        assert!(db.features.iter().any(|f| f.kind == FeatureKind::Legendary) && db.senses.contains("blindsight 60 ft."));
        assert_eq!((d.max_hp, d.xp), (db.hp, db.xp));
        // Humanoids live among others; a second run changes nothing.
        assert!(!chars.iter().find(|c| c.id == "guard").unwrap().hostile);
        let before = chars.clone();
        assert_eq!(data.import_monsters(&mut chars, true), (0, 0));
        assert_eq!(chars, before);
        // Without adding (a project): only what is there gets the SRD's data.
        let mut only = vec![CharacterDef::fresh("wolf")];
        assert_eq!(data.import_monsters(&mut only, false), (0, 1));
        assert_eq!(only.len(), 1);
    }


    #[test]
    fn spells_import_with_their_mechanics() {
        use rustpg2d_engine::skills::{Shape, SpellPart as P, SpellRange};
        use rustpg2d_engine::srd::conditions::{Buff, Condition};
        let data = SrdData::read(&fixture()).unwrap();
        let mut spells = Vec::new();
        let (added, updated, story) = data.import_spells(&mut spells);
        assert_eq!((added, updated), (9, 0));
        let get = |id: &str| spells.iter().find(|d| d.id == id).unwrap();
        // Fireball: level 3, 150 ft, a DEX save for half in a 20 ft sphere, 8d6 fire and 1d6 a slot.
        let fb = get("fireball");
        assert_eq!((fb.level, fb.range, fb.area.map(|a| (a.shape, a.feet)), fb.classes.clone()), (3, SpellRange::Feet(150), Some((Shape::Sphere, 20)), vec!["sorcerer".to_string(), "wizard".to_string()]));
        assert!(matches!(&fb.parts[..], [P::Save { half: true, .. }, P::Damage { by_level, .. }] if by_level[0] == (3, "8d6".to_string()) && by_level[1] == (4, "9d6".to_string())));
        // Fire Bolt: a cantrip spell attack whose dice grow with the caster's level.
        assert!(matches!(&get("fire-bolt").parts[..], [P::Attack { ranged: true }, P::Damage { by_level, .. }] if by_level.iter().any(|(l, d)| *l == 5 && d == "2d10")));
        // Cure Wounds heals with the modifier; Hold Person paralyzes on a failed WIS save (with concentration).
        assert!(matches!(&get("cure-wounds").parts[..], [P::Heal { by_level }] if by_level[0].1 == "1d8 + MOD"));
        let hp = get("hold-person");
        assert!(hp.concentration && hp.duration_rounds == 10 && matches!(&hp.parts[..], [P::Save { half: false, .. }, P::Condition { condition: Condition::Paralyzed, on_fail: true }]));
        // Bless: three creatures and one a slot, a d4 on attacks and saves; Magic Missile hits.
        assert!(get("bless").parts.contains(&P::Buff { buff: Buff::AttackDie { dice: "1d4".into(), minus: false } }));
        assert!(matches!(&get("magic-missile").parts[..], [P::Damage { .. }]));
        // Light without its save; Sleep's hit point pool; Detect Magic's glow.
        assert!(matches!(&get("light").parts[..], [P::Light { .. }]));
        assert!(matches!(&get("sleep").parts[..], [P::HpPool { condition: Condition::Unconscious, .. }]));
        assert!(matches!(&get("detect-magic").parts[..], [P::Buff { buff: Buff::Detect { .. } }]));
        assert!(!story.contains(&"detect-magic".to_string()) && !story.contains(&"sleep".to_string()));
        // A second run changes nothing; an existing spell keeps its icon.
        spells.iter_mut().find(|d| d.id == "light").unwrap().icon = rustpg2d_engine::defs::SpriteRef::new("items", 27);
        let before = spells.clone();
        assert_eq!(data.import_spells(&mut spells).1, 0);
        assert_eq!(spells, before);
    }

    #[test]
    fn classes_import_with_their_levels_features_and_subclasses() {
        use rustpg2d_engine::classes::{ClassTraits, FeatureChoice, FeatureEffect, FightingStyle, HeroClass};
        let data = SrdData::read(&fixture()).unwrap();
        let mut classes = Vec::new();
        let (added, updated, unmapped) = data.import_classes(&mut classes);
        assert_eq!((added, updated), (3, 0));
        assert!(unmapped.contains(&"dungeoneers-pack".to_string()) && !unmapped.contains(&"dagger".to_string()), "only what the fixture's equipment lacks: {unmapped:?}");
        let r = classes.iter().find(|c| c.id == "rogue").unwrap();
        assert!(["thieves-tools", "dagger", "burglars-pack", "arrows"].iter().all(|i| r.equipment.iter().any(|e| e.item == *i)), "{:?}", r.equipment);
        let f = classes.iter().find(|c| c.id == "fighter").unwrap();
        assert_eq!((f.hit_die, f.saves.clone(), f.subclass_level, f.subclass_title.as_str()), (10, vec!["str".to_string(), "con".to_string()], 3, "Martial Archetype"));
        assert_eq!(f.skill_choices, 2);
        assert!(f.equipment.iter().any(|i| i.item == "chainmail") && f.equipment.iter().any(|i| i.item == "iron_sword"), "{:?}", f.equipment);
        assert_eq!(f.feature("fighter-fighting-style").and_then(|x| x.choice.clone()).map(|c| matches!(c, FeatureChoice::Options { choose: 1, .. })), Some(true));
        assert_eq!(f.feature("fighter-fighting-style-archery").unwrap().effects, vec![FeatureEffect::FightingStyle { style: FightingStyle::Archery }]);
        assert!(f.improvement_at(4) && f.improvement_at(6) && !f.improvement_at(5));
        // A level-5 champion with archery: two attacks, crits on 19.
        let hc = HeroClass { class: "fighter".into(), subclass: "champion".into(), picks: [("fighter-fighting-style".to_string(), vec!["fighter-fighting-style-archery".to_string()])].into() };
        let t = ClassTraits::of(f, 5, &hc);
        assert_eq!((t.attacks, t.crit_from, t.styles.clone()), (2, 19, vec![FightingStyle::Archery]));
        // The rogue's numbers: sneak attack dice by level; expertise is a choice.
        let r = classes.iter().find(|c| c.id == "rogue").unwrap();
        assert_eq!((r.number(1, "sneak_attack.dice_count"), r.number(5, "sneak_attack.dice_count"), r.hit_die), (1, 3, 8));
        assert_eq!(r.feature("rogue-expertise-1").and_then(|x| x.choice.clone()), Some(FeatureChoice::Expertise { count: 2 }));
        let w = classes.iter().find(|c| c.id == "wizard").unwrap();
        assert_eq!((w.hit_die, w.subclass_level, w.subclasses[0].id.as_str()), (6, 2, "evocation"));
        assert!(w.number(1, "spellcasting.spell_slots_level_1") >= 2, "the slots for the spells item");
        // The features that act: a fighter's Second Wind and Action Surge with their resources, a
        // rogue's Sneak Attack, Cunning Action and Uncanny Dodge.
        let abilities = rustpg2d_engine::srd::Abilities::default();
        let t2 = ClassTraits::of(f, 2, &hc);
        let acts: Vec<&str> = t2.acts.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(acts, ["second-wind", "action-surge"]);
        assert_eq!((t2.resource_max(f, "second-wind", &abilities), t2.resource_max(f, "action-surge", &abilities)), (Some(1), Some(1)));
        assert_eq!(t2.recharge("action-surge"), rustpg2d_engine::classes::Recharge::ShortRest);
        assert!(ClassTraits::of(f, 1, &hc).resource_max(f, "action-surge", &abilities).is_none(), "action surge comes at 2");
        let rt = ClassTraits::of(r, 5, &HeroClass { class: "rogue".into(), ..Default::default() });
        assert!(rt.riders.contains(&FeatureEffect::SneakAttack) && rt.riders.contains(&FeatureEffect::ReactionHalveDamage), "{:?}", rt.riders);
        assert_eq!(rt.acts.iter().filter(|a| a.id.starts_with("cunning-action")).count(), 3);
        // The wizard casts with INT from a spellbook; a fighter doesn't cast.
        let sc = w.spellcasting.clone().expect("the wizard casts");
        assert_eq!((sc.ability, sc.kind, sc.ritual), (rustpg2d_engine::srd::Ability::Int, rustpg2d_engine::classes::CasterKind::Spellbook, true));
        assert!(f.spellcasting.is_none());
        // A second run changes nothing.
        let before = classes.clone();
        assert_eq!(data.import_classes(&mut classes).1, 0);
        assert_eq!(classes, before);
    }


    #[test]
    fn magic_items_import_with_their_rules_and_a_second_run_changes_nothing() {
        use rustpg2d_engine::classes::{BonusTo, FeatureEffect};
        use rustpg2d_engine::defs::*;
        use rustpg2d_engine::srd::magic_items::Rarity;
        let data = SrdData::read(&fixture()).unwrap();
        let mut sword = ItemDef::fresh("iron_sword");
        (sword.name, sword.icon) = ("Iron Sword".into(), SpriteRef::new("icons_3d", 4));
        sword.weapon = Some(Weapon { dice: "1d8".into(), ..Weapon::melee(3, 0.35) });
        let mut potion = ItemDef::fresh("potion");
        (potion.name, potion.consumable) = ("Red Potion".into(), true);
        let mut items = vec![sword, potion];
        data.import_equipment(&mut items, true, &[]);
        let spells: Vec<rustpg2d_engine::skills::SpellDef> = ["magic-missile", "cure-wounds"].iter().map(|id| rustpg2d_engine::skills::SpellDef::fresh(id)).collect();
        let (added, updated, story) = data.import_magic_items(&mut items, &spells, true, &[]);
        // 12 entries: the potion of healing (variants), the +1 weapon and the scroll aren't items; the
        // common potion of healing is the library's potion; six ready-made +1 items, two silvered.
        assert_eq!((added, updated, story), (12 - 3 - 1 + 6 + 2, 1, 1), "{:?}", items.iter().map(|i| &i.id).collect::<Vec<_>>());
        let get = |id: &str| items.iter().find(|i| i.id == id).cloned().unwrap();
        let silver = get("arrow-silvered");
        assert!(silver.silvered && silver.magic.is_none() && silver.max_stack > 1, "{silver:?}");
        let ring = get("ring-of-protection");
        let m = ring.magic.as_ref().unwrap();
        assert_eq!((m.rarity, m.attunement, ring.armor.as_ref().map(|a| a.slot)), (Rarity::Rare, true, Some(EquipSlot::Ring)));
        assert_eq!(m.effects, vec![FeatureEffect::Bonus { to: BonusTo::Ac, amount: 1 }, FeatureEffect::Bonus { to: BonusTo::Saves, amount: 1 }]);
        assert_eq!(ring.icon.sheet, "srd_magic");
        let wand = get("wand-of-magic-missiles").magic.unwrap();
        assert_eq!((wand.charges, wand.recharge.as_str(), wand.destroy_on_last, wand.spells[0].spell.as_str(), wand.spells[0].upcast_to), (7, "1d6+1", true, "magic-missile", 7));
        // The library's potion keeps its name and gets the healing; the greater one is new.
        let p = get("potion");
        assert_eq!((p.name.as_str(), p.srd.as_str(), p.use_on), ("Red Potion", "potion-of-healing-common", UseTarget::Choose));
        assert!(matches!(&p.magic.unwrap().use_parts[0], rustpg2d_engine::skills::SpellPart::Heal { by_level } if by_level[0].1 == "2d4 + 2"));
        assert_eq!(get("potion-of-healing-greater").cost_cp, 15_000);
        // A flaming longsword is the library's sword made magic (its look kept).
        let ft = get("flame-tongue");
        assert_eq!((ft.icon.sheet.as_str(), ft.srd.as_str(), ft.weapon.as_ref().map(|w| w.dice.clone())), ("icons_3d", "longsword", Some("1d8".into())));
        let plus = get("longsword-plus-1");
        assert_eq!((plus.name.as_str(), plus.magic_bonus(), plus.icon.sheet.as_str()), ("+1 Longsword", 1, "icons_3d"));
        assert_eq!(get("belt-of-giant-strength-hill").armor.map(|a| a.slot), Some(EquipSlot::Belt));
        assert!(get("mithral-armor").armor.is_some_and(|a| a.str_min == 0 && !a.stealth_disadvantage));
        assert!(get("deck-of-many-things").magic.unwrap().story_only());
        assert!(!items.iter().any(|i| i.id == "weapon-1" || i.id.starts_with("spell-scroll")));
        // The staff of healing's spells: only those the project has (no mass cure wounds here).
        assert_eq!(get("staff-of-healing").magic.unwrap().spells.len(), 1);
        // The equipment import leaves the magic longswords alone.
        let before = items.clone();
        data.import_equipment(&mut items, true, &[]);
        assert_eq!(data.import_magic_items(&mut items, &spells, true, &[]), (0, 0, 1));
        assert_eq!(items, before);
        // Every generic icon is on the sheet.
        assert!(items.iter().filter(|i| i.icon.sheet == magic_icon::SHEET).all(|i| i.icon.frame < magic_icon::FRAMES));
    }

    #[test]
    fn equipment_imports_with_the_srd_rules_and_a_second_run_changes_nothing() {
        use rustpg2d_engine::defs::*;
        let data = SrdData::read(&fixture()).unwrap();
        // A library's iron sword and chainmail stand for the longsword and chain mail; its coin is gold.
        let mut sword = ItemDef::fresh("iron_sword");
        (sword.name, sword.icon) = ("Iron Sword".into(), SpriteRef::new("icons_3d", 4));
        sword.weapon = Some(Weapon { dice: "1d8".into(), ..Weapon::melee(3, 0.35) });
        let mut mail = ItemDef::fresh("chainmail");
        mail.armor = Some(Armor { defense: 3, speed: -0.15, ..Armor::clothing(EquipSlot::Body) });
        let mut items = vec![sword, mail, ItemDef { id: "coin".into(), name: "Coin".into(), ..Default::default() }];
        let (added, updated) = data.import_equipment(&mut items, true, &[]);
        assert_eq!((added, updated), (25 - 2 + 4, 3), "{:?}", items.iter().map(|i| &i.id).collect::<Vec<_>>());
        let get = |items: &[ItemDef], id: &str| items.iter().find(|i| i.id == id).cloned().unwrap();
        let s = get(&items, "iron_sword");
        let w = s.weapon.as_ref().unwrap();
        assert_eq!((s.name.as_str(), s.icon.sheet.as_str(), s.srd.as_str(), s.cost_cp, s.weight), ("Iron Sword", "icons_3d", "longsword", 1500, 3.0));
        assert_eq!((w.category, w.properties.clone(), w.versatile.as_str()), (Some(WeaponCategory::Martial), vec![WeaponProperty::Versatile], "1d10"));
        assert!(!items.iter().any(|i| i.id == "longsword"), "the library's sword is the longsword");
        let m = get(&items, "chainmail").armor.unwrap();
        assert_eq!((m.category, m.base_ac, m.dex_max, m.str_min, m.stealth_disadvantage, m.speed), (Some(ArmorCategory::Heavy), 16, Some(0), 13, true, 0.0));
        let hp = get(&items, "half-plate-armor").armor.unwrap();
        assert_eq!((hp.category, hp.base_ac, hp.dex_max), (Some(ArmorCategory::Medium), 15, Some(2)));
        assert_eq!(get(&items, "shield").armor.map(|a| (a.slot, a.base_ac)), Some((EquipSlot::Offhand, 2)));
        // The dagger: simple, finesse, light, thrown 20/60; a crossbow loads and shoots bolts.
        let d = get(&items, "dagger").weapon.unwrap();
        assert_eq!((d.category, d.thrown_ft, d.dice.as_str()), (Some(WeaponCategory::Simple), Some((20, 60)), "1d4"));
        assert!([WeaponProperty::Finesse, WeaponProperty::Light, WeaponProperty::Thrown].iter().all(|p| d.properties.contains(p)));
        let x = get(&items, "crossbow-light").weapon.unwrap();
        let r = x.ranged.as_ref().unwrap();
        assert_eq!((r.range, r.long_range, r.ammo.as_str(), x.two_handed), (16.0, 64.0, "crossbow-bolt", true));
        assert_eq!(get(&items, "arrow").cost_cp, 5, "20 arrows cost 1 gp");
        assert_eq!((get(&items, "thieves-tools").srd.as_str(), get(&items, "thieves-tools").cost_cp), ("thieves-tools", 2500));
        // Coins: the old coin is the gold piece; four more.
        let coin = get(&items, "coin");
        assert_eq!((coin.name.as_str(), coin.coin_cp), ("Gold Piece", 100));
        assert_eq!(items.iter().filter(|i| i.coin_cp > 0).map(|i| i.coin_cp).sum::<u32>(), 1161);
        // The burglar's pack unpacks what the data has of it (a backpack, candles, a lantern, rations).
        let pack = get(&items, "burglars-pack");
        assert!(pack.consumable && pack.description.contains("5 × Candle"), "{}", pack.description);
        let gives: Vec<(String, u32)> = pack.on_use[0].actions.iter().filter_map(|a| match a { rustpg2d_engine::events::Action::GiveItem { item, count } => Some((item.clone(), *count)), _ => None }).collect();
        assert_eq!(gives, vec![("backpack".to_string(), 1), ("candle".into(), 5), ("lantern-hooded".into(), 1), ("rations-1-day".into(), 5)]);
        assert!(get(&items, "lantern-hooded").light.is_some() && get(&items, "backpack").bag_slots == 6);
        // A second run changes nothing; an icon a project gave an imported item stays.
        items.iter_mut().find(|i| i.id == "dagger").unwrap().icon = SpriteRef::new("items", 3);
        let before = items.clone();
        assert_eq!(data.import_equipment(&mut items, true, &[]), (0, 0));
        assert_eq!(items, before);
        // A project gets only what its classes start with (and what packs hold) and the coins.
        let mut project = vec![ItemDef::fresh("iron_sword")];
        let (added, _) = data.import_equipment(&mut project, false, &["burglars-pack".to_string(), "dagger".to_string()]);
        assert_eq!(added, 2 + 4 + 5, "{:?}", project.iter().map(|i| &i.id).collect::<Vec<_>>());
    }

}
