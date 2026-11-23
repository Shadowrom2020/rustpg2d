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
use rustpg2d_engine::skills::{SpellDef, SpellEffect};
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

def_facts!(rustpg2d_engine::ground::MaterialDef, |d| format!("texture {}; {:?} edge, priority {}; {:?}{}", d.texture, d.edge, d.priority, d.walk, if d.step.is_empty() { String::new() } else { format!("; steps {}", d.step) }));
def_facts!(rustpg2d_engine::decor::ScatterDef, |d| format!("{} props [{}]; density {} per tile, spacing {} px; {:?}", d.props.len(), d.props.iter().map(|(p, w)| format!("{p} x{w}")).collect::<Vec<_>>().join(", "), d.density, d.spacing, d.layer));
def_facts!(rustpg2d_engine::fonts::FontDef, |d| {
    let pixel = if d.pixel > 0 { format!("pixel font, {} px grid", d.pixel) } else { "smooth font".to_string() };
    format!("file {}; {pixel}; scale {}{}", d.file, d.scale, if d.license.is_empty() { String::new() } else { format!("; licence {}", d.license) })
});
def_facts!(rustpg2d_engine::skin::SkinDef, |d| {
    let pics: Vec<String> = d.pictures().iter().map(|(part, p)| format!("{part} {}:{}", p.sheet, p.frame)).collect();
    let colors = d.text.normal.iter().chain(&d.text.highlight).count() + d.colors.len();
    format!("{}; {} colours set", if pics.is_empty() { "no pictures".to_string() } else { pics.join(", ") }, colors)
});
def_facts!(rustpg2d_engine::classes::ClassDef, |d| format!("\"{}\"; d{} hit die; saves [{}]; {} skills of [{}]; subclass at {} ({}); {} features", d.name, d.hit_die, d.saves.join(", "), d.skill_choices, d.skill_options.join(", "), d.subclass_level, d.subclasses.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(", "), d.features.len()));
def_facts!(rustpg2d_engine::diseases::DiseaseDef, |d| format!("\"{}\"; {} DC {}; onset {}{}; {}", d.name, d.save.short(), d.dc, if d.onset.is_empty() { "at once" } else { &d.onset }, if d.onset_days { " days" } else if d.onset.is_empty() { "" } else { " hours" }, d.course.label()));
def_facts!(rustpg2d_engine::backgrounds::BackgroundDef, |d| format!("\"{}\"; proficiencies [{}]; {} language(s) to choose; {} starting item(s); feature {}", d.name, d.proficiencies.join(", "), d.language_choices, d.equipment.len(), if d.feature.is_empty() { "-" } else { d.feature.as_str() }));
def_facts!(rustpg2d_engine::common_events::CommonEventDef, |d| {
    let runs = match &d.trigger {
        rustpg2d_engine::common_events::CommonTrigger::Call => "when called".to_string(),
        rustpg2d_engine::common_events::CommonTrigger::Parallel { flag } => format!("parallel while flag {flag}"),
        rustpg2d_engine::common_events::CommonTrigger::Autorun { flag } => format!("autorun while flag {flag}"),
    };
    format!("{} actions; runs {runs}", d.actions.len())
});
def_facts!(rustpg2d_engine::cutscenes::CutsceneDef, |d| {
    let cues: usize = d.tracks.iter().map(|t| t.cues.len()).sum();
    format!("\"{}\"; {} track(s), {cues} cue(s), {:.1} s{}", d.name, d.tracks.len(), d.length(), if d.map.is_empty() { String::new() } else { format!("; map {}", d.map) })
});
def_facts!(SheetDef, |d| {
    let scale = if d.scale > 1 { format!(" at {}x", d.scale) } else { String::new() };
    let tags = if d.tags.is_empty() { String::new() } else { format!("; animations [{}]", d.tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")) };
    format!("image {}; {}x{} px frames{scale}; {:?}{tags}", d.image, d.frame_w, d.frame_h, d.kind)
});
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
    "\"{}\"; hp {}; {:?}{}{}{}{}{}",
    d.name,
    d.max_hp,
    d.behavior,
    d.stats.as_ref().map_or(String::new(), |s| format!("; stat block {} (AC {}, HP {}, CR {})", if s.template.is_empty() { "by hand" } else { &s.template }, s.ac, s.hp, s.cr)),
    if d.hostile { "; hostile" } else { "" },
    if d.shop.is_some() { "; shop" } else { "" },
    if d.triggers.is_empty() { String::new() } else { format!("; {} triggers", d.triggers.len()) },
    if d.description.is_empty() { String::new() } else { format!("; {}", d.description) }
));
def_facts!(SoundDef, |d| format!("file {}; volume {}", d.file, d.volume));
def_facts!(QuestDef, |d| format!("\"{}\"; {} stages [{}]; xp {}", d.title, d.stages.len(), d.stages.join(" | "), d.xp));
def_facts!(StructureDef, |d| format!("\"{}\"; {}x{} tiles; category {}; {} entities", d.name, d.w, d.h, d.category, d.entities.len()));
def_facts!(SpellDef, |d| format!("\"{}\"; mana {}; cooldown {}; needs proficiency {}; {:?}", d.name, d.mana, d.cooldown, if d.skill.is_empty() { "-" } else { d.skill.as_str() }, d.effect));
def_facts!(RecipeDef, |d| format!("makes {} x{} from [{}]{}{}", d.result, d.count, d.ingredients.iter().map(|i| format!("{} x{}", i.item, i.count)).collect::<Vec<_>>().join(", "), if d.station.is_empty() { String::new() } else { format!(" at {}", d.station) }, if d.skill.is_empty() { String::new() } else { format!("; {}{}", d.skill, if d.level > 0 { " (proficiency needed)" } else { "" }) }));
def_facts!(AchievementDef, |d| format!("\"{}\"; {}{} conditions", d.name, if d.hidden { "hidden; " } else { "" }, d.conditions.len()));
def_facts!(rustpg2d_engine::defs::ModelDef, |d| format!(
    "\"{}\" {}{}; height {} px; clips [{}]; {} colour features{}",
    d.label(),
    d.file,
    if d.clip_files.is_empty() { String::new() } else { format!(" + {} clip file(s)", d.clip_files.len()) },
    d.height,
    d.clips.iter().map(|c| format!("{}={}", c.anim, c.clip)).collect::<Vec<_>>().join(", "),
    d.features.len(),
    if d.playable { "; playable" } else { "" }
));
def_facts!(rustpg2d_engine::races::RaceDef, |d| format!(
    "\"{}\"{}; bodies [{}] / [{}]; hp {:+}, speed {:+}%, defense {:+}; proficiencies [{}]{}",
    d.name,
    if d.playable { "; playable" } else { "" },
    d.bodies_male.join(", "),
    d.bodies_female.join(", "),
    d.max_hp,
    d.speed,
    d.defense,
    d.skills.iter().map(|s| s.skill.clone()).collect::<Vec<_>>().join(", "),
    if d.size != 1.0 { format!("; size {}", d.size) } else { String::new() }
));

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
    let value = with_stat_template(value);
    let parsed: T = serde_json::from_value(value).map_err(|e| format!("the definition isn't valid: {e}. def_template shows the shape"))?;
    match pos {
        Some(i) => list[i] = parsed,
        None => list.push(parsed),
    }
    Ok((id, pos.is_some()))
}

/// A character's `stats: {template: "guard", hp: 20}`: the SRD template's block with the given
/// fields over it (so an agent needn't copy the whole stat block).
fn with_stat_template(mut value: Value) -> Value {
    let Some(stats) = value.get_mut("stats").filter(|s| s.is_object()) else { return value };
    let Some(t) = stats.get("template").and_then(|t| t.as_str()).and_then(rustpg2d_engine::srd::npc_template) else { return value };
    let mut base = serde_json::to_value(t).unwrap_or_default();
    super::patch::merge(&mut base, stats);
    *stats = base;
    value
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
        RefKind::Sheet => (
            val(&SheetDef::new(id, "sprites/<file>.png", 32, 32)),
            "kind: Sprite (props, effects) | Emotions (icons over heads; its tags are the emotions); frame_w/frame_h in image px; scale: image px per world px (1 default; 2 = high-resolution art); tags: named frame ranges [{name, first, last, fps}].",
        ),
        RefKind::Item => (
            val(&ItemDef::fresh(id)),
            "icon: {sheet, frame} of a sprite sheet; weapon: {damage, damage_type (Slashing, Piercing, Bludgeoning, Fire… the SRD's 13; null = untyped), cooldown, ranged: null | {projectile, speed, range, ammo}, reach, arc, knockback, two_handed, effect: a condition on hit {condition: Poisoned|Prone|… or null, seconds (0 = until removed), level (exhaustion), save: {ability: \"con\", dc: 12} or null, ongoing: {dice: \"1d4\", damage_type: Poison (null = healing)} or null}}; armor: {slot: Clothing|Head|Body|Offhand|Back, defense, speed, resistances: [\"fire\"]} (Back = a backpack: bag_slots = the extra inventory slots while the player wears it; its model with hold Back sits on the upper back) (worn items count in play; figures' art shows what they wear); held: how figures with hand marks hold it {sprite (none: the icon), grip: [x, y] hold point as fractions of the picture, points: degrees, the picture's direction that lies on the hand's grip axis (icons -45), scale (0.5), hand: Right|Left|Both (none: by its kind)}; consumable + on_use triggers (ItemUse owner: UseItem event) = potions; food; teaches: a skill or spell id; tool: Pickaxe|Axe|Hoe|Rod… name used by props' gather; value in coins; crop: {stages: [{sprite, hours}], harvest: loot}; anims: {attack, idle, walk, block} = the animations 3D characters play holding it (weapons, shields), names models map (attack_1h|attack_2h|attack_thrust|attack_bow|attack_crossbow|attack_cast|attack_unarmed, idle_1h|idle_2h|idle_bow|idle_spear, block_shield|block_weapon|block_2h); empty = estimated from the weapon, a model without the clip plays its plain one.",
        ),
        RefKind::Prop => (
            val(&PropDef::fresh(id)),
            "sprite: {sheet, frame} (or {sheet, frame, w, h} for a block of frames); solid; low: true = while solid it stops walking but not shots and sight (barrels, chairs, fences; solid props that aren't low stop shots: trees, statues, stalls); hitbox [w, h] px at the feet; states: [{name, sprite, solid, light}] switched by SetState / set_state (default_state names the first); triggers (Prop owner: Interact, Enter, UseItem, Custom, Unlock, Lock…); container: {size, items: [{item, count}], conditions, locked_text, open_frame, open_state}; lock: {key, consume_key, on_unlock: Nothing|Disappear|{Open:{frame}}|{ToState:{state}}, locked_text, unlock_text, relock, pick_difficulty}; use_spot: furniture {pose: Sit|Lie|Stand, offset, facing, sleep_until}; gather: {tool, hits, loot, then_state}; regrow: {from, to, hours}; plot: a farm plot; carry: how characters carry it {grip, points, scale} (none: middle of its lower half, horizontal along the hands, 0.6).",
        ),
        RefKind::Character => (val(&crate::builder::default_character(db, id, "New character")), "appearance: {model (a 3D model id; none: the mannequin), colors: {feature: [r,g,b]} (the model's colour features; missing = as made)}; equipment / inventory: the items it wears and carries (count in play; weapon, left hand and tools show in its hands: items' `model` or `held` picture); behavior: Static | {Wander:{radius}} | {Chase:{..}}; hostile + weapon/damage/defense/xp/loot for enemies; touch_effect: a condition its melee hits give (the shape of a weapon's effect: {condition, seconds, save, ongoing}); shop: {title, currency, items: [{item, count, unlimited, price}], sell_rate, open_on_talk}; triggers (Character owner: Interact, Defeat, HpBelow, Custom…) hold its dialogs; portrait / moods: {sheet, frame} for dialog portraits; light: carried light; stats: its SRD stat block (abilities [str,dex,con,int,wis,cha], ac, hp, hit_dice \"2d8+2\", speed_ft, darkvision_ft, proficiencies [perception, save:dex], languages, cr \"1/8\", xp; template = one of the SRD NPCs: acolyte, bandit, bandit-captain, commoner, guard, knight, mage, noble, orc, goblin, priest, scout, thug, veteran…: {template: \"guard\"} fills in the template's values, fields given beside it change them; attacks: [{name, bonus, dice \"1d6+2\", damage_type, reach_ft, range_ft, long_ft}], multiattack: attacks per Attack action; null = none). Turn-based fights use the stat block (its hp replaces max_hp when max_hp > 0); without attacks a character attacks with its weapon or damage (1 = 1d4 … 5 = 1d12)."),
        RefKind::Sound => (val(&SoundDef { id: id.into(), file: "sounds/<file>.ogg".into(), volume: 1.0, caption: String::new() }), "file: WAV or Ogg Vorbis under the project; volume 0..1; caption: what it sounds like for players with Captions on (\"Door creaks\"; music: the track's name; translated; empty = none). Used by PlaySound / PlayMusic, map music, battle music and the project's built-in sounds."),
        RefKind::Quest => (
            val(&QuestDef {
                id: id.into(),
                title: "New quest".into(),
                description: String::new(),
                stages: vec!["Find out more".into(), "Return to the giver".into()],
                giver: String::new(),
                offer: rustpg2d_engine::quest::QuestOffer { lines: vec!["I need your help.".into()], question: "Will you help?".into(), accept: "Yes".into(), decline: "Not now".into(), accepted: vec!["Thank you!".into()], declined: vec![] },
                steps: vec![
                    rustpg2d_engine::quest::QuestStep { done: rustpg2d_engine::quest::StepGoal::Manual, ..Default::default() },
                    rustpg2d_engine::quest::QuestStep { done: rustpg2d_engine::quest::StepGoal::Talk { character: String::new(), lines: vec!["Well done!".into()], when: vec![], speaker: String::new() }, ..Default::default() },
                ],
                rewards: rustpg2d_engine::quest::QuestRewards { items: vec![rustpg2d_engine::quest::RewardItem { item: "coin".into(), count: 10 }], ..Default::default() },
                ..Default::default()
            }),
            "stages: the journal lines in order (stage 0 is the first after Start); xp on completion; targets (optional, one per stage): {map, name} = where the stage leads (a placed character, prop or zone by name), shown as a quest marker. Triggers drive it: Action Quest{quest, op: Start|Advance|SetStage|Complete|Fail, stage}, Cond Quest{quest, check: NotStarted|Active|Completed|Failed|AtStage|StageAtLeast, stage}. The quest designer: giver = who offers it and takes it back: a placed character's name (just that one) or a character definition (every character placed from it); giver_name: its name in the dialogs (\"\" = its character's name, e.g. \"Tilda\" for a merchant_f) (\"\" = started by triggers / scripts; then offer, requires and Talk steps to the giver do nothing); requires: conditions (as in triggers) for the offer, e.g. [{cond:{Quest:{quest:\"other\", check:\"Completed\", stage:0}}, negate:false}] for a follow-up; offer: {lines, question (\"\" = starts after the lines), accept, decline, accepted, declined}; steps (one per stage, by index): {done: \"Manual\" | {Conditions:[conditions]} | {Talk:{character (a placed name or definition; \"\" = the giver), lines, when: [conditions, e.g. HasItem for a hand-in], speaker (its name in the dialog; \"\" = its character's / the giver's)}} | \"Reach\" (enter the stage's target zone / interact with its target), giver_line (said by the giver while the step is current), first_to: {map, name} (a Talk step whose `when` doesn't hold yet leads there instead of to its character: the cart with the flour, the oven), on_start / on_done: actions (as in triggers, e.g. {RunScript:{function}})}; rewards: {items: [{item, count}], skill_points, actions} given on completion (xp too). objects: [{map, name}] = placed items / props / characters that belong to the quest: hidden until it is started, then shown once while it is active.",
        ),
        RefKind::Structure => (val(&StructureDef { id: id.into(), name: "New structure".into(), w: 2, h: 2, ..Default::default() }), "w, h: size in tiles; materials: material ids, ground: (w*4) x (h*4) cells row by row (0 = keep the map's, 255 = no ground, else a materials slot + 1); decor_defs: prop ids, decor: decorations {def (slot), x, y (feet, pixels from the top-left), flip, state, layer Flat|Sorted|Above}; clear: w*h bytes (1 = placing removes the map's decorations there); collision: w*h bytes (0 keep, 1 solid, 2 walkable, 3 automatic); entities with positions relative to the top-left. Easier: build it on a map and capture_structure."),
        RefKind::Spell => (val(&SpellDef::new(id, "New spell", "", icon.clone(), 5, 1.0, SpellEffect::Heal { amount: 5 })), "effect: {Heal:{amount}} | {Bolt:{damage, projectile (sheet), speed, range}} | {Blast:{..}} | {Light:{..}} | {Script:{function}}; mana cost; cooldown s; skill + level required to learn (empty skill = none). Items with teaches = this id teach it."),
        RefKind::Recipe => (val(&RecipeDef::new(id, "<item id>", 1, vec![], "")), "result + count from ingredients [{item, count}]; station: a prop id the player must use (empty = anywhere, craftable from the inventory), station_state: the state it must be in (a lit fire); skill + level required; tools: item ids that must be in the inventory; quality: [{level, result}] better results at higher skill."),
        RefKind::Achievement => (val(&AchievementDef { id: id.into(), name: "New achievement".into(), description: String::new(), icon, hidden: false, conditions: vec![] }), "conditions: checked while playing (same shapes as trigger conditions: Quest, Flag, Var, Defeated, Level, HasItem, …); hidden: shown as ??? until unlocked; also unlocked by the UnlockAchievement action / unlock_achievement()."),
        RefKind::Race => (val(&rustpg2d_engine::races::RaceDef::fresh(id)), "bodies_male / bodies_female: model ids (the creator offers them with a colour row per colour feature, and they get the race's size); max_hp / defense: added for a player of this race, speed: percent; skills: [{skill, level: 1}] = proficiencies (SRD skills like perception, tools like smiths-tools, saves like save:con) a hero of the race has; size: 1 = as drawn (trolls 1.5: bigger sprite and hitbox); playable: offered in the creator. Characters name their race in `race`. choices: [{id, label, kind: Ability|Skill|Tool|Language|Ancestry|Cantrip, choose, options, grants, names, colors: {option: {feature: [r,g,b]}}}] (colors: picking the option colours the models' features; the creator offers such a choice on page 1 and no rows for those features)."),
        RefKind::Model => (
            val(&rustpg2d_engine::defs::ModelDef::new(id, "models/<file>.glb")),
            "file: a rigged .glb under models/ (Meshy, Blender…); clip_files: more .glb files with clips of the same rig; shared_clips: clip files of another model with the same bone names, fitted to this one's bones (share_model_clips sets them); height: standing height in world px (40 = a person); clips: [{anim: walk|idle|attack|hurt|sit|sleep|fish|farm|craft|chop|mine|sweep|read|sneak or a weapon animation (attack_1h, attack_2h, attack_thrust, attack_bow, attack_crossbow, attack_cast, attack_unarmed, idle_1h, idle_2h, idle_bow, idle_spear, block_shield, block_weapon, block_2h), clip: the clip's name in the files, fps (12), hit: 0..1 where an attack's blow lands / a block's guard is up (0.42)}]; features: colour features of its texture (as a figure sheet's); playable / female: offered in the character creator. Characters use it with appearance.model.",
        ),
        RefKind::Material => (
            val(&rustpg2d_engine::ground::MaterialDef::fresh(id)),
            "texture: a seamless PNG under the project (up to 256 x 256 px, sizes in steps of 8), repeated over the painted ground; edge: Crisp (hard pixel-art edge, ragged by edge_noise px, steps of edge_pixel px) | Soft (a fade); priority 0..255: the higher material is drawn over the lower where they meet; rim: null | {color: [r,g,b], width: px} a fringe outside its edge; walk: Walk | Solid (blocks walking, shots, sight) | Low (blocks walking only: water); step: footstep sound id; scroll: [x, y] px per second (flowing water).",
        ),
        RefKind::Scatter => (
            val(&rustpg2d_engine::decor::ScatterDef::fresh(id)),
            "props: [[prop id, weight], …] placed at random by the scatter brush / the scatter agent tool (weights: how often each comes); density: decorations per tile (0.05 a sparse wood, 0.5 flowers); spacing: least distance between two in px; flip: mirror half of them; layer: Flat | Sorted | Above.",
        ),
        RefKind::Font => (
            val(&rustpg2d_engine::fonts::FontDef { file: "fonts/<file>.ttf".into(), ..rustpg2d_engine::fonts::FontDef::fresh(id) }),
            "file: a TTF / OTF under the project (write_asset it into fonts/); pixel: a pixel font's grid size (texts are drawn at whole multiples of it), 0 = a smooth font; scale: size against the built-in font (1 = as big); offset_y: baseline shift as a part of the size (+ = down); license: its licence text file (exported with it); credit. The project's `fonts` (in project.ron: ui, dialog, hud, title = font ids, fallback = font ids tried per character) chooses what the game draws with; empty = the built-in font (Latin-1 only).",
        ),
        RefKind::Skin => (
            val(&rustpg2d_engine::skin::SkinDef { frame: Some(rustpg2d_engine::skin::NineSlice::new("<sheet>")), ..rustpg2d_engine::skin::SkinDef::fresh(id) }),
            "A UI skin; every part is optional (left out = the built-in look). Pictures are nine-slices {sheet, frame, corner (corner size in picture px, 4), scale (screen px per picture px, 2), tile (repeat edges / middle instead of stretching)} of a sprite sheet: frame (every window and menu panel), dialog (dialog / choice box; none = frame), name_box (a box for the speaker's name on the dialog box's top edge), hud (the HUD panel; none = frame), button, button_hover (also the open tab), cursor (the selection marker of menus, lists, slots), toast, hp.frame (behind the hearts), mana.frame, xp.frame (around those bars). Colours are \"#rrggbb\" or \"#rrggbbaa\": background, border (panels without a picture), button_fill, button_hover_fill, cursor_fill, toast_fill, toast_text, text {normal, dim, highlight, disabled, title, positive, negative}, hp/mana/xp {fill, back} (hp = the hearts), colors: [[name, colour], …] = colour names for dialog texts ({c:name}). The project's `skin` (set_project_config) picks the one the game uses; empty = built in. High contrast (Options) always shows the built-in look.",
        ),
        RefKind::CommonEvent => (
            val(&rustpg2d_engine::common_events::CommonEventDef { actions: vec![rustpg2d_engine::events::Action::Wait { seconds: 1.0 }], ..rustpg2d_engine::common_events::CommonEventDef::fresh(id) }),
            "A common event: name; trigger: Call (only when an action CallCommon {id} or call_common(\"id\") runs it) | {Parallel: {flag}} (its actions again and again while the flag is set, beside play, in a queue of its own; clearing the flag stops it at once) | {Autorun: {flag}} (queued into the main event queue whenever it is idle while the flag is set: the player waits; clear the flag in its actions to end it); actions: the same actions as triggers (see the actions resource).",
        ),
        RefKind::Class => (
            val(&rustpg2d_engine::classes::ClassDef::fresh(id)),
            "A class (SRD 5.1; the twelve come from the starter library): hit_die (6-12), saves [str, con], proficiencies (armor / weapon / tool ids), skill_choices + skill_options, equipment [{item, count}], subclass_level + subclass_title, levels (20: {features: [ids], numbers: {rage_count: 2}}), subclasses [{id, name, levels: [[3, [feature ids]]]}], features [{id, name, level, text, effects: [FeatureEffect…], choice}]. Library_add the SRD's rather than writing one.",
        ),
        RefKind::Disease => (
            val(&rustpg2d_engine::diseases::DiseaseDef { exhaustion: 1, course: rustpg2d_engine::diseases::Course::Exhaustion, ..rustpg2d_engine::diseases::DiseaseDef::fresh(id) }),
            "A disease (SRD 5.1; Cackle Fever, Sewer Plague, Sight Rot and a diseased rat's come from the starter library): name, description; save (Con) + dc: the save against catching it (0 = none); onset: dice of hours (onset_days: of days) until the symptoms show (\"\" = at once); exhaustion: levels at the onset, keeps_exhaustion: rests don't lower it; no_natural_healing (only magic heals), no_rest_healing, half_hit_dice; max_hp_loss: dice every 24 hours (at 0 the hero dies); sight: failing sight (-1 per long rest to attacks and sight checks, blinded at -5); condition: one while sick; when_hurt: {damage, damage_type, condition, seconds} a fit when hurt unless a save at the current DC succeeds; course after each long rest: None | {ShrinkingDc: {dice}} | Exhaustion | Worsens. Monsters give it with a rider's disease (stat block attacks' on_hit), events with ExposeDisease, scripts with infect(id).",
        ),
        RefKind::Background => (
            val(&rustpg2d_engine::backgrounds::BackgroundDef { proficiencies: vec!["insight".into(), "religion".into()], language_choices: 1, ..rustpg2d_engine::backgrounds::BackgroundDef::fresh(id) }),
            "A background the character creator offers (SRD 5.1): name, description; proficiencies: skill / tool ids (insight, thieves-tools); language_choices: how many languages the hero picks; equipment: [{item, count}] starting items; feature + feature_text: its rule in words.",
        ),
        RefKind::Cutscene => (
            val(&rustpg2d_engine::cutscenes::CutsceneDef {
                tracks: vec![rustpg2d_engine::cutscenes::CutsceneTrack {
                    kind: rustpg2d_engine::cutscenes::TrackKind::Actor,
                    target: "player".into(),
                    cues: vec![rustpg2d_engine::cutscenes::Cue { at: 0.5, wait: true, action: rustpg2d_engine::events::Action::WalkTo { target: "player".into(), x: 5, y: 5 } }],
                }],
                ..rustpg2d_engine::cutscenes::CutsceneDef::fresh(id)
            }),
            "A cut-scene as a timeline: name; map (the map it is made for: the editor previews it there); tracks [{kind: Actor | Camera | Dialog | Sound | Screen, target (an Actor track's character: \"player\" or a name), cues: [{at: seconds from the start, wait: true = the scene waits until it is done (a dialog closed, a walk arrived; the cues after it come that much later) / false = it runs alongside, action: any action (WalkTo, Face, Emote, Camera, Dialog, PlaySound, ShowPicture, …)}]}]. PlayCutscene {function: id} (or cutscene(\"id\") in scripts) plays it; a script function of that name otherwise.",
        ),
        RefKind::Map => (Value::Null, "maps are not definitions"),
        RefKind::Skill => (Value::Null, "skills are the SRD's (perception, stealth, ...): no definitions since 0.28"),
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

    /// `edit_frames`: frame operations on a sheet's image (see `frame_ops`).
    pub(crate) fn mcp_edit_frames(&mut self, p: EditFramesParams) -> Result<Reply, String> {
        use crate::frame_ops::{anim_index, Frames};
        self.mcp_need_project()?;
        let d = self.project.db.sheet(&p.sheet).ok_or_else(|| format!("\"{}\" is not a sprite sheet.{}", p.sheet, suggestions(&p.sheet, self.project.db.sheets.iter().map(|t| t.id.as_str()))))?.clone();
        let path = self.project.root.join(&d.image);
        let mut img = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| rustpg2d_engine::imageio::Rgba::decode(&b).map_err(|e| format!("{e:#}")))?;
        let mut f = Frames { img: &mut img, frame: (d.frame_w.max(1), d.frame_h.max(1)) };
        let (cols, rows) = (f.cols(), f.rows());
        let need = |v: Option<u32>, what: &str, max: u32| v.filter(|v| *v < max).ok_or(format!("{what} (below {max}) is needed for {}", p.op));
        let msg = match p.op.as_str() {
            "mirror_rows" => {
                let anim = match &p.anim {
                    Some(a) => Some(anim_index(a).ok_or("anim: walk, idle, attack or hurt")?),
                    None => None,
                };
                format!("Right rows {:?} are now the mirrored Left rows", f.mirror_left_to_right(anim))
            }
            "copy_row" => {
                let (from, to) = (need(p.row, "row", rows)?, need(p.to_row, "to_row", rows)?);
                let frames = f.row(from);
                f.put_row(to, &frames);
                format!("Row {from} copied to row {to}")
            }
            "clear_row" => {
                let r = need(p.row, "row", rows)?;
                f.clear_row(r);
                format!("Row {r} cleared")
            }
            "duplicate" => {
                let k = need(p.frame, "frame", cols * rows)?;
                if !f.duplicate(k) {
                    return Err(format!("frame {k} is in the last column"));
                }
                format!("Frame {k} copied into frame {}", k + 1)
            }
            "nudge" => {
                let k = need(p.frame, "frame", cols * rows)?;
                f.nudge(k, p.dx.unwrap_or(0), p.dy.unwrap_or(0));
                format!("Frame {k} moved by {},{}", p.dx.unwrap_or(0), p.dy.unwrap_or(0))
            }
            "replace_color" => {
                let (from, to) = (p.from.ok_or("from: [r, g, b] is needed")?, p.to.ok_or("to: [r, g, b] is needed")?);
                let frames: Option<Vec<u32>> = match p.scope.as_deref().unwrap_or("sheet") {
                    "frame" => Some(vec![need(p.frame, "frame", cols * rows)?]),
                    "row" => {
                        let r = need(p.row, "row", rows)?;
                        Some((r * cols..(r + 1) * cols).collect())
                    }
                    _ => None,
                };
                format!("Replaced {} pixel(s)", f.replace_color(from, [to[0], to[1], to[2], 255], frames.as_deref()))
            }
            "fill_standing" => format!("{} empty idle / hurt frame(s) got the standing frame", f.fill_standing()),
            other => return Err(format!("unknown op \"{other}\": mirror_rows, copy_row, clear_row, duplicate, nudge, replace_color or fill_standing")),
        };
        std::fs::write(&path, img.encode().map_err(|e| format!("{e:#}"))?).map_err(|e| e.to_string())?;
        self.reload_assets();
        text(format!("{msg} in {} ({}); the art was reloaded.", d.id, d.image))
    }

    /// `sheet_colors`: a sheet's exact colours with counts, rows and features.
    pub(crate) fn mcp_import_model(&mut self, p: ImportModelParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let path = std::path::PathBuf::from(&p.path);
        if !path.is_file() {
            return Err(format!("no file {}", p.path));
        }
        if let Some(id) = &p.clips_for {
            if self.project.db.model(id).is_none() {
                return Err(format!("no model \"{id}\"{}", suggestions(id, self.project.db.models.iter().map(|m| m.id.as_str()))));
            }
        }
        let rel = crate::files::import_asset(&self.project.root, &path, "models").map_err(|e| format!("{e:#}"))?;
        let msg = self.use_model_file(p.clips_for.as_deref().unwrap_or(""), p.clips_for.is_some(), &rel);
        if msg.contains("can't be used") {
            return Err(msg);
        }
        text(msg)
    }

    /// `model_sheet`: the model as the game draws it.
    pub(crate) fn mcp_model_sheet(&mut self, p: ModelSheetParams) -> Result<Reply, String> {
        use rustpg2d_engine::entity::Facing;
        self.mcp_need_project()?;
        let db = &self.project.db;
        let d = db.model(&p.model).ok_or_else(|| format!("no model \"{}\"{}", p.model, suggestions(&p.model, db.models.iter().map(|m| m.id.as_str()))))?;
        let mut look = rustpg2d_engine::defs::Appearance::default();
        // A piece: on the first figure it fits, in its own slot.
        match &d.part {
            Some(part) => {
                look.model = part.fits.iter().find(|b| db.model(b).is_some()).cloned().ok_or("the piece lists no figure of this project under the figures it fits: nothing to show it on")?;
                look.parts.insert(part.slot.clone(), d.id.clone());
            }
            None => look.model = d.id.clone(),
        }
        look.parts.extend(p.parts.clone().unwrap_or_default());
        for (name, hex) in p.colors.clone().unwrap_or_default() {
            let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).map_err(|_| format!("colour \"{hex}\": RRGGBB expected"))?;
            look.colors.insert(name, [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        }
        for item in p.worn.iter().flatten().chain(&p.weapon).chain(&p.offhand).chain(&p.back) {
            if db.item(item).is_none() {
                return Err(format!("no item \"{item}\"{}", suggestions(item, db.items.iter().map(|i| i.id.as_str()))));
            }
        }
        let gear = rustpg2d_engine::render::model::Gear { weapon: p.weapon.clone(), offhand: p.offhand.clone(), back: p.back.clone(), worn: p.worn.clone().unwrap_or_default(), ready: true, ..Default::default() };
        let anims = p.anims.clone().unwrap_or_else(|| vec!["idle".into(), "walk".into(), "attack".into()]);
        let facings: Vec<Facing> = match &p.facings {
            None => vec![Facing::Down, Facing::Left, Facing::Right, Facing::Up],
            Some(f) => f
                .iter()
                .map(|f| match f.as_str() {
                    "down" => Ok(Facing::Down),
                    "left" => Ok(Facing::Left),
                    "right" => Ok(Facing::Right),
                    "up" => Ok(Facing::Up),
                    other => Err(format!("facing \"{other}\": down, left, right or up")),
                })
                .collect::<Result<_, _>>()?,
        };
        let moments = p.moments.unwrap_or(4).clamp(1, 8) as usize;
        let zoom = p.zoom.unwrap_or(2).clamp(1, 4);
        let ground = macroquad::color::Color::from_rgba(74, 112, 62, 255);
        let (img, cell) = self.assets.models.sheet(db, &self.assets.sheets, &look, &gear, &anims, &facings, moments, ground).ok_or_else(|| {
            let err = self.assets.models.errors.iter().find(|e| e.contains(&format!("model {}:", look.model))).cloned();
            err.unwrap_or_else(|| format!("{} isn't loaded, or the sheet would be larger than 8192 pixels (fewer animations or moments)", look.model))
        })?;
        let (w, h) = (img.width() as u32, img.height() as u32);
        let mut out = rustpg2d_engine::imageio::Rgba::new(w * zoom, h * zoom);
        for y in 0..h * zoom {
            for x in 0..w * zoom {
                let i = ((y / zoom * w + x / zoom) * 4) as usize;
                let o = ((y * w * zoom + x) * 4) as usize;
                out.px[o..o + 4].copy_from_slice(&img.bytes[i..i + 4]);
            }
        }
        let png = out.encode().map_err(|e| format!("{e:#}"))?;
        let parts = rustpg2d_engine::render::model::parts_of(db, &look.model, &look, &gear);
        let note = format!(
            "{} as the game draws it, enlarged {zoom}x: rows {} (top to bottom), columns {} with {moments} moments each; cells of {}x{} px. Pieces shown: {}.",
            look.model,
            anims.join(", "),
            facings.iter().map(|f| format!("{f:?}").to_lowercase()).collect::<Vec<_>>().join(", "),
            cell.0 * zoom,
            cell.1 * zoom,
            if parts.is_empty() { "none".to_string() } else { parts.join(", ") }
        );
        Ok(Reply::TextImages(note, vec![png]))
    }

    /// `model_check`: what is wrong with a model, and its measures.
    pub(crate) fn mcp_model_check(&mut self, p: ModelCheckParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let db = &self.project.db;
        let d = db.model(&p.model).ok_or_else(|| format!("no model \"{}\"{}", p.model, suggestions(&p.model, db.models.iter().map(|m| m.id.as_str()))))?;
        let Some(lines) = self.assets.models.check(db, &d.id) else {
            let err = self.assets.models.errors.iter().find(|e| e.contains(&format!("model {}:", d.id))).cloned();
            return Err(err.unwrap_or_else(|| format!("{} isn't loaded (its file {})", d.id, d.file)));
        };
        let problems = lines.iter().filter(|l| l.starts_with('!')).count();
        let size = std::fs::metadata(self.project.root.join(&d.file)).map(|m| m.len() / 1024).unwrap_or(0);
        text(format!("{} ({}), {} ({size} KB): {}\n{}", d.label(), d.id, d.file, if problems == 0 { "no problems".to_string() } else { format!("{problems} problem(s), marked !") }, lines.join("\n")))
    }

    /// `kit_build`: runs the figure kit's build script and takes its models into the project.
    pub(crate) fn mcp_kit_build(&mut self, p: KitBuildParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let kit = std::env::var_os("RUSTPG2D_KIT").map(std::path::PathBuf::from).or_else(|| crate::repo_mode::root().map(|r| r.join("scripts/art/kit"))).filter(|k| k.join("build.py").is_file());
        let kit = kit.ok_or("the figure kit's sources (scripts/art/kit with build.py) aren't here: start the editor in the R(ust)PG2D repository, or set RUSTPG2D_KIT to the kit's folder")?;
        let out = std::env::temp_dir().join(format!("rustpg2d-kit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let python = std::env::var("RUSTPG2D_PYTHON").unwrap_or_else(|_| "python3".into());
        let ids = p.ids.clone().unwrap_or_default();
        let run = std::process::Command::new(&python).arg(kit.join("build.py")).arg("--out").arg(&out).args(&ids).output().map_err(|e| format!("starting {python}: {e} (the kit needs python3 with numpy)"))?;
        let said = format!("{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
        if !run.status.success() {
            let _ = std::fs::remove_dir_all(&out);
            return Err(format!("the kit's build failed:\n{}", said.trim()));
        }
        /// How the kit says an item model is held (`gear.json`).
        #[derive(serde::Deserialize)]
        struct Hold {
            file: String,
            grip: [f32; 3],
            axis: [f32; 3],
            front: [f32; 3],
        }
        let take = || -> Result<(usize, Vec<rustpg2d_engine::defs::ModelDef>, Vec<Hold>), String> {
            let defs: Vec<rustpg2d_engine::defs::ModelDef> = serde_json::from_str(&std::fs::read_to_string(out.join("kit.json")).map_err(|e| format!("reading kit.json: {e}"))?).map_err(|e| format!("kit.json: {e}"))?;
            let holds: Vec<Hold> = match std::fs::read_to_string(out.join("gear.json")) {
                Ok(t) => serde_json::from_str(&t).map_err(|e| format!("gear.json: {e}"))?,
                Err(_) => Vec::new(),
            };
            let dir = self.project.root.join("models");
            std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
            let mut files = 0;
            for f in std::fs::read_dir(out.join("models")).map_err(|e| e.to_string())?.flatten() {
                std::fs::copy(f.path(), dir.join(f.file_name())).map_err(|e| format!("copying {}: {e}", f.path().display()))?;
                files += 1;
            }
            Ok((files, defs, holds))
        };
        let taken = take();
        let _ = std::fs::remove_dir_all(&out);
        let (files, defs, holds) = taken?;
        if !ids.is_empty() && defs.is_empty() && holds.is_empty() {
            return Err(format!("the kit has no model named {}", ids.join(", ")));
        }
        let names: Vec<String> = defs.iter().map(|d| d.id.clone()).collect();
        for d in defs {
            match self.project.db.models.iter_mut().find(|m| m.id == d.id) {
                Some(m) => *m = d,
                None => self.project.db.models.push(d),
            }
        }
        // The gear: every item that uses one of its model files is held as the kit says.
        let mut held = 0;
        for h in &holds {
            for it in self.project.db.items.iter_mut() {
                if let Some(m) = it.model.as_mut().filter(|m| m.file == h.file) {
                    (m.grip, m.axis, m.front) = (h.grip, h.axis, h.front);
                    held += 1;
                }
            }
        }
        self.project.db.reindex();
        self.defs_dirty = true;
        self.reload_assets();
        let shown = if names.len() > 12 { format!("{} … ({} models)", names[..12].join(", "), names.len()) } else { names.join(", ") };
        self.mcp_after_write(format!("Built {files} files of the figure kit and wrote their definitions: {shown}; {held} item(s) hold the kit's {} item model(s) as it says. Look at one with model_sheet and check it with model_check.", holds.len()))
    }

    pub(crate) fn mcp_model_info(&mut self, p: ModelInfoParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let d = self.project.db.model(&p.model).ok_or_else(|| format!("no model \"{}\"{}", p.model, suggestions(&p.model, self.project.db.models.iter().map(|m| m.id.as_str()))))?.clone();
        let Some(i) = self.assets.models.info(&d.id) else {
            let err = self.assets.models.errors.iter().find(|e| e.contains(&format!("model {}:", d.id))).cloned();
            return Err(err.unwrap_or_else(|| format!("{} isn't loaded (its file {})", d.id, d.file)));
        };
        let mut out = vec![
            format!(
                "{} ({}): {} triangles, {} bones{}, height {} px, file {}{}{}",
                d.label(),
                d.id,
                i.triangles,
                i.bones.len(),
                if i.hands { ", hands" } else { ", no hand bones" },
                d.height,
                d.file,
                if d.clip_files.is_empty() { String::new() } else { format!(" + {}", d.clip_files.join(", ")) },
                if d.shared_clips.is_empty() { String::new() } else { format!(", shared clips (share_model_clips): {}", d.shared_clips.join(", ")) }
            ),
            format!("clips: {}", i.clips.iter().map(|(n, s)| format!("{n} ({s:.1} s)")).collect::<Vec<_>>().join(", ")),
            format!("animations: {}", rustpg2d_engine::model::ANIMS.iter().map(|a| format!("{a}={}", d.clip_of(a).map_or("-", |c| c.clip.as_str()))).collect::<Vec<_>>().join(", ")),
        ];
        let more: Vec<String> = rustpg2d_engine::model::all_anims()
            .skip(rustpg2d_engine::model::ANIMS.len())
            .filter_map(|a| d.clip_of(a).map(|c| format!("{a}={}{}", c.clip, if rustpg2d_engine::model::has_hit(a) { format!(" (hit {:.2})", c.hit_point()) } else { String::new() })))
            .collect();
        if !more.is_empty() {
            out.push(format!("sneaking and weapon animations: {}", more.join(", ")));
        }
        match &d.rig {
            Some(r) => out.push(format!("rig template: {} (turned {} deg, {} joint(s) moved: fit_rig)", r.template, r.turn, r.joints.len())),
            None if !i.rigged => out.push("no skeleton of its own: give it a rig template with fit_rig (rig_quadruped, rig_bird, rig_flyer, rig_blob, rig_floating)".into()),
            None => {}
        }
        if let Some(t) = self.assets.models.texture_image(&d.id) {
            let mut n: BTreeMap<[u8; 3], u32> = BTreeMap::new();
            for px in t.bytes.as_chunks::<4>().0.iter().filter(|p| p[3] > 0) {
                *n.entry([px[0], px[1], px[2]]).or_default() += 1;
            }
            let mut list: Vec<_> = n.into_iter().collect();
            list.sort_by_key(|x| std::cmp::Reverse(x.1));
            out.push(format!("texture colours ({}), most used first, with where they are on the model (0 = feet, 1 = top):", list.len()));
            let at = self.assets.models.texture_heights(&d.id);
            for (c, k) in list {
                let feature = d.features.iter().find(|f| f.shades.contains(&c)).map_or(String::new(), |f| format!("  feature {}", f.name));
                let place = at.get(&c).map_or(String::new(), |(h, _)| format!("  at {h:.2}"));
                out.push(format!("[{}, {}, {}] #{:02x}{:02x}{:02x}  {k} px{place}{feature}", c[0], c[1], c[2], c[0], c[1], c[2]));
            }
        }
        text(out.join("\n"))
    }

    pub(crate) fn mcp_fit_rig(&mut self, p: FitRigParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let joints = p.joints.map(|j| j.into_iter().map(|j| rustpg2d_engine::defs::JointNudge { joint: j.joint, offset: j.offset }).collect());
        let msg = self.fit_rig(&p.model, p.template.as_deref(), p.turn, joints)?;
        self.watch_resnapshot();
        text(msg)
    }

    pub(crate) fn mcp_share_clips(&mut self, p: ShareClipsParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let msg = self.share_clips(&p.model, &p.from, &p.anims, p.replace)?;
        self.watch_resnapshot();
        text(msg)
    }

    pub(crate) fn mcp_sheet_colors(&mut self, p: SheetColorsParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let d = self.project.db.sheet(&p.sheet).ok_or_else(|| format!("\"{}\" is not a sprite sheet.{}", p.sheet, suggestions(&p.sheet, self.project.db.sheets.iter().map(|t| t.id.as_str()))))?.clone();
        let tex = self.assets.sheets.get(&d.id).ok_or("its image isn't loaded")?;
        let mut info: BTreeMap<[u8; 3], u32> = BTreeMap::new();
        for px in tex.image.bytes.as_chunks::<4>().0.iter().filter(|p| p[3] > 0) {
            *info.entry([px[0], px[1], px[2]]).or_default() += 1;
        }
        let mut list: Vec<_> = info.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut out = vec![format!("{} colours in {} ({:?}); most used first.", list.len(), d.image, d.kind)];
        for (c, n) in list.iter().take(160) {
            out.push(format!("[{}, {}, {}] #{:02x}{:02x}{:02x}  {n} px", c[0], c[1], c[2], c[0], c[1], c[2]));
        }
        if list.len() > 160 {
            out.push(format!("… and {} rarer colours", list.len() - 160));
        }
        text(out.join("\n"))
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
            RefKind::Material => db.material(id).map(|d| d.texture.clone()),
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
                    if !s.skill.is_empty() && rustpg2d_engine::srd::Spec::parse(&s.skill, false).is_none() {
                        return Err(format!("\"{}\" is no SRD skill or tool (arcana, religion, ...)", s.skill));
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
        if matches!(kind, RefKind::Material | RefKind::Sheet | RefKind::Sound) {
            self.reload_assets();
        }
        // A model: the loaded one takes the new definition; a new model or other files are read.
        if kind == RefKind::Model {
            let fits = self.project.db.model(&id).cloned().is_some_and(|d| self.assets.models.set_def(&d));
            if !fits {
                self.reload_assets();
            }
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
                    if kind == RefKind::Prop {
                        // (Decorations of it count too.)
                        n += map.decor.iter().filter(|d| map.decor_defs.get(d.def as usize).is_some_and(|p| p == id)).count();
                    }
                    if kind == RefKind::Character {
                        n += map.zones.iter().filter(|z| matches!(&z.kind, rustpg2d_engine::map::ZoneKind::Spawner { character, .. } if character == id)).count();
                        n += map.zones.iter().filter(|z| matches!(&z.kind, rustpg2d_engine::map::ZoneKind::Encounter { monsters, .. } if monsters.iter().any(|m| m.character == id))).count();
                    }
                }
                RefKind::Material => n += map.materials.iter().filter(|t| *t == id).count(),
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
            RefKind::Prop => Some(rustpg2d_engine::script_refs::IdKind::Prop),
            RefKind::CommonEvent => Some(rustpg2d_engine::script_refs::IdKind::CommonEvent),
            RefKind::Class => Some(rustpg2d_engine::script_refs::IdKind::Class),
            RefKind::Disease => Some(rustpg2d_engine::script_refs::IdKind::Disease),
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

    pub(crate) fn mcp_find_usages(&mut self, p: DefParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let kind = if matches!(p.kind.as_str(), "map" | "maps") { RefKind::Map } else { parse_def_kind(&p.kind)? };
        let known = if kind == RefKind::Map { self.maps.clone() } else { self.def_ids(kind) };
        if !known.contains(&p.id) {
            return Err(format!("no {} \"{}\".{}", kind.name(), p.id, suggestions(&p.id, known.iter().map(String::as_str))));
        }
        let list = self.find_usages(kind, &p.id);
        if list.is_empty() {
            return text(format!("{} \"{}\" isn't used anywhere.", kind.name(), p.id));
        }
        let lines: Vec<String> = list.iter().map(|e| format!("- {} {}: {} [{}]", e.what.name(), e.label, e.detail, e.target.describe())).collect();
        text(format!("{} usage(s) of {} \"{}\":\n{}", list.len(), kind.name(), p.id, lines.join("\n")))
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
        if matches!(kind, RefKind::Material | RefKind::Sheet | RefKind::Sound) {
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
                out.push(format!("{name}:{}: {}{}", issue.line, if issue.warning { "warning: " } else { "" }, issue.message));
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
        self.problems.stale = true;
        self.watch_resnapshot();
        let check = self.check_scripts_text(Some(&path));
        text(format!("{} {} ({} lines).\n{check}", if existed { "Wrote" } else { "Created" }, path.file_name().unwrap_or_default().to_string_lossy(), p.text.lines().count()))
    }

    pub(crate) fn mcp_problems(&mut self, p: ProblemsParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let severity = match p.severity.as_deref() {
            Some(s) => Some(crate::problems::Severity::parse(s).ok_or_else(|| format!("unknown severity \"{s}\": use \"error\" or \"warning\""))?),
            None => None,
        };
        let area = match p.area.as_deref() {
            Some(a) => Some(crate::problems::Area::parse(a).ok_or_else(|| format!("unknown area \"{a}\": use project, definitions, maps, scripts or translations"))?),
            None => None,
        };
        self.refresh_problems();
        let list: Vec<String> = self.problems.list.iter().filter(|x| severity.is_none_or(|s| s == x.severity) && area.is_none_or(|a| a == x.area)).map(|x| format!("- {} [{}]", x.line(), x.target.describe())).collect();
        let (errors, warnings) = self.problems.counts();
        if list.is_empty() {
            return text(format!("No problems found{} ({errors} error(s), {warnings} warning(s) in the whole project).", if severity.is_some() || area.is_some() { " with these filters" } else { "" }));
        }
        text(format!("{} problem(s) ({errors} error(s), {warnings} warning(s) in the whole project):\n{}", list.len(), list.join("\n")))
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
            let mark = match (api.deprecated, api.since) {
                (Some(instead), _) => format!(" [DEPRECATED: use {instead}]"),
                (None, "") => String::new(),
                (None, v) => format!(" [since {v}]"),
            };
            let line = format!("{}({}){mark} — {}", api.name, api.params.iter().map(|(n, k)| format!("{n}: {k:?}")).collect::<Vec<_>>().join(", "), api.doc);
            if f.is_empty() || line.to_lowercase().contains(&f) {
                out.push(line);
            }
        }
        let mut s = format!("Script API {} — engine functions ({}; parameter kinds say what the string is: Item, Map, Spawn, Sound, Quest, Target = \"self\" | \"player\" | an entity name):\n{}", rustpg2d_engine::script_api::API_VERSION, out.len(), out.join("\n"));
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
            RefKind::Skill => Some(rustpg2d_engine::srd::abilities::proficiency_name(&a.1)),
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
        let font = rel.starts_with("fonts/") && (lower.ends_with(".ttf") || lower.ends_with(".otf"));
        let licence = rel.starts_with("fonts/") && lower.ends_with(".txt");
        let media = ["sprites/", "sounds/", "materials/"].iter().any(|f| rel.starts_with(f)) && (image || lower.ends_with(".wav") || lower.ends_with(".ogg"));
        let model = rel.starts_with("models/") && lower.ends_with(".glb");
        if !(media || font || licence || model) || rel.split('/').any(|c| c.is_empty() || c == "." || c == "..") {
            return Err(format!("\"{rel}\": give a path under sprites/, materials/ or sounds/ ending in .png, .wav or .ogg, under models/ ending in .glb, or under fonts/ ending in .ttf, .otf or .txt (a font's licence)"));
        }
        let bytes = match (&p.base64, &p.source) {
            (Some(b), None) => rustpg2d_engine::hash::base64_decode(b).ok_or("base64 is not valid Base64")?,
            (None, Some(src)) => std::fs::read(src).map_err(|e| format!("reading {src}: {e}"))?,
            _ => return Err("give either base64 or source".into()),
        };
        let what = if model {
            let clips = rustpg2d_engine::model::clip_names(&bytes).map_err(|e| format!("not a glTF binary (.glb): {e:#}"))?;
            format!("3D model file, {} clips", clips.len())
        } else if font {
            let n = rustpg2d_engine::fonts::Coverage::of(&bytes).ok_or("not a TTF / OTF font")?.len();
            format!("font, {n} characters")
        } else if licence {
            String::from_utf8(bytes.clone()).map_err(|_| "a licence text must be UTF-8 text")?;
            "text".to_string()
        } else if image {
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
        let db = &self.project.db;
        let users: Vec<String> = db
            .materials
            .iter()
            .filter(|d| d.texture == rel)
            .map(|d| format!("material {}", d.id))
            .chain(db.models.iter().filter(|d| d.file == rel || d.clip_paths().any(|f| *f == rel)).map(|d| format!("model {}", d.id)))
            .chain(db.items.iter().filter(|d| d.model.as_ref().is_some_and(|m| m.file == rel)).map(|d| format!("item {}", d.id)))
            .chain(db.sheets.iter().filter(|d| d.image == rel).map(|d| format!("sheet {}", d.id)))
            .chain(db.sounds.iter().filter(|d| d.file == rel).map(|d| format!("sound {}", d.id)))
            .chain(db.fonts.iter().filter(|d| d.file == rel || d.license == rel).map(|d| format!("font {}", d.id)))
            .collect();
        // A model file nothing uses yet is read when its definition arrives.
        if !(model && users.is_empty()) {
            self.reload_assets();
        }
        text(format!(
            "{} {rel} ({what}). {}",
            if existed { "Replaced" } else { "Wrote" },
            if users.is_empty() { "No definition uses it yet: add one with upsert_def (materials / sheets / sounds / fonts / models).".to_string() } else { format!("Used by {}; the art was reloaded.", users.join(", ")) }
        ))
    }
}
