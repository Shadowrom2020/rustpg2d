//! The SRD 5.1's own words for what the engine has as constants (conditions, damage types,
//! languages, alignments, ability scores, skills) and the ids of its races and backgrounds:
//! `rules.ron`, written by the editor's `srd_import` from the 5e-database and built into the
//! engine. The editor shows them as hover texts; tests keep the constants equal to the data.
//! [`ATTRIBUTION`] is the licence's attribution a project using SRD content carries.

use serde::{Deserialize, Serialize};

/// The attribution the SRD 5.1's licence (CC BY 4.0) asks for, and where the data came from. It
/// goes into exports (`SRD-5.1-ATTRIBUTION.txt`), the credits and the starter library of a
/// project that uses SRD content (`ProjectConfig::srd`).
pub const ATTRIBUTION: &str = "This work includes material taken from the System Reference Document 5.1 (\"SRD 5.1\") by Wizards of the Coast LLC and available at https://dnd.wizards.com/resources/systems-reference-document. The SRD 5.1 is licensed under the Creative Commons Attribution 4.0 International License available at https://creativecommons.org/licenses/by/4.0/legalcode.\n\nThe SRD's data was taken from 5e-bits/5e-database (https://github.com/5e-bits/5e-database, MIT License).";

/// The attribution file's name in exports and the starter library.
pub const ATTRIBUTION_FILE: &str = "SRD-5.1-ATTRIBUTION.txt";

/// A named thing with its SRD text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct Language {
    pub id: String,
    pub name: String,
    /// "Standard" or "Exotic".
    pub kind: String,
    pub speakers: Vec<String>,
    pub script: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct Alignment {
    pub id: String,
    pub name: String,
    pub abbreviation: String,
    pub text: String,
}

/// Everything `rules.ron` holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct RulesText {
    pub conditions: Vec<Entry>,
    pub damage_types: Vec<Entry>,
    pub languages: Vec<Language>,
    pub alignments: Vec<Alignment>,
    /// The six ability scores (id `str`, name `Strength`).
    pub abilities: Vec<Entry>,
    /// The 18 skills (id `acrobatics`).
    pub skills: Vec<Entry>,
    /// Skill -> its ability (`acrobatics` -> `dex`).
    pub skill_abilities: std::collections::BTreeMap<String, String>,
    /// The ids of the SRD's races and subraces, and of its backgrounds (`Problems` asks for the
    /// attribution when a project has them).
    pub races: Vec<String>,
    pub backgrounds: Vec<String>,
    /// The ids of its monsters (the starter library's characters of the same ids).
    pub monsters: Vec<String>,
}

/// What of a project is SRD content: its races and backgrounds with the SRD's ids, characters
/// with a stat block from an SRD template or of an SRD monster ("race hill-dwarf", "background acolyte", "character
/// guard (stat block guard)").
pub fn srd_content(db: &crate::defs::Database) -> Vec<String> {
    let r = rules();
    let mut out: Vec<String> = db.races.iter().filter(|x| r.races.contains(&x.id)).map(|x| format!("race {}", x.id)).collect();
    out.extend(db.backgrounds.iter().filter(|b| r.backgrounds.contains(&b.id)).map(|b| format!("background {}", b.id)));
    out.extend(db.characters.iter().filter_map(|c| {
        let t = &c.stats.as_ref()?.template;
        (!t.is_empty() && (super::npc_template(t).is_some() || r.monsters.contains(t))).then(|| format!("character {} (stat block {t})", c.id))
    }));
    out
}

const RULES: &str = include_str!("rules.ron");

/// The built-in rules text.
pub fn rules() -> &'static RulesText {
    use std::sync::OnceLock;
    static R: OnceLock<RulesText> = OnceLock::new();
    R.get_or_init(|| {
        ron::from_str(RULES).unwrap_or_else(|e| {
            eprintln!("[rules] rules.ron: {e}");
            RulesText::default()
        })
    })
}

fn find<'a>(list: &'a [Entry], id: &str) -> Option<&'a str> {
    list.iter().find(|e| e.id == id).map(|e| e.text.as_str())
}

/// The SRD's text of a condition (`poisoned`).
pub fn condition_text(id: &str) -> Option<&'static str> {
    find(&rules().conditions, id)
}

/// The SRD's text of a damage type (`fire`).
pub fn damage_text(id: &str) -> Option<&'static str> {
    find(&rules().damage_types, id)
}

/// The SRD's text of a skill (`stealth`) or an ability (`dex`).
pub fn skill_text(id: &str) -> Option<&'static str> {
    find(&rules().skills, id).or_else(|| find(&rules().abilities, id))
}

/// A language: "Exotic, script Draconic, spoken by dragons, dragonborn".
pub fn language_text(id: &str) -> Option<String> {
    let l = rules().languages.iter().find(|l| l.id == id)?;
    let mut t = l.kind.clone();
    if !l.script.is_empty() {
        t += &format!(", script {}", l.script);
    }
    if !l.speakers.is_empty() {
        t += &format!(", spoken by {}", l.speakers.join(", "));
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srd::{abilities, Condition, DamageType};

    #[test]
    fn the_rules_text_is_built_in() {
        let r = rules();
        assert_eq!((r.conditions.len(), r.damage_types.len(), r.languages.len(), r.alignments.len(), r.abilities.len(), r.skills.len()), (15, 13, 16, 9, 6, 18));
        assert!(condition_text("poisoned").unwrap().contains("disadvantage"));
        assert!(damage_text("fire").is_some());
        assert!(language_text("draconic").unwrap().contains("Exotic"));
        assert!(r.races.contains(&"hill-dwarf".to_string()) && r.backgrounds.contains(&"acolyte".to_string()));
        assert!(r.monsters.len() == 334 && r.monsters.contains(&"adult-red-dragon".to_string()), "{}", r.monsters.len());
        // A character with a monster's stat block is SRD content.
        let mut db = crate::defs::Database::default();
        db.characters.push(crate::defs::CharacterDef { stats: Some(crate::srd::StatBlock { template: "wolf".into(), ..Default::default() }), ..crate::defs::CharacterDef::fresh("dire") });
        assert_eq!(srd_content(&db), vec!["character dire (stat block wolf)".to_string()]);
        assert!(!ATTRIBUTION.contains("D&D") && !ATTRIBUTION.contains("Dungeons"), "no trademarks");
        // The repository and the starter library carry the same file.
        assert_eq!(include_str!("../../../../SRD-5.1-ATTRIBUTION.txt"), ATTRIBUTION);
        assert_eq!(include_str!("../../../editor/assets/starter_library/SRD-5.1-ATTRIBUTION.txt"), ATTRIBUTION);
    }

    /// The engine's hand-written constants are the SRD's: the same ids and names.
    #[test]
    fn the_constants_match_the_srd() {
        let r = rules();
        let ids = |l: &[Entry]| l.iter().map(|e| (e.id.clone(), e.name.clone())).collect::<Vec<_>>();
        let conditions: Vec<(String, String)> = Condition::ALL.iter().map(|c| (c.id().to_string(), c.name().to_string())).collect();
        assert_eq!(ids(&r.conditions), conditions);
        let damage: Vec<(String, String)> = DamageType::ALL.iter().map(|t| (t.id().to_string(), t.name().to_string())).collect();
        assert_eq!(ids(&r.damage_types), damage);
        let skills: Vec<(String, String)> = abilities::SKILLS.iter().map(|(id, name, _)| (id.to_string(), name.to_string())).collect();
        let mut srd_skills = ids(&r.skills);
        srd_skills.sort();
        let mut ours = skills.clone();
        ours.sort();
        assert_eq!(srd_skills, ours);
        let langs: Vec<(String, String)> = crate::backgrounds::LANGUAGES.iter().map(|(id, n)| (id.to_string(), n.to_string())).collect();
        let srd_langs: Vec<(String, String)> = r.languages.iter().map(|l| (l.id.clone(), l.name.clone())).collect();
        assert_eq!(srd_langs, langs);
        for (id, _, a) in abilities::SKILLS {
            assert_eq!(r.skill_abilities.get(id).map(String::as_str), Some(a.id()), "{id}'s ability");
        }
        let abil: Vec<String> = crate::srd::Ability::ALL.iter().map(|a| a.id().to_string()).collect();
        assert_eq!(r.abilities.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), abil);
    }
}
