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
    Tileset,
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
}

impl RefKind {
    /// Every kind: the twelve definition lists in `defs/` file order ([`crate::project::DEFS`]),
    /// then `Map`.
    pub const ALL: [RefKind; 13] = [
        RefKind::Tileset,
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
        RefKind::Map,
    ];
    /// The definition kinds in `defs/` file order.
    pub const DEFS: [RefKind; 12] = [
        RefKind::Tileset,
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
    ];

    /// The kind's word in messages and commands: "tileset", "sheet", …, "map".
    pub fn name(self) -> &'static str {
        match self {
            RefKind::Tileset => "tileset",
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
        }
    }

    /// The `defs/<file>.ron` of a definition kind (`None` for maps).
    pub fn file(self) -> Option<&'static str> {
        Some(match self {
            RefKind::Tileset => "tilesets",
            RefKind::Sheet => "sheets",
            RefKind::Item => "items",
            RefKind::Prop => "props",
            RefKind::Character => "characters",
            RefKind::Sound => "sounds",
            RefKind::Quest => "quests",
            RefKind::Structure => "structures",
            RefKind::Skill => "skills",
            RefKind::Spell => "spells",
            RefKind::Recipe => "recipes",
            RefKind::Achievement => "achievements",
            RefKind::Map => return None,
        })
    }

    /// The kind as shown to users: "Sprite sheet", "Prop", …
    pub fn label(self) -> &'static str {
        match self {
            RefKind::Tileset => "Tileset",
            RefKind::Sheet => "Sprite sheet",
            RefKind::Sound => "Sound",
            RefKind::Item => "Item",
            RefKind::Prop => "Prop",
            RefKind::Character => "Character",
            RefKind::Map => "Map",
            RefKind::Structure => "Structure",
            RefKind::Skill => "Skill",
            RefKind::Spell => "Spell",
            RefKind::Recipe => "Recipe",
            RefKind::Quest => "Quest",
            RefKind::Achievement => "Achievement",
        }
    }

    /// "item", "Items", "sprite_sheet", "npc", "maps", … to the kind; `None` for other words.
    pub fn parse(s: &str) -> Option<RefKind> {
        let k = s.trim().to_lowercase();
        Some(match k.trim_end_matches('s') {
            "tileset" => RefKind::Tileset,
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
            _ => return None,
        })
    }

    /// A definition list (everything but `Map`).
    pub fn is_def(self) -> bool {
        self != RefKind::Map
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
}

impl Renamer<'_> {
    /// A placed entity (on a map or inside a structure).
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
    }

    /// Sheets of a layered model.
    fn look(&mut self, a: &mut crate::defs::Appearance) {
        for s in [&mut a.body, &mut a.hair, &mut a.beard] {
            self.fix(RefKind::Sheet, s);
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

    fn skill(&mut self, d: &mut crate::skills::SkillDef) {
        self.fix(RefKind::Sheet, &mut d.icon.sheet);
    }

    fn spell(&mut self, d: &mut crate::skills::SpellDef) {
        self.fix(RefKind::Sheet, &mut d.icon.sheet);
        self.fix(RefKind::Skill, &mut d.skill);
        if let crate::skills::SpellEffect::Bolt { projectile, .. } = &mut d.effect {
            self.fix(RefKind::Sheet, &mut projectile.sheet);
        }
    }

    /// Furniture: the cover sprite (a bed's blanket) and the conditions.
    fn use_spot(&mut self, s: &mut Option<crate::defs::UseSpot>) {
        if let Some(s) = s {
            self.fix(RefKind::Sheet, &mut s.cover.sheet);
            self.conditions(&mut s.conditions);
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
        for t in &mut d.tilesets {
            self.fix(RefKind::Tileset, t);
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

    fn conditions(&mut self, cs: &mut [Condition]) {
        for c in cs {
            match &mut c.cond {
                Cond::HasItem { item, .. } => self.fix(RefKind::Item, item),
                Cond::Defeated { character, .. } => self.fix(RefKind::Character, character),
                _ => {}
            }
        }
    }

    fn actions(&mut self, acts: &mut [Action]) {
        crate::events::walk_actions_mut(acts, &mut |a| match a {
            Action::GiveItem { item, .. } | Action::TakeItem { item, .. } => self.fix(RefKind::Item, item),
            Action::Equip { item, .. } | Action::GiveItemTo { item, .. } | Action::TakeItemFrom { item, .. } => self.fix(RefKind::Item, item),
            Action::Teleport { map, .. } | Action::SetSpawners { map, .. } | Action::SetWeather { map, .. } => self.fix(RefKind::Map, map),
            Action::PlaySound { sound } | Action::PlayMusic { sound } => self.fix(RefKind::Sound, sound),
            Action::Fly { sprite, .. } | Action::Effect { sprite, .. } => self.fix(RefKind::Sheet, &mut sprite.sheet),
            Action::ShowImage { sheet, .. } => self.fix(RefKind::Sheet, sheet),
            Action::If { conditions, .. } => self.conditions(conditions),
            _ => {}
        });
    }
}

/// Updates references inside definitions and the project config. Returns the number changed.
/// (The definition's own `id` must be renamed by the caller.)
pub fn rename_in_db(db: &mut Database, config: &mut ProjectConfig, kind: RefKind, old: &str, new: &str) -> usize {
    let mut r = Renamer { kind, old, new, count: 0, found: None };
    for d in &mut db.items {
        r.fix(RefKind::Sheet, &mut d.icon.sheet);
        r.fix(RefKind::Sheet, &mut d.held);
        r.fix(RefKind::Spell, &mut d.teaches);
        if let Some(a) = &mut d.armor {
            r.fix(RefKind::Sheet, &mut a.overlay);
            r.fix(RefKind::Sheet, &mut a.overlay_f);
        }
        r.triggers(&mut d.on_use);
        r.weapon(&mut d.weapon);
        r.crop(&mut d.crop);
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
        r.triggers(&mut d.triggers);
        r.weapon(&mut d.weapon);
        r.shop(&mut d.shop);
        r.loot(&mut d.loot);
        r.look(&mut d.appearance);
        r.gear(&mut d.equipment, &mut d.inventory);
        r.portraits(&mut d.portrait, &mut d.moods);
    }
    for d in &mut db.structures {
        r.structure(d);
    }
    for d in &mut db.skills {
        r.skill(d);
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
    r.fix(RefKind::Character, &mut config.player);
    r.fix(RefKind::Sheet, &mut config.player_portrait.sheet);
    let cc = &mut config.creation;
    for s in [&mut cc.body_male, &mut cc.body_female].into_iter().chain(&mut cc.hair_styles).chain(&mut cc.beards) {
        r.fix(RefKind::Sheet, s);
    }
    r.fix(RefKind::Map, &mut config.start_map);
    for (_, s) in config.sfx.fields_mut() {
        r.fix(RefKind::Sound, s);
    }
    r.count
}

/// Updates references inside a map. Returns the number changed.
pub fn rename_in_map(map: &mut Map, kind: RefKind, old: &str, new: &str) -> usize {
    let mut r = Renamer { kind, old, new, count: 0, found: None };
    for t in &mut map.tilesets {
        r.fix(RefKind::Tileset, t);
    }
    r.fix(RefKind::Sound, &mut map.music);
    for l in &mut map.parallax {
        r.fix(RefKind::Sheet, &mut l.sheet);
    }
    r.triggers(&mut map.triggers);
    for e in &mut map.entities {
        r.entity(e);
    }
    for z in &mut map.zones {
        match &mut z.kind {
            ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } => r.fix(RefKind::Map, target_map),
            ZoneKind::Spawner { character, .. } => r.fix(RefKind::Character, character),
            ZoneKind::FishingSpot { rod } => r.fix(RefKind::Item, rod),
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
    Renamer { kind: RefKind::Map, old: "", new: "", count: 0, found: Some(Vec::new()) }
}

/// Everything an item definition refers to (icon / overlay / projectile sheets, ammo, items
/// and sounds used by its triggers, ...).
pub fn refs_of_item(d: &crate::defs::ItemDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.fix(RefKind::Sheet, &mut d.icon.sheet);
    r.fix(RefKind::Sheet, &mut d.held);
    if let Some(a) = &mut d.armor {
        r.fix(RefKind::Sheet, &mut a.overlay);
        r.fix(RefKind::Sheet, &mut a.overlay_f);
    }
    r.triggers(&mut d.on_use);
    r.weapon(&mut d.weapon);
    r.fix(RefKind::Spell, &mut d.teaches);
    r.crop(&mut d.crop);
    r.found.unwrap_or_default()
}

pub fn refs_of_skill(d: &crate::skills::SkillDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.skill(&mut d);
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
    r.found.unwrap_or_default()
}

/// Everything a structure refers to (tilesets, and the definitions of its entities with what
/// those need).
pub fn refs_of_structure(d: &crate::defs::StructureDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.structure(&mut d);
    r.found.unwrap_or_default()
}

/// Every id a trigger list refers to (items, maps, sounds, characters, spells, …).
pub fn refs_of_triggers(ts: &[Trigger]) -> Vec<(RefKind, String)> {
    let mut ts = ts.to_vec();
    let mut r = collector();
    r.triggers(&mut ts);
    r.found.unwrap_or_default()
}

pub fn refs_of_character(d: &crate::defs::CharacterDef) -> Vec<(RefKind, String)> {
    let mut d = d.clone();
    let mut r = collector();
    r.triggers(&mut d.triggers);
    r.weapon(&mut d.weapon);
    r.shop(&mut d.shop);
    r.loot(&mut d.loot);
    r.look(&mut d.appearance);
    r.gear(&mut d.equipment, &mut d.inventory);
    r.portraits(&mut d.portrait, &mut d.moods);
    r.found.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::ItemDef;

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
        db.structures.push(crate::defs::StructureDef { id: "hut".into(), tilesets: vec!["terrain".into()], entities: vec![e], ..Default::default() });
        let mut cfg = ProjectConfig::new("t");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Tileset, "terrain", "land"), 1);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Prop, "chest", "box"), 1);
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Item, "key", "gate_key"), 1);
        let d = &db.structures[0];
        assert_eq!((d.tilesets[0].as_str(), d.entities[0].def.as_str()), ("land", "box"));
        let mut refs = refs_of_structure(d);
        refs.sort();
        assert_eq!(refs, vec![(RefKind::Tileset, "land".to_string()), (RefKind::Item, "gate_key".to_string()), (RefKind::Prop, "box".to_string())]);
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
            sfx: Default::default(),
            inventory_size: 24,
            creation: Default::default(),
        day_minutes: 24.0, start_hour: 8.0,
        skills: Default::default(),
        progression: Default::default(),
        dialog_portraits: true,
        companion_down: Default::default(),
        player_portrait: Default::default(),
        language: "en".into(),
        languages: vec![],
        };
        cfg.sfx.pickup = "blip".into();
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sheet, "items", "icons"), 1);
        assert_eq!(db.items[0].icon.sheet, "icons");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Map, "village", "town"), 1);
        assert_eq!(cfg.start_map, "town");
        assert_eq!(rename_in_db(&mut db, &mut cfg, RefKind::Sound, "blip", "ding"), 1);
        assert_eq!(cfg.sfx.pickup, "ding");
    }

    #[test]
    fn layered_characters_refer_to_their_sheets_and_items() {
        use crate::defs::{Appearance, CharacterDef, EquipSlot};
        use crate::inventory::ItemStack;
        let mut c: CharacterDef = ron::from_str(r#"(id: "npc", name: "Npc")"#).unwrap();
        c.appearance = Appearance { body: "body_m".into(), hair: "hair_short".into(), beard: "beard".into(), ..Default::default() };
        *c.equipment.slot_mut(EquipSlot::Clothing) = Some(ItemStack::new("tunic", 1));
        *c.equipment.slot_mut(EquipSlot::Weapon) = Some(ItemStack::new("sword", 1));
        c.inventory = vec![ItemStack::new("apple", 2)];
        let refs = refs_of_character(&c);
        for want in [(RefKind::Sheet, "body_m"), (RefKind::Sheet, "hair_short"), (RefKind::Sheet, "beard"), (RefKind::Item, "tunic"), (RefKind::Item, "sword"), (RefKind::Item, "apple")] {
            assert!(refs.contains(&(want.0, want.1.to_string())), "{want:?} in {refs:?}");
        }
        let mut sword: ItemDef = ron::from_str(r#"(id: "sword", name: "Sword", icon: (sheet: "items", frame: 0), held: "held_sword")"#).unwrap();
        assert!(refs_of_item(&sword).contains(&(RefKind::Sheet, "held_sword".into())));
        // Renames follow into appearance, equipment, carried items and held sprites.
        let mut db = crate::defs::Database { characters: vec![c], ..Default::default() };
        sword.held = "held_sword".into();
        db.items.push(sword);
        let mut cfg = crate::project::ProjectConfig::new("x");
        rename_in_db(&mut db, &mut cfg, RefKind::Sheet, "body_m", "body_man");
        rename_in_db(&mut db, &mut cfg, RefKind::Sheet, "held_sword", "held_blade");
        rename_in_db(&mut db, &mut cfg, RefKind::Item, "apple", "pear");
        rename_in_db(&mut db, &mut cfg, RefKind::Item, "tunic", "shirt");
        let c = &db.characters[0];
        assert_eq!(c.appearance.body, "body_man");
        assert_eq!(c.equipment.item(EquipSlot::Clothing), Some("shirt"));
        assert_eq!(c.inventory[0].item, "pear");
        assert_eq!(db.items[0].held, "held_blade");
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
