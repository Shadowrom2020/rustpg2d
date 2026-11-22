//! Class features that act in fights: resources (rages, ki, channel divinity…) and their rests,
//! the class actions of the action bar's *Class* entry ([`crate::classes::ActEffect`]), and the
//! riders and reactions that work by themselves ([`crate::classes::FeatureEffect`]: Sneak Attack,
//! Colossus Slayer, Stunning Strike, Uncanny Dodge, Deflect Missiles, Relentless Rage, Survivor,
//! Indomitable, Retaliation, Dark One's Blessing). Reactions are taken automatically while the
//! reaction is free (a prompt comes with 0.32).

use std::collections::BTreeMap;

use super::{AttackSpec, Side, Who};
use crate::classes::{ActCost, ActEffect, ActTarget, ClassAct, ClassDef, ClassTraits, FeatureEffect, Recharge};
use crate::runtime::Game;
use crate::srd::conditions::{self, Condition, ConditionSpec};
use crate::srd::{Ability, DamageType, Mode};

/// Rage lasts a minute: 10 rounds.
const RAGE_ROUNDS: u32 = 10;

/// What a fighter's class features keep going in a fight (not saved).
#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct ClassFx {
    /// Rounds of rage left (0 = not raging).
    pub rage: u32,
    /// It attacked or was hurt since its last turn (the rage goes on).
    pub rage_kept: bool,
    /// It made a Frenzy attack in this rage (exhaustion when it ends).
    pub frenzied: bool,
    /// Reckless Attack until its next turn.
    pub reckless: bool,
    /// Quickened Spell: the next spell of an action this turn takes the bonus action.
    pub quicken: bool,
    /// A Bardic Inspiration die it holds (its sides; 0 = none).
    pub inspiration: u32,
    /// Sacred Weapon: the attack bonus and the rounds left.
    pub weapon_buff: (i32, u32),
    pub temp_hp: i32,
}

/// Rounds as condition seconds: a condition given on someone's turn lasts through `rounds` of
/// the target's turns (they tick at the start of each).
fn rounds_secs(rounds: u32) -> f32 {
    (rounds.max(1) + 1) as f32 * conditions::ROUND - 0.5
}

impl Game {
    // -------------------------------------------------------------------- who has what

    /// A combatant's class definition (the hero's, a party member's).
    pub(super) fn who_classdef(&self, who: Who) -> Option<&ClassDef> {
        match who {
            Who::Player => self.hero_class(),
            Who::Npc(uid) => self.project.db.class(&self.member_class(uid)?.0.class),
        }
    }

    /// An ability modifier of a combatant.
    pub(in crate::runtime) fn who_mod(&self, who: Who, a: Ability) -> i32 {
        match who {
            Who::Player => self.world.abilities.modifier(a),
            w => self.who_block(w).map_or(0, |b| b.abilities.modifier(a)),
        }
    }

    /// A combatant's proficiency bonus.
    pub(super) fn who_prof(&self, who: Who) -> i32 {
        match who {
            Who::Player => crate::srd::abilities::proficiency_bonus(self.world.player_level()),
            w => self.who_block(w).map_or(2, |b| crate::srd::abilities::proficiency_bonus(b.proficiency_level())),
        }
    }

    /// A feature's save DC: 8 + proficiency + `a`'s modifier.
    fn feature_dc(&self, who: Who, a: Ability) -> i32 {
        8 + self.who_prof(who) + self.who_mod(who, a)
    }

    /// The class actions a combatant has.
    pub(in crate::runtime) fn class_acts(&self, who: Who) -> Vec<ClassAct> {
        self.who_traits(who).map_or(Vec::new(), |t| t.acts)
    }

    pub(super) fn who_rider<T>(&self, who: Who, f: impl Fn(&FeatureEffect) -> Option<T>) -> Option<T> {
        self.who_traits(who)?.rider(f)
    }

    pub(super) fn fx(&self, who: Who) -> ClassFx {
        self.fight.as_ref().and_then(|c| c.fighter(who)).map_or(Default::default(), |f| f.fx.clone())
    }

    fn fx_mut(&mut self, who: Who) -> Option<&mut ClassFx> {
        self.fight.as_mut()?.fighter_mut(who).map(|f| &mut f.fx)
    }

    /// Whether `who` may still use its once-per-turn feature `what` this turn (and marks it used).
    fn once(&mut self, who: Who, what: &'static str) -> bool {
        let Some(c) = &mut self.fight else { return false };
        if c.once.contains(&(who, what)) {
            return false;
        }
        c.once.push((who, what));
        true
    }

    // ----------------------------------------------------------------------- resources

    /// The uses spent of a combatant's resources (the hero's, a party member's saved ones).
    fn spent_of(&self, who: Who) -> BTreeMap<String, i32> {
        match who {
            Who::Player => self.world.spent.clone(),
            Who::Npc(uid) => self.member_save(uid).map_or_else(BTreeMap::new, |k| self.world.companions[k].spent.clone()),
        }
    }

    fn spent_mut(&mut self, who: Who) -> Option<&mut BTreeMap<String, i32>> {
        match who {
            Who::Player => Some(&mut self.world.spent),
            Who::Npc(uid) => {
                let k = self.member_save(uid)?;
                Some(&mut self.world.companions[k].spent)
            }
        }
    }

    /// The saved entry of party member `uid` (its index in `world.companions`).
    fn member_save(&self, uid: u32) -> Option<usize> {
        let l = self.loaded.entities.iter().find(|e| e.uid == uid)?.companion.as_ref()?;
        self.world.companions.iter().position(|c| c.uid == l.uid && c.map == l.map)
    }

    /// Uses of resource `id` left and the most (None = it has no such resource).
    pub(in crate::runtime) fn uses(&self, who: Who, id: &str) -> Option<(i32, i32)> {
        let t = self.who_traits(who)?;
        let max = t.resource_max(self.who_classdef(who)?, id, &self.who_abilities(who))?;
        let spent = self.spent_of(who).get(id).copied().unwrap_or(0);
        Some(((max - spent).max(0), max))
    }

    pub(super) fn who_abilities(&self, who: Who) -> crate::srd::Abilities {
        match who {
            Who::Player => self.world.abilities,
            w => self.who_block(w).map(|b| b.abilities).unwrap_or_default(),
        }
    }

    fn spend(&mut self, who: Who, id: &str, n: i32) {
        if let Some(m) = self.spent_mut(who) {
            *m.entry(id.to_string()).or_insert(0) += n;
        }
    }

    /// A rest gives back the resources: a short rest those that recharge on one (and Relentless
    /// Rage's count), a long rest all of them; the hero's and the party's.
    pub(in crate::runtime) fn rest_resources(&mut self, long: bool) {
        let keep = |t: Option<ClassTraits>, m: &mut BTreeMap<String, i32>| {
            if long {
                m.clear();
            } else {
                m.retain(|id, _| id != "relentless-rage" && t.as_ref().is_none_or(|t| t.recharge(id) != Recharge::ShortRest));
            }
        };
        let t = self.hero_traits();
        keep(t, &mut self.world.spent);
        let members: Vec<(u32, usize)> = self.loaded.entities.iter().filter(|e| e.companion.is_some()).filter_map(|e| Some((e.uid, self.member_save(e.uid)?))).collect();
        let on_map: Vec<usize> = members.iter().map(|m| m.1).collect();
        for (uid, k) in members {
            let t = self.who_traits(Who::Npc(uid));
            keep(t, &mut self.world.companions[k].spent);
        }
        // (Party members elsewhere: a long rest still counts for them.)
        if long {
            for (k, c) in self.world.companions.iter_mut().enumerate() {
                if !on_map.contains(&k) {
                    c.spent.clear();
                }
            }
        }
    }

    /// The hero's resources and their uses left (scripts' `resource()`).
    pub(in crate::runtime) fn hero_uses(&self) -> Vec<(String, i64)> {
        let Some(t) = self.hero_traits() else { return Vec::new() };
        t.resource_ids().into_iter().filter_map(|id| Some((id.clone(), self.uses(Who::Player, &id)?.0 as i64))).collect()
    }

    /// `resources=` for status: `who:id:left/max` of the hero and the party on the map.
    pub(in crate::runtime) fn resources_status(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut whos = vec![Who::Player];
        whos.extend(self.levelling_members().into_iter().map(Who::Npc));
        for w in whos {
            let Some(t) = self.who_traits(w) else { continue };
            let key = if w == Who::Player { "player".to_string() } else { self.who_key(w) };
            for id in t.resource_ids() {
                if let Some((left, max)) = self.uses(w, &id) {
                    out.push(format!("{key}:{id}:{left}/{max}"));
                }
            }
        }
        out
    }

    /// `class_fx=` for status: what class features keep going in the fight, `who:rage5`,
    /// `:frenzy`, `:reckless`, `:insp6`, `:sacred2`, `:temp7`.
    pub(in crate::runtime) fn class_fx_status(&self) -> Vec<String> {
        let Some(c) = &self.fight else { return Vec::new() };
        c.order
            .iter()
            .filter(|f| !f.out)
            .filter_map(|f| {
                let x = &f.fx;
                let mut s = String::new();
                if x.rage > 0 {
                    s += &format!(":rage{}", x.rage);
                }
                if x.frenzied {
                    s += ":frenzy";
                }
                if x.reckless {
                    s += ":reckless";
                }
                if x.inspiration > 0 {
                    s += &format!(":insp{}", x.inspiration);
                }
                if x.weapon_buff.1 > 0 {
                    s += &format!(":sacred{}", x.weapon_buff.1);
                }
                if x.temp_hp > 0 {
                    s += &format!(":temp{}", x.temp_hp);
                }
                (!s.is_empty()).then(|| format!("{}{s}", self.who_key(f.who)))
            })
            .collect()
    }

    // --------------------------------------------------------------------- class actions

    /// Whether `who` can use class action `a` now, else why not.
    pub(super) fn class_act_ready(&self, who: Who, a: &ClassAct) -> Result<(), &'static str> {
        let Some(c) = &self.fight else { return Err("No fight") };
        let t = &c.turn;
        // Wild Shape: the hero's only; in the shape it leaves it (a bonus action).
        if matches!(a.effect, ActEffect::Shape { .. }) {
            if who != Who::Player {
                return Err("Only the hero takes shapes");
            }
            if self.world.shape.is_some() {
                return if t.bonus { Err("Bonus action used") } else { Ok(()) };
            }
        }
        match a.cost {
            ActCost::Action if t.action => return Err("Action used"),
            ActCost::Bonus if t.bonus => return Err("Bonus action used"),
            ActCost::Reaction if c.fighter(who).is_some_and(|f| f.reacted) => return Err("Reaction used"),
            _ => {}
        }
        if !a.resource.is_empty() && self.uses(who, &a.resource).is_none_or(|(left, _)| left < 1) {
            return Err("No uses left");
        }
        let fx = self.fx(who);
        match &a.effect {
            ActEffect::Rage if fx.rage > 0 => Err("Already raging"),
            ActEffect::FrenzyAttack if fx.rage == 0 => Err("Only while raging"),
            ActEffect::UnarmedStrikes { .. } if !t.attacked => Err("Attack first"),
            ActEffect::Reckless if fx.reckless => Err("Already reckless"),
            ActEffect::ExtraAction if c.once.contains(&(who, "surge")) => Err("Once a turn"),
            // (Another action only helps once the first is spent.)
            ActEffect::ExtraAction if !t.action => Err("Use your action first"),
            _ => Ok(()),
        }
    }

    /// Uses class action `id` of the current fighter on `target` (itself for those that need no
    /// target): the cost paid, a use of its resource spent.
    pub(super) fn use_class_act(&mut self, id: &str, target: Who) -> Result<(), &'static str> {
        let me = self.fight.as_ref().ok_or("No fight")?.current().clone();
        let a = self.class_acts(me.who).into_iter().find(|a| a.id == id).ok_or("No such class action")?;
        self.class_act_ready(me.who, &a)?;
        let dist = self.who_dist(me.who, target);
        let ally = self.fight.as_ref().ok_or("No fight")?.fighter(target).is_some_and(|f| f.side == me.side);
        let target = match a.effect.target() {
            ActTarget::Myself => me.who,
            ActTarget::Ally { tiles, myself } => {
                if !ally || dist > tiles || (target == me.who && !myself) {
                    return Err(if tiles <= 1 { "Touch an ally" } else { "Choose an ally in range" });
                }
                target
            }
            ActTarget::Foe { tiles } => {
                if ally || target == me.who {
                    return Err("Not a foe");
                }
                if dist > tiles {
                    return Err("Out of reach");
                }
                target
            }
        };
        let level = self.who_traits(me.who).map_or(1, |t| t.level) as i32;
        let name = self.who_name(me.who);
        let act_name = crate::lang::tr(&a.name);
        // Effects that can still fail spend nothing then.
        match &a.effect {
            // (The beast is chosen in a window: the use and the action are spent then.)
            ActEffect::Shape { .. } => {
                let back = self.world.shape.is_some();
                self.wild_shape(&a)?;
                if back {
                    if let Some(c) = &mut self.fight {
                        c.turn.bonus = true;
                    }
                }
                return Ok(());
            }
            ActEffect::HealPool => {
                let (hp, max) = self.who_hp(target);
                let pool = self.uses(me.who, &a.resource).map_or(0, |u| u.0);
                let n = (max - hp).clamp(0, pool);
                if n == 0 {
                    return Err("Not hurt");
                }
                self.combat_heal(target, n);
                self.spend(me.who, &a.resource, n - 1);
                self.fight_log(format!("{name}: {act_name} > {} +{n}", self.who_name(target)));
            }
            ActEffect::Heal { dice, per_level } => {
                let n = roll_or_zero(dice) + per_level * level;
                self.combat_heal(me.who, n);
                self.fight_log(format!("{name}: {act_name} +{n}"));
            }
            ActEffect::HealAllies { feet, per_level } => {
                let mut pool = per_level * level;
                let mut allies: Vec<Who> = self.fight.as_ref().ok_or("No fight")?.order.iter().filter(|f| f.side == me.side && !f.out).map(|f| f.who).filter(|w| self.tiles_between(me.who, *w) * 5 <= *feet as i32).collect();
                allies.sort_by_key(|w| self.who_hp(*w).0);
                let mut healed = 0;
                for w in allies {
                    let (hp, max) = self.who_hp(w);
                    let n = (max / 2 - hp).clamp(0, pool);
                    if n > 0 {
                        self.combat_heal(w, n);
                        pool -= n;
                        healed += n;
                    }
                }
                if healed == 0 {
                    return Err("No one is badly hurt");
                }
                self.fight_log(format!("{name}: {act_name} +{healed}"));
            }
            ActEffect::ExtraAction => {
                let multi = self.multiattack_of(me.who);
                let c = self.fight.as_mut().ok_or("No fight")?;
                c.once.push((me.who, "surge"));
                c.turn.action = false;
                c.turn.attacks_left = multi;
                self.fight_log(format!("{name}: {act_name}"));
            }
            ActEffect::Rage => {
                if let Some(fx) = self.fx_mut(me.who) {
                    (fx.rage, fx.rage_kept, fx.frenzied) = (RAGE_ROUNDS, false, false);
                }
                self.fight_log(format!("{name}: {act_name}"));
            }
            ActEffect::UnarmedStrikes { count } => {
                let spec = self.unarmed_spec(me.who);
                let mut spec = spec.ok_or("No attack")?;
                spec.flurry = matches!(a.effect, ActEffect::UnarmedStrikes { count } if count > 1);
                self.fight_log(format!("{name}: {act_name}"));
                for _ in 0..*count {
                    if self.fight.as_ref().and_then(|c| c.fighter(target)).is_none_or(|f| f.out || f.down) {
                        break;
                    }
                    self.make_attack(me.who, target, &spec);
                }
            }
            ActEffect::FrenzyAttack => {
                let spec = self.melee_spec(me.who).ok_or("No attack")?;
                self.can_attack(me.who, target, &spec)?;
                if let Some(fx) = self.fx_mut(me.who) {
                    fx.frenzied = true;
                }
                self.fight_log(format!("{name}: {act_name}"));
                self.make_attack(me.who, target, &spec);
            }
            ActEffect::Basic { act } => {
                match act.as_str() {
                    "dash" => {
                        let speed = self.who_speed_now(me.who);
                        self.fight.as_mut().ok_or("No fight")?.turn.speed += speed;
                    }
                    "disengage" => self.fight.as_mut().ok_or("No fight")?.turn.disengaged = true,
                    "dodge" => {
                        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(me.who)) {
                            f.dodging = true;
                        }
                    }
                    "hide" => {
                        let dc = self.passive_perception(me.side);
                        let ok = self.who_check(me.who, "stealth", dc, false).is_some_and(|r| r.success == Some(true));
                        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(me.who)) {
                            f.hidden = ok;
                        }
                    }
                    _ => return Err("No such class action"),
                }
                self.fight_log(format!("{name}: {act_name}"));
            }
            ActEffect::GiveDie { number, .. } => {
                let die = self.who_classdef(me.who).map_or(6, |c| c.number(level as u32, number) as u32).max(4);
                if let Some(fx) = self.fx_mut(target) {
                    fx.inspiration = die;
                }
                self.fight_log(format!("{name}: {act_name} > {} (d{die})", self.who_name(target)));
            }
            ActEffect::AreaSave { feet, save, ability, condition, rounds, types, single, destroy } => {
                let dc = self.feature_dc(me.who, *ability);
                let tiles = (*feet / 5) as i32;
                let foes: Vec<Who> = if *single {
                    vec![target]
                } else {
                    self.fight.as_ref().ok_or("No fight")?.order.iter().filter(|f| f.side != me.side && !f.out && !f.down).map(|f| f.who).filter(|w| self.tiles_between(me.who, *w) <= tiles).collect()
                };
                let foes: Vec<Who> = foes.into_iter().filter(|w| types.is_empty() || self.who_block(*w).is_some_and(|b| types.iter().any(|t| b.creature_type.to_ascii_lowercase().contains(t.as_str())))).collect();
                self.fight_log(format!("{name}: {act_name} (DC {dc})"));
                let destroy_cr = if *destroy { self.who_traits(me.who).and_then(|t| t.riders.iter().filter_map(|e| if let FeatureEffect::DestroyUndead { cr } = e { Some(*cr) } else { None }).reduce(f32::max)) } else { None };
                for w in foes {
                    let ok = self.who_check(w, save.id(), dc, true).is_some_and(|r| r.success == Some(true));
                    if ok {
                        continue;
                    }
                    let cr = self.who_block(w).and_then(|b| crate::srd::encounter::parse_cr(&b.cr)).unwrap_or(99.0);
                    if destroy_cr.is_some_and(|d| cr <= d + 1e-6) {
                        let wname = self.who_name(w);
                        self.fight_log(format!("{wname} {}", crate::lang::tr_ui("is destroyed")));
                        self.dies(w);
                        continue;
                    }
                    if let Some(l) = self.who_conditions_mut(w) {
                        conditions::apply(l, &ConditionSpec::new(*condition, rounds_secs(*rounds)));
                    }
                }
            }
            ActEffect::WeaponBuff { ability, rounds } => {
                let bonus = self.who_mod(me.who, *ability).max(1);
                if let Some(fx) = self.fx_mut(me.who) {
                    fx.weapon_buff = (bonus, *rounds);
                }
                self.fight_log(format!("{name}: {act_name} (+{bonus})"));
            }
            ActEffect::Reckless => {
                if let Some(fx) = self.fx_mut(me.who) {
                    fx.reckless = true;
                }
                self.fight_log(format!("{name}: {act_name}"));
            }
            ActEffect::TempHp { dice, per_level } => {
                let n = roll_or_zero(dice) + per_level * level;
                if let Some(fx) = self.fx_mut(me.who) {
                    fx.temp_hp = fx.temp_hp.max(n);
                }
                self.fight_log(format!("{name}: {act_name} ({n})"));
            }
            ActEffect::FlexibleCasting { points, create } => {
                let left = self.uses(me.who, points).map_or(0, |u| u.0);
                if *create {
                    let cost = |l: u32| [2, 3, 5, 6, 7][(l as usize - 1).min(4)];
                    let spent = self.slots_spent_of(me.who);
                    let level = (1..=5u32).find(|l| spent[*l as usize - 1] > 0 && cost(*l) <= left).ok_or("No slot to create")?;
                    self.recover_slot(me.who, level);
                    self.spend(me.who, points, cost(level));
                    self.fight_log(format!("{name}: {act_name} ({} {level})", crate::lang::tr_ui("slot")));
                } else {
                    if self.spent_of(me.who).get(points.as_str()).copied().unwrap_or(0) <= 0 {
                        return Err("Your points are full");
                    }
                    let slots = self.slots_left(me.who);
                    let level = (1..=9u32).find(|l| slots[*l as usize - 1] > 0).ok_or("No spell slot left")?;
                    self.spend_slot(me.who, level);
                    self.spend(me.who, points, -(level as i32).min(self.spent_of(me.who).get(points.as_str()).copied().unwrap_or(0)));
                    self.fight_log(format!("{name}: {act_name} (+{level})"));
                }
            }
            ActEffect::Quicken { points, cost } => {
                if self.uses(me.who, points).is_none_or(|u| u.0 < *cost) {
                    return Err("Not enough points");
                }
                self.spend(me.who, points, *cost);
                if let Some(fx) = self.fx_mut(me.who) {
                    fx.quicken = true;
                }
                self.fight_log(format!("{name}: {act_name}"));
            }
            ActEffect::Boon { buff, rounds } => {
                if let Some(l) = self.who_conditions_mut(me.who) {
                    conditions::apply_buff(l, buff.clone(), (*rounds).max(1) as f32 * conditions::ROUND, &a.id);
                }
                self.fight_log(format!("{name}: {act_name}"));
            }
            ActEffect::Breath { by_level } => {
                let a = self.breath_action(me.who, by_level).ok_or("No draconic ancestry")?;
                self.use_action(me.who, &a, super::casting::CastAt::Tile(self.who_tile(target)));
            }
            ActEffect::Spell { spell, level: slot } => {
                let d = self.project.db.spell(spell).cloned().ok_or("No such spell")?;
                let slot = (*slot).max(d.level);
                let at = if d.area.is_some() { super::casting::CastAt::Tile(self.who_tile(target)) } else { super::casting::CastAt::On(target) };
                let (targets, point) = self.spell_targets(me.who, &d, slot, at)?;
                self.fight_log(format!("{name}: {act_name} ({})", crate::lang::tr(&d.name)));
                self.spell_impact(&d, crate::grid::feet_spot(point, self.ts()));
                self.resolve_spell(me.who, &d, slot, &targets, Some(point));
            }
            ActEffect::Script { function } => {
                self.sync_script_player();
                let t = self.who_tile(target);
                let key = if me.who == Who::Player { "player".to_string() } else { self.who_key(me.who) };
                let args = vec![rhai::Dynamic::from(key), rhai::Dynamic::from(t.0 as f64 + 0.5), rhai::Dynamic::from(t.1 as f64 + 0.5)];
                let actions = self.scripts.call_args(&mut self.world, function, args, None);
                self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
                self.fight_log(format!("{name}: {act_name}"));
            }
        }
        if !a.resource.is_empty() {
            self.spend(me.who, &a.resource, 1);
        }
        if let Some(c) = &mut self.fight {
            match a.cost {
                ActCost::Action => c.turn.action = true,
                ActCost::Bonus => c.turn.bonus = true,
                ActCost::Reaction => {
                    if let Some(f) = c.fighter_mut(me.who) {
                        f.reacted = true;
                    }
                }
                ActCost::Free => {}
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------- outside fights

    /// Whether a class action's effect works outside a fight (healing, a script); the others
    /// (rage, extra actions, strikes, saves of foes, buffs that last rounds) need one.
    pub(in crate::runtime) fn works_outside(effect: &ActEffect) -> bool {
        matches!(effect, ActEffect::Heal { .. } | ActEffect::HealPool | ActEffect::HealAllies { .. } | ActEffect::Script { .. } | ActEffect::Shape { .. } | ActEffect::Boon { .. })
    }

    /// A dragonborn's breath as a save action: its ancestry's damage type, shape and save, the
    /// dice of its level, DC 8 + CON + proficiency.
    fn breath_action(&self, who: Who, by_level: &[(u32, String)]) -> Option<crate::srd::StatAction> {
        use crate::skills::{Area, Shape};
        use crate::srd::DamageType as D;
        let race = self.project.db.race_full(&self.world.player_race)?;
        let choice = race.choices.iter().find(|c| c.kind == crate::races::ChoiceKind::Ancestry)?;
        let pick = self.world.choices.get(&choice.id)?.first()?;
        let t = choice.grants.get(pick).and_then(|t| D::parse(t))?;
        let level = self.who_traits(who).map_or(1, |t| t.level).max(self.world.player_level());
        let dice = crate::skills::dice_at(by_level, level)?.to_string();
        let (area, save) = match t {
            D::Acid | D::Lightning => (Area { shape: Shape::Line, feet: 30 }, Ability::Dex),
            D::Poison | D::Cold => (Area { shape: Shape::Cone, feet: 15 }, Ability::Con),
            _ => (Area { shape: Shape::Cone, feet: 15 }, Ability::Dex),
        };
        let dc = 8 + self.who_mod(who, Ability::Con) + crate::srd::abilities::proficiency_bonus(level);
        Some(crate::srd::StatAction { name: "Breath Weapon".into(), area: Some(area), save, dc, dice, damage_type: Some(t), half: true, ..Default::default() })
    }

    /// Spends one use of resource `id` of `who`'s.
    pub(in crate::runtime) fn pay_class_resource(&mut self, who: Who, id: &str) {
        self.spend(who, id, 1);
    }

    /// Spends class action `id`'s use and its cost of the turn (in a fight).
    pub(in crate::runtime) fn pay_class_act(&mut self, who: Who, id: &str) {
        let Some(a) = self.class_acts(who).into_iter().find(|a| a.id == id) else { return };
        if !a.resource.is_empty() {
            self.spend(who, &a.resource, 1);
        }
        if let Some(c) = &mut self.fight {
            match a.cost {
                ActCost::Action => c.turn.action = true,
                ActCost::Bonus => c.turn.bonus = true,
                ActCost::Reaction => {
                    if let Some(f) = c.fighter_mut(who) {
                        f.reacted = true;
                    }
                }
                ActCost::Free => {}
            }
        }
    }

    /// The hero's class actions for the action bar outside fights, with why one can't be used now.
    pub(in crate::runtime) fn class_acts_outside(&self) -> Vec<(ClassAct, Option<&'static str>)> {
        self.class_acts(Who::Player)
            .into_iter()
            .map(|a| {
                let why = if !Self::works_outside(&a.effect) {
                    Some("Only in a fight")
                } else if !a.resource.is_empty() && self.uses(Who::Player, &a.resource).is_none_or(|(left, _)| left < 1) {
                    Some("No uses left")
                } else {
                    None
                };
                (a, why)
            })
            .collect()
    }

    /// Uses the hero's class action `id` outside a fight (the action bar, a favorite): a use of
    /// its resource, no action. Lay on Hands with companions around asks whom first.
    pub(in crate::runtime) fn use_class_act_outside(&mut self, id: &str) -> Result<(), &'static str> {
        if self.fight.is_some() {
            return Err("In a fight: use the action bar");
        }
        let (a, why) = self.class_acts_outside().into_iter().find(|(a, _)| a.id == id).ok_or("No such class action")?;
        // (Wild Shape: leaving the shape needs no use.)
        if matches!(a.effect, ActEffect::Shape { .. }) && self.world.shape.is_some() {
            return self.wild_shape(&a);
        }
        if let Some(why) = why {
            return Err(why);
        }
        let level = self.who_traits(Who::Player).map_or(1, |t| t.level) as i32;
        let act_name = crate::lang::tr(&a.name);
        match &a.effect {
            ActEffect::HealPool => {
                if self.start_heal_pick(crate::runtime::healing::HealSource::ClassAct(id.to_string())) {
                    return Ok(());
                }
                return self.class_heal_outside(id, Who::Player);
            }
            ActEffect::Heal { dice, per_level } => {
                if self.hp() >= self.player_max_hp() {
                    return Err("Not hurt");
                }
                let n = roll_or_zero(dice) + per_level * level;
                self.combat_heal(Who::Player, n);
                self.ui.toast(format!("{act_name} +{n}"));
            }
            ActEffect::HealAllies { feet, per_level } => {
                let mut pool = per_level * level;
                let ts = self.ts();
                let reach = *feet as f32 / 5.0 * ts;
                let mut allies = vec![Who::Player];
                allies.extend(self.loaded.entities.iter().filter(|e| e.active && e.companion.is_some() && e.body.pos.distance(self.player.pos) <= reach).map(|e| Who::Npc(e.uid)));
                allies.sort_by_key(|w| self.who_hp(*w).0);
                let mut healed = 0;
                for w in allies {
                    let (hp, max) = self.who_hp(w);
                    let n = (max / 2 - hp).clamp(0, pool);
                    if n > 0 {
                        self.combat_heal(w, n);
                        pool -= n;
                        healed += n;
                    }
                }
                if healed == 0 {
                    return Err("No one is badly hurt");
                }
                self.ui.toast(format!("{act_name} +{healed}"));
            }
            // (The beast is chosen in the action bar's list: the use is spent then.)
            ActEffect::Shape { .. } => return self.wild_shape(&a),
            ActEffect::Boon { buff, rounds } => {
                conditions::apply_buff(&mut self.world.effects, buff.clone(), (*rounds).max(1) as f32 * conditions::ROUND, &a.id);
                self.ui.toast(act_name.to_string());
            }
            ActEffect::Script { function } => {
                self.sync_script_player();
                let t = self.player_tile();
                let args = vec![rhai::Dynamic::from("player".to_string()), rhai::Dynamic::from(t.0 as f64 + 0.5), rhai::Dynamic::from(t.1 as f64 + 0.5)];
                let actions = self.scripts.call_args(&mut self.world, function, args, None);
                self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
            }
            _ => return Err("Only in a fight"),
        }
        if !a.resource.is_empty() {
            self.spend(Who::Player, &a.resource, 1);
        }
        Ok(())
    }

    /// Lay on Hands outside a fight on `target` (the hero or a companion): as much as it lacks,
    /// from the pool.
    pub(in crate::runtime) fn class_heal_outside(&mut self, id: &str, target: Who) -> Result<(), &'static str> {
        let a = self.class_acts(Who::Player).into_iter().find(|a| a.id == id).ok_or("No such class action")?;
        let (hp, max) = self.who_hp(target);
        let pool = self.uses(Who::Player, &a.resource).map_or(0, |u| u.0);
        let n = (max - hp).clamp(0, pool);
        if n == 0 {
            return Err(if pool == 0 { "No uses left" } else { "Not hurt" });
        }
        self.combat_heal(target, n);
        self.spend(Who::Player, &a.resource, n);
        self.ui.toast(format!("{} > {} +{n}", crate::lang::tr(&a.name), self.who_name(target)));
        Ok(())
    }

    /// Who class action `id` goes to without a named target: itself, the nearest foe or the
    /// most hurt ally in range.
    pub(super) fn class_act_default_target(&self, id: &str) -> Option<Who> {
        let c = self.fight.as_ref()?;
        let me = c.current().clone();
        let a = self.class_acts(me.who).into_iter().find(|a| a.id == id)?;
        match a.effect.target() {
            ActTarget::Myself => Some(me.who),
            ActTarget::Foe { .. } => c.order.iter().filter(|f| f.side != me.side && !f.out).map(|f| f.who).min_by_key(|w| self.tiles_between(me.who, *w)),
            ActTarget::Ally { tiles, myself } => c
                .order
                .iter()
                .filter(|f| f.side == me.side && !f.out && (myself || f.who != me.who))
                .map(|f| f.who)
                .filter(|w| self.tiles_between(me.who, *w) <= tiles)
                .min_by_key(|w| {
                    let (hp, max) = self.who_hp(*w);
                    (hp * 100 / max.max(1), self.tiles_between(me.who, *w))
                }),
        }
    }

    fn tiles_between(&self, a: Who, b: Who) -> i32 {
        self.who_dist(a, b)
    }

    /// The attacks the Attack action makes (Extra Attack, Multiattack).
    pub(super) fn multiattack_of(&self, who: Who) -> u32 {
        if who == Who::Player && self.world.shape.is_none() {
            self.hero_traits().map_or(1, |t| t.attacks.max(1))
        } else {
            self.who_block(who).map_or(1, |b| b.multiattack.max(1))
        }
    }

    /// An unarmed strike (Martial Arts' die and DEX for a monk; 1 + STR otherwise).
    fn unarmed_spec(&self, who: Who) -> Option<AttackSpec> {
        let t = self.who_traits(who).unwrap_or_default();
        let (str_m, dex_m) = (self.who_mod(who, Ability::Str), self.who_mod(who, Ability::Dex));
        let (m, ability) = if t.martial_arts && dex_m > str_m { (dex_m, Ability::Dex) } else { (str_m, Ability::Str) };
        let die = if t.martial_arts { self.who_classdef(who).map_or(4, |c| c.number(t.level, "martial_arts.dice_value")).max(4) } else { 0 };
        let base = if die > 0 { format!("1d{die}") } else { "1".into() };
        let dice = if m == 0 { base } else { format!("{base}{m:+}") };
        Some(AttackSpec { name: crate::lang::tr_ui("Unarmed strike").into_owned(), bonus: m + self.who_prof(who), dice, damage_type: Some(DamageType::Bludgeoning), reach: 1, crit_on: t.crit_from.min(20), ability: Some(ability), ..Default::default() })
    }

    // -------------------------------------------------------------------- the turn's start

    /// A fighter's turn starts: rage goes on or ends, Reckless Attack and Sacred Weapon count
    /// down, Survivor heals.
    pub(super) fn class_turn_start(&mut self, who: Who) {
        if let Some(c) = &mut self.fight {
            c.once.clear();
        }
        let fx = self.fx(who);
        if fx.rage > 0 {
            let over = !fx.rage_kept || fx.rage <= 1;
            if over {
                self.end_rage(who);
            } else if let Some(x) = self.fx_mut(who) {
                x.rage -= 1;
                x.rage_kept = false;
            }
        }
        if let Some(x) = self.fx_mut(who) {
            x.reckless = false;
            if x.weapon_buff.1 > 0 {
                x.weapon_buff.1 -= 1;
            }
        }
        // Survivor: at half its hit points or fewer.
        if let Some((base, a)) = self.who_rider(who, |e| if let FeatureEffect::StartOfTurnHeal { base, ability } = e { Some((*base, *ability)) } else { None }) {
            let (hp, max) = self.who_hp(who);
            if hp > 0 && hp * 2 <= max {
                let n = base + self.who_mod(who, a);
                self.combat_heal(who, n);
                self.fight_log(format!("{}: {} +{n}", self.who_name(who), crate::lang::tr_ui("recovers")));
            }
        }
    }

    /// A rage ends (time, a turn without fighting, the fight's end): a frenzy leaves exhaustion.
    pub(super) fn end_rage(&mut self, who: Who) {
        let fx = self.fx(who);
        if fx.rage == 0 {
            return;
        }
        if let Some(x) = self.fx_mut(who) {
            (x.rage, x.rage_kept, x.frenzied) = (0, false, false);
        }
        if fx.frenzied {
            if let Some(l) = self.who_conditions_mut(who) {
                conditions::apply(l, &ConditionSpec::new(Condition::Exhaustion, 0.0));
            }
        }
        let name = self.who_name(who);
        self.fight_log(format!("{name}: {}", crate::lang::tr_ui("rage ends")));
    }

    // --------------------------------------------------------------------------- attacks

    /// An attack with what the attacker's features add: Sacred Weapon's bonus, rage's damage on
    /// STR melee attacks.
    pub(super) fn buffed(&self, who: Who, spec: &AttackSpec) -> AttackSpec {
        let mut s = spec.clone();
        let fx = self.fx(who);
        if spec.spell {
            return s;
        }
        if fx.weapon_buff.1 > 0 {
            s.bonus += fx.weapon_buff.0;
        }
        if fx.rage > 0 && !s.ranged && s.ability.is_none_or(|a| a == Ability::Str) {
            let level = self.who_traits(who).map_or(1, |t| t.level);
            let n = self.who_classdef(who).map_or(2, |c| c.number(level, "rage_damage_bonus") as i32);
            if n != 0 {
                s.dice = format!("{}{n:+}", s.dice);
            }
        }
        s
    }

    /// Whether the attacker or the target is reckless for this attack (advantage).
    pub(super) fn reckless_edge(&self, who: Who, target: Who, spec: &AttackSpec) -> bool {
        let mine = self.fx(who).reckless && !spec.ranged && !spec.spell && spec.ability.is_none_or(|a| a == Ability::Str);
        mine || self.fx(target).reckless
    }

    /// A Bardic Inspiration die turns a missed attack into a hit when it can.
    pub(super) fn inspire_attack(&mut self, who: Who, roll: &mut crate::srd::combat::AttackRoll) {
        let die = self.fx(who).inspiration;
        if die == 0 || roll.hit || roll.d20 == 1 || roll.total + (die as i32) < roll.ac {
            return;
        }
        let d = crate::srd::dice::die(die) as i32;
        if let Some(x) = self.fx_mut(who) {
            x.inspiration = 0;
        }
        roll.total += d;
        roll.hit = roll.total >= roll.ac;
        self.fight_log(format!("{}: {} +{d}", self.who_name(who), crate::lang::tr_ui("inspiration")));
    }

    /// The damage a hit's riders add: Sneak Attack, Colossus Slayer, Divine Strike (critical:
    /// their dice twice).
    pub(super) fn rider_damage(&mut self, who: Who, target: Who, spec: &AttackSpec, mode: Mode, crit: bool) -> i32 {
        if spec.spell {
            return 0;
        }
        let Some(t) = self.who_traits(who) else { return 0 };
        let mut extra = 0;
        let mul = if crit { 2 } else { 1 };
        // Sneak Attack: a finesse or ranged weapon, advantage or an ally next to the target.
        if t.riders.contains(&FeatureEffect::SneakAttack) && (spec.ranged || spec.finesse) && mode != Mode::Disadvantage {
            let side = self.fight.as_ref().and_then(|c| c.fighter(who)).map(|f| f.side);
            let ally_near = self.fight.as_ref().is_some_and(|c| {
                c.order.iter().any(|f| Some(f.side) == side && f.who != who && !f.out && !f.down && conditions::can_act(self.who_conditions(f.who)) && self.who_dist(f.who, target) <= 1)
            });
            if (mode == Mode::Advantage || ally_near) && self.once(who, "sneak") {
                let n = self.who_classdef(who).map_or(1, |c| c.number(t.level, "sneak_attack.dice_count")).max(1) * mul;
                let d = roll_or_zero(&format!("{n}d6"));
                extra += d;
                self.fight_log(format!("{}: {} +{d}", self.who_name(who), crate::lang::tr_ui("sneak attack")));
            }
        }
        for r in &t.riders {
            if let FeatureEffect::ExtraDamage { dice, hurt_target, .. } = r {
                let Some((_, d)) = dice.iter().filter(|(l, _)| *l <= t.level).max_by_key(|(l, _)| *l) else { continue };
                let (hp, max) = self.who_hp(target);
                if *hurt_target && hp >= max {
                    continue;
                }
                if !self.once(who, if *hurt_target { "hurt" } else { "strike" }) {
                    continue;
                }
                let d = if crit { crate::srd::combat::crit_dice(d) } else { d.clone() };
                extra += roll_or_zero(&d);
            }
        }
        extra
    }

    /// The damage parts a melee weapon hit gets from `who`'s features (Improved Divine Smite).
    pub(super) fn melee_riders(&self, who: Who, spec: &AttackSpec, crit: bool) -> Vec<(i32, Option<DamageType>)> {
        if spec.spell || spec.ranged {
            return Vec::new();
        }
        let Some(t) = self.who_traits(who) else { return Vec::new() };
        t.riders
            .iter()
            .filter_map(|r| if let FeatureEffect::MeleeDamage { dice, damage_type } = r { Some((dice.clone(), *damage_type)) } else { None })
            .map(|(d, ty)| (roll_or_zero(&if crit { crate::srd::combat::crit_dice(&d) } else { d }), ty.or(spec.damage_type)))
            .collect()
    }

    /// Divine Smite at `slot` on `target`: its radiant dice (2d8, 1d8 a level above 1st, at most
    /// 5d8; 1d8 more against the feature's creature types; doubled on a critical hit), the slot
    /// spent.
    pub(super) fn smite_part(&mut self, who: Who, target: Who, slot: u32, crit: bool) -> (i32, Option<DamageType>) {
        let Some((ty, against)) = self.who_rider(who, |e| if let FeatureEffect::Smite { damage_type, extra_against } = e { Some((*damage_type, extra_against.clone())) } else { None }) else { return (0, None) };
        self.spend_slot(who, slot);
        let kind = self.who_block(target).map(|b| b.creature_type.to_ascii_lowercase()).unwrap_or_default();
        let more = !kind.is_empty() && against.iter().any(|a| kind.contains(&a.to_ascii_lowercase()));
        let n = (1 + slot).min(5) + u32::from(more);
        let n = if crit { n * 2 } else { n };
        let d = roll_or_zero(&format!("{n}d8"));
        self.fight_log(format!("{}: {} ({} {slot}) +{d}", self.who_name(who), crate::lang::tr_ui("Divine Smite"), crate::lang::tr_ui("slot")));
        (d, ty)
    }

    /// Song of Rest: the best die of the party's bards (the hero's or a companion's), 0 = none.
    pub(in crate::runtime) fn song_of_rest_die(&self) -> u32 {
        let mut who = vec![Who::Player];
        who.extend(self.loaded.entities.iter().filter(|e| e.active && e.companion.is_some()).map(|e| Who::Npc(e.uid)));
        who.into_iter().filter_map(|w| self.who_rider(w, |e| if let FeatureEffect::SongOfRest { die } = e { Some(*die) } else { None })).max().unwrap_or(0)
    }

    /// The target's reaction to a hit's damage, `react` (chosen in `reactions.rs`): Deflect
    /// Missiles against a ranged weapon, Uncanny Dodge against an attacker it sees. Returns the
    /// damage left.
    pub(super) fn react_to_hit(&mut self, attacker: Who, target: Who, spec: &AttackSpec, amount: i32, react: super::reactions::ReactChoice) -> i32 {
        use super::reactions::ReactChoice;
        let free = self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| !f.reacted && !f.down && !f.out) && conditions::can_act(self.who_conditions(target));
        if !free || spec.spell || amount <= 0 || !matches!(react, ReactChoice::Deflect | ReactChoice::UncannyDodge) {
            return amount;
        }
        let tname = self.who_name(target);
        if spec.ranged && react == ReactChoice::Deflect {
            if let Some((dice, a, per)) = self.who_rider(target, |e| if let FeatureEffect::ReactionReduceRanged { dice, ability, per_level } = e { Some((dice.clone(), *ability, *per_level)) } else { None }) {
                let level = self.who_traits(target).map_or(1, |t| t.level) as i32;
                let cut = roll_or_zero(&dice) + self.who_mod(target, a) + per * level;
                self.set_reacted(target);
                self.fight_log(format!("{tname}: {} (-{cut})", crate::lang::tr_ui("deflects the missile")));
                return (amount - cut).max(0);
            }
        }
        let seen = !self.fight.as_ref().and_then(|c| c.fighter(attacker)).is_some_and(|f| f.hidden) && !conditions::rules(self.who_conditions(attacker)).invisible;
        if seen && react == ReactChoice::UncannyDodge && self.who_rider(target, |e| (*e == FeatureEffect::ReactionHalveDamage).then_some(())).is_some() {
            self.set_reacted(target);
            self.fight_log(format!("{tname}: {}", crate::lang::tr_ui("uncanny dodge")));
            return amount / 2;
        }
        amount
    }

    fn set_reacted(&mut self, who: Who) {
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.reacted = true;
        }
    }

    /// After a hit: the saves its features call for (Stunning Strike, Open Hand Technique).
    pub(super) fn on_hit_saves(&mut self, who: Who, target: Who, spec: &AttackSpec) {
        if spec.spell || self.fight.as_ref().and_then(|c| c.fighter(target)).is_none_or(|f| f.out || f.down) {
            return;
        }
        let Some(t) = self.who_traits(who) else { return };
        for r in &t.riders {
            let FeatureEffect::OnHitSave { resource, save, ability, condition, rounds, melee, flurry } = r else { continue };
            if (*melee && spec.ranged) || (*flurry && !spec.flurry) || conditions::has(self.who_conditions(target), *condition) {
                continue;
            }
            if !resource.is_empty() {
                if self.uses(who, resource).is_none_or(|u| u.0 < 1) || !self.once(who, "stun") {
                    continue;
                }
                self.spend(who, resource, 1);
            }
            let dc = self.feature_dc(who, *ability);
            let ok = self.who_check(target, save.id(), dc, true).is_some_and(|r| r.success == Some(true));
            if !ok {
                if let Some(l) = self.who_conditions_mut(target) {
                    conditions::apply(l, &ConditionSpec::new(*condition, if *rounds == 0 { 0.0 } else { rounds_secs(*rounds) }));
                }
                self.fight_log(format!("{}: {}", self.who_name(target), crate::lang::tr_ui(condition.name())));
            }
        }
    }

    /// After a monster's hit: what its attack does besides damage (a wolf's bite: a STR save or
    /// prone; a giant spider's: a CON save against poison damage, half on a success; a grapple
    /// with its escape DC).
    pub(super) fn hit_riders(&mut self, who: Who, target: Who, spec: &AttackSpec) {
        for r in &spec.riders {
            if self.fight.as_ref().and_then(|c| c.fighter(target)).is_none_or(|f| f.out || f.down) {
                return;
            }
            let tname = self.who_name(target);
            let (saved, save_text) = match r.save {
                Some(a) => {
                    // (A poisonous bite or sting: advantages against poison count.)
                    self.poison_save = r.damage_type == Some(DamageType::Poison) || r.condition == Some(conditions::Condition::Poisoned);
                    let rec = self.who_check(target, a.id(), r.dc, true);
                    self.poison_save = false;
                    let ok = rec.as_ref().is_some_and(|x| x.success == Some(true));
                    (ok, format!("{} {}/{} {}", a.short(), rec.as_ref().map_or(0, |x| x.total), r.dc, crate::lang::tr_ui(if ok { "saved" } else { "failed" })))
                }
                None => (false, String::new()),
            };
            // A diseased bite: a failed save (or the hit, without one) infects the hero.
            if !r.disease.is_empty() && !saved && target == Who::Player {
                if let Some(def) = self.project.db.disease(&r.disease).cloned() {
                    self.catch_disease(&def);
                }
                if r.save.is_some() && r.dice.is_empty() && r.condition.is_none() {
                    self.fight_log(format!("{tname}: {save_text}"));
                }
            }
            if !r.dice.is_empty() {
                let full = super::roll_damage(&r.dice, false);
                let n = if !saved { full } else if r.half { full / 2 } else { 0 };
                self.fight_log(format!("{tname}: {save_text}{}{n}", if save_text.is_empty() { "" } else { ", " }));
                if n > 0 {
                    self.combat_damage_parts(target, &[(n, r.damage_type)], false, Some(who), spec.hit);
                }
            }
            let Some(c) = r.condition else {
                continue;
            };
            let alive = self.fight.as_ref().and_then(|f| f.fighter(target)).is_some_and(|f| !f.out && !f.down);
            let immune = self.who_block(target).is_some_and(|b| b.immune_to(c));
            if saved || !alive || immune {
                if saved && r.dice.is_empty() {
                    self.fight_log(format!("{tname}: {save_text}"));
                }
                continue;
            }
            let tag = self.who_tag(who);
            if let Some(l) = self.who_conditions_mut(target) {
                conditions::apply(l, &ConditionSpec::new(c, if r.rounds == 0 { 0.0 } else { rounds_secs(r.rounds) }));
                conditions::set_origin(l, c, &tag, r.escape_dc);
            }
            // (A grapple: the grappler holds it; it ends when they part.)
            if c == conditions::Condition::Grappled {
                if let Some(f) = self.fight.as_mut().and_then(|f| f.fighter_mut(who)) {
                    f.grappling = Some(target);
                }
            }
            let saved_text = if save_text.is_empty() || !r.dice.is_empty() { String::new() } else { format!("{save_text}, ") };
            self.fight_log(format!("{tname}: {saved_text}{}", crate::lang::tr_ui(c.name())));
        }
    }

    // ---------------------------------------------------------------------------- damage

    /// Damage about to land on `target`: rage's resistance and temporary hit points take their
    /// part; being hurt keeps a rage going. Returns what is left.
    pub(super) fn class_absorb(&mut self, target: Who, amount: i32, t: Option<DamageType>) -> i32 {
        let fx = self.fx(target);
        let mut n = amount;
        if fx.rage > 0 && matches!(t, Some(DamageType::Bludgeoning | DamageType::Piercing | DamageType::Slashing)) {
            n /= 2;
        }
        if n > 0 {
            if let Some(x) = self.fx_mut(target) {
                x.rage_kept = true;
                let soak = x.temp_hp.min(n);
                x.temp_hp -= soak;
                n -= soak;
            }
        }
        n
    }

    /// Rage's resistance on one part of the damage: half of bludgeoning, piercing and slashing.
    pub(super) fn rage_halves(&self, target: Who, n: i32, t: Option<DamageType>) -> i32 {
        if self.fx(target).rage > 0 && matches!(t, Some(DamageType::Bludgeoning | DamageType::Piercing | DamageType::Slashing)) {
            n / 2
        } else {
            n
        }
    }

    /// Hit points about to drop to 0: Relentless Rage's CON save keeps 1. True = it stays up.
    pub(super) fn relentless(&mut self, who: Who) -> bool {
        // (Relentless Endurance: 1 hit point instead, once per its rest.)
        if let Some(res) = self.who_rider(who, |e| if let FeatureEffect::Endurance { resource } = e { Some(resource.clone()) } else { None }) {
            if self.uses(who, &res).is_some_and(|(left, _)| left > 0) {
                self.spend(who, &res, 1);
                self.fight_log(format!("{}: {}", self.who_name(who), crate::lang::tr_ui("relentless endurance")));
                return true;
            }
        }
        if self.fx(who).rage == 0 {
            return false;
        }
        let Some(dc) = self.who_rider(who, |e| if let FeatureEffect::DropTo1 { dc } = e { Some(*dc) } else { None }) else { return false };
        let times = self.spent_of(who).get("relentless-rage").copied().unwrap_or(0);
        self.spend(who, "relentless-rage", 1);
        let ok = self.who_check(who, "con", dc + 5 * times, true).is_some_and(|r| r.success == Some(true));
        if ok {
            self.fight_log(format!("{}: {}", self.who_name(who), crate::lang::tr_ui("relentless rage")));
        }
        ok
    }

    /// `target` was hurt by `by`: Dark One's Blessing when it dropped a foe, Retaliation when it
    /// still stands next to its attacker.
    pub(super) fn after_damage(&mut self, target: Who, by: Option<Who>) {
        let Some(by) = by.filter(|b| *b != target) else { return };
        let sides = self.fight.as_ref().map(|c| (c.fighter(by).map(|f| f.side), c.fighter(target).map(|f| f.side)));
        let foes = sides.is_some_and(|(a, b)| a.is_some() && b.is_some() && a != b);
        if !foes {
            return;
        }
        let (hp, _) = self.who_hp(target);
        if hp <= 0 {
            if let Some((a, per)) = self.who_rider(by, |e| if let FeatureEffect::OnKillTempHp { ability, per_level } = e { Some((*ability, *per_level)) } else { None }) {
                let level = self.who_traits(by).map_or(1, |t| t.level) as i32;
                let n = (self.who_mod(by, a) + per * level).max(1);
                if let Some(x) = self.fx_mut(by) {
                    x.temp_hp = x.temp_hp.max(n);
                }
                self.fight_log(format!("{}: {} {n}", self.who_name(by), crate::lang::tr_ui("temporary HP")));
            }
            return;
        }
        let current = self.fight.as_ref().is_some_and(|c| c.current().who == target);
        let free = self.fight.as_ref().and_then(|c| c.fighter(target)).is_some_and(|f| !f.reacted && !f.down && !f.out);
        if current || !free || self.tiles_between(by, target) > 1 || !conditions::can_act(self.who_conditions(target)) {
            return;
        }
        if self.who_rider(target, |e| (*e == FeatureEffect::ReactionAttack).then_some(())).is_none() {
            return;
        }
        if let Some(spec) = self.melee_spec(target) {
            self.set_reacted(target);
            self.fight_log(format!("{}: {}", self.who_name(target), crate::lang::tr_ui("retaliates")));
            self.make_attack(target, by, &spec);
        }
    }

    // ----------------------------------------------------------------------------- saves

    /// A saving throw's mode from the saver's features: advantage on STR while raging.
    pub(super) fn class_save_mode(&self, who: Who, what: &str) -> Mode {
        let str_based = what.eq_ignore_ascii_case("str") || what.eq_ignore_ascii_case("save:str") || what.eq_ignore_ascii_case("athletics");
        if str_based && self.fx(who).rage > 0 {
            return Mode::Advantage;
        }
        // (Brave, Fey Ancestry: against a condition; Gnome Cunning: against magic.)
        let ability = crate::srd::Ability::parse(what.trim_start_matches("save:"));
        let traits = self.who_traits(who);
        let against = self.save_against.is_some_and(|c| traits.as_ref().is_some_and(|t| t.riders.iter().any(|e| matches!(e, FeatureEffect::SaveAdvantageAgainst { conditions } if conditions.contains(&c)))));
        let magic = self.magic_save && ability.is_some_and(|a| traits.as_ref().is_some_and(|t| t.riders.iter().any(|e| matches!(e, FeatureEffect::MagicSaves { abilities } if abilities.contains(&a)))));
        if against || magic {
            Mode::Advantage
        } else {
            Mode::Normal
        }
    }

    /// A failed saving throw: Bardic Inspiration's die when it would turn it, else Indomitable
    /// rolls it again. Returns the new record when one of them changed it.
    pub(super) fn rescue_save(&mut self, who: Who, rec: &crate::srd::RollRecord, again: impl FnOnce(&mut Game) -> Option<crate::srd::RollRecord>) -> Option<crate::srd::RollRecord> {
        if rec.success != Some(false) {
            return None;
        }
        let die = self.fx(who).inspiration;
        if die > 0 && rec.total + die as i64 >= rec.dc as i64 {
            let d = crate::srd::dice::die(die) as i64;
            if let Some(x) = self.fx_mut(who) {
                x.inspiration = 0;
            }
            let mut r = rec.clone();
            r.total += d;
            r.modifier += d;
            r.success = Some(r.total >= r.dc as i64);
            self.fight_log(format!("{}: {} +{d}", self.who_name(who), crate::lang::tr_ui("inspiration")));
            if let Some(last) = self.world.rolls.last_mut() {
                *last = r.clone();
            }
            return Some(r);
        }
        let res = self.who_rider(who, |e| if let FeatureEffect::RerollSave { resource } = e { Some(resource.clone()) } else { None })?;
        if self.uses(who, &res).is_none_or(|u| u.0 < 1) {
            return None;
        }
        self.spend(who, &res, 1);
        self.fight_log(format!("{}: {}", self.who_name(who), crate::lang::tr_ui("indomitable")));
        again(self)
    }

    // -------------------------------------------------------------------------------- AI

    /// A party member's class actions on its AI turn: Rage when foes are near, a heal below half
    /// its hit points, Action Surge when its attacks are spent and a foe is in reach, Martial Arts'
    /// and Flurry of Blows' strikes after attacking. True when it did one (the AI's step is over).
    pub(super) fn ai_class_step(&mut self, who: Who, side: Side) -> bool {
        let acts = self.class_acts(who);
        if acts.is_empty() {
            return false;
        }
        let Some(c) = &self.fight else { return false };
        let turn = c.turn.clone();
        let foes: Vec<Who> = c.order.iter().filter(|f| f.side != side && !f.out && !f.down).map(|f| f.who).collect();
        if foes.is_empty() {
            return false;
        }
        let near = |g: &Game, tiles: i32| foes.iter().copied().filter(|f| g.tiles_between(who, *f) <= tiles).min_by_key(|f| g.who_hp(*f).0);
        let (hp, max) = self.who_hp(who);
        for a in &acts {
            if self.class_act_ready(who, a).is_err() {
                continue;
            }
            let target = match &a.effect {
                ActEffect::Rage if near(self, 6).is_some() => Some(who),
                ActEffect::Heal { .. } if hp * 2 < max => Some(who),
                ActEffect::HealPool if hp * 2 < max => Some(who),
                ActEffect::ExtraAction if turn.action && turn.attacks_left == 0 && near(self, 1).is_some() => Some(who),
                // (Bonus strikes once the Attack action's attacks are made; no Frenzy: its exhaustion.)
                ActEffect::UnarmedStrikes { .. } if turn.attacked && turn.attacks_left == 0 => near(self, 1),
                _ => None,
            };
            if let Some(t) = target {
                if self.use_class_act(&a.id, t).is_ok() {
                    return true;
                }
            }
        }
        false
    }
}

/// Rolls dice ("1d10"; empty = 0).
fn roll_or_zero(dice: &str) -> i32 {
    if dice.trim().is_empty() {
        return 0;
    }
    crate::srd::Expr::parse(dice).map(|e| e.roll(&crate::srd::dice::NoMods).total.max(0)).unwrap_or(0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healing_class_actions_work_outside_fights() {
        let yes = [ActEffect::Heal { dice: "1d10".into(), per_level: 1 }, ActEffect::HealPool, ActEffect::HealAllies { feet: 30, per_level: 5 }, ActEffect::Script { function: "f".into() }];
        let no = [ActEffect::Rage, ActEffect::ExtraAction, ActEffect::Reckless, ActEffect::UnarmedStrikes { count: 2 }, ActEffect::TempHp { dice: String::new(), per_level: 1 }];
        assert!(yes.iter().all(Game::works_outside));
        assert!(!no.iter().any(Game::works_outside));
    }
}
