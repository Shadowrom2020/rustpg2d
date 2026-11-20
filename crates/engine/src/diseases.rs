//! The SRD's *Diseases* (`defs/diseases.ron`): what a disease does once its symptoms show
//! (exhaustion, no natural healing, a shrinking hit point maximum, failing sight, a fit when hurt)
//! and how it runs its course over long rests. The hero catches them (`WorldState::diseases`);
//! the runtime runs them (`runtime/diseases.rs`).

use serde::{Deserialize, Serialize};

use crate::srd::{Ability, Condition, DamageType};

/// How a disease runs its course after each long rest.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Course {
    /// It stays until cured (magic, a remedy, a healer).
    #[default]
    None,
    /// A save against the current DC: a success lowers the DC by `dice`; at 0 it is over
    /// (Cackle Fever).
    ShrinkingDc { dice: String },
    /// A save: a failure adds a level of exhaustion, a success takes one away; below one level
    /// it is over (Sewer Plague).
    Exhaustion,
    /// It gets worse every rest: one more step (Sight Rot's -1 per rest, blinded at -5).
    Worsens,
}

impl Course {
    pub fn label(&self) -> &'static str {
        match self {
            Course::None => "Until cured",
            Course::ShrinkingDc { .. } => "Saves lower the DC",
            Course::Exhaustion => "Saves move exhaustion",
            Course::Worsens => "Worsens each rest",
        }
    }
}

/// A fit when the sick creature is hurt (Cackle Fever's mad laughter): a save at the disease's
/// current DC, else damage and a condition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhenHurt {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub damage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<DamageType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    /// How long the condition lasts.
    pub seconds: f32,
}

impl Default for WhenHurt {
    fn default() -> Self {
        WhenHurt { damage: "1d10".into(), damage_type: Some(DamageType::Psychic), condition: Some(Condition::Incapacitated), seconds: 60.0 }
    }
}

/// A disease.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiseaseDef {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The saving throw against catching it (DC 0 = none: it is caught).
    pub save: Ability,
    pub dc: i32,
    /// Dice of hours (or days) until the symptoms show ("" = at once).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub onset: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub onset_days: bool,
    /// Levels of exhaustion when the symptoms show.
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub exhaustion: u8,
    /// Rests don't take its exhaustion away (only its course or a cure).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub keeps_exhaustion: bool,
    /// No hit points from long rests or hit dice (only magic heals).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_natural_healing: bool,
    /// No hit points from long rests.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_rest_healing: bool,
    /// Half the hit points from hit dice.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub half_hit_dice: bool,
    /// The hit point maximum drops by these dice every 24 hours ("" = never); at 0 the creature
    /// dies.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub max_hp_loss: String,
    /// Failing sight: -1 per step to attack rolls and checks by sight; blinded at -5.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sight: bool,
    /// A condition while the symptoms last (poisoned).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_hurt: Option<WhenHurt>,
    pub course: Course,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

impl Default for DiseaseDef {
    fn default() -> Self {
        DiseaseDef {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            save: Ability::Con,
            dc: 11,
            onset: String::new(),
            onset_days: false,
            exhaustion: 0,
            keeps_exhaustion: false,
            no_natural_healing: false,
            no_rest_healing: false,
            half_hit_dice: false,
            max_hp_loss: String::new(),
            sight: false,
            condition: None,
            when_hurt: None,
            course: Course::None,
        }
    }
}

impl DiseaseDef {
    /// A new one as the editor creates it.
    pub fn fresh(id: &str) -> Self {
        DiseaseDef { id: id.into(), name: "New disease".into(), ..Default::default() }
    }

    /// Hours until the symptoms show, from a roll of `onset` (`rolled`).
    pub fn onset_hours(&self, rolled: i32) -> f64 {
        if self.onset.is_empty() {
            0.0
        } else if self.onset_days {
            rolled.max(0) as f64 * 24.0
        } else {
            rolled.max(0) as f64
        }
    }
}

/// The steps of failing sight until blindness (Sight Rot).
pub const SIGHT_BLIND: i32 = 5;

/// A disease the hero has (`WorldState::diseases`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActiveDisease {
    pub id: String,
    /// When the symptoms show (game hours).
    pub onset_at: f64,
    /// The symptoms showed (their exhaustion and condition are given).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sick: bool,
    /// The save's DC now (Cackle Fever's shrinks).
    pub dc: i32,
    /// Steps it got worse (Sight Rot's penalty).
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub stage: i32,
    /// Hit point maximum lost.
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub hp_lost: i32,
    /// When the hit point maximum last dropped (game hours).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_loss: Option<f64>,
}

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}

/// What a long rest's save did to a disease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestOutcome {
    /// Nothing changed (no course).
    Stays,
    /// It got better (the DC fell, a level of exhaustion less).
    Better,
    /// It got worse (more exhaustion, worse sight).
    Worse,
    /// It is over.
    Cured,
}

impl ActiveDisease {
    pub fn new(def: &DiseaseDef, onset_at: f64) -> Self {
        ActiveDisease { id: def.id.clone(), onset_at, sick: false, dc: def.dc, stage: 0, hp_lost: 0, last_loss: None }
    }

    /// A long rest's course: `saved` is the save against [`Self::dc`] (none for `Worsens`),
    /// `shrink` the roll of a shrinking DC's dice, `exhaustion` the hero's level now.
    pub fn rest(&mut self, def: &DiseaseDef, saved: Option<bool>, shrink: i32, exhaustion: u8) -> RestOutcome {
        if !self.sick {
            return RestOutcome::Stays;
        }
        match (&def.course, saved) {
            (Course::None, _) => RestOutcome::Stays,
            (Course::ShrinkingDc { .. }, Some(true)) => {
                self.dc -= shrink.max(1);
                if self.dc <= 0 {
                    RestOutcome::Cured
                } else {
                    RestOutcome::Better
                }
            }
            (Course::Exhaustion, Some(true)) if exhaustion <= 1 => RestOutcome::Cured,
            (Course::Exhaustion, Some(true)) => RestOutcome::Better,
            (Course::Exhaustion, Some(false)) => RestOutcome::Worse,
            (Course::Worsens, _) => {
                self.stage = (self.stage + 1).min(SIGHT_BLIND);
                RestOutcome::Worse
            }
            (_, _) => RestOutcome::Stays,
        }
    }

    /// The sight penalty now (0 without failing sight).
    pub fn sight_penalty(&self, def: &DiseaseDef) -> i32 {
        if def.sight && self.sick {
            (self.stage + 1).min(SIGHT_BLIND)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sewer() -> DiseaseDef {
        DiseaseDef { id: "sewer_plague".into(), dc: 11, onset: "1d4".into(), onset_days: true, exhaustion: 1, keeps_exhaustion: true, half_hit_dice: true, no_rest_healing: true, course: Course::Exhaustion, ..Default::default() }
    }

    #[test]
    fn diseases_run_their_course() {
        let d = sewer();
        assert_eq!(d.onset_hours(3), 72.0);
        let mut a = ActiveDisease::new(&d, 72.0);
        assert_eq!(a.rest(&d, Some(false), 0, 1), RestOutcome::Stays, "no symptoms yet");
        a.sick = true;
        assert_eq!([a.rest(&d, Some(false), 0, 1), a.rest(&d, Some(true), 0, 2), a.rest(&d, Some(true), 0, 1)], [RestOutcome::Worse, RestOutcome::Better, RestOutcome::Cured]);
        // Cackle Fever: each success lowers the DC; at 0 it is over.
        let cackle = DiseaseDef { id: "cackle_fever".into(), dc: 13, course: Course::ShrinkingDc { dice: "1d6".into() }, ..Default::default() };
        let mut c = ActiveDisease { sick: true, ..ActiveDisease::new(&cackle, 0.0) };
        assert_eq!(c.rest(&cackle, Some(false), 6, 0), RestOutcome::Stays);
        assert_eq!((c.rest(&cackle, Some(true), 6, 0), c.dc), (RestOutcome::Better, 7));
        assert_eq!(c.rest(&cackle, Some(true), 6, 0), RestOutcome::Better);
        assert_eq!(c.rest(&cackle, Some(true), 4, 0), RestOutcome::Cured);
        // Sight Rot: -1 at first, one step worse each rest, blinded at -5.
        let rot = DiseaseDef { id: "sight_rot".into(), sight: true, course: Course::Worsens, ..Default::default() };
        let mut r = ActiveDisease { sick: true, ..ActiveDisease::new(&rot, 0.0) };
        assert_eq!(r.sight_penalty(&rot), 1);
        for _ in 0..6 {
            r.rest(&rot, None, 0, 0);
        }
        assert_eq!(r.sight_penalty(&rot), SIGHT_BLIND);
        // Files keep only what differs; an old save without diseases loads.
        let text = ron::to_string(&d).unwrap();
        assert!(!text.contains("sight") && !text.contains("when_hurt"), "{text}");
        assert_eq!(ron::from_str::<DiseaseDef>(&text).unwrap(), d);
        assert_eq!(ron::from_str::<ActiveDisease>("(id: \"x\")").unwrap().stage, 0);
    }
}
