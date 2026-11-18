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
            r.traits.push(RaceTrait { id: id.into(), name: s(t, "name").into(), description: desc(t) });
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
    /// (size, type, speeds, senses, features). Returns (added, updated).
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
        let mut add = |index: &str, count: u32, equipment: &mut Vec<rustpg2d_engine::inventory::ItemStack>| match library_item(index) {
            Some(item) => match equipment.iter_mut().find(|i: &&mut rustpg2d_engine::inventory::ItemStack| i.item == item) {
                Some(st) => st.count += count,
                None => equipment.push(rustpg2d_engine::inventory::ItemStack { item: item.into(), count }),
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
        }
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

/// The library item standing for an SRD item or equipment category (until 0.31's equipment).
fn library_item(index: &str) -> Option<&'static str> {
    Some(match index {
        "chain-mail" | "scale-mail" | "ring-mail" | "splint-armor" | "plate-armor" => "chainmail",
        "leather-armor" | "studded-leather-armor" | "hide-armor" | "padded-armor" => "leather_armor",
        "shield" => "wooden_shield",
        "longsword" | "martial-weapons" | "martial-melee-weapons" => "iron_sword",
        "shortsword" | "scimitar" | "rapier" => "sword",
        "greataxe" | "battleaxe" | "handaxe" => "axe",
        "spear" | "javelin" | "simple-weapons" | "simple-melee-weapons" | "trident" | "pike" | "halberd" | "glaive" => "spear",
        "shortbow" | "longbow" => "bow",
        "crossbow-light" | "crossbow-heavy" | "crossbow-hand" => "crossbow",
        "arrow" => "arrows",
        "crossbow-bolt" => "bolts",
        _ => return None,
    })
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
    let style = |st| vec![E::FightingStyle { style: st }];
    let saves = |a: &[&str]| a.iter().map(|x| E::Proficient { what: format!("save:{x}") }).collect::<Vec<_>>();
    match id {
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
        "martial-arts" => vec![E::MartialArts],
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
    }
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
    let text = desc(a);
    let feet_after = |word: &str| -> Option<&str> { text.split(word).nth(1).map(|t| t.trim_start()) };
    let reach_ft = feet_after("reach ").and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    let (range_ft, long_ft) = feet_after("range ")
        .and_then(|t| t.split(' ').next())
        .and_then(|r| r.split_once('/').map(|(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(0))))
        .unwrap_or((0, 0));
    Some(rustpg2d_engine::srd::StatAttack { name: s(a, "name").into(), bonus, dice: dice.replace(' ', ""), damage_type, reach_ft, range_ft, long_ft })
}

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
    Ok(updated)
}

/// Imports into the `defs/` of a library or project folder; returns a one-line report.
pub fn import_into(checkout: &Path, root: &Path) -> Result<String> {
    let data = SrdData::read(checkout)?;
    let mut p = rustpg2d_engine::project::Project::load(root)?;
    let (added, updated) = data.import(&mut p.db.races, &mut p.db.backgrounds);
    let (m_added, m_updated) = data.import_monsters(&mut p.db.characters, true);
    let (c_added, c_updated, unmapped) = data.import_classes(&mut p.db.classes);
    let defs = root.join("defs");
    std::fs::write(defs.join("races.ron"), rustpg2d_engine::project::ron_text(&p.db.races)?)?;
    std::fs::write(defs.join("backgrounds.ron"), rustpg2d_engine::project::ron_text(&p.db.backgrounds)?)?;
    if m_added + m_updated > 0 {
        std::fs::write(defs.join("characters.ron"), rustpg2d_engine::project::ron_text(&p.db.characters)?)?;
    }
    std::fs::write(defs.join("classes.ron"), rustpg2d_engine::project::ron_text(&p.db.classes)?)?;
    Ok(format!(
        "{}: {added} added, {updated} updated ({} races, {} backgrounds); monsters: {m_added} added, {m_updated} characters updated ({} characters); classes: {c_added} added, {c_updated} updated (starting equipment without a library item: {})",
        root.display(),
        p.db.races.len(),
        p.db.backgrounds.len(),
        p.db.characters.len(),
        if unmapped.is_empty() { "-".to_string() } else { unmapped.join(", ") }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn classes_import_with_their_levels_features_and_subclasses() {
        use rustpg2d_engine::classes::{ClassTraits, FeatureChoice, FeatureEffect, FightingStyle, HeroClass};
        let data = SrdData::read(&fixture()).unwrap();
        let mut classes = Vec::new();
        let (added, updated, unmapped) = data.import_classes(&mut classes);
        assert_eq!((added, updated), (3, 0));
        assert!(!unmapped.is_empty(), "packs and daggers have no library item yet");
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
        // A second run changes nothing.
        let before = classes.clone();
        assert_eq!(data.import_classes(&mut classes).1, 0);
        assert_eq!(classes, before);
    }

}
