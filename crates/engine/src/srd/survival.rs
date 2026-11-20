//! The SRD's survival rules (*Adventuring*): food and water (days without food beyond 3 + CON,
//! less than a day's water: exhaustion), carrying capacity and encumbrance (the plain rule and
//! the variant), the forced march. Pure logic; the runtime applies it (`runtime/survival.rs`).

use serde::{Deserialize, Serialize};

/// How carrying is limited (`RulesSettings::encumbrance`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encumbrance {
    /// The SRD's rule: STR × 15 lb; more (up to twice) only at 5 ft, more than that not at all.
    #[default]
    Plain,
    /// The SRD's variant: over STR × 5 lb −10 ft, over STR × 10 lb −20 ft and disadvantage on STR,
    /// DEX and CON checks, saves and attacks.
    Variant,
}

/// What a load does to the hero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Load {
    #[default]
    Light,
    /// The variant's first step: −10 ft.
    Encumbered,
    /// The variant's second step: −20 ft and disadvantage.
    Heavy,
    /// Over the capacity (the plain rule): pushing or dragging at 5 ft.
    Dragging,
    /// Over twice the capacity: no moving at all.
    Over,
}

impl Load {
    /// The step of `weight` lb for a STR score under `rule`.
    pub fn of(weight: f32, strength: i32, rule: Encumbrance) -> Load {
        let s = strength.max(0) as f32;
        if weight > s * 30.0 {
            Load::Over
        } else if weight > s * 15.0 {
            Load::Dragging
        } else if rule == Encumbrance::Variant && weight > s * 10.0 {
            Load::Heavy
        } else if rule == Encumbrance::Variant && weight > s * 5.0 {
            Load::Encumbered
        } else {
            Load::Light
        }
    }

    /// The speed with this load from an unloaded `speed` (feet).
    pub fn speed(self, speed: u32) -> u32 {
        match self {
            Load::Light => speed,
            Load::Encumbered => speed.saturating_sub(10),
            Load::Heavy => speed.saturating_sub(20),
            Load::Dragging => speed.min(5),
            Load::Over => 0,
        }
    }

    /// Disadvantage on STR, DEX and CON checks and saves and on attack rolls (the variant's
    /// heavy load and beyond; the plain rule has none).
    pub fn disadvantage(self, rule: Encumbrance) -> bool {
        rule == Encumbrance::Variant && matches!(self, Load::Heavy | Load::Dragging | Load::Over)
    }

    /// `load=`'s word (`""` = light).
    pub fn word(self) -> &'static str {
        match self {
            Load::Light => "",
            Load::Encumbered => "encumbered",
            Load::Heavy => "heavy",
            Load::Dragging => "dragging",
            Load::Over => "over",
        }
    }
}

/// The hero's carrying capacity in pounds (STR × 15).
pub fn capacity(strength: i32) -> f32 {
    strength.max(0) as f32 * 15.0
}

/// Food, water and the march of the current day (`WorldState::survival`, saved).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Survival {
    /// Days of food eaten today.
    pub food: f32,
    /// Days of water drunk today.
    pub water: f32,
    /// Days without food so far (halves count: half rations).
    pub unfed: f32,
    /// The last day whose end was checked (`WorldState::day`); `None` = not started yet.
    pub day: Option<u32>,
    /// Hours walked since the last long rest (the forced march).
    pub march: f32,
    /// Hours of the march past 8 already saved against.
    pub march_saves: u32,
    /// The day that ended last had some food and water (a sleep past midnight still counts it).
    pub fed_before: bool,
}

/// What the end of a day without enough food or water asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DayEnd {
    /// Levels of exhaustion for sure.
    pub exhaustion: u8,
    /// A CON save DC 15 for water (a failure: one level); with disadvantage when `exhausted`.
    pub water_save: bool,
    /// Went hungry (beyond the days a body lasts) / thirsty (less than a day's water).
    pub hungry: bool,
    pub thirsty: bool,
}

impl Survival {
    /// Ends the day: counts the days without food, says what the lack of food and water costs,
    /// and starts the next day. `con` = the CON modifier, `exhausted` = any level already.
    pub fn end_day(&mut self, con: i32, exhausted: bool) -> DayEnd {
        let mut out = DayEnd::default();
        // Food: a full day resets the count, half a day counts as half a day without.
        if self.food >= 1.0 {
            self.unfed = 0.0;
        } else {
            self.unfed += if self.food >= 0.5 { 0.5 } else { 1.0 };
            let lasts = (3 + con).max(1) as f32;
            if self.unfed > lasts {
                out.exhaustion += 1;
                out.hungry = true;
            }
        }
        // Water: half a day calls for a save, less is a level for sure (two when already exhausted).
        if self.water < 1.0 {
            out.thirsty = true;
            if self.water >= 0.5 {
                out.water_save = true;
            } else {
                out.exhaustion += if exhausted { 2 } else { 1 };
            }
        }
        self.fed_before = self.food > 0.0 && self.water > 0.0;
        self.food = 0.0;
        self.water = 0.0;
        out
    }

    /// The forced march: the DCs of the CON saves now due for the hours walked past 8 (DC 10 + 1
    /// per hour past 8), each once.
    pub fn march_saves_due(&mut self) -> Vec<i32> {
        let past = (self.march - 8.0).floor().max(0.0) as u32;
        let due: Vec<i32> = (self.march_saves + 1..=past).map(|h| 10 + h as i32).collect();
        self.march_saves = self.march_saves.max(past);
        due
    }

    /// Whether a long rest now lowers exhaustion: some food and water today (or the day that
    /// just ended, for a sleep past midnight).
    pub fn fed(&self) -> bool {
        (self.food > 0.0 && self.water > 0.0) || self.fed_before
    }

    /// A long rest ends the march (and uses up the last day's meals).
    pub fn rested(&mut self) {
        self.fed_before = false;
        self.march = 0.0;
        self.march_saves = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn food_and_water_by_the_srd() {
        // CON +0: three days without food are fine, the fourth costs a level.
        let mut s = Survival { water: 1.0, ..Default::default() };
        for _ in 0..3 {
            assert_eq!(s.end_day(0, false).exhaustion, 0);
            s.water = 1.0;
        }
        let d = s.end_day(0, false);
        assert!(d.hungry && d.exhaustion == 1);
        // Eating a full day resets it; half rations count half.
        s = Survival { food: 1.0, water: 1.0, unfed: 5.0, ..Default::default() };
        s.end_day(0, false);
        assert_eq!(s.unfed, 0.0);
        s = Survival { food: 0.5, water: 1.0, ..Default::default() };
        s.end_day(-5, false);
        assert_eq!(s.unfed, 0.5);
        // At least one day even with a low CON.
        s = Survival { water: 1.0, ..Default::default() };
        assert_eq!(s.end_day(-5, false).exhaustion, 0);
        s.water = 1.0;
        assert_eq!(s.end_day(-5, false).exhaustion, 1);
        // Water: half a day = a save, less = a level, two when already exhausted.
        s = Survival { food: 1.0, water: 0.5, ..Default::default() };
        assert_eq!(s.end_day(0, false), DayEnd { water_save: true, thirsty: true, ..Default::default() });
        s = Survival { food: 1.0, ..Default::default() };
        assert_eq!(s.end_day(0, false).exhaustion, 1);
        s = Survival { food: 1.0, ..Default::default() };
        assert_eq!(s.end_day(0, true).exhaustion, 2);
        assert_eq!((s.food, s.water), (0.0, 0.0));
        // A rest past midnight still counts the meals of the day that just ended, once.
        s = Survival { food: 0.5, water: 1.0, ..Default::default() };
        s.end_day(0, false);
        assert!(s.fed());
        s.rested();
        assert!(!s.fed());
    }

    #[test]
    fn loads_by_the_srd() {
        // STR 16: capacity 240, dragging up to 480; the variant at 80 and 160.
        assert_eq!(capacity(16), 240.0);
        let plain = |w| Load::of(w, 16, Encumbrance::Plain);
        let variant = |w| Load::of(w, 16, Encumbrance::Variant);
        assert_eq!([plain(100.0), plain(240.0), plain(241.0), plain(481.0)], [Load::Light, Load::Light, Load::Dragging, Load::Over]);
        assert_eq!([variant(80.0), variant(81.0), variant(161.0), variant(300.0)], [Load::Light, Load::Encumbered, Load::Heavy, Load::Dragging]);
        assert_eq!([Load::Light, Load::Encumbered, Load::Heavy, Load::Dragging, Load::Over].map(|l| l.speed(30)), [30, 20, 10, 5, 0]);
        assert!(Load::Heavy.disadvantage(Encumbrance::Variant) && !Load::Encumbered.disadvantage(Encumbrance::Variant) && !Load::Dragging.disadvantage(Encumbrance::Plain));
    }

    #[test]
    fn the_forced_march() {
        let mut s = Survival { march: 7.9, ..Default::default() };
        assert!(s.march_saves_due().is_empty());
        s.march = 9.5;
        assert_eq!(s.march_saves_due(), vec![11]);
        assert!(s.march_saves_due().is_empty());
        s.march = 11.2;
        assert_eq!(s.march_saves_due(), vec![12, 13]);
        s.rested();
        assert_eq!((s.march, s.march_saves), (0.0, 0));
    }
}
