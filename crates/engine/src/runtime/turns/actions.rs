//! The SRD's actions in combat: Attack (with Multiattack and the off-hand's bonus attack), Cast a
//! Spell, Dash, Disengage, Dodge, Help, Hide, Ready, Search, Use an Object, Grapple and Shove;
//! the checks and contests they roll.

use super::{Aim, Side, Who};
use crate::grid::{feet_spot, TilePos};
use crate::runtime::Game;
use crate::skills::SpellEffect;
use crate::srd::conditions::{self, Condition, ConditionSpec};
use crate::srd::{Mode, RollRecord};

/// The action bar's entries, in its order (keys 1-8 pick the first eight).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum TurnAct {
    Attack,
    Cast,
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
    pub const BAR: [TurnAct; 13] = [TurnAct::Attack, TurnAct::Cast, TurnAct::Dash, TurnAct::Disengage, TurnAct::Dodge, TurnAct::Help, TurnAct::Hide, TurnAct::Ready, TurnAct::Search, TurnAct::Grapple, TurnAct::Shove, TurnAct::OffHand, TurnAct::EndTurn];

    pub fn label(self) -> &'static str {
        match self {
            TurnAct::Attack => "Attack",
            TurnAct::Cast => "Cast",
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
    pub(super) fn who_check(&mut self, who: Who, what: &str, dc: i32, save: bool) -> Option<RollRecord> {
        if who == Who::Player {
            return self.world.check(what, dc, Mode::Normal, save);
        }
        let block = self.who_block(who)?;
        let spec = crate::srd::Spec::parse(what, save)?;
        let profs: std::collections::BTreeSet<String> = block.proficiencies.iter().map(|p| crate::srd::abilities::proficiency_id(p)).collect();
        let roller = crate::srd::check::Roller { abilities: &block.abilities, proficiencies: &profs, level: block.proficiency_level() };
        let r = conditions::rules(self.who_conditions(who));
        let mode = if (save && r.save_disadvantage) || (!save && r.check_disadvantage) { Mode::Disadvantage } else { Mode::Normal };
        let mut rec = spec.roll(&roller, dc, mode);
        rec.label = format!("{}: {}", self.who_name(who), rec.label);
        self.world.log_roll(rec.clone());
        Some(rec)
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
                } else {
                    Ok(())
                }
            }
            TurnAct::Attack if t.attacks_left > 0 && t.attacked => Ok(()),
            TurnAct::Cast if self.known_spells(c.current().who).is_empty() => Err("No spells"),
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
            TurnAct::Attack | TurnAct::Help | TurnAct::Grapple | TurnAct::Shove | TurnAct::OffHand => {
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
                // Each press: the next spell the caster knows.
                let spells = self.known_spells(who);
                let c = self.fight.as_ref().unwrap();
                let next = match &c.aim {
                    Aim::Cast(s) => spells.iter().position(|x| x == s).map_or(0, |k| (k + 1) % spells.len()),
                    _ => 0,
                };
                let spell = spells[next].clone();
                self.set_aim(Aim::Cast(spell));
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
                if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
                    f.readied = true;
                }
                say(self, "readies an attack");
            }
            TurnAct::Hide => {
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
    fn passive_perception(&self, side: Side) -> i32 {
        let Some(c) = &self.fight else { return 10 };
        c.order
            .iter()
            .filter(|f| f.side != side && !f.out && !f.down)
            .map(|f| match f.who {
                Who::Player => 10 + self.world.check_bonus("perception", false).unwrap_or(0) as i32,
                w => self.who_block(w).map_or(10, |b| {
                    let prof = b.proficiencies.iter().any(|p| crate::srd::abilities::proficiency_id(p) == "perception");
                    10 + b.abilities.modifier(crate::srd::Ability::Wis) + if prof { crate::srd::abilities::proficiency_bonus(b.proficiency_level()) } else { 0 }
                }),
            })
            .max()
            .unwrap_or(10)
    }

    /// The cursor's aim, and the cursor onto the nearest fitting target.
    fn set_aim(&mut self, aim: Aim) {
        let who = self.fight.as_ref().unwrap().current().who;
        let side = self.fight.as_ref().unwrap().current().side;
        let here = self.who_tile(who);
        let allies = matches!(&aim, Aim::Cast(s) if self.spell_heals(s));
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
    pub(super) fn who_at(&self, t: TilePos) -> Option<Who> {
        let c = self.fight.as_ref()?;
        c.order.iter().filter(|f| !f.out).map(|f| f.who).find(|w| self.who_tile(*w) == t)
    }

    /// Does the targeted act `aim` on `target`; Ok once done (the action spent).
    pub(super) fn act_on(&mut self, aim: &Aim, target: Who) -> Result<(), &'static str> {
        let c = self.fight.as_ref().ok_or("No fight")?;
        let me = c.current().clone();
        let foe = c.fighter(target).is_some_and(|f| f.side != me.side);
        let dist = {
            let (a, b) = (self.who_tile(me.who), self.who_tile(target));
            a.0.abs_diff(b.0).max(a.1.abs_diff(b.1)) as i32
        };
        match aim {
            Aim::Move => Err("Choose an action"),
            Aim::Attack => {
                self.act_ready(TurnAct::Attack)?;
                if !foe {
                    return Err("Not a foe");
                }
                // The first attack that reaches (melee next to the target, else a shot).
                let specs = self.attack_specs(me.who);
                let spec = specs.iter().filter(|s| self.can_attack(me.who, target, s).is_ok()).min_by_key(|s| if dist <= 1 { s.ranged as i32 } else { !s.ranged as i32 }).cloned();
                let Some(spec) = spec else { return Err(specs.first().map_or("No attack", |s| self.can_attack(me.who, target, s).err().unwrap_or("Out of reach"))) };
                if !self.pay_ammo(me.who, &spec) {
                    return Err("Out of ammunition");
                }
                self.make_attack(me.who, target, &spec);
                let c = self.fight.as_mut().unwrap();
                c.turn.attacked = true;
                c.turn.attacks_left = c.turn.attacks_left.saturating_sub(1);
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
                if self.fight.as_ref().unwrap().turn.action {
                    return Err("Action used");
                }
                if !foe || dist > 1 {
                    return Err("Out of reach");
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
                    if let Some(l) = self.who_conditions_mut(target) {
                        conditions::apply(l, &ConditionSpec::new(Condition::Grappled, 0.0));
                    }
                    if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(me.who)) {
                        f.grappling = Some(target);
                    }
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
            Aim::Cast(spell) => self.cast_in_fight(me.who, spell, target),
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
        if self.tile_blocks_npc(to.0, to.1) || self.who_at(to).is_some() {
            return;
        }
        let p = feet_spot(to, self.ts());
        match self.who_index(target) {
            None => self.player.pos = p,
            Some(i) => {
                self.loaded.entities[i].body.pos = p;
                self.occupy(i);
            }
        }
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
        if self.world.item_count(&r.ammo) == 0 {
            return false;
        }
        self.world.take_item(&r.ammo, 1);
        true
    }

    /// The hero's off-hand weapon (a weapon in the off-hand slot).
    pub(super) fn offhand_weapon(&self, who: Who) -> Option<crate::defs::Weapon> {
        if who != Who::Player {
            return None;
        }
        let id = self.world.equipment.item(crate::defs::EquipSlot::Offhand)?;
        self.project.db.item(id).and_then(|d| d.weapon.clone())
    }

    /// The spells a combatant can cast in a fight (the hero's learned ones).
    pub(super) fn known_spells(&self, who: Who) -> Vec<String> {
        if who != Who::Player || !self.magic_on() {
            return Vec::new();
        }
        let mut v: Vec<String> = self.project.db.spells.iter().filter(|s| self.world.spells.contains(&s.id)).map(|s| s.id.clone()).collect();
        v.dedup();
        v
    }

    pub(super) fn spell_heals(&self, id: &str) -> bool {
        self.project.db.spell(id).is_some_and(|d| matches!(d.effect, SpellEffect::Heal { .. }))
    }

    /// Cast a Spell (the action): a bolt is a ranged spell attack (INT), a blast a DEX save for
    /// half in its radius around the target, a heal heals the target, a light or script runs.
    fn cast_in_fight(&mut self, who: Who, id: &str, target: Who) -> Result<(), &'static str> {
        if self.fight.as_ref().unwrap().turn.action {
            return Err("Action used");
        }
        let d = self.project.db.spell(id).cloned().ok_or("No such spell")?;
        if let Some(_why) = self.spell_requirement(id) {
            return Err("Can't cast that now");
        }
        if self.world.mana < d.mana as f32 {
            return Err("Not enough mana");
        }
        let int = self.world.abilities.modifier(crate::srd::Ability::Int);
        let prof = crate::srd::abilities::proficiency_bonus(self.world.player_level());
        let (a, t) = (self.who_tile(who), self.who_tile(target));
        let dist = a.0.abs_diff(t.0).max(a.1.abs_diff(t.1)) as f32;
        match &d.effect {
            SpellEffect::Bolt { damage, projectile, range, damage_type, .. } => {
                let spec = super::AttackSpec {
                    name: d.name.clone(),
                    bonus: int + prof,
                    dice: crate::defs::Weapon::melee(*damage, 1.0).damage_dice(),
                    damage_type: *damage_type,
                    ranged: true,
                    reach: 0,
                    range: range.round().max(1.0) as i32,
                    long: range.round().max(1.0) as i32,
                    crit_on: 20,
                    effect: None,
                    projectile: Some(projectile.clone()),
                };
                self.can_attack(who, target, &spec)?;
                self.pay_spell(&d);
                self.make_attack(who, target, &spec);
            }
            SpellEffect::Blast { damage, radius, damage_type } => {
                if dist > 12.0 {
                    return Err("Out of range");
                }
                self.pay_spell(&d);
                let dc = 8 + prof + int;
                let dice = crate::defs::Weapon::melee(*damage, 1.0).damage_dice();
                let center = t;
                let caught: Vec<Who> = self.fight.as_ref().unwrap().order.iter().filter(|f| !f.out && f.who != who).map(|f| f.who).filter(|w| {
                    let p = self.who_tile(*w);
                    (p.0.abs_diff(center.0).max(p.1.abs_diff(center.1)) as f32) <= radius.max(0.5)
                }).collect();
                let at = feet_spot(center, self.ts());
                self.effects.push((at, 0.0));
                for w in caught {
                    let n = crate::srd::Expr::parse(&dice).map(|e| e.roll(&crate::srd::dice::NoMods).total.max(1)).unwrap_or(1) as i32;
                    let saved = self.who_check(w, "dex", dc, true).is_some_and(|r| r.success == Some(true));
                    self.combat_damage(w, if saved { n / 2 } else { n }, *damage_type, false, Some(who));
                }
                self.fight_log(format!("{}: {} (DC {dc})", self.who_name(who), d.name));
            }
            SpellEffect::Heal { amount } => {
                let ally = self.fight.as_ref().unwrap().fighter(target).is_some_and(|f| f.side == self.fight.as_ref().unwrap().current().side);
                if !ally || dist > 1.0 {
                    return Err("Touch an ally");
                }
                self.pay_spell(&d);
                self.combat_heal(target, *amount);
                self.fight_log(format!("{}: {} > {}", self.who_name(who), d.name, self.who_name(target)));
            }
            SpellEffect::Light { radius, seconds } => {
                self.pay_spell(&d);
                self.player_light_for(*radius, *seconds, crate::events::spell_light_color(), false);
            }
            SpellEffect::Script { function } => {
                self.pay_spell(&d);
                self.sync_script_player();
                let ts = self.ts();
                let p = feet_spot(t, ts);
                let args = vec![rhai::Dynamic::from((p.x / ts) as f64), rhai::Dynamic::from((p.y / ts) as f64)];
                let actions = self.scripts.call_args(&mut self.world, function, args, None);
                self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
            }
        }
        self.fight.as_mut().unwrap().turn.action = true;
        Ok(())
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
