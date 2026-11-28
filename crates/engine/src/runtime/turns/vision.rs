//! Seeing in fights (the SRD's *Vision and Light*): areas lightly or heavily obscured by dim
//! light and darkness (darkvision sees darkness as dim light and dim light as bright), the
//! weather's fog and foliage decorations; blindsight, tremorsense and truesight; who sees whom.
//! A creature that can't see its target attacks it at disadvantage, and one it can't see attacks
//! it with advantage.

use gfx::prelude::*;

use super::Who;
use crate::grid::TilePos;
use crate::runtime::Game;
use crate::srd::conditions::{self, Condition, Sense};

/// Light below this is darkness (heavily obscured): caves, dark interiors without lights.
pub(in crate::runtime) const DARK_BELOW: f32 = 0.08;
/// Light below this is dim (lightly obscured): night outdoors, twilight, a lamp's edge.
pub(in crate::runtime) const DIM_BELOW: f32 = 0.5;

/// How obscured a tile is for a viewer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::runtime) enum Obscured {
    #[default]
    None,
    /// Dim light, patchy fog, foliage: disadvantage on Perception by sight.
    Light,
    /// Darkness, dense fog: as if blinded when looking into it.
    Heavy,
}

/// What a light level is: obscurement by the SRD's light categories.
pub(in crate::runtime) fn obscured_by_light(light: f32) -> Obscured {
    if light < DARK_BELOW {
        Obscured::Heavy
    } else if light < DIM_BELOW {
        Obscured::Light
    } else {
        Obscured::None
    }
}

/// The fog weather's obscurement `dist` tiles away at intensity `i` (0..1): lightly obscured
/// beyond `max(2, 10·(1 − i))` tiles, heavily beyond twice that.
pub(in crate::runtime) fn obscured_by_fog(dist: i32, i: f32) -> Obscured {
    let light_from = ((10.0 * (1.0 - i.clamp(0.0, 1.0))).round() as i32).max(2);
    if dist > light_from * 2 {
        Obscured::Heavy
    } else if dist > light_from {
        Obscured::Light
    } else {
        Obscured::None
    }
}

/// A sense's range in feet from a stat block's senses text ("blindsight 30 ft., darkvision 60
/// ft., passive Perception 9"); 0 when it doesn't have it.
pub fn sense_ft(senses: &str, name: &str) -> u32 {
    let l = senses.to_ascii_lowercase();
    let Some(at) = l.find(name) else { return 0 };
    l[at + name.len()..].split(|c: char| !c.is_ascii_digit()).find(|w| !w.is_empty()).and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// How one fighter sees another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum Sight {
    Clear,
    /// Lightly obscured (dim light, fog, foliage): seen, Perception at disadvantage.
    Dim(&'static str),
    /// Not seen: heavily obscured, invisible, blinded (the reason).
    Unseen(&'static str),
}

impl Game {
    /// A fighter's darkvision in feet: the hero's (race, items, a spell), a stat block's (or its
    /// senses text's), a spell on a character.
    pub(in crate::runtime) fn darkvision_of(&self, who: Who) -> u32 {
        let own = match who {
            Who::Player if self.world.shape.is_none() => self.darkvision_ft(),
            w => self.who_block(w).map_or(0, |b| b.darkvision_ft.max(sense_ft(&b.senses, "darkvision"))),
        };
        own.max(self.sense_of(who, Sense::Darkvision))
    }

    /// A fighter's special sense in feet (0 = none; `u32::MAX` = as far as it sees): a stat
    /// block's senses text, the hero's items, and spells on either (*True Seeing*).
    pub(in crate::runtime) fn sense_of(&self, who: Who, sense: Sense) -> u32 {
        let wide = |feet: u32| if feet == 0 { u32::MAX } else { feet };
        let spells = conditions::buffs(self.who_conditions(who)).filter_map(|b| if let conditions::Buff::Sense { sense: s, feet } = b { (*s == sense).then(|| wide(*feet)) } else { None }).max().unwrap_or(0);
        let own = match who {
            Who::Player if self.world.shape.is_none() => self.hero_traits().map_or(0, |t| t.senses.iter().filter(|(s, _)| *s == sense).map(|(_, f)| wide(*f)).max().unwrap_or(0)),
            w if sense == Sense::SeeInvisible => self.who_block(w).map_or(0, |b| sense_ft(&b.senses, "see invisib")),
            w => self.who_block(w).map_or(0, |b| sense_ft(&b.senses, sense.id())),
        };
        own.max(spells)
    }

    /// `viewer` perceives `target` without seeing it: blindsight or tremorsense in range (it
    /// finds the hidden and the invisible, in darkness too).
    pub(in crate::runtime) fn perceives(&self, viewer: Who, target: Who) -> bool {
        let feet = self.who_dist(viewer, target) as u32 * 5;
        [Sense::Blindsight, Sense::Tremorsense].into_iter().any(|s| {
            let r = self.sense_of(viewer, s);
            r > 0 && feet <= r
        })
    }

    /// How obscured tile `t` is for `viewer` (and why): its light (darkvision within its range
    /// counts darkness as dim and dim as bright), the weather's fog by the distance, foliage.
    pub(in crate::runtime) fn tile_obscured(&self, viewer: Who, t: TilePos) -> (Obscured, &'static str) {
        // (A spell's fog or darkness: heavily obscured, darkvision or not.)
        if let Some(why) = self.area_obscures(t) {
            return (Obscured::Heavy, why);
        }
        let ts = self.ts();
        let dist = self.dist_to(viewer, t);
        let mut light = obscured_by_light(self.light_level(vec2((t.0 as f32 + 0.5) * ts, (t.1 as f32 + 0.5) * ts)));
        if light != Obscured::None && (dist as u32) * 5 <= self.darkvision_of(viewer) {
            light = if light == Obscured::Heavy { Obscured::Light } else { Obscured::None };
        }
        let mut out = (light, if light == Obscured::Heavy { "dark" } else { "dim" });
        let w = self.weather();
        if w.kind == crate::map::WeatherKind::Fog {
            let fog = obscured_by_fog(dist, w.intensity);
            if fog > out.0 {
                out = (fog, "fog");
            }
        }
        if out.0 == Obscured::None && self.loaded.occupancy.has(t.0, t.1, crate::runtime::occupancy::OCC_FOLIAGE) {
            out = (Obscured::Light, "foliage");
        }
        out
    }

    /// How `viewer` sees `target`: blindsight, tremorsense or truesight in range see it clearly
    /// (truesight and blindsight an invisible one too); otherwise the least obscured tile of its
    /// space decides, and an invisible one isn't seen.
    pub(in crate::runtime) fn sees(&self, viewer: Who, target: Who) -> Sight {
        let feet = self.who_dist(viewer, target) as u32 * 5;
        let within = |s: Sense| {
            let r = self.sense_of(viewer, s);
            r > 0 && feet <= r
        };
        if within(Sense::Blindsight) || within(Sense::Truesight) || within(Sense::Tremorsense) {
            return Sight::Clear;
        }
        if conditions::rules(self.who_conditions(viewer)).blinded {
            return Sight::Unseen("blind");
        }
        // (See Invisibility: the invisible are seen like anyone else.)
        if conditions::has(self.who_conditions(target), Condition::Invisible) && !within(Sense::SeeInvisible) {
            return Sight::Unseen("invisible");
        }
        let best = self.who_space(target).into_iter().map(|t| self.tile_obscured(viewer, t)).min_by_key(|o| o.0).unwrap_or((Obscured::None, ""));
        match best.0 {
            Obscured::None => Sight::Clear,
            Obscured::Light => Sight::Dim(best.1),
            Obscured::Heavy => Sight::Unseen(best.1),
        }
    }

    /// Whether `viewer` can see `target` at all.
    pub(in crate::runtime) fn can_see(&self, viewer: Who, target: Who) -> bool {
        !matches!(self.sees(viewer, target), Sight::Unseen(_))
    }

    /// Whether character `uid` is a foe of the hero's side in the fight that the hero can't see
    /// (it is drawn faded).
    pub(in crate::runtime) fn unseen_foe(&self, uid: u32) -> bool {
        let who = Who::Npc(uid);
        self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| f.side != super::Side::Party && !f.out) && !self.can_see(Who::Player, who)
    }

    /// `sight=` for status: each foe in the fight as the hero sees it.
    pub(in crate::runtime) fn sight_status(&self) -> String {
        let Some(c) = &self.fight else { return "-".into() };
        let v: Vec<String> = c
            .order
            .iter()
            .filter(|f| f.side != super::Side::Party && !f.out)
            .map(|f| {
                let s = match self.sees(Who::Player, f.who) {
                    Sight::Clear => "clear".to_string(),
                    Sight::Dim(r) => format!("dim:{r}"),
                    Sight::Unseen(r) => format!("unseen:{r}"),
                };
                format!("{}:{s}", self.who_key(f.who))
            })
            .collect();
        if v.is_empty() { "-".into() } else { v.join(",") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_fog_obscure() {
        assert_eq!((obscured_by_light(0.0), obscured_by_light(0.13), obscured_by_light(0.8)), (Obscured::Heavy, Obscured::Light, Obscured::None));
        // Fog at 0.5: light beyond 5 tiles, heavy beyond 10.
        assert_eq!((obscured_by_fog(5, 0.5), obscured_by_fog(6, 0.5), obscured_by_fog(11, 0.5)), (Obscured::None, Obscured::Light, Obscured::Heavy));
        assert_eq!(obscured_by_fog(3, 1.0), Obscured::Light, "dense fog: from 2 tiles on");
    }

    #[test]
    fn senses_come_from_the_text() {
        let s = "blindsight 30 ft., darkvision 60 ft., passive Perception 9";
        assert_eq!((sense_ft(s, "blindsight"), sense_ft(s, "darkvision"), sense_ft(s, "truesight")), (30, 60, 0));
    }
}
