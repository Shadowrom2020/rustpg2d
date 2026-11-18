//! The one-time conversion of a project of 0.27 or older to the SRD rules (0.28): the skill tree
//! (`defs/skills.ron`) becomes proficiencies. Each skill is mapped to an SRD skill or tool by its
//! effect or name ([`proficiency_for`]); its id is renamed to that proficiency everywhere (races,
//! recipes, spells, conditions, actions, scripts), and old skill levels in events become the
//! SRD's (`refactor::srd_upgrade_db`). Locks and recipe quality steps convert themselves when they
//! are read (`LockSpec`, `QualityStep`). The editor runs it on open, with a backup.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::project::{Project, ProjectConfig};
use crate::refactor::RefKind;

/// A skill of 0.27's `defs/skills.ron` (only what the conversion needs).
#[derive(Deserialize)]
struct OldSkill {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default = "no_effect")]
    effect: OldEffect,
}

fn no_effect() -> OldEffect {
    OldEffect::None
}

/// 0.27's `SkillEffect`, frozen for reading.
#[allow(dead_code)]
#[derive(Deserialize)]
enum OldEffect {
    None,
    MeleeDamage { percent: f32 },
    RangedDamage { percent: f32 },
    SpellDamage { percent: f32 },
    Defense { points: i32 },
    Speed { percent: f32 },
    Lockpick,
    Haggle { percent: f32 },
    Mana { points: u32 },
    ManaRegen { per_second: f32 },
    Script { function: String },
    Gather { tool: String, percent: f32 },
    FoodBonus { percent: f32 },
    SneakAttack { percent: f32 },
}

/// Crafting and trade skill names of earlier projects and the SRD tool they become.
const TOOL_NAMES: [(&str, &str); 22] = [
    ("alchemy", "alchemists-supplies"),
    ("smithing", "smiths-tools"),
    ("blacksmithing", "smiths-tools"),
    ("cooking", "cooks-utensils"),
    ("herbalism", "herbalism-kit"),
    ("brewing", "brewers-supplies"),
    ("carpentry", "carpenters-tools"),
    ("woodworking", "carpenters-tools"),
    ("woodcarving", "woodcarvers-tools"),
    ("leatherworking", "leatherworkers-tools"),
    ("jewelcrafting", "jewelers-tools"),
    ("tailoring", "weavers-tools"),
    ("weaving", "weavers-tools"),
    ("pottery", "potters-tools"),
    ("masonry", "masons-tools"),
    ("tinkering", "tinkers-tools"),
    ("painting", "painters-supplies"),
    ("cartography", "cartographers-tools"),
    ("calligraphy", "calligraphers-supplies"),
    ("navigation", "navigators-tools"),
    ("glassblowing", "glassblowers-tools"),
    ("cobbling", "cobblers-tools"),
];

/// The SRD proficiency an old skill becomes: by its effect (lockpicking = Thieves' Tools,
/// haggling = Persuasion, sneak attacks = Stealth, gathering = Athletics), else by its id or name
/// (an SRD skill such as Arcana, a tool such as Smith's Tools, a crafting name such as
/// "alchemy"); `None` = nothing fits (combat and magic skills: classes bring those).
fn proficiency_for(s: &OldSkill) -> Option<String> {
    let by_effect = match &s.effect {
        OldEffect::Lockpick => Some("thieves-tools"),
        OldEffect::Haggle { .. } => Some("persuasion"),
        OldEffect::SneakAttack { .. } => Some("stealth"),
        OldEffect::Gather { .. } => Some("athletics"),
        _ => None,
    };
    if let Some(p) = by_effect {
        return Some(p.into());
    }
    for key in [s.id.as_str(), s.name.as_str()] {
        if let Some((id, _, _)) = crate::srd::abilities::skill(key).or_else(|| crate::srd::abilities::tool(key)) {
            return Some(id.into());
        }
        let k = key.trim().to_ascii_lowercase();
        if let Some((_, tool)) = TOOL_NAMES.iter().find(|(n, _)| *n == k) {
            return Some(tool.to_string());
        }
        if k == "sneaking" || k == "sneak" {
            return Some("stealth".into());
        }
        if k == "lockpicking" {
            return Some("thieves-tools".into());
        }
    }
    None
}

/// What converting a project did.
#[derive(Debug, Default)]
pub struct Report {
    /// Old skill id -> the proficiency it became.
    pub mapped: Vec<(String, String)>,
    /// Old skills nothing fits (references to them are left; they are never proficient).
    pub dropped: Vec<String>,
    /// References and events changed (definitions, maps, scripts).
    pub changes: usize,
    /// Where a status effect of 0.27 that has no condition (haste, strength, weakness, shield)
    /// was taken out: "item elixir_strength", "map village".
    pub lost_effects: Vec<String>,
    pub backup: Option<PathBuf>,
}

impl Report {
    pub fn summary(&self) -> String {
        let mapped: Vec<String> = self.mapped.iter().map(|(a, b)| format!("{a} = {b}")).collect();
        let mut s = if mapped.is_empty() && self.dropped.is_empty() {
            "Converted to the SRD 5.1 rules: status effects became conditions".to_string()
        } else {
            format!("Converted to the SRD 5.1 rules: skills became proficiencies ({}), {} references updated, status effects became conditions", if mapped.is_empty() { "none".into() } else { mapped.join(", ") }, self.changes)
        };
        if !self.dropped.is_empty() {
            s += &format!("; no SRD match for {} (combat and magic come with classes)", self.dropped.join(", "));
        }
        if !self.lost_effects.is_empty() {
            s += &format!("; haste, strength, weakness and shield effects have no SRD condition and were taken out of {} (spells bring them back)", self.lost_effects.join(", "));
        }
        if let Some(b) = &self.backup {
            s += &format!("; backup: {}", b.display());
        }
        s
    }
}

/// Whether a project folder still has to be converted (its `rules_version` is older).
pub fn needs_conversion(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("project.ron")) else { return false };
    ron::from_str::<ProjectConfig>(&text).is_ok_and(|c| c.rules_version < crate::srd::RULES_VERSION)
}

/// Converts the project at `root` and saves it (`backup`: copy the folder there first).
pub fn convert_project(root: &Path, backup: Option<&Path>) -> Result<Report> {
    if let Some(b) = backup {
        crate::migrate::copy_dir(root, b).context("making a backup")?;
    }
    let mut project = Project::load(root)?;
    let from_version = project.config.rules_version;
    let mut report = Report { backup: backup.map(Path::to_path_buf), ..Default::default() };
    let skills_file = root.join("defs").join("skills.ron");
    let old: Vec<OldSkill> = match std::fs::read_to_string(&skills_file) {
        Ok(text) => ron::from_str(&text).with_context(|| format!("reading {}", skills_file.display()))?,
        Err(_) => Vec::new(),
    };
    let maps = project.list_maps();
    let mut loaded = Vec::new();
    for name in &maps {
        loaded.push(project.load_map(name).with_context(|| format!("loading map {name}"))?);
    }
    // (Version 1 had its skills converted already.)
    let old = if from_version >= 1 { Vec::new() } else { old };
    let scripts = project.script_files();
    let mut sources: Vec<(PathBuf, String)> = scripts.iter().filter_map(|p| std::fs::read_to_string(p).ok().map(|s| (p.clone(), s))).collect();
    for s in &old {
        let Some(new) = proficiency_for(s) else {
            report.dropped.push(s.id.clone());
            continue;
        };
        if new != s.id {
            report.changes += crate::refactor::rename_in_db(&mut project.db, &mut project.config, RefKind::Skill, &s.id, &new);
            for m in &mut loaded {
                report.changes += crate::refactor::rename_in_map(m, RefKind::Skill, &s.id, &new);
            }
            for (_, src) in &mut sources {
                let (text, lines) = crate::script_refs::rename_in_script(src, RefKind::Skill, &s.id, &new);
                report.changes += lines.len();
                *src = text;
            }
        }
        report.mapped.push((s.id.clone(), new));
    }
    report.changes += crate::refactor::srd_upgrade_db(&mut project.db, &mut project.config);
    for m in &mut loaded {
        report.changes += crate::refactor::srd_upgrade_map(m);
    }
    // Skills nothing fits are no proficiency: races lose them, recipes and spells no longer need
    // them. Races keep one entry per proficiency; a recipe's level only says "needed" now.
    let dropped = |id: &str| report.dropped.iter().any(|d| d == id);
    for r in &mut project.db.races {
        let mut seen = std::collections::BTreeSet::new();
        r.skills.retain(|s| !s.skill.is_empty() && !dropped(&s.skill) && seen.insert(s.skill.clone()));
        for s in &mut r.skills {
            s.level = 1;
        }
    }
    for r in &mut project.db.recipes {
        if dropped(&r.skill) {
            r.skill.clear();
            r.level = 0;
        }
        r.level = r.level.min(1);
    }
    for d in &mut project.db.spells {
        if dropped(&d.skill) {
            d.skill.clear();
        }
        d.level = 0;
    }
    // Status effects of 0.27 read as conditions; those without one are taken out.
    report.lost_effects = drop_lost_effects(&mut project.db, &mut loaded);
    // Every map is written again: locks are saved with their DC.
    for m in &loaded {
        project.save_map(m).with_context(|| format!("saving map {}", m.name))?;
    }
    for (path, src) in &sources {
        std::fs::write(path, src).with_context(|| format!("writing {}", path.display()))?;
    }
    project.config.rules_version = crate::srd::RULES_VERSION;
    project.save_defs()?;
    project.save_config()?;
    if skills_file.is_file() {
        std::fs::remove_file(&skills_file).context("removing defs/skills.ron")?;
    }
    Ok(report)
}

/// Takes out what the status effects of 0.27 without a condition (haste, strength, weakness,
/// shield) became: *Apply condition* actions and weapon / touch effects that do nothing.
/// Returns where ("item elixir_strength", "map forest").
pub fn drop_lost_effects(db: &mut crate::defs::Database, maps: &mut [crate::map::Map]) -> Vec<String> {
    use crate::events::{Action, Trigger};
    fn prune(list: &mut Vec<Action>) -> usize {
        let before = list.len();
        list.retain(|a| !matches!(a, Action::ApplyEffect { effect, .. } if effect.is_nothing()));
        let mut n = before - list.len();
        for a in list.iter_mut() {
            if let Action::If { then, otherwise, .. } | Action::Check { then, otherwise, .. } = a {
                n += prune(then) + prune(otherwise);
            }
        }
        n
    }
    fn triggers(ts: &mut [Trigger]) -> usize {
        ts.iter_mut().map(|t| prune(&mut t.actions)).sum()
    }
    fn effect(e: &mut Option<crate::srd::ConditionSpec>) -> usize {
        match e {
            Some(s) if s.is_nothing() => {
                *e = None;
                1
            }
            _ => 0,
        }
    }
    let mut out = Vec::new();
    for d in &mut db.items {
        let n = triggers(&mut d.on_use) + d.weapon.as_mut().map_or(0, |w| effect(&mut w.effect));
        if n > 0 {
            out.push(format!("item {}", d.id));
        }
    }
    for d in &mut db.props {
        if triggers(&mut d.triggers) > 0 {
            out.push(format!("prop {}", d.id));
        }
    }
    for d in &mut db.characters {
        let n = triggers(&mut d.triggers) + effect(&mut d.touch_effect) + d.weapon.as_mut().map_or(0, |w| effect(&mut w.effect));
        if n > 0 {
            out.push(format!("character {}", d.id));
        }
    }
    for d in &mut db.common_events {
        if prune(&mut d.actions) > 0 {
            out.push(format!("common event {}", d.id));
        }
    }
    for d in &mut db.quests {
        let n = d.steps.iter_mut().map(|s| prune(&mut s.on_start) + prune(&mut s.on_done)).sum::<usize>() + prune(&mut d.rewards.actions);
        if n > 0 {
            out.push(format!("quest {}", d.id));
        }
    }
    for m in maps.iter_mut() {
        let n = triggers(&mut m.triggers) + m.zones.iter_mut().map(|z| triggers(&mut z.triggers)).sum::<usize>() + m.entities.iter_mut().map(|e| triggers(&mut e.triggers)).sum::<usize>();
        if n > 0 {
            out.push(format!("map {}", m.name));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: &str, effect: &str) -> OldSkill {
        ron::from_str(&format!("(id: \"{id}\", name: \"{id}\", effect: {effect})")).unwrap()
    }

    #[test]
    fn effects_without_a_condition_are_taken_out() {
        let mut db = crate::defs::Database::default();
        let items: Vec<crate::defs::ItemDef> = ron::from_str(
            r#"[(id: "elixir", name: "Elixir", icon: (sheet: "items", frame: 0), on_use: [(on: UseItem(item: ""), actions: [ApplyEffect(target: "player", effect: (kind: Strength, strength: 30.0, seconds: 60.0)), Heal(amount: 2), If(conditions: [], then: [ApplyEffect(target: "player", effect: (kind: Haste, strength: 30.0, seconds: 30.0))], otherwise: [])])]),
               (id: "stew", name: "Stew", icon: (sheet: "items", frame: 0), on_use: [(on: UseItem(item: ""), actions: [ApplyEffect(target: "player", effect: (kind: Regen, strength: 1.0, seconds: 8.0))])])]"#,
        )
        .unwrap();
        db.items = items;
        let lost = drop_lost_effects(&mut db, &mut []);
        assert_eq!(lost, vec!["item elixir"]);
        let acts = &db.items[0].on_use[0].actions;
        assert_eq!(acts.len(), 2, "the heal and the empty If stay");
        assert!(matches!(&acts[1], crate::events::Action::If { then, .. } if then.is_empty()));
        // Regeneration became healing every round: it stays.
        assert!(matches!(&db.items[1].on_use[0].actions[0], crate::events::Action::ApplyEffect { effect, .. } if effect.ongoing.as_ref().is_some_and(|o| o.damage_type.is_none())));
    }

    #[test]
    fn old_skills_map_to_srd_proficiencies() {
        let cases = [
            ("lockpicking", "Lockpick", Some("thieves-tools")),
            ("haggling", "Haggle(percent: 5.0)", Some("persuasion")),
            ("sneaking", "SneakAttack(percent: 25.0)", Some("stealth")),
            ("mining", "Gather(tool: \"pickaxe\", percent: 15.0)", Some("athletics")),
            ("arcana", "Mana(points: 10)", Some("arcana")),
            ("alchemy", "None", Some("alchemists-supplies")),
            ("smithing", "None", Some("smiths-tools")),
            ("cooking", "FoodBonus(percent: 25.0)", Some("cooks-utensils")),
            ("swordsmanship", "MeleeDamage(percent: 10.0)", None),
            ("scavenging", "Script(function: \"scavenge\")", None),
        ];
        for (id, effect, want) in cases {
            assert_eq!(proficiency_for(&skill(id, effect)).as_deref(), want, "{id}");
        }
    }

    #[test]
    fn a_0_27_project_is_converted() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-srd-convert-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("defs")).unwrap();
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::create_dir_all(dir.join("maps")).unwrap();
        std::fs::write(dir.join("project.ron"), "(name: \"Old\", start_map: \"\", start_spawn: \"\", player: \"\", skills: (enabled: true, magic: true, start_points: 2))").unwrap();
        std::fs::write(dir.join("defs/skills.ron"), "[(id: \"lockpicking\", name: \"Lockpicking\", effect: Lockpick), (id: \"smithing\", name: \"Smithing\"), (id: \"swordsmanship\", name: \"Swords\", effect: MeleeDamage(percent: 10.0))]").unwrap();
        std::fs::write(
            dir.join("defs/recipes.ron"),
            "[(id: \"sword\", result: \"iron_sword\", skill: \"smithing\", level: 3, quality: [(level: 2, result: \"fine_iron_sword\")]), (id: \"duel\", result: \"x\", skill: \"swordsmanship\", level: 2)]",
        )
        .unwrap();
        std::fs::write(dir.join("defs/races.ron"), "[(id: \"dwarf\", name: \"Dwarf\", skills: [(skill: \"smithing\", level: 2), (skill: \"lockpicking\", level: 1), (skill: \"swordsmanship\", level: 1)])]").unwrap();
        std::fs::write(
            dir.join("defs/props.ron"),
            "[(id: \"chest\", name: \"Chest\", sprite: (sheet: \"s\", frame: 0), lock: Some((key: \"\", pick_difficulty: 3)), triggers: [(on: Interact, conditions: [(cond: Skill(skill: \"lockpicking\", op: Ge, level: 2))], actions: [SetSkill(skill: \"smithing\", level: 3), Lockpick(difficulty: 2, var: \"p\")])])]",
        )
        .unwrap();
        std::fs::write(dir.join("scripts/main.rhai"), "fn a() { if skill(\"smithing\") > 1 { say(\"x\"); } set_skill(\"lockpicking\", 2); }\n").unwrap();
        assert!(needs_conversion(&dir));
        let r = convert_project(&dir, None).unwrap();
        assert_eq!(r.mapped, vec![("lockpicking".to_string(), "thieves-tools".to_string()), ("smithing".to_string(), "smiths-tools".to_string())]);
        assert_eq!(r.dropped, vec!["swordsmanship".to_string()]);
        assert!(!dir.join("defs/skills.ron").exists());
        assert!(!needs_conversion(&dir));
        let p = Project::load(&dir).unwrap();
        let recipe = &p.db.recipes[0];
        assert_eq!((recipe.skill.as_str(), recipe.level, recipe.quality[0].dc), ("smiths-tools", 1, 12));
        assert_eq!((p.db.recipes[1].skill.as_str(), p.db.recipes[1].level), ("", 0), "a dropped skill gates nothing");
        let race: Vec<(&str, u32)> = p.db.races[0].skills.iter().map(|s| (s.skill.as_str(), s.level)).collect();
        assert_eq!(race, vec![("smiths-tools", 1), ("thieves-tools", 1)]);
        let chest = &p.db.props[0];
        assert_eq!(chest.lock.as_ref().unwrap().pick_dc, 15);
        let t = &chest.triggers[0];
        assert_eq!(t.conditions[0].cond, crate::events::Cond::Skill { skill: "thieves-tools".into(), op: crate::events::CmpOp::Ge, level: 1 });
        assert_eq!(t.actions[0], crate::events::Action::SetSkill { skill: "smiths-tools".into(), level: 1 });
        assert_eq!(t.actions[1], crate::events::Action::Lockpick { difficulty: 12, var: "p".into() });
        let script = std::fs::read_to_string(dir.join("scripts/main.rhai")).unwrap();
        assert!(script.contains("skill(\"smiths-tools\")") && script.contains("set_skill(\"thieves-tools\""), "{script}");
        let props = std::fs::read_to_string(dir.join("defs/props.ron")).unwrap();
        assert!(props.contains("pick_dc: 15") && !props.contains("pick_difficulty"), "{props}");
        assert!(r.summary().contains("lockpicking = thieves-tools"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
