//! The SRD's actions in combat: Attack (with Multiattack and the off-hand's bonus attack), Cast a
//! Spell, Dash, Disengage, Dodge, Help, Hide, Ready, Search, Use an Object, Grapple and Shove;
//! the checks and contests they roll.

use super::{Aim, Side, Who};
use crate::grid::{feet_spot, TilePos};
use crate::runtime::Game;
use crate::srd::conditions::{self, Condition, ConditionSpec};
use crate::srd::{Mode, RollRecord};

/// The action bar's entries, in its order (keys 1-8 pick the first eight).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum TurnAct {
    Attack,
    Cast,
    /// A class action (Rage, Second Wind…): each press the next one.
    Class,
    Dash,
    Disengage,
    Dodge,
    Help,
    Hide,
    Ready,
    Search,
    Grapple,
    Shove,
    OffHand,
    EndTurn,
}

impl TurnAct {
    pub const BAR: [TurnAct; 14] = [TurnAct::Attack, TurnAct::Cast, TurnAct::Class, TurnAct::Dash, TurnAct::Disengage, TurnAct::Dodge, TurnAct::Help, TurnAct::Hide, TurnAct::Ready, TurnAct::Search, TurnAct::Grapple, TurnAct::Shove, TurnAct::OffHand, TurnAct::EndTurn];

    pub fn label(self) -> &'static str {
        match self {
            TurnAct::Attack => "Attack",
            TurnAct::Cast => "Cast",
            TurnAct::Class => "Class",
            TurnAct::Dash => "Dash",
            TurnAct::Disengage => "Disengage",
            TurnAct::Dodge => "Dodge",
            TurnAct::Help => "Help",
            TurnAct::Hide => "Hide",
            TurnAct::Ready => "Ready",
            TurnAct::Search => "Search",
            TurnAct::Grapple => "Grapple",
            TurnAct::Shove => "Shove",
            TurnAct::OffHand => "Off-hand",
            TurnAct::EndTurn => "End turn",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            TurnAct::Attack => "attack",
            TurnAct::Cast => "cast",
            TurnAct::Class => "class",
            TurnAct::Dash => "dash",
            TurnAct::Disengage => "disengage",
            TurnAct::Dodge => "dodge",
            TurnAct::Help => "help",
            TurnAct::Hide => "hide",
            TurnAct::Ready => "ready",
            TurnAct::Search => "search",
            TurnAct::Grapple => "grapple",
            TurnAct::Shove => "shove",
            TurnAct::OffHand => "offhand",
            TurnAct::EndTurn => "end",
        }
    }

    pub fn parse(s: &str) -> Option<TurnAct> {
        TurnAct::BAR.into_iter().find(|a| a.id().eq_ignore_ascii_case(s.trim()))
    }
}

impl Game {
    /// A check (or `save`) of a combatant: the hero's own, a character's from its stat block;
    /// into the roll log ("Grumbold: Athletics").
    pub(in crate::runtime) fn who_check(&mut self, who: Who, what: &str, dc: i32, save: bool) -> Option<RollRecord> {
        // (Aura of Protection: a paladin's CHA on the saves of allies near it.)
        // (Bless / Bane on saves: a die, counted like the aura.)
        // (A party member's magic items on saves: the hero's are in its training.)
        let items = if save && who != Who::Player { self.bonus_of(who, crate::classes::BonusTo::Saves) } else { 0 };
        let aura = if save { self.aura_bonus(who) + self.buff_roll(who, false) + items } else { 0 };
        let mut rec = self.who_check_once(who, what, dc, save, aura)?;
        // A failed save: Bardic Inspiration's die, Indomitable's second roll.
        if save {
            let (w, d) = (what.to_string(), dc);
            if let Some(r) = self.rescue_save(who, &rec, move |g| g.who_check_once(who, &w, d, true, aura)) {
                return Some(r);
            }
            // (Legendary Resistance: a monster's failed save becomes a success.)
            if rec.success == Some(false) && who != Who::Player && self.legendary_resist(who) {
                rec.success = Some(true);
            }
        }
        Some(rec)
    }

    /// One roll of [`Self::who_check`] (`aura`: the bonus to saves).
    fn who_check_once(&mut self, who: Who, what: &str, dc: i32, save: bool, aura: i32) -> Option<RollRecord> {
        // (Rage: advantage on STR checks and saves.)
        let class_mode = if save { self.class_save_mode(who, what).combine(self.buff_save_mode(who, what)) } else { self.class_save_mode(who, what) };
        // (Squeezed into a smaller space: disadvantage on DEX saves.)
        let class_mode = if save && what.eq_ignore_ascii_case("dex") && self.fight.is_some() && self.squeezed(who) { class_mode.combine(Mode::Disadvantage) } else { class_mode };
        if who == Who::Player {
            let mut rec = self.world.check(what, dc - aura, class_mode, save)?;
            if aura != 0 {
                (rec.modifier, rec.total, rec.dc) = (rec.modifier + aura as i64, rec.total + aura as i64, dc);
                if let Some(last) = self.world.rolls.last_mut() {
                    *last = rec.clone();
                }
            }
            return Some(rec);
        }
        let block = self.who_block(who)?;
        let spec = crate::srd::Spec::parse(what, save)?;
        let profs: std::collections::BTreeSet<String> = block.proficiencies.iter().map(|p| crate::srd::abilities::proficiency_id(p)).collect();
        let roller = crate::srd::check::Roller { abilities: &block.abilities, proficiencies: &profs, level: block.proficiency_level(), training: None };
        let r = conditions::rules(self.who_conditions(who));
        let mode = if (save && r.save_disadvantage) || (!save && r.check_disadvantage) { class_mode.combine(Mode::Disadvantage) } else { class_mode };
        // (Guidance's die, Enhance Ability's and Beacon of Hope's advantage.)
        let (adv, dice, spent) = conditions::check_boons(self.who_conditions(who), spec.ability, &spec.proficiency, save);
        let mode = if adv { mode.combine(Mode::Advantage) } else { mode };
        let die: i32 = dice.iter().map(|d| crate::srd::Expr::parse(d).map(|e| e.roll(&crate::srd::dice::NoMods).total).unwrap_or(0) as i32).sum();
        if spent {
            if let Some(l) = self.who_conditions_mut(who) {
                conditions::spend_check_dice(l);
            }
        }
        let aura = aura + die;
        let mut rec = spec.roll(&roller, dc - aura, mode);
        if aura != 0 {
            (rec.modifier, rec.total, rec.dc) = (rec.modifier + aura as i64, rec.total + aura as i64, dc);
        }
        rec.label = format!("{}: {}", self.who_name(who), rec.label);
        self.world.log_roll(rec.clone());
        Some(rec)
    }

    /// The best aura bonus to `who`'s saving throws: the CHA modifier (at least 1) of a fighter
    /// on its side with Aura of Protection within the aura's reach (itself included).
    fn aura_bonus(&self, who: Who) -> i32 {
        let Some(c) = &self.fight else { return 0 };
        let side = c.fighter(who).map(|f| f.side);
        c.order
            .iter()
            .filter(|f| Some(f.side) == side && !f.out && !f.down)
            .filter_map(|f| {
                let t = self.who_traits(f.who)?;
                let (feet, bonus, _) = t.aura?;
                if !bonus || (self.who_dist(f.who, who) as u32 * 5) > feet {
                    return None;
                }
                let cha = match f.who {
                    Who::Player => self.world.abilities.modifier(crate::srd::Ability::Cha),
                    w => self.who_block(w).map_or(0, |b| b.abilities.modifier(crate::srd::Ability::Cha)),
                };
                Some(cha.max(1))
            })
            .max()
            .unwrap_or(0)
    }

    /// Whether the current fighter can still take `act` (the action / bonus action is free, it
    /// has a target where one is needed), else why not.
    pub(super) fn act_ready(&self, act: TurnAct) -> Result<(), &'static str> {
        let Some(c) = &self.fight else { return Err("No fight") };
        let t = &c.turn;
        match act {
            TurnAct::EndTurn => Ok(()),
            TurnAct::OffHand => {
                if t.bonus {
                    Err("Bonus action used")
                } else if !t.attacked {
                    Err("Attack first")
                } else if self.offhand_weapon(c.current().who).is_none() {
                    Err("No off-hand weapon")
                } else if !self.both_light(c.current().who) {
                    // (Two-weapon fighting: light weapons in both hands.)
                    Err("Both weapons must be light")
                } else {
                    Ok(())
                }
            }
            TurnAct::Attack if t.attacks_left > 0 && t.attacked => Ok(()),
            TurnAct::Cast if self.cast_options(c.current().who).is_empty() && self.item_casts(c.current().who).iter().all(|x| x.1.is_some()) => Err("No spells"),
            TurnAct::Class if self.class_acts(c.current().who).is_empty() => Err("No class actions"),
            // (Class actions cost what they cost: some are bonus actions or free.)
            TurnAct::Class => Ok(()),
            _ if t.action => Err("Action used"),
            _ => Ok(()),
        }
    }

    /// Takes an action of the bar that needs no target (Dash, Disengage, Dodge, Hide, Ready,
    /// Search, End turn); targeted ones set the cursor's aim.
    pub(in crate::runtime) fn take_act(&mut self, act: TurnAct) -> Result<(), &'static str> {
        self.act_ready(act)?;
        let who = self.fight.as_ref().unwrap().current().who;
        let name = self.who_name(who);
        let say = |g: &mut Game, what: &str| g.fight_log(format!("{name}: {}", crate::lang::tr_ui(what)));
        match act {
            TurnAct::EndTurn => {
                self.next_turn();
                return Ok(());
            }
            // Attack with a weapon that can be thrown too: strike or throw? (The choice window.)
            TurnAct::Attack if self.open_fight_menu(super::menu::MenuKind::Attack, self.attack_rows(who)) => return Ok(()),
            // Held (a monster's grapple, a web): Grapple tries to escape instead.
            TurnAct::Grapple if self.escape_dc(who).is_some() => {
                self.escape(who);
                if let Some(c) = &mut self.fight {
                    c.turn.action = true;
                }
                return Ok(());
            }
            TurnAct::Attack | TurnAct::Help | TurnAct::Grapple | TurnAct::Shove | TurnAct::OffHand => {
                if let Some(c) = &mut self.fight {
                    c.attack_mode = None;
                }
                let aim = match act {
                    TurnAct::Attack => Aim::Attack,
                    TurnAct::Help => Aim::Help,
                    TurnAct::Grapple => Aim::Grapple,
                    TurnAct::Shove => Aim::Shove,
                    _ => Aim::OffHand,
                };
                self.set_aim(aim);
                return Ok(());
            }
            TurnAct::Cast => {
                // The spells in a window: each one it can cast now at each slot level, the others
                // greyed with the reason.
                if !self.open_fight_menu(super::menu::MenuKind::Cast, self.cast_rows(who)) {
                    return Err("No spells");
                }
                return Ok(());
            }
            TurnAct::Class => {
                // The class actions in a window (those it can't use now greyed).
                if !self.open_fight_menu(super::menu::MenuKind::Class, self.class_rows(who)) {
                    return Err("No class actions");
                }
                return Ok(());
            }
            TurnAct::Dash => {
                let speed = self.who_speed_now(who);
                self.fight.as_mut().unwrap().turn.speed += speed;
                say(self, "dashes");
            }
            TurnAct::Disengage => {
                self.fight.as_mut().unwrap().turn.disengaged = true;
                say(self, "disengages");
            }
            TurnAct::Dodge => {
                if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
                    f.dodging = true;
                }
                say(self, "dodges");
            }
            TurnAct::Ready => {
                // The choice window: the attack or a spell.
                if !self.open_fight_menu(super::menu::MenuKind::Ready, self.ready_rows(who)) {
                    return Err("Nothing to ready");
                }
                return Ok(());
            }
            TurnAct::Hide => {
                // Only out of plain sight: obscured (dim light, darkness, fog, foliage) or behind
                // three-quarters cover from every foe that could see it.
                let side = self.fight.as_ref().unwrap().current().side;
                let foes: Vec<Who> = self.fight.as_ref().unwrap().order.iter().filter(|f| f.side != side && !f.out && !f.down).map(|f| f.who).collect();
                if foes.iter().any(|f| self.sees(*f, who) == super::vision::Sight::Clear && self.cover(*f, who) < super::Cover::ThreeQuarters) {
                    return Err("Nowhere to hide here");
                }
                // Stealth against the best passive Perception of the foes.
                let side = self.fight.as_ref().unwrap().current().side;
                let dc = self.passive_perception(side);
                let ok = self.who_check(who, "stealth", dc, false).is_some_and(|r| r.success == Some(true));
                if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
                    f.hidden = ok;
                }
                say(self, if ok { "hides" } else { "fails to hide" });
            }
            TurnAct::Search => {
                // Perception against the hidden foes' Stealth (their passive: 10 + DEX).
                let side = self.fight.as_ref().unwrap().current().side;
                let hidden: Vec<Who> = self.fight.as_ref().unwrap().order.iter().filter(|f| f.side != side && f.hidden && !f.out).map(|f| f.who).collect();
                let roll = self.who_check(who, "perception", 10, false).map_or(0, |r| r.total as i32);
                let mut found = 0;
                for h in hidden {
                    if roll >= 10 + self.who_dex(h) {
                        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(h)) {
                            f.hidden = false;
                        }
                        found += 1;
                    }
                }
                say(self, if found > 0 { "finds a hidden foe" } else { "searches" });
            }
        }
        self.fight.as_mut().unwrap().turn.action = true;
        Ok(())
    }

    /// The highest passive Perception (10 + WIS, + proficiency) of the side against `side`.
    pub(super) fn passive_perception(&self, side: Side) -> i32 {
        let Some(c) = &self.fight else { return 10 };
        c.order.iter().filter(|f| f.side != side && !f.out && !f.down).map(|f| self.passive_of(f.who)).max().unwrap_or(10)
    }

    /// A combatant's passive Perception: 10 + its Perception modifier (the hero −5 when the fight
    /// finds them running: the fast pace).
    pub(in crate::runtime) fn passive_of(&self, who: Who) -> i32 {
        10 + self.check_bonus(who, "perception") as i32 - if who == Who::Player && self.hero_fast_pace() { 5 } else { 0 }
    }

    /// Who is surprised when a fight starts (the SRD's rule: a creature that notices no threat).
    /// `struck`: the hero's side struck first; while the hero sneaks, each party member rolls
    /// Stealth, and a foe whose passive Perception is below every roll is surprised. `ambush`:
    /// monsters started it; those the hero can't see roll Stealth, and a party member whose
    /// passive Perception is below every roll is surprised.
    pub(super) fn roll_surprise(&mut self, struck: bool, ambush: bool) {
        let Some(c) = &self.fight else { return };
        // The hero struck without sneaking: who hadn't noticed them is surprised (no roll).
        if struck && !self.sneaking() {
            let unnoticed = c.unnoticed.clone();
            let surprised: Vec<Who> = c.order.iter().filter(|f| f.side == Side::Enemy && matches!(f.who, Who::Npc(u) if unnoticed.contains(&u))).map(|f| f.who).collect();
            if surprised.is_empty() {
                return;
            }
            if let Some(c) = &mut self.fight {
                for f in c.order.iter_mut() {
                    f.surprised = surprised.contains(&f.who);
                }
            }
            let names: Vec<String> = surprised.iter().map(|w| self.who_name(*w)).collect();
            let line = format!("{}: {} {}", crate::lang::tr_ui("Unnoticed"), names.join(", "), crate::lang::tr_ui("surprised"));
            self.fight_log(line);
            return;
        }
        let (hiders, seekers): (Vec<Who>, Vec<Who>) = if struck && self.sneaking() {
            (c.order.iter().filter(|f| f.side == Side::Party).map(|f| f.who).collect(), c.order.iter().filter(|f| f.side == Side::Enemy).map(|f| f.who).collect())
        } else if ambush {
            let unseen: Vec<Who> = c.order.iter().filter(|f| f.side == Side::Enemy).map(|f| f.who).filter(|w| self.cover(Who::Player, *w) == super::Cover::Total).collect();
            (unseen, c.order.iter().filter(|f| f.side == Side::Party).map(|f| f.who).collect())
        } else {
            return;
        };
        // (Everyone of the hiding side must stay unnoticed: some in plain sight spoil it.)
        if hiders.is_empty() || (ambush && hiders.len() < c.order.iter().filter(|f| f.side == Side::Enemy).count()) {
            return;
        }
        // (A sneaking hero keeps the Stealth roll made when the sneaking began.)
        let kept = if struck && self.sneaking() && self.combat.sneak_roll > 0 { Some(self.combat.sneak_roll) } else { None };
        let mut n = 0;
        let rolls: Vec<(Who, i32)> = hiders
            .iter()
            .map(|w| match (w, kept) {
                (Who::Player, Some(r)) => (*w, r),
                _ => {
                    n += 1;
                    (*w, self.who_check(*w, "stealth", 0, false).map_or(0, |r| r.total as i32))
                }
            })
            .collect();
        let lowest = rolls.iter().map(|r| r.1).min().unwrap_or(0);
        // (The Stealth rolls have no DC of their own: plain rolls in the log.)
        let len = self.world.rolls.len();
        for r in self.world.rolls.iter_mut().skip(len.saturating_sub(n)) {
            (r.success, r.dc) = (None, 0);
        }
        // (A seeker notices a hider whose Stealth its passive Perception reaches: −5 when it sees
        // the hider only dimly; one it can't see at all it doesn't notice.)
        let notices = |g: &Game, s: Who, h: Who, roll: i32| match g.sees(s, h) {
            super::vision::Sight::Clear => g.passive_of(s) >= roll,
            super::vision::Sight::Dim(_) => g.passive_of(s) - 5 >= roll,
            super::vision::Sight::Unseen(_) => false,
        };
        let all = seekers.len();
        let surprised: Vec<Who> = seekers.into_iter().filter(|s| !rolls.iter().any(|(h, r)| notices(self, *s, *h, *r))).collect();
        // (A sneaking hero no one noticed is still hidden: their first attack has advantage.)
        let unseen = struck && kept.is_some() && surprised.len() == all;
        if let Some(c) = &mut self.fight {
            for f in c.order.iter_mut() {
                f.surprised = surprised.contains(&f.who);
                if unseen && f.who == Who::Player {
                    f.hidden = true;
                }
            }
        }
        let names: Vec<String> = surprised.iter().map(|w| self.who_name(*w)).collect();
        let line = format!("{} {lowest}: {}", crate::lang::tr_ui("Stealth"), if names.is_empty() { crate::lang::tr_ui("no one is surprised").into_owned() } else { format!("{} {}", names.join(", "), crate::lang::tr_ui("surprised")) });
        self.fight_log(line);
    }

    /// The cursor's aim, and the cursor onto the nearest fitting target.
    pub(super) fn set_aim(&mut self, aim: Aim) {
        let who = self.fight.as_ref().unwrap().current().who;
        let side = self.fight.as_ref().unwrap().current().side;
        let here = self.who_tile(who);
        let feature = match &aim {
            Aim::Feature(id) => self.class_acts(who).into_iter().find(|a| a.id == *id).map(|a| a.effect.target()),
            _ => None,
        };
        let allies = matches!(feature, Some(crate::classes::ActTarget::Ally { .. }));
        // (A class action used on itself: the cursor on the user.)
        if matches!(feature, Some(crate::classes::ActTarget::Myself)) {
            let c = self.fight.as_mut().unwrap();
            c.aim = aim;
            c.cursor = here;
            return;
        }
        let near = self
            .fight
            .as_ref()
            .unwrap()
            .order
            .iter()
            .filter(|f| !f.out && f.who != who && (f.side == side) == allies)
            .map(|f| self.who_tile(f.who))
            .min_by_key(|t| t.0.abs_diff(here.0).max(t.1.abs_diff(here.1)));
        let c = self.fight.as_mut().unwrap();
        c.aim = aim;
        if let Some(t) = near {
            c.cursor = t;
        }
    }

    /// The fighter standing on tile `t`.
    pub(in crate::runtime) fn who_at(&self, t: TilePos) -> Option<Who> {
        let c = self.fight.as_ref()?;
        c.order.iter().filter(|f| !f.out).map(|f| f.who).find(|w| self.who_space(*w).contains(&t))
    }

    /// Does the targeted act `aim` on `target`; Ok once done (the action spent).
    pub(super) fn act_on(&mut self, aim: &Aim, target: Who) -> Result<(), &'static str> {
        let c = self.fight.as_ref().ok_or("No fight")?;
        let me = c.current().clone();
        let foe = c.fighter(target).is_some_and(|f| f.side != me.side);
        let dist = self.who_dist(me.who, target);
        match aim {
            Aim::Move => Err("Choose an action"),
            Aim::Attack => {
                self.act_ready(TurnAct::Attack)?;
                if !foe {
                    return Err("Not a foe");
                }
                // The first attack that reaches (melee next to the target, else a shot); a weapon
                // that can be thrown only when Attack › Throw chose it.
                let throw = self.fight.as_ref().is_some_and(|c| c.attack_mode == Some(true));
                let specs: Vec<_> = self.attack_specs(me.who).into_iter().filter(|s| if throw { s.thrown } else { !s.thrown }).collect();
                let spec = specs.iter().filter(|s| self.can_attack(me.who, target, s).is_ok()).min_by_key(|s| if dist <= 1 { s.ranged as i32 } else { !s.ranged as i32 }).cloned();
                let Some(spec) = spec else { return Err(specs.first().map_or("No attack", |s| self.can_attack(me.who, target, s).err().unwrap_or("Out of reach"))) };
                if !self.pay_ammo(me.who, &spec) {
                    return Err("Out of ammunition");
                }
                self.make_attack(me.who, target, &spec);
                let c = self.fight.as_mut().unwrap();
                c.attack_mode = None;
                c.turn.attacked = true;
                // (A loading weapon: one attack with it per action.)
                // (A net: one attack with it per action too.)
                c.turn.attacks_left = if spec.loading || spec.net { 0 } else { c.turn.attacks_left.saturating_sub(1) };
                c.turn.action = true;
                Ok(())
            }
            Aim::OffHand => {
                self.act_ready(TurnAct::OffHand)?;
                let w = self.offhand_weapon(me.who).ok_or("No off-hand weapon")?;
                let spec = self.hero_spec(&w, true);
                self.can_attack(me.who, target, &spec)?;
                self.make_attack(me.who, target, &spec);
                self.fight.as_mut().unwrap().turn.bonus = true;
                Ok(())
            }
            Aim::Help => {
                if self.fight.as_ref().unwrap().turn.action {
                    return Err("Action used");
                }
                // An ally at 0 HP next to it: first aid (a healer's kit, else Medicine DC 10).
                let down = self.fight.as_ref().unwrap().fighter(target).is_some_and(|f| f.down && !f.saves.stable);
                if !foe && down && dist <= 1 {
                    self.first_aid(me.who, target);
                    self.fight.as_mut().unwrap().turn.action = true;
                    return Ok(());
                }
                if !foe || dist > 1 {
                    return Err("Help against a foe next to you");
                }
                let name = self.who_name(me.who);
                let c = self.fight.as_mut().unwrap();
                c.helped.push(target);
                c.turn.action = true;
                c.log(format!("{name}: {}", crate::lang::tr_ui("helps")));
                Ok(())
            }
            Aim::Grapple | Aim::Shove => {
                // Grapple on the one it holds: lets go (free).
                if *aim == Aim::Grapple && self.fight.as_ref().unwrap().fighter(me.who).is_some_and(|f| f.grappling == Some(target)) {
                    self.release_grapple(me.who);
                    return Ok(());
                }
                if self.fight.as_ref().unwrap().turn.action {
                    return Err("Action used");
                }
                if !foe || dist > 1 {
                    return Err("Out of reach");
                }
                // At most one size larger than the grappler or shover.
                if !crate::srd::combat::can_grapple(self.who_size(me.who), self.who_size(target)) {
                    return Err("Too big");
                }
                // Athletics against the target's Athletics or Acrobatics (its better one).
                let theirs = self.contest_roll(target);
                let ok = self.who_check(me.who, "athletics", theirs + 1, false).is_some_and(|r| r.success == Some(true));
                self.fight.as_mut().unwrap().turn.action = true;
                let name = self.who_name(me.who);
                if !ok {
                    self.fight_log(format!("{name}: {}", crate::lang::tr_ui("fails")));
                    return Ok(());
                }
                if *aim == Aim::Grapple {
                    let tag = self.who_tag(me.who);
                    if let Some(l) = self.who_conditions_mut(target) {
                        conditions::apply(l, &ConditionSpec::new(Condition::Grappled, 0.0));
                        conditions::set_origin(l, Condition::Grappled, &tag, 0);
                    }
                    if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(me.who)) {
                        f.grappling = Some(target);
                    }
                    // (Dragging it: the rest of the turn at the speed it has now.)
                    self.refresh_turn_speed(me.who);
                    self.fight_log(format!("{name}: {}", crate::lang::tr_ui("grapples")));
                } else {
                    let push = self.fight.as_ref().is_some_and(|c| c.shove_push);
                    if push {
                        self.push_away(me.who, target);
                        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("shoves")));
                    } else {
                        if let Some(l) = self.who_conditions_mut(target) {
                            conditions::apply(l, &ConditionSpec::new(Condition::Prone, 0.0));
                        }
                        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("knocks prone")));
                    }
                }
                Ok(())
            }
            Aim::Cast(spell, slot) => {
                let scroll = self.fight.as_ref().and_then(|c| c.scroll.clone()).is_some_and(|s| s == *spell);
                let _ = (foe, dist);
                if scroll {
                    let t = self.who_tile(target);
                    if let Some(c) = &mut self.fight {
                        c.cursor = t;
                    }
                    return self.confirm_spell(spell, *slot);
                }
                self.cast_spell_in_fight(spell, *slot, super::CastAt::On(target), false)
            }
            Aim::Feature(id) => self.use_class_act(id, target),
        }
    }

    /// The target's side of a grapple or shove: d20 + its better Athletics or Acrobatics.
    fn contest_roll(&mut self, who: Who) -> i32 {
        let mut best = 0;
        for skill in ["athletics", "acrobatics"] {
            if let Some(r) = self.who_check(who, skill, 0, false) {
                best = best.max(r.total);
                // (A contest's other side: a plain roll in the log, no DC of its own.)
                if let Some(last) = self.world.rolls.last_mut() {
                    (last.success, last.dc) = (None, 0);
                }
            }
        }
        best as i32
    }

    /// Shoves `target` one tile away from `from` (when that tile is free).
    fn push_away(&mut self, from: Who, target: Who) {
        let (a, t) = (self.who_tile(from), self.who_tile(target));
        let to = (t.0 + (t.0 - a.0).signum(), t.1 + (t.1 - a.1).signum());
        // (Its whole space must be free there: walls, and anyone else.)
        let n = self.space_n(target);
        let Some((space, false)) = self.space_at(to, n) else { return };
        if self.fighter_tiles(target).iter().any(|(_, _, p)| space.contains(p)) {
            return;
        }
        let p = if n > 1 { self.space_feet(crate::srd::combat::space_anchor(to, n), n) } else { feet_spot(to, self.ts()) };
        match self.who_index(target) {
            None => self.player.pos = p,
            Some(i) => {
                self.loaded.entities[i].body.pos = p;
                self.occupy(i);
            }
        }
    }

    /// A check's modifier for `who` (`athletics`: its STR and proficiency), without rolling.
    pub(super) fn check_bonus(&self, who: Who, what: &str) -> i64 {
        let Some(spec) = crate::srd::Spec::parse(what, false) else { return 0 };
        if who == Who::Player {
            return spec.modifier(&self.world.roller());
        }
        let Some(block) = self.who_block(who) else { return 0 };
        let profs: std::collections::BTreeSet<String> = block.proficiencies.iter().map(|p| crate::srd::abilities::proficiency_id(p)).collect();
        spec.modifier(&crate::srd::check::Roller { abilities: &block.abilities, proficiencies: &profs, level: block.proficiency_level(), training: None })
    }

    /// What holds `who`: a grapple or restraint with an escape DC (a monster's hit, a net), or a
    /// grapple by a fighter (DC 0: a contest against its Athletics); the condition, the DC and
    /// who holds it.
    pub(super) fn escape_dc(&self, who: Who) -> Option<(Condition, i32, String)> {
        self.who_conditions(who)
            .iter()
            .find(|a| matches!(a.condition, Some(Condition::Grappled | Condition::Restrained)) && (a.escape > 0 || (a.condition == Some(Condition::Grappled) && !a.by.is_empty())))
            .map(|a| (a.condition.unwrap(), a.escape, a.by.clone()))
    }

    /// The size of a combatant: the hero's race's, a stat block's (Medium when unknown).
    pub(super) fn who_size(&self, who: Who) -> crate::srd::Size {
        match who {
            Who::Player if self.world.shape.is_none() => self.player_race().and_then(|r| crate::srd::Size::parse(&r.size_category)).unwrap_or_default(),
            w => self.who_block(w).map_or(crate::srd::Size::Medium, |b| b.size),
        }
    }

    /// `who` lets go of the creature it grapples (free).
    pub(super) fn release_grapple(&mut self, who: Who) {
        let Some(held) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)).and_then(|f| f.grappling.take()) else { return };
        let tag = self.who_tag(who);
        if let Some(l) = self.who_conditions_mut(held) {
            if conditions::origin_of(l, Condition::Grappled) == tag {
                conditions::remove(l, Condition::Grappled);
            }
        }
        let name = self.who_name(who);
        self.fight_log(format!("{name} {}", crate::lang::tr_ui("lets go")));
        self.refresh_turn_speed(who);
    }

    /// The current turn's speed after it changed mid-turn (a grapple taken or let go).
    fn refresh_turn_speed(&mut self, who: Who) {
        let speed = self.who_speed(who);
        if let Some(c) = self.fight.as_mut().filter(|c| c.current().who == who) {
            c.turn.speed = speed;
        }
    }

    /// The Escape action: Athletics or Acrobatics (the better one) against the escape DC; the
    /// grapple or restraint ends on a success.
    pub(super) fn escape(&mut self, who: Who) {
        let Some((c, dc, by)) = self.escape_dc(who) else { return };
        let skill = if self.check_bonus(who, "acrobatics") > self.check_bonus(who, "athletics") { "acrobatics" } else { "athletics" };
        let name = self.who_name(who);
        let holder = self.who_of_tag(&by);
        // (No DC: a contest against the grappler's Athletics.)
        let dc = if dc > 0 {
            dc
        } else {
            let theirs = holder.and_then(|h| self.who_check(h, "athletics", 0, false)).map_or(10, |r| r.total as i32);
            if let Some(last) = self.world.rolls.last_mut() {
                (last.success, last.dc) = (None, 0);
            }
            theirs + 1
        };
        let ok = self.who_check(who, skill, dc, false).is_some_and(|r| r.success == Some(true));
        if ok {
            if let Some(l) = self.who_conditions_mut(who) {
                conditions::remove(l, c);
            }
            if let Some(f) = holder.and_then(|h| self.fight.as_mut().and_then(|f| f.fighter_mut(h))) {
                if f.grappling == Some(who) {
                    f.grappling = None;
                }
            }
            self.fight_log(format!("{name} {}", crate::lang::tr_ui("escapes")));
        } else {
            self.fight_log(format!("{name} {}", crate::lang::tr_ui("can't escape")));
        }
    }

    /// First aid for a fighter at 0 HP: the hero's healer's kit makes it stable (a use spent),
    /// else a Medicine check (DC 10) does on a success.
    fn first_aid(&mut self, who: Who, target: Who) {
        let name = self.who_name(who);
        if who == Who::Player {
            let kit = self.world.inventory.item_ids().into_iter().find(|id| self.project.db.item(id).is_some_and(|d| d.srd == "healers-kit" && d.max_charges() > 0) && self.charges_left(Who::Player, id).unwrap_or(0) > 0);
            if let Some(kit) = kit {
                if self.spend_charges(Who::Player, &kit, 1).is_ok() {
                    let what = self.project.db.item(&kit).map_or(kit.clone(), |d| crate::lang::tr(&d.name));
                    self.stabilize(target, &format!("{name}: {what}"));
                    return;
                }
            }
        }
        let ok = self.who_check(who, "medicine", 10, false).is_some_and(|r| r.success == Some(true));
        if ok {
            self.stabilize(target, &format!("{name}: {}", crate::lang::tr_ui("Medicine")));
        } else {
            self.fight_log(format!("{name}: {} {}", crate::lang::tr_ui("Medicine"), crate::lang::tr_ui("failed")));
        }
    }

    /// A fighter at 0 HP becomes stable: no more death saves (Spare the Dying, first aid).
    pub(super) fn stabilize(&mut self, target: Who, how: &str) {
        let tname = self.who_name(target);
        let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(target)).filter(|f| f.down && !f.out) else { return };
        f.saves.stable = true;
        let line = if how.is_empty() { format!("{tname} {}", crate::lang::tr_ui("is stable")) } else { format!("{how}: {tname} {}", crate::lang::tr_ui("is stable")) };
        self.fight_log(line);
    }

    /// Ammunition for a ranged attack of the hero (`false` = none left).
    fn pay_ammo(&mut self, who: Who, spec: &super::AttackSpec) -> bool {
        if who != Who::Player || !spec.ranged {
            return true;
        }
        let Some(r) = self.attack_weapon().ranged else { return true };
        if r.ammo.is_empty() {
            return true;
        }
        self.spend_ammo(&r.ammo)
    }

    /// The hero's off-hand weapon (a weapon in the off-hand slot).
    pub(super) fn offhand_weapon(&self, who: Who) -> Option<crate::defs::Weapon> {
        if who != Who::Player {
            return None;
        }
        let id = self.world.equipment.item(crate::defs::EquipSlot::Offhand)?;
        self.project.db.item(id).and_then(|d| d.weapon.clone())
    }

    /// The hero used an item from the inventory during a fight: on their own turn it takes the
    /// action (`Err` = not now).
    pub(in crate::runtime) fn fight_item_use(&mut self) -> Result<(), &'static str> {
        let Some(c) = &self.fight else { return Ok(()) };
        if c.current().who != Who::Player || c.current().ai {
            return Err("Not your turn");
        }
        if c.turn.action {
            return Err("Action used");
        }
        self.fight.as_mut().unwrap().turn.action = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TurnAct;

    /// Autoplay names every entry of the action bar by its id; End turn is the last entry.
    #[test]
    fn the_action_bar_parses_its_ids() {
        for a in TurnAct::BAR {
            assert_eq!(TurnAct::parse(a.id()), Some(a));
        }
        assert_eq!((TurnAct::parse("DASH"), TurnAct::parse("fly")), (Some(TurnAct::Dash), None));
        assert_eq!(TurnAct::BAR.last(), Some(&TurnAct::EndTurn));
    }
}
