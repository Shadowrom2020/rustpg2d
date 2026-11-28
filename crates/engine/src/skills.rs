//! Magic: spell definitions (`defs/spells.ron`), the SRD 5.1's spells since 0.30: a spell's level,
//! casting time, range, area, components, duration, concentration, the classes that have it and
//! its mechanics as [`SpellPart`]s (attack or save, damage, healing, conditions, buffs…); the
//! runtime casts them with spell slots (`runtime/spells.rs`). (The skill tree that lived here
//! until 0.27 became the SRD's skills and proficiencies.)

use serde::{Deserialize, Serialize};

use crate::defs::SpriteRef;

/// Project settings for magic (`project.ron` `skills`; its old `enabled` and `start_points` of
/// the skill tree are ignored, and so are `max_mana` and `mana_regen` since spells use slots).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct SkillSettings {
    /// Magic: spells learned from items, cast with mana.
    pub magic: bool,
    /// Mana at level 1.
    pub max_mana: u32,
    /// Mana regained per second.
    pub mana_regen: f32,
}

impl Default for SkillSettings {
    fn default() -> Self {
        SkillSettings { magic: false, max_mana: 20, mana_regen: 0.5 }
    }
}

impl SkillSettings {
    pub fn magic_on(&self) -> bool {
        self.magic
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SpellDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: SpriteRef,
    /// Mana spent per cast.
    #[serde(default)]
    pub mana: u32,
    /// Seconds before it can be cast again.
    #[serde(default)]
    pub cooldown: f32,
    /// Needs proficiency in this skill (an SRD skill such as `arcana`, or a tool) to be learned
    /// and cast ("" = no requirement).
    #[serde(default)]
    pub skill: String,
    /// The SRD spell level: 0 = a cantrip, 1-9 (until 0.29 the skill tree's level, unused).
    #[serde(default)]
    pub level: u32,
    /// The school (`evocation`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub school: String,
    #[serde(default)]
    pub casting: SpellCast,
    #[serde(default)]
    pub range: SpellRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<Area>,
    /// "V, S, M" and the material ("a tiny ball of bat guano and sulfur").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub components: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub material: String,
    #[serde(default)]
    pub ritual: bool,
    #[serde(default)]
    pub concentration: bool,
    /// How long its effects last in rounds of 6 seconds (0 = instantaneous), and as the SRD
    /// says it ("Up to 1 minute").
    #[serde(default)]
    pub duration_rounds: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub duration: String,
    /// The classes whose spell list has it (`wizard`, `cleric`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// What it does (empty with `story_only`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<SpellPart>,
    /// Usable only through events and scripts (*Cast spell*, `cast_spell`): its effect is the
    /// story's (divinations, summons, travel).
    #[serde(default)]
    pub story_only: bool,
    /// A material component worth 10 gp or more: the caster needs these items, and casting uses
    /// them up (cheaper components are the component pouch's or the focus's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub costly: Option<Costly>,
    /// How it looks: a projectile flying to the target, an effect where it lands, a sound.
    #[serde(default, skip_serializing_if = "SpellVisual::is_empty")]
    pub visual: SpellVisual,
}

/// How long casting takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum SpellCast {
    #[default]
    Action,
    Bonus,
    Reaction,
    /// Minutes (out of fights; a ritual adds 10).
    Minutes(u32),
}

impl SpellCast {
    pub fn label(self) -> String {
        match self {
            SpellCast::Action => "1 action".into(),
            SpellCast::Bonus => "1 bonus action".into(),
            SpellCast::Reaction => "1 reaction".into(),
            SpellCast::Minutes(m) if m >= 60 && m % 60 == 0 => format!("{} hour{}", m / 60, if m == 60 { "" } else { "s" }),
            SpellCast::Minutes(m) => format!("{m} minute{}", if m == 1 { "" } else { "s" }),
        }
    }
}

/// How far a spell reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum SpellRange {
    /// The caster (an area starts at it: cones, lines, spheres around it).
    Caster,
    Touch,
    Feet(u32),
    /// As far as the caster sees (and further: miles, unlimited).
    Sight,
}

impl Default for SpellRange {
    fn default() -> Self {
        SpellRange::Feet(30)
    }
}

impl SpellRange {
    /// Tiles (5 ft each; touch = 1, the caster = 0, sight = 40).
    pub fn tiles(self) -> i32 {
        match self {
            SpellRange::Caster => 0,
            SpellRange::Touch => 1,
            SpellRange::Feet(f) => (f / 5).max(1) as i32,
            SpellRange::Sight => 40,
        }
    }

    pub fn label(self) -> String {
        match self {
            SpellRange::Caster => "Self".into(),
            SpellRange::Touch => "Touch".into(),
            SpellRange::Feet(f) => format!("{f} feet"),
            SpellRange::Sight => "Sight".into(),
        }
    }
}

/// An area's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Shape {
    /// Around the point (radius).
    Sphere,
    /// Around the point (radius), as high as it is (the grid is flat).
    Cylinder,
    /// A square of the size, the point at its middle.
    Cube,
    /// From the caster toward the point: as long as the size, as wide at its end.
    Cone,
    /// From the caster toward the point, 5 ft wide.
    Line,
}

/// A spell's area: its shape and size in feet (a sphere's radius, a cube's side, a cone's or
/// line's length).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Area {
    pub shape: Shape,
    pub feet: u32,
}

impl Area {
    /// The tiles it covers: `origin` = the caster's tile, `point` = the aimed tile.
    pub fn tiles(&self, origin: (i32, i32), point: (i32, i32)) -> Vec<(i32, i32)> {
        let n = (self.feet / 5).max(1) as i32;
        let mut out = Vec::new();
        match self.shape {
            Shape::Sphere | Shape::Cylinder => {
                // (A radius in tiles from the point's middle: a round blob on the grid.)
                for y in -n..=n {
                    for x in -n..=n {
                        if x * x + y * y <= n * n + n / 2 {
                            out.push((point.0 + x, point.1 + y));
                        }
                    }
                }
            }
            Shape::Cube => {
                let lo = -(n - 1) / 2;
                for y in lo..lo + n {
                    for x in lo..lo + n {
                        out.push((point.0 + x, point.1 + y));
                    }
                }
            }
            Shape::Cone | Shape::Line => {
                let (dx, dy) = ((point.0 - origin.0) as f32, (point.1 - origin.1) as f32);
                let len = (dx * dx + dy * dy).sqrt();
                let (ux, uy) = if len < 0.01 { (1.0, 0.0) } else { (dx / len, dy / len) };
                for y in -n..=n {
                    for x in -n..=n {
                        let (fx, fy) = (x as f32, y as f32);
                        let along = fx * ux + fy * uy;
                        let across = (fx * uy - fy * ux).abs();
                        let inside = match self.shape {
                            // (As wide as far: a 53-degree cone, the SRD's.)
                            Shape::Cone => along > 0.0 && along <= n as f32 + 0.5 && across <= along * 0.5 + 0.5,
                            _ => along > 0.0 && along <= n as f32 + 0.5 && across <= 0.5,
                        };
                        if inside {
                            out.push((origin.0 + x, origin.1 + y));
                        }
                    }
                }
            }
        }
        out
    }

    pub fn label(&self) -> String {
        let shape = match self.shape {
            Shape::Sphere => "radius",
            Shape::Cylinder => "cylinder",
            Shape::Cube => "cube",
            Shape::Cone => "cone",
            Shape::Line => "line",
        };
        format!("{} ft {shape}", self.feet)
    }
}

/// What a spell does: its parts act in order on each target. New variants are appended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum SpellPart {
    /// A spell attack roll (spellcasting modifier + proficiency) against each target; the parts
    /// after it act on a hit (a critical hit doubles the dice).
    Attack { ranged: bool },
    /// Each target saves `ability` against the caster's spell DC; damage is halved on a success
    /// (`half`) or none, conditions only on a failure.
    Save { ability: crate::srd::Ability, half: bool },
    /// Damage dice by slot level (by character level for a cantrip): the entry of the highest
    /// level reached; `MOD` = the spellcasting modifier.
    Damage {
        by_level: Vec<(u32, String)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
    /// Healing by slot level (`1d8 + MOD`).
    Heal { by_level: Vec<(u32, String)> },
    /// Temporary hit points by slot level.
    TempHp { by_level: Vec<(u32, String)> },
    /// A condition for the spell's duration (with concentration: until it ends); `on_fail`:
    /// only on a failed save or a hit.
    Condition { condition: crate::srd::conditions::Condition, #[serde(default = "yes")] on_fail: bool },
    /// How many creatures it affects (`count` + `per_slot` for each slot level above the
    /// spell's: Bless 3 + 1); for an attack without a save, separate hits (Magic Missile's darts).
    Targets { count: u32, #[serde(default)] per_slot: u32 },
    /// A boon or bane for the duration (Bless, Bane, Shield of Faith, Mage Armor, Longstrider).
    Buff { buff: crate::srd::conditions::Buff },
    /// A light around the caster (or the target) for the duration (radius in tiles).
    Light { radius: f32 },
    /// Opens a locked door, chest or gate it is aimed at (Knock).
    Unlock,
    /// Calls a script function: `function(caster, x, y, slot)` (caster = "player" or a name; the
    /// target tile; the slot level).
    Script { function: String },
    /// Stabilizes a creature at 0 hit points (Spare the Dying): no more death saves.
    Stabilize,
    /// Ends the first of these conditions the target has (Lesser Restoration: blinded, deafened,
    /// paralyzed or poisoned; Protection from Poison: poisoned); with `disease`, else one of the
    /// hero's diseases.
    Cure {
        conditions: Vec<crate::srd::conditions::Condition>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        disease: bool,
        /// And one level of exhaustion (Greater Restoration).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        exhaustion: bool,
    },
    /// The target takes a beast's shape for the spell's duration (Polymorph): one of the project's
    /// characters whose stat block is a beast, of a challenge rating up to `max_cr` (`""` = the
    /// target's level or challenge rating). The hero casting it on themselves chooses the beast
    /// among the ones they know; a foe becomes the weakest, an ally the strongest.
    Shape {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        max_cr: String,
    },
    /// `count` creatures of character definition `creature` appear next to the caster, on its
    /// side, for the spell's duration (an instantaneous spell's stay until they drop to 0 hit
    /// points or the map is left): Find Familiar, Conjure Animals. Summoned by the party, they are
    /// companions the AI plays. Cast again by the same caster, the earlier ones vanish.
    Summon {
        creature: String,
        #[serde(default = "one")]
        count: u32,
    },
    /// The caster moves to the aimed free tile within range (Misty Step, Dimension Door); cast
    /// outside a fight, as far as the range goes ahead of it.
    Teleport,
    /// `by_level` dice of hit points: the creatures in the area (or the targets), lowest current
    /// hit points first, get `condition` for the spell's duration while the pool covers their hit
    /// points (Sleep: unconscious, waking when hurt; Color Spray: blinded).
    HpPool { by_level: Vec<(u32, String)>, condition: crate::srd::conditions::Condition },
    /// A boon on the caster aimed at the target: its `mark` names the target (Hunter's Mark's
    /// damage, True Strike's advantage).
    Mark { buff: crate::srd::conditions::Buff },
    /// The spell's area stays for its duration (Fog Cloud, Darkness, Spirit Guardians, Moonbeam,
    /// Spike Growth, Silence, walls): see [`Lasting`].
    Lasting { effect: Lasting },
    /// Ends the spells on the target and the lasting areas at the aimed tile (Dispel Magic): those
    /// of the slot's level or lower at once, a higher one with a check of the caster's
    /// spellcasting ability against DC 10 + its level.
    Dispel,
    /// Counterspell: cast as a reaction when a creature in range casts a spell, it stops it (a
    /// spell of the slot's level or lower at once, a higher one with the check).
    Counter,
    /// Brings back the fallen party member nearest the caster (within reach): one dead for at
    /// most `within_hours` hours (Revivify 1 minute, Raise Dead 10 days) or only down, with 1 hit
    /// point or all (`full`: Resurrection).
    Revive {
        within_hours: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        full: bool,
    },
    /// The armed traps within the spell's range are found (Find Traps).
    RevealTraps,
}

/// A costly material component (Revivify's diamonds): `count` of item `item`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct Costly {
    pub item: String,
    pub count: u32,
}

/// What a lasting area does while it stays (a spell's [`SpellPart::Lasting`]). With `repeat` the
/// spell's other parts (its save, damage, condition) act again on a creature that starts its turn
/// in the area or enters it (once a turn; with `per_tile` on every tile it moves in it).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct Lasting {
    /// Heavily obscured: who is inside can't be seen (Fog Cloud, Cloudkill).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub obscures: bool,
    /// Magical darkness: heavily obscured, darkvision doesn't see through it (Darkness).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dark: bool,
    /// Difficult terrain (Spike Growth, Web; Spirit Guardians' half speed for foes).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub difficult: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub repeat: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub per_tile: bool,
    /// The parts don't act when it is cast, only later (Spirit Guardians, Spike Growth).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub later: bool,
    /// It moves with the caster (Spirit Guardians).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub follows: bool,
    /// Only the caster's foes are hindered and hurt.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub foes_only: bool,
    /// No spell with a verbal component is cast inside (Silence).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub silence: bool,
    /// A wall across the aimed tile, `feet` long, that stops walking and sight (Wall of Stone).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub wall: bool,
    /// The radius in feet when the spell has no area (Spirit Guardians 15), a wall's length.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub feet: u32,
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// An HP pool's victims (Sleep): of `(hit points, who)`, the lowest first while `pool` covers
/// their hit points.
pub fn hp_pool_pick<T>(mut list: Vec<(i32, T)>, mut pool: i32) -> Vec<T> {
    list.sort_by_key(|x| x.0);
    let mut out = Vec::new();
    for (hp, t) in list {
        if hp > pool {
            break;
        }
        pool -= hp;
        out.push(t);
    }
    out
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

/// A spell's look.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct SpellVisual {
    /// Flies from the caster to the target (a sheet: all its frames animate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projectile: Option<SpriteRef>,
    /// An effect sheet played where it lands (or on each target), and its scale.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub impact: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub impact_scale: f32,
    /// A sound when cast.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sound: String,
}

fn is_zero(n: &f32) -> bool {
    *n == 0.0
}

impl SpellVisual {
    pub fn is_empty(&self) -> bool {
        *self == SpellVisual::default()
    }
}

/// The dice of `by_level` at `level` (the highest entry not above it; the first when below all).
pub fn dice_at(by_level: &[(u32, String)], level: u32) -> Option<&str> {
    by_level.iter().filter(|(l, _)| *l <= level).max_by_key(|(l, _)| *l).or_else(|| by_level.iter().min_by_key(|(l, _)| *l)).map(|(_, d)| d.as_str())
}

impl SpellDef {
    pub fn new(id: &str, name: &str, description: &str, icon: SpriteRef, mana: u32, cooldown: f32) -> Self {
        SpellDef {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            icon,
            mana,
            cooldown,
            skill: String::new(),
            level: 0,
            school: String::new(),
            casting: SpellCast::Action,
            range: SpellRange::default(),
            area: None,
            components: String::new(),
            material: String::new(),
            ritual: false,
            concentration: false,
            duration_rounds: 0,
            duration: String::new(),
            classes: Vec::new(),
            parts: Vec::new(),
            story_only: false,
            costly: None,
            visual: SpellVisual::default(),
        }
    }

    /// The editor's "+ New" value: a level 1 spell of an action.
    pub fn fresh(id: &str) -> Self {
        let mut d = SpellDef::new(id, "New spell", "", SpriteRef::default(), 0, 0.0);
        d.level = 1;
        d.components = "V, S".into();
        d.duration = "Instantaneous".into();
        d
    }


    /// The parts' script functions (the library carries them with the spell).
    pub fn scripts(&self) -> Vec<&str> {
        self.parts.iter().filter_map(|p| if let SpellPart::Script { function } = p { Some(function.as_str()) } else { None }).collect()
    }

    /// Whether it heals (its parts have healing and no damage).
    pub fn heals(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, SpellPart::Heal { .. })) && !self.parts.iter().any(|p| matches!(p, SpellPart::Damage { .. }))
    }

    /// Whether it is cast at foes (an attack, a save, damage).
    /// Whether it has a verbal component (Silence stops it).
    pub fn verbal(&self) -> bool {
        self.components.split(',').any(|c| c.trim() == "V")
    }

    /// Whether it may be aimed at an empty tile (a summons, a teleport, Dispel Magic on an area).
    pub fn aims_at_tile(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, SpellPart::Summon { .. } | SpellPart::Teleport | SpellPart::Dispel | SpellPart::Revive { .. }))
    }

    pub fn offensive(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, SpellPart::Attack { .. } | SpellPart::Save { .. } | SpellPart::Damage { .. } | SpellPart::HpPool { .. }))
    }
    pub fn requires(mut self, skill: &str) -> Self {
        self.skill = skill.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_cover_their_tiles() {
        let caster = (0, 0);
        // A 20 ft sphere: 4 tiles around the point, round.
        let sphere = Area { shape: Shape::Sphere, feet: 20 }.tiles(caster, (10, 10));
        assert!(sphere.contains(&(10, 10)) && sphere.contains(&(14, 10)) && sphere.contains(&(10, 6)) && !sphere.contains(&(14, 14)) && !sphere.contains(&(15, 10)));
        // A 15 ft cube: 3 x 3 tiles on the point.
        let cube = Area { shape: Shape::Cube, feet: 15 }.tiles(caster, (5, 5));
        assert_eq!(cube.len(), 9);
        assert!(cube.contains(&(4, 4)) && cube.contains(&(6, 6)));
        // A 15 ft cone toward the east: 3 tiles long, widening, not behind the caster.
        let cone = Area { shape: Shape::Cone, feet: 15 }.tiles(caster, (5, 0));
        assert!(cone.contains(&(1, 0)) && cone.contains(&(3, 1)) && cone.contains(&(3, -1)) && !cone.contains(&(-1, 0)) && !cone.contains(&(4, 0)) && !cone.contains(&(1, 2)));
        // A 30 ft line toward the south: 6 tiles, one wide.
        let line = Area { shape: Shape::Line, feet: 30 }.tiles(caster, (0, 3));
        assert_eq!(line.len(), 6);
        assert!(line.iter().all(|t| t.0 == 0 && t.1 > 0));
    }

    #[test]
    fn dice_by_level_take_the_highest_reached() {
        let by = vec![(1, "1d10".to_string()), (5, "2d10".to_string()), (11, "3d10".to_string())];
        assert_eq!((dice_at(&by, 1), dice_at(&by, 4), dice_at(&by, 5), dice_at(&by, 20)), (Some("1d10"), Some("1d10"), Some("2d10"), Some("3d10")));
        assert_eq!(dice_at(&[(3, "8d6".to_string())], 1), Some("8d6"), "below every entry: the first");
        assert_eq!(dice_at(&[], 3), None);
        // A new spell round-trips through its file.
        let mut d = SpellDef::fresh("x");
        d.parts = vec![SpellPart::Save { ability: crate::srd::Ability::Wis, half: false }, SpellPart::Condition { condition: crate::srd::conditions::Condition::Frightened, on_fail: true }];
        d.area = Some(Area { shape: Shape::Cone, feet: 30 });
        let back: SpellDef = ron::from_str(&ron::to_string(&d).unwrap()).unwrap();
        assert_eq!((back.parts, back.area, back.level), (d.parts, d.area, d.level));
        // 0.35 wrote `effect: Parts` (its old effects' marker): read and left out.
        assert!(ron::from_str::<SpellDef>("(id: \"a\", name: \"A\", effect: Parts, level: 2)").unwrap().level == 2);
    }

    #[test]
    fn old_skill_settings_still_load() {
        let s: SkillSettings = ron::from_str("(enabled: true, magic: true, start_points: 3, max_mana: 30)").unwrap();
        assert!(s.magic_on());
        assert_eq!(s.max_mana, 30);
    }

    #[test]
    fn an_hp_pool_takes_the_weakest_first() {
        assert_eq!(crate::skills::hp_pool_pick(vec![(15, "orc"), (7, "goblin"), (5, "rat"), (30, "ogre")], 22), vec!["rat", "goblin"]);
        assert!(crate::skills::hp_pool_pick(vec![(15, "orc")], 14).is_empty());
    }
}
