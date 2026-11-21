//! Reactions in fights (the SRD's *Reactions*): the hero is asked when one is possible (Options ›
//! Gameplay › Reactions: *Ask*), or the game decides by rule (*Automatic*, and always for
//! companions and monsters). Before an attack's roll: Protection (a fighter with the style and a
//! shield next to the attacked ally: disadvantage). On a hit: the *Shield* spell (+5 AC for a
//! round) or a monster's Parry (+AC against a melee hit) when that turns the hit into a miss,
//! Uncanny Dodge (half damage), Deflect Missiles. After the hero was hurt: *Hellish Rebuke* at the
//! attacker. An asked reaction holds the attack (`PendingReact`) until the window's answer
//! resumes it; opportunity attacks are reactions too, made by rule (`turns/mod.rs`).

use super::menu::{MenuKind, MenuPick, MenuRow};
use super::{AttackSpec, Game, Who};
use crate::classes::{FeatureEffect, FightingStyle};
use crate::lang::tr_ui;
use crate::srd::combat::AttackRoll;
use crate::srd::conditions;

/// A reaction (or none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum ReactChoice {
    Pass,
    Shield,
    Parry,
    UncannyDodge,
    Deflect,
    HellishRebuke,
    Protection,
    Counterspell,
}

impl ReactChoice {
    pub(in crate::runtime) fn label(self) -> &'static str {
        match self {
            ReactChoice::Pass => "No reaction",
            ReactChoice::Shield => "Shield",
            ReactChoice::Parry => "Parry",
            ReactChoice::UncannyDodge => "Uncanny Dodge",
            ReactChoice::Deflect => "Deflect Missiles",
            ReactChoice::HellishRebuke => "Hellish Rebuke",
            ReactChoice::Protection => "Protection",
            ReactChoice::Counterspell => "Counterspell",
        }
    }

    /// The status / autoplay name.
    pub(in crate::runtime) fn id(self) -> &'static str {
        match self {
            ReactChoice::Pass => "pass",
            ReactChoice::Shield => "shield",
            ReactChoice::Parry => "parry",
            ReactChoice::UncannyDodge => "uncanny_dodge",
            ReactChoice::Deflect => "deflect",
            ReactChoice::HellishRebuke => "hellish_rebuke",
            ReactChoice::Protection => "protection",
            ReactChoice::Counterspell => "counterspell",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            ReactChoice::Pass => "Keep your reaction",
            ReactChoice::Shield => "+5 AC until your next turn: the hit misses",
            ReactChoice::Parry => "+AC against this melee attack",
            ReactChoice::UncannyDodge => "Half the damage",
            ReactChoice::Deflect => "Less damage from the missile",
            ReactChoice::HellishRebuke => "Fire at the attacker, DEX save for half",
            ReactChoice::Protection => "Disadvantage on the attack against your ally",
            ReactChoice::Counterspell => "Stop the spell being cast (a slot)",
        }
    }
}

/// How an attack goes on after a reaction was decided (`attack_with`'s options).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AttackOpts {
    /// Protection decided before the roll (None = not asked yet).
    pub protected: Option<bool>,
}

/// Where an asked reaction holds the attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReactWhen {
    /// Before the roll (Protection).
    Before,
    /// After the roll hit (Shield, Uncanny Dodge…).
    Hit,
    /// After the damage (Hellish Rebuke at the attacker).
    Hurt,
    /// A spell being cast (Counterspell).
    Cast,
}

/// A spell held for the hero's Counterspell: what it would do.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct PendingCast {
    pub(super) spell: String,
    pub(super) slot: u32,
    pub(super) targets: Vec<Who>,
    pub(super) point: crate::path::TilePos,
}

/// An attack held for the hero's answer.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct PendingReact {
    pub(super) when: ReactWhen,
    pub(super) who: Who,
    pub(super) target: Who,
    pub(super) spec: AttackSpec,
    pub(super) roll: Option<AttackRoll>,
    pub(super) logged: String,
    /// The choices offered (status `react=`).
    pub(super) options: Vec<ReactChoice>,
    /// The spell held (Counterspell).
    pub(super) cast: Option<PendingCast>,
}

/// What a reactor does now: nothing possible, waits for the hero's answer, or this one.
pub(super) enum ReactNow {
    None,
    Wait,
    Use(ReactChoice),
}

impl Game {
    /// Whether the hero is asked (Options › Gameplay › Reactions: Ask) for its reactions; while
    /// one is asked, a second one is decided by rule.
    fn asks_reactions(&self) -> bool {
        !self.settings.auto_reactions && self.fight.as_ref().is_some_and(|c| c.pending_react.is_none() && c.pending_blow.is_none())
    }

    /// `who` can react now: its reaction unused, standing, able to act.
    fn reaction_free(&self, who: Who) -> bool {
        self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| !f.reacted && !f.down && !f.out) && conditions::can_act(self.who_conditions(who))
    }

    /// Whether `who` holds a shield in the off hand.
    fn holds_shield(&self, who: Who) -> bool {
        let db = &self.project.db;
        let id = match who {
            Who::Player => self.world.equipment.item(crate::defs::EquipSlot::Offhand).map(str::to_string),
            Who::Npc(_) => self.who_index(who).and_then(|i| self.loaded.entities[i].equipment.item(crate::defs::EquipSlot::Offhand).map(str::to_string)),
        };
        id.and_then(|id| db.item(&id)).and_then(|d| d.armor.as_ref()).is_some_and(|a| a.category == Some(crate::defs::ArmorCategory::Shield))
    }

    /// The lowest slot `who` can cast spell `id` with now (None = it can't).
    fn reaction_slot(&self, who: Who, id: &str) -> Option<u32> {
        self.cast_options(who).into_iter().filter(|(s, _)| s == id).map(|(_, l)| l).min()
    }

    /// Spends `who`'s reaction and logs it.
    fn react_log(&mut self, who: Who, c: ReactChoice) {
        let name = self.who_name(who);
        self.fight_log(format!("{name}: {} ({})", tr_ui(c.label()), tr_ui("reaction")));
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.reacted = true;
        }
    }

    /// Casts reaction spell `id` of `who`'s at `targets` with its lowest slot.
    fn cast_reaction(&mut self, who: Who, id: &str, c: ReactChoice, targets: &[Who]) -> bool {
        let (Some(slot), Some(d)) = (self.reaction_slot(who, id), self.project.db.spell(id).cloned()) else { return false };
        if self.monster_casting(who).is_some() {
            self.monster_pay(who, id, slot);
        } else if slot >= 1 {
            self.spend_slot(who, slot);
        }
        self.react_log(who, c);
        self.resolve_spell(who, &d, slot, targets, None);
        true
    }

    /// Holds the attack and asks the hero (the Reaction window).
    fn ask_reaction(&mut self, mut p: PendingReact, options: &[ReactChoice]) {
        let mut rows: Vec<MenuRow> = options.iter().map(|c| MenuRow { label: tr_ui(c.label()).into_owned(), detail: tr_ui(c.detail()).into_owned(), why: None, pick: MenuPick::React(*c) }).collect();
        rows.push(MenuRow { label: tr_ui(ReactChoice::Pass.label()).into_owned(), detail: tr_ui(ReactChoice::Pass.detail()).into_owned(), why: None, pick: MenuPick::React(ReactChoice::Pass) });
        p.options = options.to_vec();
        if let Some(c) = &mut self.fight {
            c.pending_react = Some(p);
        }
        self.open_fight_menu(MenuKind::Reaction, rows);
    }

    /// Before `who`'s attack on `target`: Protection by a fighter on the target's side next to it
    /// with the style, a shield and its reaction, who sees the attacker.
    pub(super) fn protection_before(&mut self, who: Who, target: Who, spec: &AttackSpec) -> ReactNow {
        let Some(c) = &self.fight else { return ReactNow::None };
        let side = c.fighter(target).map(|f| f.side);
        let guards: Vec<Who> = c.order.iter().filter(|f| Some(f.side) == side && f.who != target && f.who != who).map(|f| f.who).collect();
        for g in guards {
            let style = self.who_traits(g).is_some_and(|t| t.styles.contains(&FightingStyle::Protection));
            if !style || !self.reaction_free(g) || self.who_dist(g, target) > 1 || !self.holds_shield(g) || !self.can_see(g, who) {
                continue;
            }
            if g == Who::Player && !spec.spell && self.asks_reactions() {
                self.ask_reaction(PendingReact { when: ReactWhen::Before, who, target, spec: spec.clone(), roll: None, logged: String::new(), options: Vec::new(), cast: None }, &[ReactChoice::Protection]);
                return ReactNow::Wait;
            }
            self.react_log(g, ReactChoice::Protection);
            return ReactNow::Use(ReactChoice::Protection);
        }
        ReactNow::None
    }

    /// `target` was hit by `who`: its reactions (the hero asked; by rule the first: Shield or
    /// Parry when they turn the hit into a miss, else Deflect Missiles or Uncanny Dodge).
    pub(super) fn hit_reaction(&mut self, who: Who, target: Who, spec: &AttackSpec, roll: &AttackRoll, logged: &str) -> ReactNow {
        if !self.reaction_free(target) {
            return ReactNow::None;
        }
        let mut options = Vec::new();
        // (A natural 20 hits whatever the AC.)
        let natural = roll.d20 == 20;
        if !natural && roll.total < roll.ac + 5 && self.reaction_slot(target, "shield").is_some() {
            options.push(ReactChoice::Shield);
        }
        let parry = if target == Who::Player { 0 } else { self.who_block(target).map_or(0, |b| b.parry()) };
        if parry > 0 && !spec.ranged && !spec.spell && !natural && roll.total < roll.ac + parry && self.can_see(target, who) {
            options.push(ReactChoice::Parry);
        }
        if !spec.spell {
            let seen = !self.fight.as_ref().and_then(|c| c.fighter(who)).is_some_and(|f| f.hidden) && !conditions::rules(self.who_conditions(who)).invisible;
            if spec.ranged && self.who_rider(target, |e| matches!(e, FeatureEffect::ReactionReduceRanged { .. }).then_some(())).is_some() {
                options.push(ReactChoice::Deflect);
            }
            if seen && self.who_rider(target, |e| (*e == FeatureEffect::ReactionHalveDamage).then_some(())).is_some() {
                options.push(ReactChoice::UncannyDodge);
            }
        }
        if options.is_empty() {
            return ReactNow::None;
        }
        // (A spell attack goes on at once: by rule.)
        if target == Who::Player && !spec.spell && self.asks_reactions() {
            self.ask_reaction(PendingReact { when: ReactWhen::Hit, who, target, spec: spec.clone(), roll: Some(*roll), logged: logged.to_string(), options: Vec::new(), cast: None }, &options);
            return ReactNow::Wait;
        }
        ReactNow::Use(options[0])
    }

    /// `target` (the hero) was hurt by `who`: Hellish Rebuke at it when the hero can cast it.
    pub(super) fn hurt_reaction(&mut self, who: Who, target: Who) {
        if target != Who::Player || who == Who::Player || !self.reaction_free(target) || self.hp() <= 0 || self.reaction_slot(target, "hellish-rebuke").is_none() {
            return;
        }
        let range = self.project.db.spell("hellish-rebuke").map_or(12, |d| d.range.tiles().max(1));
        if self.who_dist(target, who) > range || !self.can_see(target, who) || self.fight.as_ref().and_then(|c| c.fighter(who)).is_none_or(|f| f.out || f.down) {
            return;
        }
        if self.asks_reactions() {
            self.ask_reaction(PendingReact { when: ReactWhen::Hurt, who, target, spec: AttackSpec::default(), roll: None, logged: String::new(), options: Vec::new(), cast: None }, &[ReactChoice::HellishRebuke]);
            return;
        }
        self.cast_reaction(target, "hellish-rebuke", ReactChoice::HellishRebuke, &[who]);
    }

    /// A reaction raising the AC against this hit (Shield, Parry): true when the hit misses now.
    pub(super) fn ac_reaction(&mut self, target: Who, c: ReactChoice, roll: &AttackRoll) -> bool {
        let bonus = match c {
            ReactChoice::Shield => {
                if !self.cast_reaction(target, "shield", c, &[target]) {
                    return false;
                }
                5
            }
            ReactChoice::Parry => {
                self.react_log(target, c);
                self.who_block(target).map_or(0, |b| b.parry())
            }
            _ => return false,
        };
        roll.d20 != 20 && roll.total < roll.ac + bonus
    }

    /// The Reaction window's answer: the held attack goes on with it.
    pub(super) fn resume_reaction(&mut self, c: ReactChoice) {
        let Some(p) = self.fight.as_mut().and_then(|f| f.pending_react.take()) else { return };
        if let Some(f) = self.fight.as_mut().filter(|f| f.menu.as_ref().is_some_and(|m| m.kind == MenuKind::Reaction)) {
            f.menu = None;
        }
        match p.when {
            ReactWhen::Before => {
                let protected = c == ReactChoice::Protection;
                if protected {
                    self.react_log(Who::Player, c);
                }
                self.attack_with(p.who, p.target, &p.spec, AttackOpts { protected: Some(protected) });
            }
            ReactWhen::Hit => {
                let Some(roll) = p.roll else { return };
                self.land_attack(p.who, p.target, &p.spec, roll, p.logged, c);
            }
            ReactWhen::Hurt => {
                if c == ReactChoice::HellishRebuke {
                    self.cast_reaction(Who::Player, "hellish-rebuke", c, &[p.who]);
                }
            }
            ReactWhen::Cast => {
                let Some(cast) = p.cast else { return };
                let Some(d) = self.project.db.spell(&cast.spell).cloned() else { return };
                let countered = c == ReactChoice::Counterspell && self.do_counter(Who::Player, p.who, &d, cast.slot);
                if !countered {
                    self.resolve_spell(p.who, &d, cast.slot, &cast.targets, Some(cast.point));
                }
            }
        }
    }

    /// The counterspell `who` can cast now: its id and lowest slot.
    fn counter_of(&self, who: Who) -> Option<(String, u32)> {
        self.cast_options(who).into_iter().filter(|(id, _)| self.project.db.spell(id).is_some_and(|d| d.casting == crate::skills::SpellCast::Reaction && d.parts.iter().any(|p| matches!(p, crate::skills::SpellPart::Counter)))).min_by_key(|x| x.1)
    }

    /// `caster` casts spell `d` at `slot`: a foe in range that sees it may counter it (the hero
    /// is asked; by rule a spell of 1st level or higher is countered). True when the spell is
    /// stopped or held for the answer.
    pub(super) fn counter_spell(&mut self, caster: Who, d: &crate::skills::SpellDef, slot: u32, targets: &[Who], point: crate::path::TilePos) -> bool {
        if d.parts.iter().any(|p| matches!(p, crate::skills::SpellPart::Counter)) {
            return false;
        }
        let Some(c) = &self.fight else { return false };
        let side = c.fighter(caster).map(|f| f.side);
        let foes: Vec<Who> = c.order.iter().filter(|f| Some(f.side) != side && !f.out && !f.down).map(|f| f.who).collect();
        for w in foes {
            let Some((id, _)) = self.counter_of(w) else { continue };
            let range = self.project.db.spell(&id).map_or(12, |s| s.range.tiles().max(1));
            if !self.reaction_free(w) || self.who_dist(w, caster) > range || !self.can_see(w, caster) {
                continue;
            }
            if w == Who::Player && self.asks_reactions() {
                let cast = PendingCast { spell: d.id.clone(), slot, targets: targets.to_vec(), point };
                self.ask_reaction(PendingReact { when: ReactWhen::Cast, who: caster, target: Who::Player, spec: AttackSpec::default(), roll: None, logged: String::new(), options: Vec::new(), cast: Some(cast) }, &[ReactChoice::Counterspell]);
                return true;
            }
            if d.level == 0 {
                continue;
            }
            return self.do_counter(w, caster, d, slot);
        }
        false
    }

    /// `w` casts its Counterspell at `caster`'s spell `d` (cast at `slot`): it stops a spell of
    /// its own slot's level or lower, a higher one with a check against DC 10 + the level. True
    /// when the spell is stopped.
    fn do_counter(&mut self, w: Who, caster: Who, d: &crate::skills::SpellDef, slot: u32) -> bool {
        let Some((id, cslot)) = self.counter_of(w) else { return false };
        if self.monster_casting(w).is_some() {
            self.monster_pay(w, &id, cslot);
        } else if cslot >= 1 {
            self.spend_slot(w, cslot);
        }
        self.react_log(w, ReactChoice::Counterspell);
        let level = slot.max(d.level);
        let stopped = level <= cslot || {
            let a = self.spell_ability(w);
            self.who_check(w, a.id(), 10 + level as i32, false).is_some_and(|r| r.success == Some(true))
        };
        let name = self.who_name(caster);
        let spell = crate::lang::tr(&d.name).to_string();
        self.fight_log(format!("{name}: {spell} {}", crate::lang::tr_ui(if stopped { "is countered" } else { "goes on" })));
        stopped
    }

    /// Status `react=`: the reactions asked now (`shield|pass`), `-` when none.
    pub(in crate::runtime) fn react_status(&self) -> String {
        self.fight.as_ref().and_then(|c| c.pending_react.as_ref()).map_or("-".into(), |p| p.options.iter().map(|c| c.id()).chain(["pass"]).collect::<Vec<_>>().join("|"))
    }
}

#[cfg(test)]
mod tests {
    use crate::srd::{FeatureKind, StatBlock, StatFeature};

    #[test]
    fn parry_reads_the_reaction_text() {
        let mut b = StatBlock::default();
        assert_eq!(b.parry(), 0);
        b.features.push(StatFeature { kind: FeatureKind::Reaction, name: "Parry".into(), text: "The knight adds 2 to its AC against one melee attack that would hit it.".into() });
        assert_eq!(b.parry(), 2);
        b.features[0].kind = FeatureKind::Action;
        assert_eq!(b.parry(), 0);
    }
}
