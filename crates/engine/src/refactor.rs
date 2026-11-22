//! Renaming ids: updates every reference in definitions, project config and maps.
//!
//! Strings inside Rhai scripts are not rewritten.

use crate::defs::{Database, SpriteRef};
use crate::entity::EntityKind;
use crate::events::{Action, Cond, Condition, Trigger, TriggerOn};
use crate::map::{Map, ZoneKind};
use crate::project::ProjectConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum RefKind {
    Sheet,
    Item,
    Prop,
    Character,
    Sound,
    Map,
    /// Multi-tile asset (`defs::StructureDef`).
    Structure,
    Skill,
    Spell,
    /// Crafting recipe (`crafting::RecipeDef`).
    Recipe,
    Quest,
    Achievement,
    /// Race (`races::RaceDef`).
    Race,
    /// 3D character model (`defs::ModelDef`).
    Model,
    /// Ground material (`ground::MaterialDef`).
    Material,
    /// Scatter set (`decor::ScatterDef`).
    Scatter,
    /// Font (`fonts::FontDef`).
    Font,
    /// UI skin (`skin::SkinDef`).
    Skin,
    /// Common event (`common_events::CommonEventDef`).
    CommonEvent,
    /// Background (`backgrounds::BackgroundDef`).
    Background,
    /// Class (`classes::ClassDef`).
    Class,
    /// Disease (`diseases::DiseaseDef`).
    Disease,
}

impl RefKind {
    /// Every kind: the definition lists in `defs/` file order ([`crate::project::DEFS`]), with
    /// `Skill` (a proficiency id, no definition) where `skills.ron` was until 0.27, then `Map`.
    pub const ALL: [RefKind; 22] = [
        RefKind::Sheet,
        RefKind::Item,
        RefKind::Prop,
        RefKind::Character,
        RefKind::Sound,
        RefKind::Quest,
        RefKind::Structure,
        RefKind::Skill,
        RefKind::Spell,
        RefKind::Recipe,
        RefKind::Achievement,
        RefKind::Race,
        RefKind::Model,
        RefKind::Material,
        RefKind::Scatter,
        RefKind::Font,
        RefKind::Skin,
        RefKind::CommonEvent,
        RefKind::Background,
        RefKind::Class,
        RefKind::Disease,
        RefKind::Map,
    ];
    /// The definition kinds in `defs/` file order.
    pub const DEFS: [RefKind; 20] = [
        RefKind::Sheet,
        RefKind::Item,
        RefKind::Prop,
        RefKind::Character,
        RefKind::Sound,
        RefKind::Quest,
        RefKind::Structure,
        RefKind::Spell,
        RefKind::Recipe,
        RefKind::Achievement,
        RefKind::Race,
        RefKind::Model,
        RefKind::Material,
        RefKind::Scatter,
        RefKind::Font,
        RefKind::Skin,
        RefKind::CommonEvent,
        RefKind::Background,
        RefKind::Class,
        RefKind::Disease,
    ];

    /// The kind's word in messages and commands: "sheet", "item", …, "map".
    pub fn name(self) -> &'static str {
        match self {
            RefKind::Sheet => "sheet",
            RefKind::Item => "item",
            RefKind::Prop => "prop",
            RefKind::Character => "character",
            RefKind::Sound => "sound",
            RefKind::Map => "map",
            RefKind::Structure => "structure",
            RefKind::Skill => "skill",
            RefKind::Spell => "spell",
            RefKind::Recipe => "recipe",
            RefKind::Quest => "quest",
            RefKind::Achievement => "achievement",
            RefKind::Race => "race",
            RefKind::Model => "model",
            RefKind::Material => "material",
            RefKind::Scatter => "scatter",
            RefKind::Font => "font",
            RefKind::Skin => "skin",
            RefKind::CommonEvent => "common_event",
            RefKind::Background => "background",
            RefKind::Class => "class",
            RefKind::Disease => "disease",
        }
    }

    /// The `defs/<file>.ron` of a definition kind (`None` for maps).
    pub fn file(self) -> Option<&'static str> {
        Some(match self {
            RefKind::Sheet => "sheets",
            RefKind::Item => "items",
            RefKind::Prop => "props",
            RefKind::Character => "characters",
            RefKind::Sound => "sounds",
            RefKind::Quest => "quests",
            RefKind::Structure => "structures",
            RefKind::Skill => return None,
            RefKind::Spell => "spells",
            RefKind::Recipe => "recipes",
            RefKind::Achievement => "achievements",
            RefKind::Race => "races",
            RefKind::Model => "models",
            RefKind::Material => "materials",
            RefKind::Scatter => "scatter",
            RefKind::Font => "fonts",
            RefKind::Skin => "skins",
            RefKind::CommonEvent => "common_events",
            RefKind::Background => "backgrounds",
            RefKind::Class => "classes",
            RefKind::Disease => "diseases",
            RefKind::Map => return None,
        })
    }

    /// The kind as shown to users: "Sprite sheet", "Prop", …
    pub fn label(self) -> &'static str {
        match self {
            RefKind::Sheet => "Sprite sheet",
            RefKind::Sound => "Sound",
            RefKind::Item => "Item",
            RefKind::Prop => "Prop",
            RefKind::Character => "Character",
            RefKind::Map => "Map",
            RefKind::Structure => "Structure",
            RefKind::Skill => "Skill or proficiency",
            RefKind::Spell => "Spell",
            RefKind::Recipe => "Recipe",
            RefKind::Quest => "Quest",
            RefKind::Achievement => "Achievement",
            RefKind::Race => "Race",
            RefKind::Model => "3D model",
            RefKind::Material => "Material",
            RefKind::Scatter => "Scatter set",
            RefKind::Font => "Font",
            RefKind::Skin => "UI skin",
            RefKind::CommonEvent => "Common event",
            RefKind::Background => "Background",
            RefKind::Class => "Class",
            RefKind::Disease => "Disease",
        }
    }

    /// "item", "Items", "sprite_sheet", "npc", "maps", … to the kind; `None` for other words.
    pub fn parse(s: &str) -> Option<RefKind> {
        let k = s.trim().to_lowercase();
        Some(match k.trim_end_matches('s') {
            "sheet" | "spritesheet" | "sprite_sheet" | "sprite sheet" => RefKind::Sheet,
            "item" => RefKind::Item,
            "prop" => RefKind::Prop,
            "character" | "npc" => RefKind::Character,
            "sound" => RefKind::Sound,
            "map" => RefKind::Map,
            "structure" => RefKind::Structure,
            "skill" => RefKind::Skill,
            "spell" => RefKind::Spell,
            "recipe" => RefKind::Recipe,
            "quest" => RefKind::Quest,
            "achievement" => RefKind::Achievement,
            "race" => RefKind::Race,
            "model" | "3d model" => RefKind::Model,
            "material" | "ground" => RefKind::Material,
            "scatter" | "scatter set" | "scatter_set" => RefKind::Scatter,
            "font" => RefKind::Font,
            "skin" | "ui skin" | "ui_skin" => RefKind::Skin,
            "common_event" | "common event" | "commonevent" => RefKind::CommonEvent,
            "background" => RefKind::Background,
            // (Plurals lose their s: "class" and "classes" end up as "cla" and "classe".)
            "cla" | "classe" => RefKind::Class,
            "disease" => RefKind::Disease,
            _ => return None,
        })
    }

    /// A definition list (everything but `Map` and `Skill`: skills and tools are the SRD's,
    /// referred to by their proficiency ids).
    pub fn is_def(self) -> bool {
        !matches!(self, RefKind::Map | RefKind::Skill)
    }
}

/// `base`, or `base_2`, `base_3`, …: the first id `taken` says is free (new maps, zones,
/// imported sheets, library copies).
pub fn unique_id(base: &str, taken: impl Fn(&str) -> bool) -> String {
    std::iter::once(base.to_string()).chain((2..).map(|n| format!("{base}_{n}"))).find(|c| !taken(c)).unwrap_or_else(|| base.to_string())
}

struct Renamer<'a> {
    kind: RefKind,
    old: &'a str,
    new: &'a str,
    count: usize,
    /// Collecting mode: records every non-empty reference instead of renaming.
    found: Option<Vec<(RefKind, String)>>,
    /// Also converts 0.27's skill levels in events to the SRD rules ([`srd_upgrade_db`]).
    srd: bool,
}

impl Renamer<'_> {
    /// A placed entity (on a map or inside a structure).
    /// A magic item: the spells cast from it, who may attune
    /// (renamed with the class or race; not a dependency).
    fn magic(&mut self, m: &mut Option<crate::srd::magic_items::MagicItem>) {
        let Some(m) = m else { return };
        for sp in &mut m.spells {
            self.fix(RefKind::Spell, &mut sp.spell);
        }
        if self.found.is_none() {
            for w in &mut m.attune_by {
                self.fix(RefKind::Class, w);
                self.fix(RefKind::Race, w);
            }
        }
    }

    fn crop(&mut self, c: &mut Option<crate::defs::Crop>) {
        if let Some(c) = c {
            for s in &mut c.stages {
                self.fix(RefKind::Sheet, &mut s.sprite.sheet);
            }
            self.loot(&mut c.harvest);
        }
    }

    fn portraits(&mut self, p: &mut SpriteRef, moods: &mut std::collections::BTreeMap<String, SpriteRef>) {
        self.fix(RefKind::Sheet, &mut p.sheet);
        for s in moods.values_mut() {
            self.fix(RefKind::Sheet, &mut s.sheet);
        }
    }

    fn entity(&mut self, e: &mut crate::entity::EntityInstance) {
        if let Some(p) = &mut e.portrait {
            self.fix(RefKind::Sheet, &mut p.sheet);
        }
        let k = match e.kind {
            EntityKind::Prop => RefKind::Prop,
            EntityKind::Character => RefKind::Character,
            EntityKind::Item => RefKind::Item,
        };
        self.fix(k, &mut e.def);
        self.triggers(&mut e.triggers);
        self.container(&mut e.container);
        self.lock(&mut e.lock);
        self.shop(&mut e.shop);
        if let Some(l) = &mut e.loot {
            self.loot(l);
        }
        if let Some(a) = &mut e.appearance {
            self.look(a);
        }
        if let Some(eq) = &mut e.equipment {
            self.gear(eq, &mut []);
        }
        if let Some(inv) = &mut e.inventory {
            self.gear(&mut Default::default(), inv);
        }
        self.stat_spells(&mut e.stats);
        self.stat_diseases(&mut e.stats);
    }

    /// The diseases a stat block's attacks give (a diseased rat's bite): renamed with the disease,
    /// and a dependency (the library carries it with the monster).
    fn stat_diseases(&mut self, s: &mut Option<crate::srd::StatBlock>) {
        for a in s.iter_mut().flat_map(|s| s.attacks.iter_mut()) {
            for r in &mut a.on_hit {
                self.fix(RefKind::Disease, &mut r.disease);
            }
        }
    }

    /// A stat block's spells (renamed with the spell; not a dependency: a monster casts only the
    /// spells the project has).
    fn stat_spells(&mut self, s: &mut Option<crate::srd::StatBlock>) {
        if self.found.is_some() {
            return;
        }
        if let Some(c) = s.as_mut().and_then(|s| s.spellcasting.as_mut()) {
            for sp in &mut c.spells {
                self.fix(RefKind::Spell, &mut sp.spell);
            }
        }
    }

    /// The model of a look (and a look of an older version: the model named like its figure).
    fn look(&mut self, a: &mut crate::defs::Appearance) {
        self.fix(RefKind::Model, &mut a.body);
        self.fix(RefKind::Model, &mut a.model);
        // Its chosen pieces (model ids; a style name or "-" is no reference).
        for p in a.parts.values_mut().filter(|p| p.as_str() != crate::defs::NO_PART) {
            self.fix(RefKind::Model, p);
        }
    }

    /// Items worn and carried.
    fn gear(&mut self, eq: &mut crate::state::Equipment, carried: &mut [crate::inventory::ItemStack]) {
        for slot in crate::defs::EquipSlot::ALL {
            if let Some(s) = eq.slot_mut(slot) {
                self.fix(RefKind::Item, &mut s.item);
            }
        }
        for s in carried {
            self.fix(RefKind::Item, &mut s.item);
        }
    }

    fn spell(&mut self, d: &mut crate::skills::SpellDef) {
        self.fix(RefKind::Sheet, &mut d.icon.sheet);
        self.fix(RefKind::Skill, &mut d.skill);
        if let crate::skills::SpellEffect::Bolt { projectile, .. } = &mut d.effect {
            self.fix(RefKind::Sheet, &mut projectile.sheet);
        }
        // The look: its projectile, impact sheet and sound; the classes whose list has it.
        if let Some(p) = &mut d.visual.projectile {
            self.fix(RefKind::Sheet, &mut p.sheet);
        }
        self.fix(RefKind::Sheet, &mut d.visual.impact);
        self.fix(RefKind::Sound, &mut d.visual.sound);
        // (Its costly component's items too.)
        if let Some(c) = &mut d.costly {
            self.fix(RefKind::Item, &mut c.item);
        }
        // (The creatures it summons travel with it.)
        for p in &mut d.parts {
            if let crate::skills::SpellPart::Summon { creature, .. } = p {
                self.fix(RefKind::Character, creature);
            }
        }
        // (Renamed with their class; not a dependency: a spell travels without the classes.)
        if self.found.is_none() {
            for c in &mut d.classes {
                self.fix(RefKind::Class, c);
            }
        }
    }

    /// Furniture: the cover sprite (a bed's blanket) and the conditions.
    fn use_spot(&mut self, s: &mut Option<crate::defs::UseSpot>) {
        if let Some(s) = s {
            self.fix(RefKind::Sheet, &mut s.cover.sheet);
            self.conditions(&mut s.conditions);
        }
    }

    fn race(&mut self, d: &mut crate::races::RaceDef) {
        // The models the creator offers (named like the figures they replaced).
        for s in d.bodies_male.iter_mut().chain(&mut d.bodies_female) {
            self.fix(RefKind::Model, s);
        }
        for s in &mut d.skills {
            self.fix(RefKind::Skill, &mut s.skill);
        }
    }

    fn recipe(&mut self, d: &mut crate::crafting::RecipeDef) {
        self.fix(RefKind::Item, &mut d.result);
        for s in &mut d.ingredients {
            self.fix(RefKind::Item, &mut s.item);
        }
        self.fix(RefKind::Skill, &mut d.skill);
        for q in &mut d.quality {
            self.fix(RefKind::Item, &mut q.result);
        }
    }

    fn structure(&mut self, d: &mut crate::defs::StructureDef) {
        for m in &mut d.materials {
            self.fix(RefKind::Material, m);
        }
        for p in &mut d.decor_defs {
            self.fix(RefKind::Prop, p);
        }
        for e in &mut d.entities {
            self.entity(e);
        }
    }

    fn lock(&mut self, l: &mut Option<crate::defs::LockSpec>) {
        if let Some(l) = l {
            self.fix(RefKind::Item, &mut l.key);
        }
    }

    fn container(&mut self, c: &mut Option<crate::defs::ContainerSpec>) {
        if let Some(c) = c {
            for s in &mut c.items {
                self.fix(RefKind::Item, &mut s.item);
            }
            self.conditions(&mut c.conditions);
        }
    }

    fn shop(&mut self, s: &mut Option<crate::defs::ShopSpec>) {
        if let Some(s) = s {
            self.fix(RefKind::Item, &mut s.currency);
            for e in &mut s.items {
                self.fix(RefKind::Item, &mut e.item);
            }
        }
    }

    fn loot(&mut self, loot: &mut [crate::defs::LootEntry]) {
        for e in loot {
            self.fix(RefKind::Item, &mut e.item);
        }
    }

    fn weapon(&mut self, w: &mut Option<crate::defs::Weapon>) {
        if let Some(r) = w.as_mut().and_then(|w| w.ranged.as_mut()) {
            self.fix(RefKind::Sheet, &mut r.projectile.sheet);
            self.fix(RefKind::Item, &mut r.ammo);
        }
    }

    fn fix(&mut self, kind: RefKind, s: &mut String) {
        if let Some(found) = &mut self.found {
            if !s.is_empty() {
                found.push((kind, s.clone()));
            }
            return;
        }
        if kind == self.kind && s == self.old {
            *s = self.new.to_string();
            self.count += 1;
        }
    }

    fn triggers(&mut self, ts: &mut [Trigger]) {
        for t in ts {
            if let TriggerOn::UseItem { item } = &mut t.on {
                self.fix(RefKind::Item, item);
            }
            self.conditions(&mut t.conditions);
            self.actions(&mut t.actions);
        }
    }

    /// A quest's markers and what the quest editor describes: giver, conditions, steps, rewards.
    fn quest(&mut self, q: &mut crate::quest::QuestDef) {
        for t in q.targets.iter_mut().chain(q.objects.iter_mut()) {
            self.fix(RefKind::Map, &mut t.map);
        }
        // (Givers and talk characters may be placed names, not definitions: renamed with a
        // definition, but not listed as its dependencies.)
        let collecting = self.found.is_some();
        if !collecting {
            self.fix(RefKind::Character, &mut q.giver);
        }
        self.conditions(&mut q.requires);
        for s in &mut q.steps {
            match &mut s.done {
                crate::quest::StepGoal::Conditions(c) => self.conditions(c),
                crate::quest::StepGoal::Talk { character, when, .. } => {
                    if !collecting {
                        self.fix(RefKind::Character, character);
                    }
                    self.conditions(when);
                }
                _ => {}
            }
            self.actions(&mut s.on_start);
            self.actions(&mut s.on_done);
            self.fix(RefKind::Map, &mut s.first_to.map);
        }
        for i in &mut q.rewards.items {
            self.fix(RefKind::Item, &mut i.item);
        }
        self.actions(&mut q.rewards.actions);
    }

    fn conditions(&mut self, cs: &mut [Condition]) {
        for c in cs {
            match &mut c.cond {
                Cond::Skill { skill, op, level } => {
                    self.fix(RefKind::Skill, skill);
                    if self.srd {
                        if let Some((o, l)) = srd_skill_compare(*op, *level) {
                            if (o, l) != (*op, *level) {
                                (*op, *level) = (o, l);
                                self.count += 1;
                            }
                        }
                    }
                }
                Cond::HasItem { item, .. } | Cond::Attuned { item } => self.fix(RefKind::Item, item),
                Cond::HasDisease { disease } => self.fix(RefKind::Disease, disease),
                // (A definition id there follows its renames; a name stays.)
                Cond::KnockedOut { name } => self.fix(RefKind::Character, name),
                Cond::Defeated { character, .. } => self.fix(RefKind::Character, character),
                Cond::PlayerRace { race } => self.fix(RefKind::Race, race),
                Cond::Class { class, .. } => self.fix(RefKind::Class, class),
                _ => {}
            }
        }
    }

    fn actions(&mut self, acts: &mut [Action]) {
        crate::events::walk_actions_mut(acts, &mut |a| match a {
            Action::GiveItem { item, .. } | Action::TakeItem { item, .. } => self.fix(RefKind::Item, item),
            Action::Equip { item, .. } | Action::GiveItemTo { item, .. } | Action::TakeItemFrom { item, .. } | Action::ShopStock { item, .. } | Action::Poison { item, .. } => self.fix(RefKind::Item, item),
            Action::Teleport { map, .. } | Action::SetSpawners { map, .. } | Action::SetWeather { map, .. } => self.fix(RefKind::Map, map),
            Action::PlaySound { sound, .. } | Action::PlayMusic { sound, .. } | Action::PlayMusicEffect { sound, .. } => self.fix(RefKind::Sound, sound),
            Action::CallCommon { id } => self.fix(RefKind::CommonEvent, id),
            Action::ExposeDisease { disease, .. } | Action::CureDisease { disease } => self.fix(RefKind::Disease, disease),
            Action::SetClass { class, .. } => self.fix(RefKind::Class, class),
            Action::Fly { sprite, .. } | Action::Effect { sprite, .. } => self.fix(RefKind::Sheet, &mut sprite.sheet),
            Action::ShowImage { sheet, .. } | Action::ShowPicture { sheet, .. } => self.fix(RefKind::Sheet, sheet),
            Action::TeachSpell { spell, .. } | Action::CastSpell { spell, .. } => self.fix(RefKind::Spell, spell),
            Action::If { conditions, .. } => self.conditions(conditions),
            Action::SetSkill { skill, level } => {
                self.fix(RefKind::Skill, skill);
                if self.srd && *level > 1 {
                    *level = 1;
                    self.count += 1;
                }
            }
            Action::Lockpick { difficulty, .. } if self.srd && (1..10).contains(difficulty) => {
                *difficulty = crate::srd::old_level_dc(*difficulty);
                self.count += 1;
            }
            _ => {}
        });
    }
}

/// Updates references inside definitions and the project config. Returns the number changed.
/// (The definition's own `id` must be renamed by the caller.)
pub fn rename_in_db(db: &mut Database, config: &mut ProjectConfig, kind: RefKind, old: &str, new: &str) -> usize {
    db_pass(db, config, kind, old, new, false)
}

/// Converts the events of a 0.27 project's definitions to the SRD rules: *Skill* conditions
/// compare proficiency (1 / 0) instead of a level, *Set skill* sets a proficiency, the
/// *Lockpick* action's difficulty becomes a DC. Returns the number changed.
pub fn srd_upgrade_db(db: &mut Database, config: &mut ProjectConfig) -> usize {
    db_pass(db, config, RefKind::Skill, "\u{0}", "\u{0}", true)
}

/// [`srd_upgrade_db`] for a map.
pub fn srd_upgrade_map(map: &mut Map) -> usize {
    map_pass(map, RefKind::Skill, "\u{0}", "\u{0}", true)
}

/// A 0.27 *Skill level* comparison as a proficiency comparison (proficiency counts as level 1
/// or more): "≥ 2" = proficient, "= 0" = not; `None` = it can't hold for any level.
fn srd_skill_compare(op: crate::events::CmpOp, level: i64) -> Option<(crate::events::CmpOp, i64)> {
    use crate::events::CmpOp;
    let none = op.apply(0, level);
    let some = (1..=20).any(|l| op.apply(l, level));
    match (none, some) {
        (false, true) => Some((CmpOp::Ge, 1)),
        (true, false) => Some((CmpOp::Eq, 0)),
        (true, true) => Some((CmpOp::Ge, 0)),
        (false, false) => None,
    }
}

fn db_pass(db: &mut Database, config: &mut ProjectConfig, kind: RefKind, old: &str, new: &str, srd: bool) -> usize {
    let mut r = Renamer { kind, old, new, count: 0, found: None, srd };
    // A renamed map keeps its title.
    if kind == RefKind::Map {
        if let Some(t) = config.map_titles.remove(old) {
            config.map_titles.insert(new.to_string(), t);
            r.count += 1;
        }
    }
    for d in &mut db.items {
        r.fix(RefKind::Sheet, &mut d.icon.sheet);
        r.fix(RefKind::Spell, &mut d.teaches);
        r.fix(RefKind::Prop, &mut d.furniture);
        r.triggers(&mut d.on_use);
        r.weapon(&mut d.weapon);
        r.crop(&mut d.crop);
        r.magic(&mut d.magic);
    }
    for d in &mut db.props {
        r.fix(RefKind::Sheet, &mut d.sprite.sheet);
        for st in &mut d.states {
            r.fix(RefKind::Sheet, &mut st.sprite.sheet);
        }
        r.triggers(&mut d.triggers);
        r.container(&mut d.container);
        r.lock(&mut d.lock);
        r.use_spot(&mut d.use_spot);
        if let Some(g) = &mut d.gather {
            r.loot(&mut g.loot);
        }
    }
    for d in &mut db.characters {
        r.fix(RefKind::Race, &mut d.race);
        r.triggers(&mut d.triggers);
        r.weapon(&mut d.weapon);
        r.shop(&mut d.shop);
        r.loot(&mut d.loot);
        r.look(&mut d.appearance);
        r.gear(&mut d.equipment, &mut d.inventory);
        r.portraits(&mut d.portrait, &mut d.moods);
        r.stat_spells(&mut d.stats);
        r.stat_diseases(&mut d.stats);
    }
    for d in &mut db.structures {
        r.structure(d);
    }
    for d in &mut db.spells {
        r.spell(d);
    }
    for d in &mut db.recipes {
        r.recipe(d);
    }
    for d in &mut db.achievements {
        r.fix(RefKind::Sheet, &mut d.icon.sheet);
        r.conditions(&mut d.conditions);
    }
    for d in &mut db.races {
        r.race(d);
    }
    for q in &mut db.quests {
        r.quest(q);
    }
    for d in &mut db.models {
        if let Some(rig) = &mut d.rig {
            r.fix(RefKind::Model, &mut rig.template);
        }
        for p in &mut d.parts {
            r.fix(RefKind::Model, p);
        }
        for f in d.part.iter_mut().flat_map(|p| &mut p.fits) {
            r.fix(RefKind::Model, f);
        }
    }
    for d in &mut db.materials {
        r.fix(RefKind::Sound, &mut d.step);
    }
    for d in &mut db.scatter {
        for (p, _) in &mut d.props {
            r.fix(RefKind::Prop, p);
        }
    }
    for d in &mut db.props {
        for p in &mut d.pieces {
            r.fix(RefKind::Prop, p);
        }
    }
    r.fix(RefKind::Character, &mut config.player);
    r.fix(RefKind::Character, &mut config.creation.store);
    r.fix(RefKind::Sheet, &mut config.player_portrait.sheet);
    let cc = &mut config.creation;
    for s in [&mut cc.body_male, &mut cc.body_female] {
        r.fix(RefKind::Model, s);
    }
    r.fix(RefKind::Map, &mut config.start_map);
    for (_, s) in config.sfx.fields_mut() {
        r.fix(RefKind::Sound, s);
    }
    r.fix(RefKind::Sound, &mut config.battle_music);
    let f = &mut config.fonts;
    for s in [&mut f.ui, &mut f.dialog, &mut f.hud, &mut f.title].into_iter().chain(f.fallback.iter_mut()) {
        r.fix(RefKind::Font, s);
    }
    r.fix(RefKind::Skin, &mut config.skin);
    for d in &mut db.common_events {
        r.actions(&mut d.actions);
    }
    for d in &mut db.backgrounds {
        for s in &mut d.equipment {
            r.fix(RefKind::Item, &mut s.item);
        }
    }
    for d in &mut db.classes {
        for s in &mut d.equipment {
            r.fix(RefKind::Item, &mut s.item);
        }
    }
    for c in &mut db.characters {
        if let Some(hc) = &mut c.class {
            r.fix(RefKind::Class, &mut hc.class);
        }
    }
    // The title screen's map, pictures and music.
    let t = &mut config.title;
    match &mut t.background {
        crate::menus::TitleBackground::Map { map, .. } => r.fix(RefKind::Map, map),
        crate::menus::TitleBackground::Picture { sheet } => r.fix(RefKind::Sheet, sheet),
    }
    if let Some(l) = &mut t.logo {
        r.fix(RefKind::Sheet, &mut l.sheet);
    }
    r.fix(RefKind::Sound, &mut t.music);
    for d in &mut db.skins {
        for p in d.pictures_mut() {
            r.fix(RefKind::Sheet, &mut p.sheet);
        }
    }
    r.count
}

/// Updates references inside a map. Returns the number changed.
pub fn rename_in_map(map: &mut Map, kind: RefKind, old: &str, new: &str) -> usize {
    map_pass(map, kind, old, new, false)
}

fn map_pass(map: &mut Map, kind: RefKind, old: &str, new: &str, srd: bool) -> usize {
    let mut r = Renamer { kind, old, new, count: 0, found: None, srd };
    for m in &mut map.materials {
        r.fix(RefKind::Material, m);
    }
    for d in &mut map.decor_defs {
        r.fix(RefKind::Prop, d);
    }
    r.fix(RefKind::Sound, &mut map.music);
    if let Some(m) = &mut map.battle_music {
        r.fix(RefKind::Sound, m);
    }
    for l in &mut map.parallax {
        r.fix(RefKind::Sheet, &mut l.sheet);
    }
    r.triggers(&mut map.triggers);
    for e in &mut map.entities {
        r.entity(e);
    }
    for z in &mut map.zones {
        match &mut z.kind {
            ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } | ZoneKind::Drop { target_map, .. } => r.fix(RefKind::Map, target_map),
            ZoneKind::Spawner { character, .. } => r.fix(RefKind::Character, character),
            ZoneKind::Encounter { monsters, .. } => monsters.iter_mut().for_each(|m| r.fix(RefKind::Character, &mut m.character)),
            ZoneKind::FishingSpot { rod } => r.fix(RefKind::Item, rod),
            ZoneKind::Ambience { sound, .. } => r.fix(RefKind::Sound, sound),
            _ => {}
        }
        r.triggers(&mut z.triggers);
        if let Some(l) = &mut z.loot {
            r.loot(l);
        }
    }
    for l in [&mut map.edges.north, &mut map.edges.south, &mut map.edges.east, &mut map.edges.west].into_iter().flatten() {
        r.fix(RefKind::Map, &mut l.target_map);
    }
    if kind == RefKind::Map && map.name == old {
        map.name = new.to_string();
        r.count += 1;
    }
    r.count
}

fn collector() -> Renamer<'static> {
    Renamer { kind: RefKind::Map, old: "", new: "", count: 0, found: Some(Vec::new()), srd: false }
}

/// The items a background starts the hero with.
pub fn refs_of_background(d: &crate::backgrounds::BackgroundDef) -> Vec<(RefKind, String)> {
    d.equipment.iter().filter(|s| !s.item.is_empty()).map(|s| (RefKind::Item, s.item.clone())).collect()
}

/// The items a class starts with.
pub fn refs_of_class(d: &crate::classes::ClassDef) -> Vec<(RefKind, String)> {
    d.equipment.iter().filter(|s| !s.item.is_empty()).map(|s| (RefKind::Item, s.item.clone())).collect()
}

/// What a common event's actions use (items, sounds, other common events, ...).
pub fn refs_of_common_event(d: &crate::common_events::CommonEventDef) -> Vec<(RefKind, String)> {
    let mut acts = d.actions.clone();
    let mut r = collector();
    r.actions(&mut acts);
    r.found.unwrap_or_default()
}

/// Everything an item definition refers to (icon / projectile sheets, ammo, items and sounds used
/// by its triggers, ...).
/// A model's rig template.
pub fn refs_of_skin(d: &crate::skin::SkinDef) -> Vec<(RefKind, String)> {
    let mut out: Vec<(RefKind, String)> = d.pictures().into_iter().map(|(_, p)| (RefKind::Sheet, p.sheet.clone())).filter(|(_, s)| !s.is_empty()).collect();
    out.dedup();
    out
}

pub fn refs_of_scatter(d: &crate::decor::ScatterDef) -> Vec<(RefKind, String)> {
    d.props.iter().map(|(p, _)| (RefKind::Prop, p.clone())).collect()
}

pub fn refs_of_material(d: &crate::ground::MaterialDef) -> Vec<(RefKind, String)> {
    if d.step.is_empty() { vec![] } else { vec![(RefKind::Sound, d.step.clone())] }
}

/// A model needs its rig template and, a body, its own pieces. (A piece doesn't need the bodies
/// it fits.)
pub fn refs_of_model(d: &crate::defs::ModelDef) -> Vec<(RefKind, String)> {
    d.rig.iter().filter(|r| !r.template.is_empty()).map(|r| r.template.clone()).chain(d.parts.iter().cloned()).map(|id| (RefKind::Model, id)).collect()
}

pub fn refs_of_item(d: &crate::defs::ItemDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Sheet, &mut d.icon.sheet);
    r.triggers(&mut d.on_use);
    r.weapon(&mut d.weapon);
    r.fix(RefKind::Spell, &mut d.teaches);
    r.fix(RefKind::Prop, &mut d.furniture);
    r.crop(&mut d.crop);
    r.magic(&mut d.magic);
    r.found.unwrap_or_default()
}

pub fn refs_of_spell(d: &crate::skills::SpellDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.spell(&mut d);
    r.found.unwrap_or_default()
}

/// The result and ingredient items and the skill of a recipe.
pub fn refs_of_recipe(d: &crate::crafting::RecipeDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.recipe(&mut d);
    r.found.unwrap_or_default()
}

pub fn refs_of_prop(d: &crate::defs::PropDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Sheet, &mut d.sprite.sheet);
    for st in &mut d.states {
        r.fix(RefKind::Sheet, &mut st.sprite.sheet);
    }
    r.triggers(&mut d.triggers);
    r.container(&mut d.container);
    r.lock(&mut d.lock);
    r.use_spot(&mut d.use_spot);
    if let Some(g) = &mut d.gather {
        r.loot(&mut g.loot);
    }
    for p in d.pieces.iter_mut().filter(|p| !p.is_empty()) {
        r.fix(RefKind::Prop, p);
    }
    r.found.unwrap_or_default()
}

/// Everything a structure refers to (materials, decorations' props, and the definitions of its
/// entities with what those need).
pub fn refs_of_structure(d: &crate::defs::StructureDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.structure(&mut d);
    r.found.unwrap_or_default()
}

/// What a quest refers to: its giver and talk characters, reward items, the maps of its
/// markers, and the ids inside its conditions and actions.
pub fn refs_of_quest(d: &crate::quest::QuestDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.quest(&mut d);
    r.found.unwrap_or_default()
}

/// Every id a trigger list refers to (items, maps, sounds, characters, spells, …).
pub fn refs_of_triggers(ts: &[Trigger]) -> Vec<(RefKind, String)> {
    let mut ts = ts.to_vec();
    let mut r = collector();
    r.triggers(&mut ts);
    r.found.unwrap_or_default()
}

/// The figures and skills of a race.
pub fn refs_of_race(d: &crate::races::RaceDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.race(&mut d);
    r.found.unwrap_or_default()
}

pub fn refs_of_character(d: &crate::defs::CharacterDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Race, &mut d.race);
    r.triggers(&mut d.triggers);
    r.weapon(&mut d.weapon);
    r.shop(&mut d.shop);
    r.loot(&mut d.loot);
    r.look(&mut d.appearance);
    r.gear(&mut d.equipment, &mut d.inventory);
    r.portraits(&mut d.portrait, &mut d.moods);
    r.stat_diseases(&mut d.stats);
    r.found.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::ItemDef;

    /// A rig template is a dependency of the model using it (library exports carry it) and
    /// follows renames.
    #[test]
    fn rig_templates_travel_and_follow_renames() {
        use crate::defs::{ModelDef, RigFit};
        let mut db = Database::default();
        db.models.push(ModelDef::new("rig_quadruped", "models/rig_quadruped.glb"));
        db.models.push(ModelDef { rig: Some(RigFit { template: "rig_quadruped".into(), ..Default::default() }), ..ModelDef::new("wolf", "models/wolf.glb") });
        db.reindex();
        assert_eq!(db.refs_of(RefKind::Model, "wolf"), vec![(RefKind::Model, "rig_quadruped".to_string())]);
        let mut config = ProjectConfig::new("t");
        assert_eq!(rename_in_db(&mut db, &mut config, RefKind::Model, "rig_quadruped", "rig_four_legs"), 1);
        assert_eq!(db.models[1].rig.as_ref().unwrap().template, "rig_four_legs");
    }

    /// A stat block's spells follow a spell's rename, but don't travel with the character.
    #[test]
    fn stat_block_spells_follow_renames() {
        use crate::srd::{StatBlock, StatSpell, StatSpellcasting};
        let mut db = Database::default();
        let cast = StatSpellcasting { spells: vec![StatSpell { spell: "sacred-flame".into(), ..Default::default() }], ..Default::default() };
        db.characters.push(crate::defs::CharacterDef { stats: Some(StatBlock { spellcasting: Some(cast), ..Default::default() }), ..crate::defs::CharacterDef::fresh("cultist") });
        db.reindex();
        assert!(db.refs_of(RefKind::Character, "cultist").iter().all(|(k, _)| *k != RefKind::Spell));
        let mut config = ProjectConfig::new("t");
        assert_eq!(rename_in_db(&mut db, &mut config, RefKind::Spell, "sacred-flame", "holy-flame"), 1);
        assert_eq!(db.characters[0].stats.as_ref().unwrap().spellcasting.as_ref().unwrap().spells[0].spell, "holy-flame");
    }

    /// The kinds' files are the defs files in order (`parse_defs` / `save_defs` rely on it).
    #[test]
    fn kinds_match_the_defs_files() {
        assert_eq!(RefKind::DEFS.map(|k| k.file().unwrap()), crate::project::DEFS);
        assert!(RefKind::ALL.iter().filter(|k| k.is_def()).eq(RefKind::DEFS.iter()));
        assert_eq!(RefKind::parse("Items"), Some(RefKind::Item));
        assert_eq!(RefKind::parse("npc"), Some(RefKind::Character));
        assert_eq!(RefKind::parse(" sprite sheets "), Some(RefKind::Sheet));
        assert_eq!(RefKind::parse("maps"), Some(RefKind::Map));
        assert_eq!(RefKind::parse("dragon"), None);
        for k in RefKind::ALL {
            assert_eq!(RefKind::parse(k.name()), Some(k), "{k:?} round-trips");
        }
    }
    use crate::entity::EntityInstance;
    use crate::map::{EdgeLink, Zone};

    #[test]
    fn renames_items_everywhere() {
        let mut m = Map::new("village", 4, 4);
        let mut e = EntityInstance::new(1, EntityKind::Item, "key", 0.0, 0.0);
        e.triggers.push(Trigger {
            on: TriggerOn::UseItem { item: "key".into() },
            conditions: vec![Condition::new(Cond::HasItem { item: "key".into(), count: 1 })],
            actions: vec![Action::If {
                conditions: vec![],
                then: vec![Action::TakeItem { item: "key".into(), count: 1 }],
                otherwise: vec![Action::GiveItem { item: "coin".into(), count: 1 }],
            }],
            once: false,
        });
        m.entities.push(e);
        let mut chest = EntityInstance::new(3, EntityKind::Prop, "chest", 0.0, 0.0);
        chest.container = Some(crate::defs::ContainerSpec {
            items: vec![crate::inventory::ItemStack::new("key", 1), crate::inventory::ItemStack::new("coin", 5)],
            ..Default::default()
        });
        m.entities.push(chest);
        // A prop with the same id must not be touched.
        m.entities.push(EntityInstance::new(2, EntityKind::Prop, "key", 0.0, 0.0));
        assert_eq!(rename_in_map(&mut m, RefKind::Item, "key", "gate_key"), 5);
        let items = &m.entities[1].container.as_ref().unwrap().items;
        assert_eq!((items[0].item.as_str(), items[1].item.as_str()), ("gate_key", "coin"));
        assert_eq!(m.entities[0].def, "gate_key");
        assert_eq!(m.entities[2].def, "key");
        let t = &m.entities[0].triggers[0];
        assert_eq!(t.on, TriggerOn::UseItem { item: "gate_key".into() });
        let Action::If { then, otherwise, .. } = &t.actions[0] else { panic!() };
        assert_eq!(then[0], Action::TakeItem { item: "gate_key".into(), count: 1 });
        assert_eq!(otherwise[0], Action::GiveItem { item: "coin".into(), count: 1 });
    }

    #[test]
    fn renames_characters_in_defeated_conditions() {
        let mut m = Map::new("village", 4, 4);
        m.triggers.push(Trigger {
            on: TriggerOn::MapEnter,
            conditions: vec![Condition::new(Cond::Defeated { character: "slime".into(), op: crate::events::CmpOp::Ge, count: 3 })],
            actions: vec![],
            once: false,
        });
        assert_eq!(rename_in_map(&mut m, RefKind::Character, "slime", "jelly"), 1);
        assert_eq!(m.triggers[0].conditions[0].cond, Cond::Defeated { character: "jelly".into(), op: crate::events::CmpOp::Ge, count: 3 });
    }

    #[test]
    fn renames_reach_structures() {
        let mut db = Database::default();
        let mut e = EntityInstance::new(0, EntityKind::Prop, "chest", 0.0, 0.0);
        e.container = Some(crate::defs::ContainerSpec { items: vec![crate::inventory::ItemStack::new("key", 1)], ..Default::default() });
        db.structures.push(crate::defs::StructureDef { id: "hut".into(), materials: vec!["grass".into()], decor_defs: vec!["oak".into()], entities: vec![e], ..Default::default() });
        let mut cfg = ProjectConfig::new("t");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Material, "grass", "lawn"), 1);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Prop, "oak", "elm"), 1);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Prop, "chest", "box"), 1);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Item, "key", "gate_key"), 1);
        let d = &db.structures[0];
        assert_eq!((d.materials[0].as_str(), d.decor_defs[0].as_str(), d.entities[0].def.as_str()), ("lawn", "elm", "box"));
        let mut refs = refs_of_structure(d);
        refs.sort();
        refs.dedup();
        assert_eq!(refs, vec![(RefKind::Item, "gate_key".to_string()), (RefKind::Prop, "box".to_string()), (RefKind::Prop, "elm".to_string()), (RefKind::Material, "lawn".to_string())]);
    }

    #[test]
    fn renames_maps_sheets_and_config() {
        let mut m = Map::new("forest", 4, 4);
        m.edges.west = Some(EdgeLink::new("village"));
        m.zones.push(Zone {
            uid: 1,
            name: "p".into(),
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            kind: ZoneKind::Portal { target_map: "village".into(), target_spawn: "s".into() },
            triggers: vec![],
            loot: None,
        });
        assert_eq!(rename_in_map(&mut m, RefKind::Map, "village", "town"), 2);
        assert_eq!(m.edges.west.as_ref().unwrap().target_map, "town");

        let mut db = Database::default();
        db.items.push(ItemDef { id: "coin".into(), name: "Coin".into(), icon: SpriteRef::new("items", 0), ..Default::default() });
        let mut cfg = ProjectConfig {
            name: "p".into(),
            tile_size: 32,
            start_map: "village".into(),
            start_spawn: "s".into(),
            scale: 3.0,
            player: "hero".into(),
            battle_music: String::new(),
            sfx: Default::default(),
            inventory_size: 24,
            creation: Default::default(),
        day_minutes: 24.0, start_hour: 8.0,
        skills: Default::default(),
        rules: Default::default(),
        rules_version: 1,
        progression: Default::default(),
        dialog_portraits: true,
        auto_emotions: true,
        companion_down: Default::default(),
        factions: Default::default(),
        hooks: Vec::new(),
        plugins: Vec::new(),
        player_portrait: Default::default(),
        language: "en".into(),
        languages: vec![],
        model_atlas_mb: 32,
        eight_facings: true,
        map_titles: [("village".to_string(), "Emberbrook".to_string())].into(),
        fonts: crate::fonts::FontRoles { ui: "mono".into(), fallback: vec!["cjk".into()], ..Default::default() },
        skin: "wood".into(),
        title: Default::default(),
        menu: Default::default(),
        action_bar: Vec::new(),
        credits: String::new(),
        srd: false,
        };
        cfg.sfx.pickup = "blip".into();
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sheet, "items", "icons"), 1);
        assert_eq!(db.items[0].icon.sheet, "icons");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Map, "village", "town"), 2);
        assert_eq!(cfg.start_map, "town");
        assert_eq!(cfg.map_titles.get("town").map(String::as_str), Some("Emberbrook"));
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sound, "blip", "ding"), 1);
        assert_eq!(cfg.sfx.pickup, "ding");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Font, "cjk", "noto"), 1);
        assert_eq!(cfg.fonts.fallback, vec!["noto".to_string()]);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Font, "mono", "pixel"), 1);
        assert_eq!(cfg.fonts.resolve(crate::fonts::Role::Dialog), "pixel");
        // Skins: the project's choice, and the sheets of a skin's pictures.
        let pic = crate::skin::NineSlice::new("ui_wood");
        db.skins.push(crate::skin::SkinDef { id: "wood".into(), frame: Some(pic.clone()), cursor: Some(pic), ..Default::default() });
        assert_eq!(refs_of_skin(&db.skins[0]), vec![(RefKind::Sheet, "ui_wood".to_string())]);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Skin, "wood", "oak"), 1);
        assert_eq!(cfg.skin, "oak");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sheet, "ui_wood", "ui_oak"), 2);
        assert_eq!(db.skins[0].cursor.as_ref().unwrap().sheet, "ui_oak");
    }

    #[test]
    fn characters_refer_to_their_model_and_items() {
        use crate::defs::{Appearance, CharacterDef, EquipSlot};
        use crate::inventory::ItemStack;
        let mut c: CharacterDef = ron::from_str(r#"(id: "npc", name: "Npc")"#).unwrap();
        c.appearance = Appearance { body: "farmer".into(), colors: [("hair".to_string(), [9, 9, 9])].into(), ..Default::default() };
        *c.equipment.slot_mut(EquipSlot::Clothing) = Some(ItemStack::new("tunic", 1));
        *c.equipment.slot_mut(EquipSlot::Weapon) = Some(ItemStack::new("sword", 1));
        c.inventory = vec![ItemStack::new("apple", 2)];
        let refs = refs_of_character(&c);
        // (An older look's `body` names the model that replaced its figure.)
        for want in [(RefKind::Model, "farmer"), (RefKind::Item, "tunic"), (RefKind::Item, "sword"), (RefKind::Item, "apple")] {
            assert!(refs.contains(&(want.0, want.1.to_string())), "{want:?} in {refs:?}");
        }
        // Renames follow into the model, equipment and carried items.
        let mut db = crate::defs::Database { characters: vec![c], ..Default::default() };
        let mut cfg = crate::project::ProjectConfig::new("x");
        rename_in_db(&mut db, &mut cfg, RefKind::Model, "farmer", "farmer_m");
        rename_in_db(&mut db, &mut cfg, RefKind::Item, "apple", "pear");
        rename_in_db(&mut db, &mut cfg, RefKind::Item, "tunic", "shirt");
        let c = &db.characters[0];
        assert_eq!(c.appearance.body, "farmer_m");
        assert_eq!(c.appearance.colors["hair"], [9, 9, 9]);
        assert_eq!(c.equipment.item(EquipSlot::Clothing), Some("shirt"));
        assert_eq!(c.inventory[0].item, "pear");
    }

    #[test]
    fn portraits_travel_and_follow_renames() {
        use crate::defs::{CharacterDef, SpriteRef};
        let mut c: CharacterDef = ron::from_str(r#"(id: "elder", name: "Elder")"#).unwrap();
        assert!(c.portrait.is_empty() && c.moods.is_empty(), "old characters have none");
        c.portrait = SpriteRef::new("faces", 0);
        c.moods.insert("worried".into(), SpriteRef::new("faces_moods", 1));
        // Library export carries the portrait sheets.
        let refs = refs_of_character(&c);
        assert!(refs.contains(&(RefKind::Sheet, "faces".into())) && refs.contains(&(RefKind::Sheet, "faces_moods".into())));
        // Renaming a sheet updates definitions, the player portrait and placed characters.
        let mut db = crate::defs::Database { characters: vec![c], ..Default::default() };
        let mut config = ProjectConfig { player_portrait: SpriteRef::new("faces", 2), ..ProjectConfig::new("t") };
        assert_eq!(rename_in_db(&mut db, &mut config, RefKind::Sheet, "faces", "portraits"), 2);
        assert_eq!(db.characters[0].portrait.sheet, "portraits");
        assert_eq!(config.player_portrait.sheet, "portraits");
        let mut map = Map::new("m", 4, 4);
        let mut e = EntityInstance::new(1, EntityKind::Character, "elder", 16.0, 16.0);
        e.portrait = Some(SpriteRef::new("faces", 3));
        map.entities.push(e);
        assert_eq!(rename_in_map(&mut map, RefKind::Sheet, "faces", "portraits"), 1);
        assert_eq!(map.entities[0].portrait.as_ref().unwrap().sheet, "portraits");
    }

}
