//! Definition, script, language and asset-library tools on the main thread. Definitions travel
//! as JSON in serde's form of the engine types; writes validate the ids and files they refer to
//! before touching `project.db`, then save.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rustpg2d_engine::achievements::AchievementDef;
use rustpg2d_engine::crafting::RecipeDef;
use rustpg2d_engine::defs::*;
use rustpg2d_engine::defs::Def;
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::events::{Action, Cond, Trigger, TriggerOn, TriggerOwner};
use rustpg2d_engine::library::{self, AssetRef};
use rustpg2d_engine::quest::QuestDef;
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::skills::{SkillDef, SkillEffect, SpellDef, SpellEffect};
use serde::Serialize;
use serde_json::Value;

use crate::app::Editor;

use super::exec::{page, suggestions, text, Reply};
use super::exec_maps::parse_ref_kind;
use super::tools::*;

/// A definition kind from the agent's word ("items", "Item", "npc", …); maps are no definitions.
fn parse_def_kind(s: &str) -> Result<RefKind, String> {
    RefKind::parse(s).filter(|k| k.is_def()).ok_or_else(|| format!("kind \"{s}\" must be one of {}", RefKind::DEFS.iter().map(|k| k.file().unwrap_or("?")).collect::<Vec<_>>().join(", ")))
}

/// "items", "sprite sheets", … (the defs file's name).
fn plural(kind: RefKind) -> &'static str {
    kind.file().unwrap_or("maps")
}

/// One line of key facts about a definition, for `list_defs`.
pub(crate) trait DefFacts: Def {
    fn facts(&self) -> String;
}

macro_rules! def_facts {
    ($t:ty, |$d:ident| $facts:expr) => {
        impl DefFacts for $t {
            fn facts(&self) -> String {
                let $d = self;
                $facts
            }
        }
    };
}

def_facts!(TilesetDef, |d| format!("image {}; {} solid tiles; terrains [{}]; {} animations", d.image, d.solid.len(), d.terrains.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", "), d.animations.len()));
def_facts!(SheetDef, |d| format!("image {}; {}x{} px frames; {:?}{}{}", d.image, d.frame_w, d.frame_h, d.kind, if d.female { "; female cut" } else { "" }, if d.tags.is_empty() { String::new() } else { format!("; animations [{}]", d.tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")) }));
def_facts!(ItemDef, |d| {
    let mut f = vec![format!("\"{}\"", d.name)];
    if let Some(w) = &d.weapon {
        f.push(format!("weapon dmg {}{}", w.damage, if w.ranged.is_some() { " ranged" } else { "" }));
    }
    if let Some(a) = &d.armor {
        f.push(format!("armor {:?} def {}", a.slot, a.defense));
    }
    if d.consumable {
        f.push("consumable".into());
    }
    if d.food {
        f.push("food".into());
    }
    if !d.tool.is_empty() {
        f.push(format!("tool {}", d.tool));
    }
    if !d.teaches.is_empty() {
        f.push(format!("teaches {}", d.teaches));
    }
    f.push(format!("value {}, stack {}", d.value, d.max_stack));
    f.join("; ")
});
def_facts!(PropDef, |d| {
    let mut f = vec![format!("\"{}\"", d.name)];
    f.push(if d.solid { "solid".into() } else { "walkable".into() });
    if !d.states.is_empty() {
        f.push(format!("states [{}]", d.state_names().join(", ")));
    }
    if d.container.is_some() {
        f.push("container".into());
    }
    if d.lock.is_some() {
        f.push("lock".into());
    }
    if d.use_spot.is_some() {
        f.push("furniture".into());
    }
    if d.gather.is_some() {
        f.push("gatherable".into());
    }
    if d.plot {
        f.push("farm plot".into());
    }
    if !d.triggers.is_empty() {
        f.push(format!("{} triggers", d.triggers.len()));
    }
    f.join("; ")
});
def_facts!(CharacterDef, |d| format!(
    "\"{}\"; hp {}; {:?}{}{}{}{}",
    d.name,
    d.max_hp,
    d.behavior,
    if d.hostile { "; hostile" } else { "" },
    if d.shop.is_some() { "; shop" } else { "" },
    if d.triggers.is_empty() { String::new() } else { format!("; {} triggers", d.triggers.len()) },
    if d.description.is_empty() { String::new() } else { format!("; {}", d.description) }
));
def_facts!(SoundDef, |d| format!("file {}; volume {}", d.file, d.volume));
def_facts!(QuestDef, |d| format!("\"{}\"; {} stages [{}]; xp {}", d.title, d.stages.len(), d.stages.join(" | "), d.xp));
def_facts!(StructureDef, |d| format!("\"{}\"; {}x{} tiles; category {}; {} entities", d.name, d.w, d.h, d.category, d.entities.len()));
def_facts!(SkillDef, |d| format!("\"{}\"; max level {}; {:?}", d.name, d.max_level, d.effect));
def_facts!(SpellDef, |d| format!("\"{}\"; mana {}; cooldown {}; needs {} level {}; {:?}", d.name, d.mana, d.cooldown, d.skill, d.level, d.effect));
def_facts!(RecipeDef, |d| format!("makes {} x{} from [{}]{}{}", d.result, d.count, d.ingredients.iter().map(|i| format!("{} x{}", i.item, i.count)).collect::<Vec<_>>().join(", "), if d.station.is_empty() { String::new() } else { format!(" at {}", d.station) }, if d.skill.is_empty() { String::new() } else { format!("; {} level {}", d.skill, d.level) }));
def_facts!(AchievementDef, |d| format!("\"{}\"; {}{} conditions", d.name, if d.hidden { "hidden; " } else { "" }, d.conditions.len()));

/// Calls a generic function on the definition list of `kind` (a kind from `parse_def_kind`).
macro_rules! on_defs {
    ($db:expr, $kind:expr, $f:ident ( $($arg:expr),* )) => {
        rustpg2d_engine::with_def_kind!($kind, |T| $f(T::list_mut($db) $(, $arg)*), else => unreachable!("parse_def_kind returns definition kinds only"))
    };
}

fn ids<T: DefFacts>(list: &mut [T]) -> Vec<String> {
    list.iter().map(|d| d.id().to_string()).collect()
}

fn lines<T: DefFacts>(list: &mut [T], filter: &str) -> Vec<String> {
    let f = filter.to_lowercase();
    list.iter().filter(|d| f.is_empty() || d.id().to_lowercase().contains(&f) || d.facts().to_lowercase().contains(&f)).map(|d| format!("{}: {}", d.id(), d.facts())).collect()
}

fn get<T: DefFacts>(list: &mut [T], id: &str) -> Option<Value> {
    list.iter().find(|d| d.id() == id).and_then(|d| serde_json::to_value(d).ok())
}

/// Inserts or replaces `def` (merging into the existing one if `merge`); returns (id, replaced).
fn upsert<T: DefFacts>(list: &mut Vec<T>, def: Value, merge: bool) -> Result<(String, bool), String> {
    let id = def.get("id").and_then(|v| v.as_str()).map(str::to_string).ok_or("the definition needs an \"id\" string")?;
    if !crate::app::valid_name(&id) {
        return Err("ids use letters, digits, '_' and '-'".into());
    }
    let pos = list.iter().position(|d| d.id() == id);
    let value = match (merge, pos) {
        (true, Some(i)) => {
            let mut v = serde_json::to_value(&list[i]).map_err(|e| e.to_string())?;
            super::patch::merge(&mut v, &def);
            v
        }
        (true, None) => return Err(format!("merge: no {id} exists yet; send the whole definition (def_template) without merge")),
        (false, _) => def,
    };
    let parsed: T = serde_json::from_value(value).map_err(|e| format!("the definition isn't valid: {e}. def_template shows the shape"))?;
    match pos {
        Some(i) => list[i] = parsed,
        None => list.push(parsed),
    }
    Ok((id, pos.is_some()))
}

fn remove<T: DefFacts>(list: &mut Vec<T>, id: &str) -> bool {
    let before = list.len();
    list.retain(|d| d.id() != id);
    list.len() != before
}

/// A fresh definition of `kind` with sensible values (the editor's "+ New" defaults).
fn template(kind: RefKind, id: &str, db: &Database) -> (Value, &'static str) {
    let icon = SpriteRef::default();
    match kind {
        RefKind::Tileset => (
            val(&TilesetDef::new(id, "tilesets/<file>.png")),
            "image: PNG path under the project (32 px tiles, any columns); solid: tile indices that block; ladder: tile indices the player climbs (characters keep off); animations: [{tile, frames, fps}]; lights: [{tile, light: {radius, color, intensity, flicker}}]; terrains: auto-tiling sets (easier made in the editor).",
        ),
        RefKind::Sheet => (
            val(&SheetDef::new(id, "sprites/<file>.png", 32, 32)),
            "kind: Sprite (props, effects) | Body | Hair | Beard | Outfit | Armor | Held (layered characters: 4 rows down/left/right/up, 4 frames); frame_w/frame_h in px; tags: named frame ranges [{name, first, last, fps}].",
        ),
        RefKind::Item => (
            val(&ItemDef::fresh(id)),
            "icon: {sheet, frame} of a sprite sheet; weapon: {damage, cooldown, ranged: null | {projectile, speed, range, ammo}, reach, arc, knockback, two_handed, effect}; armor: {slot: Head|Body|Hands|Feet|OffHand|Ring, defense, speed, overlay (sheet), overlay_f}; consumable + on_use triggers (ItemUse owner: UseItem event) = potions; food; teaches: a skill or spell id; tool: Pickaxe|Axe|Hoe|Rod… name used by props' gather; held: a Held sheet drawn in the hand; value in coins; crop: {stages: [{sprite, hours}], harvest: loot}.",
        ),
        RefKind::Prop => (
            val(&PropDef::fresh(id)),
            "sprite: {sheet, frame} (or {sheet, frame, w, h} for a block of frames); solid; hitbox [w, h] px at the feet; states: [{name, sprite, solid, light}] switched by SetState / set_state (default_state names the first); triggers (Prop owner: Interact, Enter, UseItem, Custom, Unlock, Lock…); container: {size, items: [{item, count}], conditions, locked_text, open_frame, open_state}; lock: {key, consume_key, on_unlock: Nothing|Disappear|{Open:{frame}}|{ToState:{state}}, locked_text, unlock_text, relock, pick_difficulty}; use_spot: furniture {pose: Sit|Lie|Stand, offset, facing, sleep_until}; gather: {tool, hits, loot, then_state}; regrow: {from, to, hours}; plot: a farm plot.",
        ),
        RefKind::Character => (val(&crate::builder::default_character(db, id, "New character")), "appearance: {body (a Body sheet), skin, hair (sheet), hair_color, beard}; equipment / inventory: the items it wears and carries; behavior: Static | {Wander:{radius}} | {Chase:{..}}; hostile + weapon/damage/defense/xp/loot for enemies; shop: {title, currency, items: [{item, count, unlimited, price}], sell_rate, open_on_talk}; triggers (Character owner: Interact, Defeat, HpBelow, Custom…) hold its dialogs; portrait / moods: {sheet, frame} for dialog portraits; light: carried light."),
        RefKind::Sound => (val(&SoundDef { id: id.into(), file: "sounds/<file>.ogg".into(), volume: 1.0 }), "file: WAV or Ogg Vorbis under the project; volume 0..1. Used by PlaySound / PlayMusic, map music and the project's built-in sounds."),
        RefKind::Quest => (val(&QuestDef { id: id.into(), title: "New quest".into(), description: String::new(), stages: vec!["Find out more".into(), "Done".into()], xp: 0 }), "stages: the journal lines in order (stage 0 is the first after Start); xp on completion. Triggers drive it: Action Quest{quest, op: Start|Advance|SetStage|Complete|Fail, stage}, Cond Quest{quest, check: NotStarted|Active|Completed|Failed|AtStage|StageAtLeast, stage}."),
        RefKind::Structure => (val(&StructureDef { id: id.into(), name: "New structure".into(), category: String::new(), w: 2, h: 2, tilesets: vec![], layers: vec![vec![0; 4], vec![0; 4], vec![0; 4]], collision: vec![0; 4], entities: vec![] }), "layers: one Vec per map layer of w*h tile values (0 = empty; tile = slot << 16 | index with slot into `tilesets`); collision: w*h bytes (0 auto, 1 solid, 2 walkable); entities with positions relative to the top-left. Easier: build it on a map and capture it in the editor (Structures tool)."),
        RefKind::Skill => (val(&SkillDef { id: id.into(), name: "New skill".into(), description: String::new(), icon: icon.clone(), max_level: 5, effect: SkillEffect::None }), "effect: None | MeleeDamage | RangedDamage | SpellDamage | Defense | Speed | Lockpick | Haggle | Mana | ManaRegen | {Script:{function}} | {Gather:{tool, percent}} | FoodBonus; max_level; players spend skill points (K screen)."),
        RefKind::Spell => (val(&SpellDef::new(id, "New spell", "", icon.clone(), 5, 1.0, SpellEffect::Heal { amount: 5 })), "effect: {Heal:{amount}} | {Bolt:{damage, projectile (sheet), speed, range}} | {Blast:{..}} | {Light:{..}} | {Script:{function}}; mana cost; cooldown s; skill + level required to learn (empty skill = none). Items with teaches = this id teach it."),
        RefKind::Recipe => (val(&RecipeDef::new(id, "<item id>", 1, vec![], "")), "result + count from ingredients [{item, count}]; station: a prop id the player must use (empty = anywhere, craftable from the inventory), station_state: the state it must be in (a lit fire); skill + level required; tools: item ids that must be in the inventory; quality: [{level, result}] better results at higher skill."),
        RefKind::Achievement => (val(&AchievementDef { id: id.into(), name: "New achievement".into(), description: String::new(), icon, hidden: false, conditions: vec![] }), "conditions: checked while playing (same shapes as trigger conditions: Quest, Flag, Var, Defeated, Level, HasItem, …); hidden: shown as ??? until unlocked; also unlocked by the UnlockAchievement action / unlock_achievement()."),
        RefKind::Map => (Value::Null, "maps are not definitions"),
    }
}

fn val<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

fn json_str(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

impl Editor {
    fn def_ids(&mut self, kind: RefKind) -> Vec<String> {
        let db = &mut self.project.db;
        on_defs!(db, kind, ids())
    }

    pub(crate) fn mcp_list_defs(&mut self, p: ListDefsParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let kind = parse_def_kind(&p.kind)?;
        let filter = p.filter.clone().unwrap_or_default();
        let db = &mut self.project.db;
        let rows = on_defs!(db, kind, lines(&filter));
        let total = rows.len();
        text(format!("{} {}{}:\n{}", total, plural(kind), if filter.is_empty() { String::new() } else { format!(" matching \"{filter}\"") }, page(rows, p.offset.unwrap_or(0))))
    }

    pub(crate) fn mcp_get_def(&mut self, p: DefParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let kind = parse_def_kind(&p.kind)?;
        let db = &mut self.project.db;
        match on_defs!(db, kind, get(&p.id)) {
            Some(v) => text(json_str(&v)),
            None => Err(format!("no {} \"{}\".{}", kind.name(), p.id, suggestions(&p.id, self.def_ids(kind).iter().map(String::as_str)))),
        }
    }

    pub(crate) fn mcp_def_template(&mut self, p: DefTemplateParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let kind = parse_def_kind(&p.kind)?;
        let id = p.id.unwrap_or_else(|| format!("new_{}", kind.name()));
        let (v, notes) = template(kind, &id, &self.project.db);
        text(format!("{} template (upsert_def takes this shape; fields you leave out keep these defaults only where the engine has defaults, so send the whole object):\n{}\n\nFields: {notes}", kind.name(), json_str(&v)))
    }

    /// Ids a definition refers to that don't exist, and files it needs that are missing.
    fn validate_def(&self, db: &Database, kind: RefKind, id: &str) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        if library::KINDS.contains(&kind) {
            let rk = kind;
            for dep in library::deps(db, &(rk, id.to_string())) {
                if !library::exists(db, &dep) {
                    return Err(format!("it refers to {} \"{}\", which doesn't exist.{}", library::kind_label(dep.0), dep.1, suggestions(&dep.1, self.mcp_ids_of(dep.0).iter().map(String::as_str))));
                }
            }
        }
        let file = match kind {
            RefKind::Tileset => db.tileset(id).map(|d| d.image.clone()),
            RefKind::Sheet => db.sheet(id).map(|d| d.image.clone()),
            RefKind::Sound => db.sound(id).map(|d| d.file.clone()),
            _ => None,
        };
        if let Some(f) = file {
            if !self.project.root.join(&f).is_file() {
                return Err(format!("the file {f} doesn't exist under the project; put the image / sound there first (or pick one from the library: library_list)"));
            }
        }
        let (owner, triggers): (TriggerOwner, Vec<Trigger>) = match kind {
            RefKind::Item => (TriggerOwner::ItemUse, db.item(id).map(|d| d.on_use.clone()).unwrap_or_default()),
            RefKind::Prop => (TriggerOwner::Prop, db.prop(id).map(|d| d.triggers.clone()).unwrap_or_default()),
            RefKind::Character => (TriggerOwner::Character, db.character(id).map(|d| d.triggers.clone()).unwrap_or_default()),
            RefKind::Achievement => (TriggerOwner::Map, db.achievement(id).map(|d| vec![Trigger { on: TriggerOn::MapEnter, conditions: d.conditions.clone(), actions: vec![], once: false }]).unwrap_or_default()),
            _ => (TriggerOwner::Map, vec![]),
        };
        warnings.extend(self.mcp_check_triggers(owner, &triggers)?);
        match kind {
            RefKind::Quest => {
                if db.quest(id).is_some_and(|q| q.stages.is_empty()) {
                    warnings.push("the quest has no stages: the journal shows nothing while it is active".into());
                }
            }
            RefKind::Recipe => {
                if let Some(r) = db.recipe(id) {
                    if db.item(&r.result).is_none() {
                        return Err(format!("result \"{}\" is not an item", r.result));
                    }
                    if !r.station.is_empty() && db.prop(&r.station).is_none() {
                        return Err(format!("station \"{}\" is not a prop", r.station));
                    }
                }
            }
            RefKind::Spell => {
                if let Some(s) = db.spell(id) {
                    if !s.skill.is_empty() && db.skill(&s.skill).is_none() {
                        return Err(format!("skill \"{}\" doesn't exist", s.skill));
                    }
                }
            }
            _ => {}
        }
        Ok(warnings)
    }

    pub(crate) fn mcp_upsert_def(&mut self, p: UpsertDefParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let kind = parse_def_kind(&p.kind)?;
        let mut copy = self.project.db.clone();
        let db = &mut copy;
        let (id, replaced) = on_defs!(db, kind, upsert(p.def, p.merge.unwrap_or(false)))?;
        let warnings = self.validate_def(db, kind, &id)?;
        self.project.db = copy;
        self.project.db.reindex();
        self.defs_dirty = true;
        if matches!(kind, RefKind::Tileset | RefKind::Sheet | RefKind::Sound) {
            self.reload_assets();
        }
        let mut msg = format!("{} {} \"{id}\".", if replaced { "Replaced" } else { "Created" }, kind.name());
        for w in warnings {
            msg += &format!("\nWarning: {w}");
        }
        self.mcp_after_write(msg)
    }

    /// Everything that still uses a definition: other definitions, maps, scripts.
    fn users_of(&self, kind: RefKind, id: &str) -> Vec<String> {
        let mut users = Vec::new();
        if library::KINDS.contains(&kind) {
            let rk = kind;
            for (k, dep) in library::dependents(&self.project.db, &(rk, id.to_string())) {
                users.push(format!("{} {dep}", library::kind_label(k)));
            }
        }
        for m in &self.maps {
            let Ok(map) = self.mcp_load_map(m) else { continue };
            let mut n = 0;
            match kind {
                RefKind::Item | RefKind::Prop | RefKind::Character => {
                    let ek = match kind {
                        RefKind::Item => EntityKind::Item,
                        RefKind::Prop => EntityKind::Prop,
                        _ => EntityKind::Character,
                    };
                    n += map.entities.iter().filter(|e| e.kind == ek && e.def == id).count();
                    if kind == RefKind::Character {
                        n += map.zones.iter().filter(|z| matches!(&z.kind, rustpg2d_engine::map::ZoneKind::Spawner { character, .. } if character == id)).count();
                    }
                }
                RefKind::Tileset => n += map.tilesets.iter().filter(|t| *t == id).count(),
                RefKind::Sound => n += (map.music == id) as usize,
                _ => {}
            }
            // Triggers naming the id (quests, items given, sounds played, ...).
            let needle = format!("\"{id}\"");
            let trig = serde_json::to_string(&(&map.triggers, map.zones.iter().map(|z| &z.triggers).collect::<Vec<_>>(), map.entities.iter().map(|e| &e.triggers).collect::<Vec<_>>())).unwrap_or_default();
            n += trig.matches(&needle).count();
            if n > 0 {
                users.push(format!("map {m} ({n} places)"));
            }
        }
        let id_kind = match kind {
            RefKind::Item => Some(rustpg2d_engine::script_refs::IdKind::Item),
            RefKind::Sound => Some(rustpg2d_engine::script_refs::IdKind::Sound),
            RefKind::Quest => Some(rustpg2d_engine::script_refs::IdKind::Quest),
            RefKind::Skill => Some(rustpg2d_engine::script_refs::IdKind::Skill),
            RefKind::Spell => Some(rustpg2d_engine::script_refs::IdKind::Spell),
            RefKind::Sheet => Some(rustpg2d_engine::script_refs::IdKind::Sheet),
            RefKind::Character => Some(rustpg2d_engine::script_refs::IdKind::Character),
            RefKind::Recipe => Some(rustpg2d_engine::script_refs::IdKind::Recipe),
            RefKind::Achievement => Some(rustpg2d_engine::script_refs::IdKind::Achievement),
            _ => None,
        };
        if let Some(ik) = id_kind {
            for f in self.mcp_script_files() {
                let Ok(src) = std::fs::read_to_string(&f) else { continue };
                let lines: Vec<String> = rustpg2d_engine::script_refs::id_refs(&src).into_iter().filter(|(a, k)| *k == ik && a.value == id).map(|(a, _)| a.line.to_string()).collect();
                if !lines.is_empty() {
                    users.push(format!("script {}:{}", f.file_name().unwrap_or_default().to_string_lossy(), lines.join(",")));
                }
            }
        }
        if kind == RefKind::Character && self.project.config.player == id {
            users.push("the project's player character".into());
        }
        users
    }

    pub(crate) fn mcp_delete_def(&mut self, p: DeleteDefParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let kind = parse_def_kind(&p.kind)?;
        if !self.def_ids(kind).contains(&p.id) {
            return Err(format!("no {} \"{}\".{}", kind.name(), p.id, suggestions(&p.id, self.def_ids(kind).iter().map(String::as_str))));
        }
        let users = self.users_of(kind, &p.id);
        if !users.is_empty() && !p.force.unwrap_or(false) {
            return Err(format!("{} \"{}\" is still used by: {}. Change those first, or delete with force: true.", kind.name(), p.id, users.join("; ")));
        }
        let db = &mut self.project.db;
        on_defs!(db, kind, remove(&p.id));
        self.project.db.reindex();
        self.defs_dirty = true;
        if matches!(kind, RefKind::Tileset | RefKind::Sheet | RefKind::Sound) {
            self.reload_assets();
        }
        self.mcp_after_write(format!("Deleted {} \"{}\"{}. Its image / sound files were kept.", kind.name(), p.id, if users.is_empty() { String::new() } else { format!(" although it was used by: {}", users.join("; ")) }))
    }

    pub(crate) fn mcp_events_reference(&mut self, p: EventsRefParams) -> Result<Reply, String> {
        let what = p.what.unwrap_or_default().to_lowercase();
        let all = what.is_empty();
        let mut out = String::from("Triggers are {\"on\": EVENT, \"conditions\": [{\"cond\": COND, \"negate\": false}], \"actions\": [ACTION], \"once\": false}. Texts may use {player}, {sir|madam} and {var}.\n");
        if all || what.starts_with("event") {
            out += "\nEVENTS per owner (set_triggers owner -> allowed \"on\" values):\n";
            for (name, owner) in [("map", TriggerOwner::Map), ("zone", TriggerOwner::Zone), ("character (entity or def)", TriggerOwner::Character), ("prop (entity or def)", TriggerOwner::Prop), ("item pickup (entity)", TriggerOwner::Pickup), ("item def (on use)", TriggerOwner::ItemUse)] {
                let ts: Vec<String> = owner.templates().iter().map(|t| format!("{} = {}", t.label(), serde_json::to_string(t).unwrap_or_default())).collect();
                out += &format!("  {name}: {}\n", ts.join("; "));
            }
        }
        if all || what.starts_with("cond") {
            out += "\nCONDITIONS (label = JSON with example values):\n";
            for c in Cond::templates() {
                out += &format!("  {} = {}\n", c.label(), serde_json::to_string(&c).unwrap_or_default());
            }
        }
        if all || what.starts_with("action") {
            out += "\nACTIONS (label = JSON with example values; ops: Quest op Start|Advance|SetStage|Complete|Fail; VarOp Set|Add):\n";
            for a in Action::templates() {
                out += &format!("  {} = {}\n", a.label(), serde_json::to_string(&a).unwrap_or_default());
            }
        }
        text(out)
    }

    // ------------------------------------------------------------ scripts

    fn script_path(&self, file: &str) -> Result<PathBuf, String> {
        let f = file.trim();
        let f = if f.ends_with(".rhai") { f.to_string() } else { format!("{f}.rhai") };
        if f.is_empty() || f.contains(['/', '\\']) || f.starts_with('.') {
            return Err("file must be a plain name like main.rhai (in scripts/)".into());
        }
        Ok(self.project.scripts_dir().join(f))
    }

    pub(crate) fn mcp_list_scripts(&mut self) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let mut out = Vec::new();
        for f in self.mcp_script_files() {
            let src = std::fs::read_to_string(&f).unwrap_or_default();
            let fns: Vec<String> = rustpg2d_engine::script_refs::functions(&src).iter().map(|f| format!("{}({})", f.name, f.params.join(", "))).collect();
            out.push(format!("{} ({} lines): {}", f.file_name().unwrap_or_default().to_string_lossy(), src.lines().count(), if fns.is_empty() { "no functions".to_string() } else { fns.join(", ") }));
        }
        if out.is_empty() {
            out.push("no scripts yet (write_script creates scripts/<name>.rhai; functions are called by RunScript actions, Script conditions, cutscenes, spells)".into());
        }
        text(out.join("\n"))
    }

    pub(crate) fn mcp_read_script(&mut self, p: ReadScriptParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let path = self.script_path(&p.file)?;
        let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let lines: Vec<&str> = src.lines().collect();
        let from = p.from_line.unwrap_or(1).max(1);
        let to = p.to_line.unwrap_or(lines.len()).min(lines.len());
        if from > to {
            return Err(format!("the file has {} lines", lines.len()));
        }
        let body: Vec<String> = lines[from - 1..to].iter().enumerate().map(|(i, l)| format!("{:>4} {l}", from + i)).collect();
        text(format!("{} lines {from}-{to} of {}:\n{}", path.file_name().unwrap_or_default().to_string_lossy(), lines.len(), body.join("\n")))
    }

    /// Checks `files` (all scripts when empty): unknown ids, compile errors, duplicate functions.
    fn check_scripts_text(&self, only: Option<&PathBuf>) -> String {
        let mut out = Vec::new();
        let files = self.mcp_script_files();
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for f in &files {
            let Ok(src) = std::fs::read_to_string(f) else { continue };
            let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
            for func in rustpg2d_engine::script_refs::functions(&src) {
                if let Some(other) = seen.insert(func.name.clone(), name.clone()) {
                    if other != name || only.is_none_or(|o| o == f) {
                        out.push(format!("{name}: function {} is also defined in {other}", func.name));
                    }
                }
            }
            if only.is_some_and(|o| o != f) {
                continue;
            }
            for issue in rustpg2d_engine::script_refs::check_script(&src, &self.project.db, &self.refs.maps, &self.refs.spawns) {
                out.push(format!("{name}:{}: {}", issue.line, issue.message));
            }
        }
        let mut host = rustpg2d_engine::script::ScriptHost::new();
        host.load_dir(&self.project.scripts_dir());
        for e in &host.errors {
            out.push(format!("compile: {e}"));
        }
        if out.is_empty() {
            format!("Scripts OK: {} file(s), {} functions.", files.len(), host.function_names().len())
        } else {
            format!("Script problems:\n{}", out.join("\n"))
        }
    }

    pub(crate) fn mcp_write_script(&mut self, p: WriteScriptParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let path = self.script_path(&p.file)?;
        let open = self.db.script_file.as_ref() == Some(&path);
        if open && self.db.script_dirty && !p.force.unwrap_or(false) {
            return Err(format!("{} is open in the editor with unsaved edits; ask the person to save, or write with force: true to overwrite them", p.file));
        }
        std::fs::create_dir_all(self.project.scripts_dir()).map_err(|e| e.to_string())?;
        let existed = path.exists();
        std::fs::write(&path, &p.text).map_err(|e| format!("writing {}: {e}", path.display()))?;
        if open {
            self.db.script_text = p.text.clone();
            self.db.script_saved = p.text.clone();
            self.db.script_dirty = false;
        }
        self.refresh_refs();
        self.watch_resnapshot();
        let check = self.check_scripts_text(Some(&path));
        text(format!("{} {} ({} lines).\n{check}", if existed { "Wrote" } else { "Created" }, path.file_name().unwrap_or_default().to_string_lossy(), p.text.lines().count()))
    }

    pub(crate) fn mcp_check_script(&mut self, p: CheckScriptParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let only = match &p.file {
            Some(f) => Some(self.script_path(f)?),
            None => None,
        };
        text(self.check_scripts_text(only.as_ref()))
    }

    pub(crate) fn mcp_scripting_api(&mut self, p: FilterParams) -> Result<Reply, String> {
        let f = p.filter.unwrap_or_default().to_lowercase();
        let mut out = Vec::new();
        for api in crate::script_editor::API {
            let line = format!("{}({}) — {}", api.name, api.params.iter().map(|(n, k)| format!("{n}: {k:?}")).collect::<Vec<_>>().join(", "), api.doc);
            if f.is_empty() || line.to_lowercase().contains(&f) {
                out.push(line);
            }
        }
        let mut s = format!("Engine functions ({}; parameter kinds say what the string is: Item, Map, Spawn, Sound, Quest, Target = \"self\" | \"player\" | an entity name):\n{}", out.len(), out.join("\n"));
        if self.project_open {
            let own: Vec<&String> = self.refs.functions.iter().filter(|n| f.is_empty() || n.to_lowercase().contains(&f)).collect();
            if !own.is_empty() {
                s += &format!("\n\nThe project's own functions: {}", own.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
        }
        s += "\nFull reference with examples: resource rustpg2d://docs/scripting.";
        text(s)
    }

    // ------------------------------------------------------------ languages

    pub(crate) fn mcp_translation_template(&mut self, p: LangParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let msg = self.export_translation(&p.code)?;
        self.watch_resnapshot();
        text(format!("{msg} (read_translation / write_translation work on it)"))
    }

    pub(crate) fn mcp_read_translation(&mut self, p: ReadTranslationParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let code = p.code.trim().to_lowercase();
        let Some(file) = rustpg2d_engine::lang::load_file(&self.project.root, &code) else { return Err(format!("no lang/{code}.ron (translation_template creates it)")) };
        let missing_only = p.missing_only.unwrap_or(false);
        let rows: Vec<String> = file.texts.iter().filter(|(_, v)| !missing_only || v.is_empty()).map(|(k, v)| format!("{} => {}", serde_json::to_string(k).unwrap_or_default(), serde_json::to_string(v).unwrap_or_default())).collect();
        let open = file.texts.values().filter(|v| v.is_empty()).count();
        text(format!("lang/{code}.ron \"{}\": {} texts, {open} untranslated, {} runtime texts\n{}", file.name, file.texts.len(), file.ui.len(), page(rows, p.offset.unwrap_or(0))))
    }

    pub(crate) fn mcp_write_translation(&mut self, p: WriteTranslationParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let code = p.code.trim().to_lowercase();
        if code.len() < 2 || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!("\"{code}\" is not a language code (like de, fr, pt-br)"));
        }
        if code == self.project.config.language {
            return Err(format!("{code} is the game's own language"));
        }
        // The template over the current project (saved first): texts written since the file was
        // made become keys, existing translations are kept.
        self.save_all();
        let existing = rustpg2d_engine::lang::load_file(&self.project.root, &code);
        let (mut file, _) = rustpg2d_engine::lang::template(&self.project, &code, existing);
        let mut unknown = Vec::new();
        for (k, v) in &p.texts {
            if file.texts.contains_key(k) {
                file.texts.insert(k.clone(), v.clone());
            } else {
                unknown.push(k.clone());
            }
        }
        if let Some(ui) = &p.ui {
            for (k, v) in ui {
                file.ui.insert(k.clone(), v.clone());
            }
        }
        if let Some(n) = &p.name {
            file.name = n.clone();
        }
        if !self.project.config.languages.contains(&code) {
            self.project.config.languages.push(code.clone());
            self.defs_dirty = true;
        }
        let path = self.project.root.join("lang").join(format!("{code}.ron"));
        rustpg2d_engine::project::write_ron(&path, &file).map_err(|e| format!("{e:#}"))?;
        let open = file.texts.values().filter(|v| v.is_empty()).count();
        let mut msg = format!("lang/{code}.ron written: {} texts, {open} still untranslated.", file.texts.len());
        if !unknown.is_empty() {
            msg += &format!("\nWarning: {} key(s) are not texts of the game and were skipped (keys must be the original texts exactly; read_translation lists them): {}", unknown.len(), unknown.iter().take(5).map(|k| serde_json::to_string(k).unwrap_or_default()).collect::<Vec<_>>().join(", "));
        }
        self.mcp_after_write(msg)
    }

    // ------------------------------------------------------------ asset library

    fn lib_name(db: &Database, a: &AssetRef) -> String {
        match a.0 {
            RefKind::Item => db.item(&a.1).map(|d| d.name.clone()),
            RefKind::Prop => db.prop(&a.1).map(|d| d.name.clone()),
            RefKind::Character => db.character(&a.1).map(|d| d.name.clone()),
            RefKind::Structure => db.structure(&a.1).map(|d| d.name.clone()),
            RefKind::Skill => db.skill(&a.1).map(|d| d.name.clone()),
            RefKind::Spell => db.spell(&a.1).map(|d| d.name.clone()),
            _ => None,
        }
        .unwrap_or_default()
    }

    pub(crate) fn mcp_library_list(&mut self, p: LibraryListParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        self.lib.ensure_loaded();
        let Some(lib) = &self.lib.lib else { return Err(self.lib.error.clone().unwrap_or_else(|| "the asset library is not available".into())) };
        let kind = match &p.kind {
            Some(k) => Some(parse_ref_kind(k)?),
            None => None,
        };
        let f = p.filter.clone().unwrap_or_default().to_lowercase();
        let mut rows = Vec::new();
        for a in library::all_assets(&lib.db) {
            if kind.is_some_and(|k| k != a.0) {
                continue;
            }
            let name = Self::lib_name(&lib.db, &a);
            if !f.is_empty() && !a.1.to_lowercase().contains(&f) && !name.to_lowercase().contains(&f) {
                continue;
            }
            let mut deps: Vec<String> = library::deps(&lib.db, &a).iter().map(|d| format!("{} {}", library::kind_label(d.0), d.1)).collect();
            deps.sort();
            deps.dedup();
            rows.push(format!(
                "{} {}{}{}{}",
                library::kind_label(a.0),
                a.1,
                if name.is_empty() { String::new() } else { format!(" \"{name}\"") },
                if deps.is_empty() { String::new() } else { format!(" needs [{}]", deps.join(", ")) },
                if library::exists(&self.project.db, &a) { " (in the project)" } else { "" }
            ));
        }
        let total = rows.len();
        text(format!("{total} library assets{}:\n{}", if f.is_empty() { String::new() } else { format!(" matching \"{f}\"") }, page(rows, p.offset.unwrap_or(0))))
    }

    pub(crate) fn mcp_library_info(&mut self, p: DefParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        self.lib.ensure_loaded();
        let Some(lib) = &self.lib.lib else { return Err(self.lib.error.clone().unwrap_or_else(|| "the asset library is not available".into())) };
        let rk = parse_ref_kind(&p.kind)?;
        let a: AssetRef = (rk, p.id.clone());
        if !library::exists(&lib.db, &a) {
            let ids: Vec<String> = library::all_assets(&lib.db).into_iter().filter(|x| x.0 == rk).map(|x| x.1).collect();
            return Err(format!("the library has no {} \"{}\".{}", library::kind_label(rk), p.id, suggestions(&p.id, ids.iter().map(String::as_str))));
        }
        let kind = parse_def_kind(&p.kind)?;
        let mut copy = lib.db.clone();
        let db = &mut copy;
        let v = on_defs!(db, kind, get(&p.id)).unwrap_or(Value::Null);
        let deps: Vec<String> = library::deps(&lib.db, &a).iter().map(|d| format!("{} {}", library::kind_label(d.0), d.1)).collect();
        text(format!("library {} \"{}\" needs [{}]; in the project: {}\n{}", library::kind_label(rk), p.id, deps.join(", "), library::exists(&self.project.db, &a), json_str(&v)))
    }

    pub(crate) fn mcp_library_add(&mut self, p: LibraryAddParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        self.lib.ensure_loaded();
        let Some(lib) = &self.lib.lib else { return Err(self.lib.error.clone().unwrap_or_else(|| "the asset library is not available".into())) };
        let mut assets = Vec::new();
        for a in &p.assets {
            let rk = parse_ref_kind(&a.kind)?;
            let r: AssetRef = (rk, a.id.clone());
            if !library::exists(&lib.db, &r) {
                return Err(format!("the library has no {} \"{}\" (library_list)", library::kind_label(rk), a.id));
            }
            assets.push(r);
        }
        if assets.is_empty() {
            return Err("give at least one asset".into());
        }
        let msg = self.library_to_project(&assets).map_err(|e| format!("{e:#}"))?;
        self.mcp_after_write(msg.replace(" Save the definitions to keep them.", ""))
    }

    pub(crate) fn mcp_library_send(&mut self, p: LibrarySendParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let mut assets = Vec::new();
        for a in &p.assets {
            let rk = parse_ref_kind(&a.kind)?;
            let r: AssetRef = (rk, a.id.clone());
            if !library::exists(&self.project.db, &r) {
                return Err(format!("the project has no {} \"{}\".{}", library::kind_label(rk), a.id, suggestions(&a.id, self.mcp_ids_of(rk).iter().map(String::as_str))));
            }
            assets.push(r);
        }
        if assets.is_empty() {
            return Err("give at least one asset".into());
        }
        // Unsaved definitions would be sent as they are in memory, files from disk: save first.
        self.save_all();
        let msg = self.project_to_library(&assets, p.replace.unwrap_or(false), p.drop_script_triggers.unwrap_or(false)).map_err(|e| format!("{e:#}"))?;
        text(msg)
    }

    pub(crate) fn mcp_write_asset(&mut self, p: WriteAssetParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let rel = p.path.trim().replace('\\', "/");
        let lower = rel.to_lowercase();
        let image = lower.ends_with(".png");
        let folder_ok = ["sprites/", "tilesets/", "sounds/"].iter().any(|f| rel.starts_with(f));
        if !folder_ok || rel.split('/').any(|c| c.is_empty() || c == "." || c == "..") || !(image || lower.ends_with(".wav") || lower.ends_with(".ogg")) {
            return Err(format!("\"{rel}\": give a path under sprites/, tilesets/ or sounds/ ending in .png, .wav or .ogg"));
        }
        let bytes = match (&p.base64, &p.source) {
            (Some(b), None) => rustpg2d_engine::hash::base64_decode(b).ok_or("base64 is not valid Base64")?,
            (None, Some(src)) => std::fs::read(src).map_err(|e| format!("reading {src}: {e}"))?,
            _ => return Err("give either base64 or source".into()),
        };
        let what = if image {
            let img = rustpg2d_engine::imageio::Rgba::decode(&bytes).map_err(|e| format!("not a PNG image: {e:#}"))?;
            format!("{}x{} px image", img.w, img.h)
        } else {
            let pcm = rustpg2d_engine::audio::decode(&bytes).map_err(|e| format!("not a WAV / Ogg Vorbis sound: {e}"))?;
            format!("sound, {:.1} s", pcm.samples.len() as f32 / (pcm.rate.max(1) as f32 * pcm.channels.max(1) as f32))
        };
        let path = self.project.root.join(&rel);
        let existed = path.exists();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::write(&path, &bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;
        self.reload_assets();
        let db = &self.project.db;
        let users: Vec<String> = db
            .tilesets
            .iter()
            .filter(|d| d.image == rel)
            .map(|d| format!("tileset {}", d.id))
            .chain(db.sheets.iter().filter(|d| d.image == rel).map(|d| format!("sheet {}", d.id)))
            .chain(db.sounds.iter().filter(|d| d.file == rel).map(|d| format!("sound {}", d.id)))
            .collect();
        text(format!(
            "{} {rel} ({what}). {}",
            if existed { "Replaced" } else { "Wrote" },
            if users.is_empty() { "No definition uses it yet: add one with upsert_def (tilesets / sheets / sounds).".to_string() } else { format!("Used by {}; the art was reloaded.", users.join(", ")) }
        ))
    }
}
