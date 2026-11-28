//! Backgrounds (`defs/backgrounds.ron`): what the hero did before the adventure (the SRD's
//! Acolyte, or a project's own Farmhand). The character creator offers them; a background gives
//! proficiencies (skills, tools), a number of languages to choose, starting equipment and a
//! feature (a rule described in words).

use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BackgroundDef {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Proficiency ids the hero gets: skills (`insight`), tools (`thieves-tools`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proficiencies: Vec<String>,
    /// How many languages of their choice the hero learns.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub language_choices: u32,
    /// Items the hero starts with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment: Vec<ItemStack>,
    /// The background's feature: its name and what it does.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub feature: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub feature_text: String,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl BackgroundDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> BackgroundDef {
        BackgroundDef { id: id.into(), name: "New background".into(), description: String::new(), proficiencies: Vec::new(), language_choices: 0, equipment: Vec::new(), feature: String::new(), feature_text: String::new() }
    }
}

/// The SRD's languages: id, name.
pub const LANGUAGES: [(&str, &str); 16] = [
    ("common", "Common"),
    ("dwarvish", "Dwarvish"),
    ("elvish", "Elvish"),
    ("giant", "Giant"),
    ("gnomish", "Gnomish"),
    ("goblin", "Goblin"),
    ("halfling", "Halfling"),
    ("orc", "Orc"),
    ("abyssal", "Abyssal"),
    ("celestial", "Celestial"),
    ("draconic", "Draconic"),
    ("deep-speech", "Deep Speech"),
    ("infernal", "Infernal"),
    ("primordial", "Primordial"),
    ("sylvan", "Sylvan"),
    ("undercommon", "Undercommon"),
];

/// A language's name ("Deep Speech"), else the id itself.
pub fn language_name(id: &str) -> String {
    LANGUAGES.iter().find(|(i, _)| *i == id).map_or_else(|| id.to_string(), |(_, n)| n.to_string())
}

/// A language's name in the game's language: the runtime text `language:<id>` (its own key:
/// "Common", "Orc" or "Giant" are other words too), else the English name.
pub fn language_label(id: &str) -> String {
    let key = format!("language:{id}");
    let t = crate::lang::tr_ui(&key);
    if t == key {
        language_name(id)
    } else {
        t.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backgrounds_round_trip() {
        let b: BackgroundDef = ron::from_str(r#"(id: "acolyte", name: "Acolyte", proficiencies: ["insight", "religion"], language_choices: 2, feature: "Shelter of the Faithful")"#).unwrap();
        assert_eq!((b.proficiencies.len(), b.language_choices, b.equipment.len()), (2, 2, 0));
        let again: BackgroundDef = ron::from_str(&ron::to_string(&b).unwrap()).unwrap();
        assert_eq!(again, b);
        let min: BackgroundDef = ron::from_str(r#"(id: "x", name: "X")"#).unwrap();
        assert_eq!(min, BackgroundDef { name: "X".into(), ..BackgroundDef::fresh("x") });
        assert_eq!(language_name("deep-speech"), "Deep Speech");
        assert_eq!(language_name("thieves-cant"), "thieves-cant");
    }
}
