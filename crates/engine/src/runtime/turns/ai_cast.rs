//! The fight AI's spells (characters with a class, monsters with a stat block's spellcasting):
//! which spell to cast, at whom, at which slot. Healing a badly hurt ally first, then an area that
//! catches two foes or more and no ally, then a condition that takes the strongest foe out of the fight (not while
//! concentrating), then single-target damage (a slot only when the target can take it).

use super::casting::CastAt;
use super::{Side, Who};
use crate::runtime::Game;
use crate::skills::{SpellPart, SpellRange};

/// What a candidate cast does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum CastKind {
    Heal,
    Area,
    Condition,
    Damage,
}

/// A spell the AI could cast now at a target.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct Cand {
    pub spell: String,
    pub slot: u32,
    pub kind: CastKind,
    /// The average damage (or healing) at this slot; a condition's strength (`control_weight`).
    pub avg: i32,
    /// Foes / allies an area catches (single targets: 1 / 0).
    pub foes: usize,
    pub allies: usize,
    /// The target's hit points left (damage).
    pub target_hp: i32,
    /// It needs concentration and the caster already concentrates.
    pub blocked: bool,
}

/// Which candidate to cast (an index), by the AI's rules.
pub(in crate::runtime) fn pick_cast(cands: &[Cand]) -> Option<usize> {
    let ok = |c: &Cand| !c.blocked;
    let best = |kind: CastKind, f: &dyn Fn(&Cand) -> bool| {
        // (The strongest; among equals the lowest slot.)
        cands.iter().enumerate().filter(|(_, c)| c.kind == kind && ok(c) && f(c)).max_by_key(|(_, c)| (c.avg, -(c.slot as i32))).map(|(k, _)| k)
    };
    if let Some(k) = best(CastKind::Heal, &|_| true) {
        // (The strongest heal, cast at its lowest slot.)
        let spell = &cands[k].spell;
        return cands.iter().enumerate().filter(|(_, c)| c.kind == CastKind::Heal && &c.spell == spell).min_by_key(|(_, c)| c.slot).map(|(k, _)| k);
    }
    if let Some(k) = cands.iter().enumerate().filter(|(_, c)| c.kind == CastKind::Area && ok(c) && c.foes >= 2 && c.allies == 0).max_by_key(|(_, c)| (c.foes as i32 * c.avg, -(c.slot as i32))).map(|(k, _)| k) {
        return Some(k);
    }
    // A condition that takes a foe out of the fight (its strength in `avg`, see `control_weight`),
    // the strongest at its lowest slot; a weak one (prone…) isn't worth the action.
    if let Some(k) = cands.iter().enumerate().filter(|(_, c)| c.kind == CastKind::Condition && ok(c) && c.avg >= 2).max_by_key(|(_, c)| (c.avg, -(c.slot as i32))).map(|(k, _)| k) {
        return Some(k);
    }
    // Damage: the best cantrip, unless the target can take twice that: then a slot (the lowest
    // one of the strongest such spell).
    let cantrip = best(CastKind::Damage, &|c| c.slot == 0);
    let tough = cantrip.is_none_or(|k| cands.iter().any(|c| c.kind == CastKind::Damage && c.target_hp >= 2 * cands[k].avg));
    if tough {
        if let Some(k) = cands.iter().enumerate().filter(|(_, c)| c.kind == CastKind::Damage && ok(c) && c.slot > 0).max_by_key(|(_, c)| (c.avg, -(c.slot as i32))).map(|(k, _)| k) {
            let spell = &cands[k].spell;
            return cands.iter().enumerate().filter(|(_, c)| c.kind == CastKind::Damage && &c.spell == spell && ok(c)).min_by_key(|(_, c)| c.slot).map(|(k, _)| k);
        }
    }
    cantrip
}

/// How much a condition takes a foe out of the fight: 3 = it can't act (paralyzed, stunned…),
/// 2 = it fights badly (restrained, blinded, frightened…), 1 = hardly (prone, poisoned…).
fn control_weight(c: crate::srd::Condition) -> i32 {
    use crate::srd::Condition::*;
    match c {
        Paralyzed | Stunned | Unconscious | Petrified | Incapacitated => 3,
        Restrained | Blinded | Frightened | Charmed => 2,
        _ => 1,
    }
}

/// The average of a dice expression ("2d6+3" → 10).
fn average(dice: &str) -> i32 {
    crate::srd::Expr::parse(dice).map_or(0, |e| {
        let (lo, hi) = e.range(&crate::srd::dice::NoMods);
        ((lo + hi) / 2) as i32
    })
}

impl Game {
    /// The AI's spell this step, cast if one fits (true = cast).
    pub(super) fn ai_cast_step(&mut self, me: Who, side: Side) -> bool {
        let Some(c) = &self.fight else { return false };
        if c.turn.action {
            return false;
        }
        let options = self.cast_options(me);
        if options.is_empty() {
            return false;
        }
        let order: Vec<(Who, Side, bool)> = c.order.iter().filter(|f| !f.out).map(|f| (f.who, f.side, f.down)).collect();
        let concentrating = self.concentration_of(me).is_some();
        let mut cands: Vec<(Cand, CastAt)> = Vec::new();
        for (id, slot) in options {
            let Some(d) = self.project.db.spell(&id).cloned() else { continue };
            if d.casting != crate::skills::SpellCast::Action {
                continue;
            }
            let lvl = self.scale_level(me, &d, slot);
            let avg_of = |g: &Game, p: &[(u32, String)]| crate::skills::dice_at(p, lvl).map_or(0, |x| average(&g.spell_dice(me, x)));
            let damage = d.parts.iter().find_map(|p| if let SpellPart::Damage { by_level, .. } = p { Some(avg_of(self, by_level)) } else { None });
            let heal = d.parts.iter().find_map(|p| if let SpellPart::Heal { by_level } = p { Some(avg_of(self, by_level)) } else { None });
            let condition = d.parts.iter().find_map(|p| if let SpellPart::Condition { condition, on_fail: true } = p { Some(*condition) } else { None });
            let blocked = d.concentration && concentrating;
            let range = if d.range == SpellRange::Caster { 0 } else { d.range.tiles().max(1) };
            let in_range = |g: &Game, w: Who| g.who_dist(me, w) <= range.max(1);
            let base = Cand { spell: id.clone(), slot, kind: CastKind::Damage, avg: 0, foes: 1, allies: 0, target_hp: 0, blocked };
            if let (Some(h), true) = (heal, d.heals()) {
                // The most hurt ally in range that is down or below a third of its hit points.
                let hurt = order.iter().filter(|(w, s, _)| *s == side && in_range(self, *w)).filter(|(w, _, down)| {
                    let (hp, max) = self.who_hp(*w);
                    *down || (max > 0 && hp * 3 < max)
                });
                if let Some((w, _, _)) = hurt.min_by_key(|(w, _, _)| self.who_hp(*w).0) {
                    cands.push((Cand { kind: CastKind::Heal, avg: h, ..base.clone() }, CastAt::On(*w)));
                }
                continue;
            }
            // (Foes it sees, with a line to them: not behind total cover.)
            let foes: Vec<Who> = order
                .iter()
                .filter(|(w, s, down)| *s != side && !*down && self.can_see(me, *w) && self.who_dist(me, *w) <= range.max(1) + if d.area.is_some() { 4 } else { 0 } && self.cover(me, *w) != crate::srd::combat::Cover::Total)
                .map(|x| x.0)
                .collect();
            if foes.is_empty() {
                continue;
            }
            if d.area.is_some() {
                // The point that catches the most foes and none of its own side.
                let mut best: Option<(Cand, CastAt)> = None;
                for f in &foes {
                    let at = CastAt::Tile(self.who_tile(*f));
                    let Ok((hit, _)) = self.spell_targets(me, &d, slot, at) else { continue };
                    let nf = hit.iter().filter(|w| order.iter().any(|(o, s, _)| o == *w && *s != side)).count();
                    let na = hit.iter().filter(|w| order.iter().any(|(o, s, _)| o == *w && *s == side)).count();
                    if best.as_ref().is_none_or(|b| nf > b.0.foes || (nf == b.0.foes && na < b.0.allies)) {
                        best = Some((Cand { kind: CastKind::Area, avg: damage.unwrap_or(0), foes: nf, allies: na, ..base.clone() }, at));
                    }
                }
                cands.extend(best);
                continue;
            }
            let targets: Vec<Who> = foes.into_iter().filter(|w| in_range(self, *w)).collect();
            if let (Some(cond), None) = (condition, damage) {
                // The strongest foe in range without the condition yet.
                if let Some(t) = targets.iter().copied().filter(|w| !crate::srd::conditions::has(self.who_conditions(*w), cond)).max_by_key(|w| self.who_hp(*w).1) {
                    cands.push((Cand { kind: CastKind::Condition, avg: control_weight(cond), ..base.clone() }, CastAt::On(t)));
                }
                continue;
            }
            if let Some(avg) = damage {
                // The most hurt foe in range (to finish it), else the nearest.
                if let Some(t) = targets.iter().copied().min_by_key(|w| (self.who_hp(*w).0, self.who_dist(me, *w))) {
                    cands.push((Cand { kind: CastKind::Damage, avg, target_hp: self.who_hp(t).0, ..base.clone() }, CastAt::On(t)));
                }
            }
        }
        let list: Vec<Cand> = cands.iter().map(|c| c.0.clone()).collect();
        let Some(k) = pick_cast(&list) else { return false };
        let (cand, at) = cands.swap_remove(k);
        self.cast_spell_in_fight(&cand.spell, cand.slot, at, false).is_ok()
    }

    /// Whether `who` has the goblin's *Nimble Escape* (Disengage or Hide as a bonus action).
    pub(super) fn nimble(&self, who: Who) -> bool {
        self.who_block(who).is_some_and(|b| b.features.iter().any(|f| f.name.eq_ignore_ascii_case("Nimble Escape")))
    }

    /// Shove or grapple instead of attacking `target` when it pays (true = done, the action
    /// spent): a shove prone when an ally of its side is next to the target too (their melee
    /// attacks get advantage); a grapple on a caster or a shooter, with a free hand. Only when its
    /// Athletics is at least the target's better Athletics or Acrobatics, the target small enough,
    /// once per target and round.
    pub(super) fn ai_wrestle(&mut self, me: Who, side: Side, target: Who) -> bool {
        let Some(c) = &self.fight else { return false };
        let round = c.round;
        if c.ai_wrestled.contains(&(target, round)) || !crate::srd::combat::can_grapple(self.who_size(me), self.who_size(target)) {
            return false;
        }
        let mine = self.check_bonus(me, "athletics");
        let theirs = self.check_bonus(target, "athletics").max(self.check_bonus(target, "acrobatics"));
        if mine < theirs {
            return false;
        }
        let conds = self.who_conditions(target);
        let ally_next = c.order.iter().any(|a| a.side == side && a.who != me && !a.out && !a.down && self.who_dist(a.who, target) <= 1);
        let free_hand = self.who_index(me).is_none_or(|i| self.loaded.entities[i].equipment.item(crate::defs::EquipSlot::Offhand).is_none());
        let caster_or_shooter = !self.cast_options(target).is_empty() || self.attack_specs(target).iter().all(|s| s.ranged);
        let aim = if ally_next && !crate::srd::conditions::has(conds, crate::srd::Condition::Prone) {
            super::Aim::Shove
        } else if free_hand && caster_or_shooter && !crate::srd::conditions::has(conds, crate::srd::Condition::Grappled) && c.fighter(me).is_some_and(|f| f.grappling.is_none()) {
            super::Aim::Grapple
        } else {
            return false;
        };
        if let Some(c) = &mut self.fight {
            c.ai_wrestled.push((target, round));
            c.shove_push = false;
        }
        self.act_on(&aim, target).is_ok()
    }

    /// Nimble Escape's Hide as the bonus action, when it is out of plain sight (obscured or behind
    /// three-quarters cover from every foe): Stealth against the foes' best passive Perception.
    pub(super) fn ai_hide_bonus(&mut self, me: Who, side: Side) {
        let foes: Vec<Who> = self.fight.as_ref().map_or(Vec::new(), |c| c.order.iter().filter(|f| f.side != side && !f.out && !f.down).map(|f| f.who).collect());
        if foes.iter().any(|f| self.sees(*f, me) == super::vision::Sight::Clear && self.cover(*f, me) < crate::srd::combat::Cover::ThreeQuarters) {
            return;
        }
        let dc = self.passive_perception(side);
        let ok = self.who_check(me, "stealth", dc, false).is_some_and(|r| r.success == Some(true));
        if let Some(c) = &mut self.fight {
            c.turn.bonus = true;
            if let Some(f) = c.fighter_mut(me) {
                f.hidden = ok;
            }
        }
        let name = self.who_name(me);
        self.fight_log(format!("{name}: {}", crate::lang::tr_ui(if ok { "hides" } else { "fails to hide" })));
    }

    /// Whether the AI's offensive spells reach `tiles` away (a caster keeps its distance).
    pub(super) fn ai_spell_range(&self, who: Who) -> i32 {
        self.cast_options(who).iter().filter_map(|(id, _)| self.project.db.spell(id)).filter(|d| d.offensive() && d.range != SpellRange::Caster).map(|d| d.range.tiles()).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(spell: &str, slot: u32, kind: CastKind, avg: i32) -> Cand {
        Cand { spell: spell.into(), slot, kind, avg, foes: 1, allies: 0, target_hp: 9, blocked: false }
    }

    #[test]
    fn the_ai_heals_first_then_areas_then_damage() {
        let heal = cand("cure-wounds", 1, CastKind::Heal, 7);
        let heal2 = cand("cure-wounds", 2, CastKind::Heal, 12);
        let bolt = cand("fire-bolt", 0, CastKind::Damage, 5);
        let missile = cand("magic-missile", 1, CastKind::Damage, 10);
        let ball = Cand { foes: 3, ..cand("fireball", 3, CastKind::Area, 28) };
        // A hurt ally: the heal at its lowest slot.
        assert_eq!(pick_cast(&[bolt.clone(), heal2.clone(), heal.clone()]), Some(2));
        // Three foes in a Fireball and no ally: the area.
        assert_eq!(pick_cast(&[bolt.clone(), missile.clone(), ball.clone()]), Some(2));
        // An ally in it: not.
        assert_eq!(pick_cast(&[bolt.clone(), Cand { allies: 1, ..ball.clone() }]), Some(0));
        // A weak target: the cantrip; a tough one (twice the cantrip's damage): the slot.
        assert_eq!(pick_cast(&[bolt.clone(), missile.clone()]), Some(0));
        assert_eq!(pick_cast(&[Cand { target_hp: 30, ..bolt.clone() }, Cand { target_hp: 30, ..missile.clone() }]), Some(1));
        // A strong condition before damage, the strongest first; a weak one (prone) never.
        let hold = cand("hold-person", 2, CastKind::Condition, control_weight(crate::srd::Condition::Paralyzed));
        let command = cand("command", 1, CastKind::Condition, control_weight(crate::srd::Condition::Prone));
        assert_eq!(pick_cast(&[bolt.clone(), command.clone(), hold.clone()]), Some(2));
        assert_eq!(pick_cast(&[bolt.clone(), command]), Some(0));
        // Concentrating already: no new concentration spell.
        assert_eq!(pick_cast(&[Cand { blocked: true, ..hold }, bolt]), Some(1));
    }
}
