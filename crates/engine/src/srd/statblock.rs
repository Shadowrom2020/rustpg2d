//! SRD stat blocks: what a character is by the rules (abilities, armor class, hit points, speed,
//! senses, proficiencies, challenge). A character definition may have one
//! (`CharacterDef::stats`), a placed character its own (`EntityInstance::stats`). The SRD's NPC
//! stat blocks (commoner, guard, bandit, veteran…) are the templates: `npcs.ron`, written by the
//! editor's `srd_import` from the 5e-database and built into the engine.

use serde::{Deserialize, Serialize};

use super::Abilities;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatBlock {
    /// The template it came from (`guard`; "" = filled in by hand).
    pub template: String,
    /// The template's name as the SRD writes it ("Bandit Captain").
    pub name: String,
    pub abilities: Abilities,
    pub ac: i32,
    /// Its hit points (the average of `hit_dice`).
    pub hp: i32,
    /// "2d8+2".
    pub hit_dice: String,
    pub speed_ft: u32,
    pub darkvision_ft: u32,
    /// Skills and saving throws it is proficient in (`perception`, `save:dex`).
    pub proficiencies: Vec<String>,
    /// As the SRD writes them ("any one language (usually Common)").
    pub languages: String,
    /// As the SRD writes it ("any alignment", "lawful evil").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub alignment: String,
    /// Challenge rating: "0", "1/8", "2".
    pub cr: String,
    /// The experience it is worth.
    pub xp: u32,
    /// Damage types it takes half / no / double damage from (`fire`, or the SRD's words
    /// "bludgeoning, piercing, and slashing from nonmagical attacks").
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resistances: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub immunities: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vulnerabilities: Vec<String>,
    /// Conditions it can't get (`poisoned`, `exhaustion`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub condition_immunities: Vec<String>,
    /// Its attacks (the SRD's actions with an attack roll).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attacks: Vec<StatAttack>,
    /// How many attacks its Attack action makes (Multiattack; 0 = one).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub multiattack: u32,
    /// Its size (scales the look; fights: one tile for everyone until 0.32).
    #[serde(skip_serializing_if = "Size::is_medium")]
    pub size: Size,
    /// The SRD's creature type with the subtype ("beast", "humanoid (goblinoid)").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub creature_type: String,
    /// Other speeds than walking, in feet.
    #[serde(skip_serializing_if = "Speeds::is_none")]
    pub speeds: Speeds,
    /// Its senses as the SRD writes them ("blindsight 30 ft., passive Perception 10").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub senses: String,
    /// The SRD's texts of its traits and of the actions, reactions and legendary actions that
    /// aren't plain attacks (they act with 0.32; the editor and the bestiary show them).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<StatFeature>,
    /// Its spells (the SRD's *Spellcasting* and *Innate Spellcasting*): cast in fights by the AI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spellcasting: Option<StatSpellcasting>,
    /// Its actions with a saving throw instead of an attack roll (a dragon's breath, a wing
    /// beat): an area or a creature, the save, damage, a condition, recharge.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<StatAction>,
    /// Its legendary actions: taken at the end of other creatures' turns, `legendary_points` a
    /// round.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legendary: Vec<StatLegendary>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub legendary_points: u32,
    /// Legendary Resistance: failed saves it turns into successes, a day.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub legendary_resistance: u32,
    /// Regeneration: hit points at the start of its turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regeneration: Option<Regeneration>,
    /// Pack Tactics: advantage on attacks when an ally stands next to the target.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pack_tactics: bool,
}

/// An action with a saving throw (the SRD's breath weapons, a wing beat, a gaze): its area (or one
/// creature in `range_ft`), the save, the damage and a condition on a failure, how it recharges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatAction {
    pub name: String,
    /// Its area (`None` = one creature within `range_ft`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<crate::skills::Area>,
    /// A sphere around the creature itself ("each creature within 10 feet of the dragon")
    /// rather than at a point in range.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub around: bool,
    /// The range to its target or its point (cones and lines start at the creature).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub range_ft: u32,
    pub save: super::Ability,
    pub dc: i32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<super::DamageType>,
    /// Half the damage on a successful save.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub half: bool,
    /// A condition on a failed save, for some rounds (0 = until it ends).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<super::Condition>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub rounds: u32,
    /// *Recharge 5-6*: 5 (a d6 at the start of its turns brings it back); 0 = none.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub recharge: u32,
    /// Uses a day (0 = unlimited unless it recharges).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub per_day: u32,
    /// Only a legendary action (a dragon's wing attack), not one of its turn's actions.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub legendary_only: bool,
}

impl Default for StatAction {
    fn default() -> Self {
        StatAction { name: String::new(), area: None, around: false, range_ft: 0, save: super::Ability::Dex, dc: 10, dice: String::new(), damage_type: None, half: false, condition: None, rounds: 0, recharge: 0, per_day: 0, legendary_only: false }
    }
}

/// A legendary action: one of its attacks or of its save actions, and its cost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatLegendary {
    pub name: String,
    /// The attack it makes (one of `attacks`' names; "" = none).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub attack: String,
    /// The save action it takes (one of `actions`' names; "" = none).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub action: String,
    /// Legendary points it costs (1 unless "Costs 2 Actions").
    pub cost: u32,
}

/// Regeneration: hit points at the start of its turn, unless it took one of `stopped_by` since
/// (a troll's acid and fire).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Regeneration {
    pub hp: i32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stopped_by: Vec<super::DamageType>,
    /// It dies only when it starts its turn at 0 hit points without regenerating (the troll).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub revives: bool,
}

/// A stat block's spellcasting: its save DC and spell attack bonus, its spell slots and spells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatSpellcasting {
    /// Its spellcaster level (a cantrip's damage grows with it).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub level: u32,
    pub ability: super::Ability,
    pub dc: i32,
    pub attack: i32,
    /// Spell slots by level (index 0 = 1st level).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<u32>,
    pub spells: Vec<StatSpell>,
}

impl Default for StatSpellcasting {
    fn default() -> Self {
        StatSpellcasting { level: 0, ability: super::Ability::Int, dc: 10, attack: 2, slots: Vec::new(), spells: Vec::new() }
    }
}

/// One of a stat block's spells: from its slots (a cantrip at will), or innate (at will, or a
/// number of times a day).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatSpell {
    pub spell: String,
    /// Its level (0 = a cantrip).
    pub level: u32,
    /// Innate: cast without slots.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub innate: bool,
    /// Innate uses a day (0 = at will).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub per_day: u32,
}

/// A creature's size (the SRD's six).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Size {
    Tiny,
    Small,
    #[default]
    Medium,
    Large,
    Huge,
    Gargantuan,
}

impl Size {
    pub const ALL: [Size; 6] = [Size::Tiny, Size::Small, Size::Medium, Size::Large, Size::Huge, Size::Gargantuan];

    fn is_medium(&self) -> bool {
        *self == Size::Medium
    }

    pub fn name(self) -> &'static str {
        match self {
            Size::Tiny => "Tiny",
            Size::Small => "Small",
            Size::Medium => "Medium",
            Size::Large => "Large",
            Size::Huge => "Huge",
            Size::Gargantuan => "Gargantuan",
        }
    }

    pub fn parse(s: &str) -> Option<Size> {
        Size::ALL.into_iter().find(|z| z.name().eq_ignore_ascii_case(s.trim()))
    }

    /// How big its look is drawn, times a Medium creature's.
    pub fn scale(self) -> f32 {
        match self {
            Size::Tiny => 0.5,
            Size::Small => 0.75,
            Size::Medium => 1.0,
            Size::Large => 1.5,
            Size::Huge => 2.0,
            Size::Gargantuan => 3.0,
        }
    }
}

/// Speeds besides walking (feet; 0 = none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speeds {
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fly: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub swim: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub climb: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub burrow: u32,
}

impl Speeds {
    fn is_none(&self) -> bool {
        *self == Speeds::default()
    }
}

/// What a stat block feature is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureKind {
    #[default]
    Trait,
    Action,
    Reaction,
    Legendary,
}

impl FeatureKind {
    pub fn name(self) -> &'static str {
        match self {
            FeatureKind::Trait => "Trait",
            FeatureKind::Action => "Action",
            FeatureKind::Reaction => "Reaction",
            FeatureKind::Legendary => "Legendary action",
        }
    }
}

/// A trait or a special action in the SRD's words ("Pack Tactics: The wolf has advantage…").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatFeature {
    pub kind: FeatureKind,
    pub name: String,
    pub text: String,
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// An attack of a stat block: "Scimitar: +5 to hit, reach 5 ft., 1d6+3 slashing".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatAttack {
    pub name: String,
    /// The attack roll's bonus.
    pub bonus: i32,
    /// The damage dice with the modifier ("1d6+3").
    pub dice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<super::DamageType>,
    /// Melee reach in feet (0 = no melee).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub reach_ft: u32,
    /// Ranged: the normal and the long range in feet (0 = no ranged attack).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub range_ft: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub long_ft: u32,
    /// The damage with both hands on the weapon (a versatile spear's "or 1d8 + 1 if used with
    /// two hands"); "" = none. Used while the character's left hand is free.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub versatile: String,
    /// More damage of other types on a hit (a dragon's bite: "plus 7 (2d6) fire damage").
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<DamagePart>,
    /// What a hit does besides its damage (a wolf's bite: a DC 11 STR save or prone).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub on_hit: Vec<HitRider>,
}

/// Damage dice of a type: one part of an attack's damage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DamagePart {
    pub dice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<super::DamageType>,
}

/// What a monster's hit does besides its damage: a saving throw (or none) against a condition
/// and / or more damage ("must succeed on a DC 11 Strength saving throw or be knocked prone";
/// "taking 9 (2d8) poison damage on a failed save, or half as much on a successful one"; "the
/// target is grappled (escape DC 13)").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HitRider {
    /// The save that keeps it off (`None` = it happens on every hit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<super::Ability>,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub dc: i32,
    /// The condition a failed save (or the hit) gives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<super::Condition>,
    /// How long it lasts in rounds (0 = until it ends: escaped, a save, removed).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub rounds: u32,
    /// Damage on a failed save ("" = none).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<super::DamageType>,
    /// Half the damage on a successful save (else none).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub half: bool,
    /// A grapple's or restraint's escape DC (0 = none).
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub escape_dc: i32,
    /// A disease a failed save (or the hit, without a save) gives the hero (`defs/diseases.ron`;
    /// "" = none): a diseased rat's bite.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub disease: String,
}

fn is_zero_i32(n: &i32) -> bool {
    *n == 0
}

impl Default for StatBlock {
    fn default() -> Self {
        StatBlock { template: String::new(), name: String::new(), abilities: Abilities::default(), ac: 10, hp: 4, hit_dice: "1d8".into(), speed_ft: 30, darkvision_ft: 0, proficiencies: Vec::new(), languages: String::new(), alignment: String::new(), cr: "0".into(), xp: 10, resistances: Vec::new(), immunities: Vec::new(), vulnerabilities: Vec::new(), condition_immunities: Vec::new(), attacks: Vec::new(), multiattack: 0, size: Size::Medium, creature_type: String::new(), speeds: Speeds::default(), senses: String::new(), features: Vec::new(), spellcasting: None, actions: Vec::new(), legendary: Vec::new(), legendary_points: 0, legendary_resistance: 0, regeneration: None, pack_tactics: false }
    }
}

impl StatBlock {
    /// Its Parry reaction's AC bonus against a melee hit ("adds 2 to its AC against one melee
    /// attack that would hit it"; 0 = none).
    pub fn parry(&self) -> i32 {
        self.features.iter().filter(|f| f.kind == FeatureKind::Reaction && f.name.starts_with("Parry")).find_map(|f| {
            let rest = f.text.split_once(" adds ")?.1;
            let (n, tail) = rest.split_once(' ')?;
            tail.starts_with("to its AC").then(|| n.parse().ok()).flatten()
        }).unwrap_or(0)
    }

    /// What it does with damage of a type.
    pub fn defenses(&self) -> super::Defenses {
        super::Defenses { resistances: self.resistances.clone(), immunities: self.immunities.clone(), vulnerabilities: self.vulnerabilities.clone() }
    }

    /// Whether it can't get condition `c`.
    pub fn immune_to(&self, c: super::Condition) -> bool {
        self.condition_immunities.iter().any(|i| i.trim().eq_ignore_ascii_case(c.id()))
    }

    /// The level whose proficiency bonus matches its challenge rating (CR 0-4: +2, 5-8: +3…).
    pub fn proficiency_level(&self) -> u32 {
        let cr = self.cr.split('/').next().and_then(|n| n.trim().parse::<u32>().ok()).filter(|_| !self.cr.contains('/')).unwrap_or(0);
        cr.max(1)
    }

    /// Rolls a saving throw (`con`) against `dc`: its ability modifier, plus its proficiency
    /// bonus when it is proficient in that save.
    pub fn save(&self, ability: &str, dc: i32) -> Option<super::RollRecord> {
        let spec = super::Spec::parse(ability, true)?;
        let profs: std::collections::BTreeSet<String> = self.proficiencies.iter().cloned().collect();
        let roller = super::check::Roller { abilities: &self.abilities, proficiencies: &profs, level: self.proficiency_level(), training: None };
        Some(spec.roll(&roller, dc, super::Mode::Normal))
    }

    /// A field by name for scripts and status: str…cha, ac, hp, speed, darkvision, xp (`None` =
    /// no such field).
    pub fn field(&self, name: &str) -> Option<i64> {
        if let Some(a) = super::Ability::parse(name) {
            return Some(self.abilities.score(a) as i64);
        }
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "ac" | "armor_class" => self.ac as i64,
            "hp" | "hit_points" => self.hp as i64,
            "speed" => self.speed_ft as i64,
            "darkvision" => self.darkvision_ft as i64,
            "xp" => self.xp as i64,
            _ => return None,
        })
    }
}

const NPCS: &str = include_str!("npcs.ron");

/// The SRD's NPC stat blocks (the templates), by name order of the SRD.
pub fn npc_templates() -> &'static [StatBlock] {
    use std::sync::OnceLock;
    static T: OnceLock<Vec<StatBlock>> = OnceLock::new();
    T.get_or_init(|| ron::from_str(NPCS).unwrap_or_else(|e| {
        eprintln!("[rules] npcs.ron: {e}");
        Vec::new()
    }))
}

/// A template by id (`bandit-captain`).
pub fn npc_template(id: &str) -> Option<&'static StatBlock> {
    npc_templates().iter().find(|t| t.template == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stat block's special abilities save and load; one without them (0.31) reads as before.
    #[test]
    fn special_abilities_round_trip() {
        let b = StatBlock {
            actions: vec![StatAction { name: "Fire Breath".into(), area: Some(crate::skills::Area { shape: crate::skills::Shape::Cone, feet: 30 }), dice: "16d6".into(), half: true, recharge: 5, ..Default::default() }],
            legendary: vec![StatLegendary { name: "Tail Attack".into(), attack: "Tail".into(), action: String::new(), cost: 1 }],
            legendary_points: 3,
            legendary_resistance: 3,
            regeneration: Some(Regeneration { hp: 10, stopped_by: vec![super::super::DamageType::Fire], revives: true }),
            pack_tactics: true,
            ..Default::default()
        };
        let text = ron::to_string(&b).unwrap();
        assert_eq!(ron::from_str::<StatBlock>(&text).unwrap(), b);
        let plain = ron::to_string(&StatBlock::default()).unwrap();
        assert!(!plain.contains("actions") && !plain.contains("pack_tactics"), "{plain}");
        assert_eq!(ron::from_str::<StatBlock>("(ac: 12)").unwrap().actions, vec![]);
    }

    #[test]
    fn the_srd_npcs_are_built_in() {
        let t = npc_templates();
        assert_eq!(t.len(), 35, "{:?}", t.iter().map(|s| &s.template).collect::<Vec<_>>());
        let guard = npc_template("guard").unwrap();
        assert_eq!((guard.ac, guard.hp, guard.hit_dice.as_str(), guard.cr.as_str(), guard.xp), (16, 11, "2d8+2", "1/8", 25));
        assert!(guard.proficiencies.contains(&"perception".to_string()));
        assert_eq!(guard.field("ac"), Some(16));
        assert_eq!(guard.field("str"), Some(13));
        assert_eq!(guard.field("luck"), None);
        let commoner = npc_template("commoner").unwrap();
        assert_eq!((commoner.hp, commoner.xp), (4, 10));
        assert!(npc_template("werewolf-human").is_none(), "no lycanthropes");
        assert_eq!((guard.proficiency_level(), npc_template("veteran").unwrap().proficiency_level()), (1, 3));
        let s: StatBlock = ron::from_str("(template: \"x\")").unwrap();
        assert_eq!(s, StatBlock { template: "x".into(), ..Default::default() });
    }
}
