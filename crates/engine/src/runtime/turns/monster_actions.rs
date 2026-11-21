//! Monsters' special abilities in fights (`StatBlock::actions` / `legendary` / `regeneration` /
//! `pack_tactics` / `legendary_resistance`): save actions (a dragon's breath: an area, a save,
//! damage, a condition) run as a spell of their own through `resolve_spell`, recharge on a d6 at
//! the start of the monster's turns or count uses a day; legendary actions at the end of other
//! creatures' turns; Legendary Resistance turns failed saves; Regeneration heals at the start of
//! its turns unless fire (or what stops it) hit it, and keeps a troll from dying at 0 hit points;
//! Pack Tactics gives advantage next to an ally.

use std::collections::BTreeMap;

use super::{CastAt, Game, Side, Who};
use crate::skills::{Shape, SpellCast, SpellDef, SpellPart, SpellRange};
use crate::srd::conditions;
use crate::srd::StatAction;

/// A monster's fight state: spent recharge actions, uses, legendary points, Legendary
/// Resistance used, regeneration.
#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct MonsterFx {
    /// Recharge actions used (back on a d6 at the start of its turns).
    pub spent: Vec<String>,
    /// Uses of actions with uses a day (and once-a-fight actions without damage).
    pub used: BTreeMap<String, u32>,
    pub legendary_left: u32,
    /// Its legendary points were given (at its first turn, or the first chance to use them).
    pub legendary_set: bool,
    pub resist_used: u32,
    /// It took what stops its regeneration since its last turn.
    pub regen_blocked: bool,
    /// At 0 hit points, waiting to regenerate at the start of its turn (a troll).
    pub regen_down: bool,
}

impl Game {
    fn mon(&self, who: Who) -> Option<&MonsterFx> {
        self.fight.as_ref().and_then(|c| c.fighter(who)).map(|f| &f.mon)
    }

    fn mon_mut(&mut self, who: Who) -> Option<&mut MonsterFx> {
        self.fight.as_mut().and_then(|c| c.fighter_mut(who)).map(|f| &mut f.mon)
    }

    /// The spell a save action is made as: its area or target, the save, the damage, the
    /// condition (the DC comes from `action_dc` while it resolves).
    fn action_spell(a: &StatAction) -> SpellDef {
        let mut d = SpellDef::fresh(&format!("action:{}", a.name));
        d.name = a.name.clone();
        d.level = 0;
        d.casting = SpellCast::Action;
        d.components = String::new();
        d.area = a.area;
        d.range = match a.area.map(|x| x.shape) {
            Some(Shape::Cone | Shape::Line) => SpellRange::Caster,
            Some(_) if a.around => SpellRange::Caster,
            _ => SpellRange::Feet(a.range_ft.max(5)),
        };
        d.duration_rounds = a.rounds;
        d.duration = String::new();
        d.parts = vec![SpellPart::Save { ability: a.save, half: a.half }];
        if !a.dice.is_empty() {
            d.parts.push(SpellPart::Damage { by_level: vec![(0, a.dice.clone())], damage_type: a.damage_type });
        }
        if let Some(c) = a.condition {
            d.parts.push(SpellPart::Condition { condition: c, on_fail: true });
        }
        d
    }

    /// Whether `who` can use save action `a` now: recharged, uses left (a condition-only one
    /// without recharge once a fight: those who saved are safe from it).
    fn action_ready(&self, who: Who, a: &StatAction) -> bool {
        let Some(m) = self.mon(who) else { return false };
        if m.spent.contains(&a.name) {
            return false;
        }
        let used = m.used.get(&a.name).copied().unwrap_or(0);
        if a.per_day > 0 {
            return used < a.per_day;
        }
        if a.recharge == 0 && a.dice.is_empty() {
            return used == 0;
        }
        true
    }

    /// The point a save action aims at that catches the most foes and none of `who`'s side
    /// (`None` when it catches no foe).
    fn action_aim(&self, who: Who, side: Side, a: &StatAction) -> Option<(CastAt, usize)> {
        let d = Self::action_spell(a);
        let c = self.fight.as_ref()?;
        let order: Vec<(Who, Side, bool)> = c.order.iter().filter(|f| !f.out).map(|f| (f.who, f.side, f.down)).collect();
        let points: Vec<CastAt> = if a.around { vec![CastAt::Tile(self.who_tile(who))] } else { order.iter().filter(|(w, s, down)| *s != side && !*down && self.can_see(who, *w)).map(|(w, _, _)| CastAt::On(*w)).collect() };
        let mut best: Option<(CastAt, usize)> = None;
        for at in points {
            let Ok((hit, _)) = self.spell_targets(who, &d, 0, at) else { continue };
            let foes = hit.iter().filter(|w| order.iter().any(|(o, s, down)| o == *w && *s != side && !*down)).count();
            let allies = hit.iter().filter(|w| order.iter().any(|(o, s, _)| o == *w && *s == side)).count();
            if foes > 0 && allies == 0 && best.as_ref().is_none_or(|b| foes > b.1) {
                best = Some((at, foes));
            }
        }
        best
    }

    /// Uses save action `a` of `who` at `at`: the targets save, the damage and condition land, the
    /// action is spent (recharge) or counted (uses).
    pub(super) fn use_action(&mut self, who: Who, a: &StatAction, at: CastAt) {
        let d = Self::action_spell(a);
        let Ok((targets, point)) = self.spell_targets(who, &d, 0, at) else { return };
        let name = self.who_name(who);
        self.fight_log(format!("{name}: {}", crate::lang::tr(&a.name)));
        self.action_dc = Some(a.dc);
        self.resolve_spell(who, &d, 0, &targets, Some(point));
        self.action_dc = None;
        if let Some(m) = self.mon_mut(who) {
            if a.recharge > 0 {
                m.spent.push(a.name.clone());
            }
            *m.used.entry(a.name.clone()).or_default() += 1;
        }
    }

    /// The AI's save action this turn (its action): the best ready one that catches a foe and no
    /// ally, the one catching the most foes first. True when it used one.
    pub(super) fn ai_action_step(&mut self, me: Who, side: Side) -> bool {
        if self.fight.as_ref().is_none_or(|c| c.turn.action) {
            return false;
        }
        let Some(b) = self.who_block(me) else { return false };
        let mut best: Option<(StatAction, CastAt, usize)> = None;
        for a in b.actions.iter().filter(|a| !a.legendary_only && self.action_ready(me, a)) {
            if let Some((at, n)) = self.action_aim(me, side, a) {
                if best.as_ref().is_none_or(|x| n > x.2) {
                    best = Some((a.clone(), at, n));
                }
            }
        }
        let Some((a, at, _)) = best else { return false };
        if let Some(c) = &mut self.fight {
            c.turn.action = true;
        }
        self.use_action(me, &a, at);
        true
    }

    /// The start of a monster's turn: its legendary points back, spent actions recharging on a
    /// d6, regeneration (getting up from 0 hit points, or dying there when fire stopped it).
    pub(super) fn monster_turn_start(&mut self, who: Who) {
        let Some(b) = self.who_block(who).filter(|b| !b.actions.is_empty() || b.legendary_points > 0 || b.regeneration.is_some()) else { return };
        let name = self.who_name(who);
        let spent = self.mon(who).map(|m| m.spent.clone()).unwrap_or_default();
        let mut back = Vec::new();
        for s in &spent {
            let need = b.actions.iter().find(|a| a.name == *s).map_or(7, |a| a.recharge);
            let d6 = crate::srd::dice::die(6);
            if d6 >= need {
                back.push(s.clone());
                self.fight_log(format!("{name}: {} {} ({d6})", crate::lang::tr(s), crate::lang::tr_ui("recharges")));
            }
        }
        if let Some(m) = self.mon_mut(who) {
            m.spent.retain(|s| !back.contains(s));
            (m.legendary_left, m.legendary_set) = (b.legendary_points, true);
        }
        let Some(r) = b.regeneration else { return };
        let (blocked, down) = self.mon(who).map_or((false, false), |m| (m.regen_blocked, m.regen_down));
        if let Some(m) = self.mon_mut(who) {
            m.regen_blocked = false;
        }
        if down {
            if blocked {
                self.dies(who);
                return;
            }
            if let Some(m) = self.mon_mut(who) {
                m.regen_down = false;
            }
            self.combat_heal(who, r.hp);
            self.fight_log(format!("{name} {}", crate::lang::tr_ui("regenerates and gets up")));
        } else if !blocked {
            let (hp, max) = self.who_hp(who);
            if hp > 0 && hp < max {
                self.combat_heal(who, r.hp);
                self.fight_log(format!("{name} {} {}", crate::lang::tr_ui("regenerates"), r.hp.min(max - hp)));
            }
        }
    }

    /// Damage of these types reached `who`: what stops its regeneration stops it.
    pub(super) fn regeneration_hit(&mut self, who: Who, types: &[Option<crate::srd::DamageType>]) {
        let stops = self.who_block(who).and_then(|b| b.regeneration).is_some_and(|r| types.iter().flatten().any(|t| r.stopped_by.contains(t)));
        if stops {
            if let Some(m) = self.mon_mut(who) {
                m.regen_blocked = true;
            }
        }
    }

    /// A monster that regenerates back from 0 hit points (a troll) falls instead of dying. True
    /// when it did.
    pub(super) fn regeneration_falls(&mut self, who: Who) -> bool {
        if !self.who_block(who).and_then(|b| b.regeneration).is_some_and(|r| r.revives) {
            return false;
        }
        let name = self.who_name(who);
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.down = true;
            f.mon.regen_down = true;
        }
        self.fight_log(format!("{name} {}", crate::lang::tr_ui("falls, still twitching")));
        true
    }

    /// A monster at 0 hit points that will get up again (its regeneration not stopped): the
    /// fight isn't over while it lies there.
    pub(super) fn regenerating_down(&self, who: Who) -> bool {
        self.mon(who).is_some_and(|m| m.regen_down && !m.regen_blocked)
    }

    /// At the end of a creature's turn: one legendary action of a legendary monster (not the one
    /// whose turn it was) with points left: an attack on a foe in reach, else a save action that
    /// catches a foe.
    pub(super) fn legendary_step(&mut self, ended: Who) {
        // (Its points from the start of the fight, before its first turn too.)
        let fresh: Vec<Who> = self.fight.as_ref().map(|c| c.order.iter().filter(|f| !f.mon.legendary_set).map(|f| f.who).collect()).unwrap_or_default();
        for w in fresh {
            let points = self.who_block(w).map_or(0, |b| b.legendary_points);
            if let Some(m) = self.mon_mut(w) {
                (m.legendary_left, m.legendary_set) = (points, true);
            }
        }
        let Some(c) = &self.fight else { return };
        let ready: Vec<(Who, Side)> = c.order.iter().filter(|f| f.who != ended && !f.out && !f.down && f.mon.legendary_left > 0).map(|f| (f.who, f.side)).collect();
        for (who, side) in ready {
            if !conditions::can_act(self.who_conditions(who)) {
                continue;
            }
            let Some(b) = self.who_block(who) else { continue };
            let left = self.mon(who).map_or(0, |m| m.legendary_left);
            let foes = self.ai_foes(who, side);
            let mut done = false;
            for l in b.legendary.iter().filter(|l| l.cost.max(1) <= left) {
                if !l.attack.is_empty() {
                    let Some(spec) = self.attack_specs(who).into_iter().find(|s| s.name == l.attack) else { continue };
                    let Some(target) = foes.iter().copied().find(|f| !spec.ranged && self.who_dist(who, *f) <= spec.reach.max(1)) else { continue };
                    let name = self.who_name(who);
                    self.fight_log(format!("{name}: {} ({})", crate::lang::tr(&l.name), crate::lang::tr_ui("legendary action")));
                    self.make_attack(who, target, &spec);
                    done = true;
                } else if let Some(a) = b.actions.iter().find(|a| a.name == l.action) {
                    let Some((at, _)) = self.action_aim(who, side, a) else { continue };
                    self.use_action(who, a, at);
                    done = true;
                }
                if done {
                    if let Some(m) = self.mon_mut(who) {
                        m.legendary_left = m.legendary_left.saturating_sub(l.cost.max(1));
                    }
                    break;
                }
            }
            if done {
                // (One legendary action at the end of each other creature's turn.)
                return;
            }
        }
    }

    /// Legendary Resistance: `who` failed a save; with uses left it succeeds instead. True when
    /// it did.
    pub(super) fn legendary_resist(&mut self, who: Who) -> bool {
        let Some(uses) = self.who_block(who).map(|b| b.legendary_resistance).filter(|u| *u > 0) else { return false };
        if self.mon(who).is_none_or(|m| m.resist_used >= uses) {
            return false;
        }
        if let Some(m) = self.mon_mut(who) {
            m.resist_used += 1;
        }
        let name = self.who_name(who);
        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("Legendary Resistance turns the failed save")));
        true
    }

    /// Pack Tactics: `who` has it and an ally of its that can act stands next to `target`.
    pub(super) fn pack_tactics(&self, who: Who, target: Who) -> bool {
        if !self.who_block(who).is_some_and(|b| b.pack_tactics) {
            return false;
        }
        let Some(c) = &self.fight else { return false };
        let side = c.fighter(who).map(|f| f.side);
        c.order.iter().any(|f| f.who != who && Some(f.side) == side && !f.out && !f.down && conditions::can_act(self.who_conditions(f.who)) && self.who_dist(f.who, target) <= 1)
    }

    /// `monster_fx=` for status: `name:recharge(ACTIONS)|legendary N|resist N|regen(down|stopped)`.
    pub(in crate::runtime) fn monster_fx_status(&self) -> String {
        let Some(c) = &self.fight else { return "-".into() };
        let mut out = Vec::new();
        for f in c.order.iter().filter(|f| !f.out) {
            let Some(b) = self.who_block(f.who) else { continue };
            if b.actions.is_empty() && b.legendary_points == 0 && b.regeneration.is_none() && b.legendary_resistance == 0 {
                continue;
            }
            let mut parts = vec![self.who_key(f.who)];
            if !f.mon.spent.is_empty() {
                parts.push(format!("spent({})", f.mon.spent.iter().map(|s| s.replace(' ', "_")).collect::<Vec<_>>().join("+")));
            }
            if b.legendary_points > 0 {
                parts.push(format!("legendary{}", f.mon.legendary_left));
            }
            if b.legendary_resistance > 0 {
                parts.push(format!("resist{}", b.legendary_resistance.saturating_sub(f.mon.resist_used)));
            }
            if b.regeneration.is_some() {
                parts.push(format!("regen{}{}", if f.mon.regen_down { ":down" } else { "" }, if f.mon.regen_blocked { ":stopped" } else { "" }));
            }
            out.push(parts.join(":"));
        }
        if out.is_empty() {
            "-".into()
        } else {
            out.join(",")
        }
    }
}
