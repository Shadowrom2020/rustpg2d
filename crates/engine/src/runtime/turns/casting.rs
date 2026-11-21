//! Casting a spell (SRD 5.1) in a fight, and what a spell's parts do to its targets in and out
//! of fights: the action bar's *Cast* entry aims a spell at a slot level (`Aim::Cast`), the
//! targets come from its range, area (templates on the grid) and target count, then each target
//! takes a spell attack or a saving throw and the parts after it (damage, healing, conditions,
//! buffs, light, unlocking, a script). Concentration: one spell at a time, a CON save on damage.

use macroquad::prelude::*;

use super::{AttackSpec, Who};
use crate::grid::{feet_spot, TilePos};
use crate::runtime::Game;
use crate::skills::{Area, Shape, SpellDef, SpellPart, SpellRange};
use crate::srd::conditions::{self, ConditionSpec};
use crate::srd::combat::Cover;
use crate::srd::{Ability, Mode};

/// Where a spell is aimed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) enum CastAt {
    On(Who),
    Tile(TilePos),
}

/// A spell's effects last its duration (as conditions ticking on the target's turns: one round
/// more, so the last round counts).
fn spell_secs(d: &SpellDef) -> f32 {
    let rounds = if d.duration_rounds == 0 { 1 } else { d.duration_rounds };
    (rounds + 1) as f32 * conditions::ROUND - 0.5
}

impl Game {
    /// The key effects of `who`'s spells carry (`fireball@player`, `bless@130`).
    pub(in crate::runtime) fn spell_source(&self, who: Who, d: &SpellDef) -> String {
        match who {
            Who::Player => format!("{}@player", d.id),
            Who::Npc(uid) => format!("{}@{uid}", d.id),
        }
    }

    /// The tiles a spell's area covers when cast by `who` at `point`: a sphere or cylinder of a
    /// spell with the caster's range around the caster, a cube of one in front of it, cones and
    /// lines from it toward the point.
    pub(in crate::runtime) fn area_tiles(&self, who: Who, d: &SpellDef, point: TilePos) -> Vec<TilePos> {
        let Some(area) = d.area else { return vec![point] };
        let origin = self.who_tile(who);
        let n = (area.feet / 5).max(1) as i32;
        let point = match (d.range, area.shape) {
            (SpellRange::Caster, Shape::Sphere | Shape::Cylinder) => origin,
            (SpellRange::Caster, Shape::Cube) => {
                // (A cube from the caster: its near side next to it, toward the point.)
                let (dx, dy) = ((point.0 - origin.0).signum(), (point.1 - origin.1).signum());
                let (dx, dy) = if dx == 0 && dy == 0 { (1, 0) } else { (dx, dy) };
                (origin.0 + dx * (n / 2 + 1), origin.1 + dy * (n / 2 + 1))
            }
            _ => point,
        };
        Area::tiles(&area, origin, point)
    }

    /// Who a spell aimed at `at` affects in a fight.
    pub(super) fn spell_targets(&self, who: Who, d: &SpellDef, slot: u32, at: CastAt) -> Result<(Vec<Who>, TilePos), &'static str> {
        let c = self.fight.as_ref().ok_or("No fight")?;
        let reach = d.range.tiles();
        // (From the caster's space; to a target's nearest tile.)
        let dist = |t: TilePos| self.dist_to(who, t);
        let point = match at {
            CastAt::On(w) => self.who_tile(w),
            CastAt::Tile(t) => t,
        };
        let point_dist = match at {
            CastAt::On(w) => self.who_dist(who, w),
            CastAt::Tile(t) => dist(t),
        };
        if d.range != SpellRange::Caster && point_dist > reach.max(1) {
            return Err(if d.range == SpellRange::Touch { "Touch it: stand next to it" } else { "Out of range" });
        }
        if let Some(area) = d.area {
            let tiles = self.area_tiles(who, d, point);
            let self_origin = d.range == SpellRange::Caster && matches!(area.shape, Shape::Sphere | Shape::Cylinder | Shape::Cone | Shape::Line | Shape::Cube);
            let hit: Vec<Who> = c.order.iter().filter(|f| !f.out && !(self_origin && f.who == who)).map(|f| f.who).filter(|w| self.who_space(*w).iter().any(|p| tiles.contains(p))).collect();
            return Ok((hit, point));
        }
        if d.range == SpellRange::Caster {
            return Ok((vec![who], point));
        }
        // (A summons appears at an empty tile.)
        let summons = d.parts.iter().any(|p| matches!(p, SpellPart::Summon { .. }));
        let target = match at {
            CastAt::On(w) => w,
            CastAt::Tile(t) => match self.who_at(t) {
                Some(w) => w,
                None if summons => return Ok((Vec::new(), point)),
                None => return Err("No one there"),
            },
        };
        // Charmed: no harmful spell (an attack or a save) at the charmer.
        if self.charmed_by(who) == Some(target) && d.parts.iter().any(|p| matches!(p, SpellPart::Attack { .. } | SpellPart::Save { .. })) {
            return Err("Charmed");
        }
        let extra = |count: u32, per: u32| (count + per * slot.saturating_sub(d.level.max(1))) as usize;
        let darts = !d.parts.iter().any(|p| matches!(p, SpellPart::Attack { .. } | SpellPart::Save { .. })) && d.parts.iter().any(|p| matches!(p, SpellPart::Damage { .. }));
        let mut targets = vec![target];
        if let Some((count, per)) = d.parts.iter().find_map(|p| if let SpellPart::Targets { count, per_slot } = p { Some((*count, *per_slot)) } else { None }) {
            let n = extra(count, per);
            if darts {
                // (Magic Missile: every dart at the target.)
                targets = vec![target; n.max(1)];
            } else {
                // More creatures of the target's side within range, nearest first.
                let side = c.fighter(target).map(|f| f.side);
                let mut more: Vec<Who> = c.order.iter().filter(|f| !f.out && f.who != target && Some(f.side) == side).map(|f| f.who).filter(|w| self.who_dist(who, *w) <= reach.max(1)).collect();
                more.sort_by_key(|w| self.who_dist(who, *w));
                targets.extend(more.into_iter().take(n.saturating_sub(1)));
            }
        }
        Ok((targets, point))
    }

    /// Casts spell `id` of the current fighter at slot level `slot` (0 = a cantrip) at `at`:
    /// the cost and the slot paid, the targets resolved. `scroll`: cast from a scroll (no slot).
    pub(super) fn cast_spell_in_fight(&mut self, id: &str, slot: u32, at: CastAt, scroll: bool) -> Result<(), &'static str> {
        let c = self.fight.as_ref().ok_or("No fight")?;
        let who = c.current().who;
        let d = self.project.db.spell(id).cloned().ok_or("No such spell")?;
        let slot = if d.level == 0 { 0 } else { slot.max(d.level) };
        let t = &c.turn;
        match d.casting {
            crate::skills::SpellCast::Bonus if t.bonus => return Err("Bonus action used"),
            crate::skills::SpellCast::Reaction if c.fighter(who).is_some_and(|f| f.reacted) => return Err("Reaction used"),
            crate::skills::SpellCast::Action if t.action => return Err("Action used"),
            crate::skills::SpellCast::Minutes(_) => return Err("Too long to cast in a fight"),
            _ => {}
        }
        if scroll && t.action {
            return Err("Action used");
        }
        if !scroll && !self.cast_options(who).iter().any(|(s, l)| s == id && *l == slot) {
            return Err(self.cast_refusal(who, id, "You can't cast that spell"));
        }
        let (targets, point) = self.spell_targets(who, &d, slot, at)?;
        if targets.is_empty() && !d.parts.iter().any(|p| matches!(p, SpellPart::Script { .. } | SpellPart::Summon { .. })) {
            return Err("No one there");
        }
        if self.monster_casting(who).is_some() {
            self.monster_pay(who, id, slot);
        } else if !scroll && slot >= 1 {
            self.spend_slot(who, slot);
        }
        if let Some(c) = &mut self.fight {
            match (scroll, d.casting) {
                (true, _) | (_, crate::skills::SpellCast::Action) => c.turn.action = true,
                (_, crate::skills::SpellCast::Bonus) => c.turn.bonus = true,
                (_, crate::skills::SpellCast::Reaction) => {
                    if let Some(f) = c.fighter_mut(who) {
                        f.reacted = true;
                    }
                }
                _ => {}
            }
            if let Some(f) = c.fighter_mut(who) {
                f.hidden = false;
            }
        }
        let name = self.who_name(who);
        let slot_text = if slot >= 1 { format!(" ({} {slot})", crate::lang::tr_ui("slot")) } else { String::new() };
        let dc = if d.parts.iter().any(|p| matches!(p, SpellPart::Save { .. })) { format!(" DC {}", self.spell_dc(who)) } else { String::new() };
        self.fight_log(format!("{name}: {}{slot_text}{dc}", crate::lang::tr(&d.name)));
        // The look: a projectile to the point, the impact there.
        let ts = self.ts();
        let (from, to) = (self.who_pos(who) - vec2(0.0, ts * 0.5), feet_spot(point, ts) - vec2(0.0, ts * 0.5));
        let attack = d.parts.iter().any(|p| matches!(p, SpellPart::Attack { .. }));
        if let (Some(p), false) = (&d.visual.projectile, attack) {
            self.fly_visual(from, to, p.clone());
        }
        self.spell_impact(&d, feet_spot(point, ts));
        self.resolve_spell(who, &d, slot, &targets, Some(point));
        Ok(())
    }

    /// A spell's impact effect and sound where it lands (an area: a burst of its size).
    pub(in crate::runtime) fn spell_impact(&mut self, d: &SpellDef, at: Vec2) {
        let ts = self.ts();
        if !d.visual.sound.is_empty() {
            self.audio.play(&d.visual.sound);
        } else {
            self.audio.play(&self.project.config.sfx.attack);
        }
        if !d.visual.impact.is_empty() {
            let scale = if d.visual.impact_scale > 0.0 { d.visual.impact_scale } else { 1.0 };
            let sprite = crate::events::FxSprite { sheet: d.visual.impact.clone(), fps: 14.0, scale, light: 3.0, ..Default::default() };
            self.spawn_effect(sprite, at / ts);
        } else if d.area.is_some() {
            self.effects.push((at, 0.0));
        }
    }

    /// What a spell does to each of `targets` (in or out of a fight): a spell attack (its damage on
    /// a hit) or a saving throw (half or none), then damage, healing, temporary hit points,
    /// conditions and buffs for its duration (with the spell as their source), light, unlocking,
    /// a script; concentration starts.
    pub(in crate::runtime) fn resolve_spell(&mut self, caster: Who, d: &SpellDef, slot: u32, targets: &[Who], point: Option<TilePos>) {
        let lvl = self.scale_level(caster, d, slot);
        let dice_of = |g: &Game, by: &[(u32, String)]| crate::skills::dice_at(by, lvl).map(|x| g.spell_dice(caster, x));
        let roll = |dice: &str| crate::srd::Expr::parse(dice).map(|e| e.roll(&crate::srd::dice::NoMods).total.max(0)).unwrap_or(0) as i32;
        let attack = d.parts.iter().find_map(|p| if let SpellPart::Attack { ranged } = p { Some(*ranged) } else { None });
        let save = d.parts.iter().find_map(|p| if let SpellPart::Save { ability, half } = p { Some((*ability, *half)) } else { None });
        let damage = d.parts.iter().find_map(|p| if let SpellPart::Damage { by_level, damage_type } = p { Some((by_level.clone(), *damage_type)) } else { None });
        let source = self.spell_source(caster, d);
        let secs = spell_secs(d);
        let dc = self.spell_dc(caster);
        // A save spell rolls its damage once for everyone (the SRD's areas).
        let shared = match (&save, &damage) {
            (Some(_), Some((by, _))) => dice_of(self, by).map(|x| roll(&x)),
            _ => None,
        };
        if d.concentration {
            self.begin_concentration(caster, &source, secs);
        }
        for &t in targets {
            if self.fight.as_ref().and_then(|c| c.fighter(t)).is_some_and(|f| f.out) {
                continue;
            }
            let mut landed = true;
            if let Some(ranged) = attack {
                let dice = damage.as_ref().and_then(|(by, _)| dice_of(self, by)).unwrap_or_else(|| "0".into());
                let range = d.range.tiles().max(1);
                let spec = AttackSpec {
                    name: crate::lang::tr(&d.name),
                    bonus: self.spell_attack_bonus(caster),
                    dice,
                    damage_type: damage.as_ref().and_then(|x| x.1),
                    ranged,
                    reach: if ranged { 0 } else { range },
                    range: if ranged { range } else { 0 },
                    long: if ranged { range } else { 0 },
                    crit_on: 20,
                    projectile: d.visual.projectile.clone(),
                    spell: true,
                    hit: crate::srd::HitKind::Magical,
                    ..Default::default()
                };
                landed = self.make_attack(caster, t, &spec);
            }
            let mut saved = false;
            let mut save_text = String::new();
            if let Some((ability, _)) = save {
                // Cover between the effect's origin and the target: +2 / +5 to a DEX save; total
                // cover keeps it out.
                let cover = if ability == Ability::Dex { self.cover_from(self.spell_origin(caster, d, point), caster, t) } else { Cover::None };
                if cover == Cover::Total && t != caster {
                    continue;
                }
                let rec = self.who_check(t, ability.id(), dc - cover.ac_bonus(), true);
                saved = rec.as_ref().is_some_and(|r| r.success == Some(true));
                let cover_text = if cover == Cover::None { String::new() } else { format!(" [{}]", crate::lang::tr_ui(cover.id())) };
                save_text = format!("{} {}/{dc}{cover_text} {}", ability.short(), rec.as_ref().map_or(0, |r| r.total + cover.ac_bonus() as i64), crate::lang::tr_ui(if saved { "saved" } else { "failed" }));
                landed = !saved;
            }
            let tname = self.who_name(t);
            for part in &d.parts {
                match part {
                    SpellPart::Damage { by_level, damage_type } if attack.is_none() => {
                        let full = match shared {
                            Some(n) => n,
                            None => dice_of(self, by_level).map_or(0, |x| roll(&x)),
                        };
                        let half = save.is_some_and(|s| s.1);
                        // (Evasion: none on a passed DEX save, half on a failed one.)
                        let evasion = save.is_some_and(|s| s.0 == Ability::Dex) && self.who_traits(t).is_some_and(|tr| tr.evasion);
                        let n = match (save.is_some(), saved, half, evasion) {
                            (false, _, _, _) => full,
                            (true, true, _, true) => 0,
                            (true, true, true, false) => full / 2,
                            (true, true, false, false) => 0,
                            (true, false, _, true) => full / 2,
                            (true, false, _, false) => full,
                        };
                        let line = if save_text.is_empty() { format!("{tname}: {n}") } else { format!("{tname}: {save_text}, {n}") };
                        self.fight_log(line);
                        if n > 0 {
                            self.combat_damage_magic(t, n, *damage_type, Some(caster));
                        }
                    }
                    SpellPart::Heal { by_level } => {
                        let n = dice_of(self, by_level).map_or(0, |x| roll(&x));
                        if n > 0 && self.fight.is_some() {
                            self.fight_log(format!("{tname}: +{n} HP"));
                        }
                        if n > 0 {
                            match (self.fight.is_none(), t) {
                                (true, Who::Player) => self.heal(n),
                                _ => self.combat_heal(t, n),
                            }
                        }
                    }
                    SpellPart::TempHp { by_level } => {
                        let n = dice_of(self, by_level).map_or(0, |x| roll(&x));
                        self.give_temp_hp(t, n);
                    }
                    SpellPart::Condition { condition, on_fail } => {
                        if *on_fail && !landed {
                            if !save_text.is_empty() && damage.is_none() {
                                self.fight_log(format!("{tname}: {save_text}"));
                            }
                            continue;
                        }
                        let tag = self.who_tag(caster);
                        if let Some(l) = self.who_conditions_mut(t) {
                            conditions::apply_from(l, &ConditionSpec::new(*condition, secs), &source);
                            conditions::set_origin(l, *condition, &tag, 0);
                        }
                        let saved_text = if save_text.is_empty() || damage.is_some() { String::new() } else { format!("{save_text}, ") };
                        self.fight_log(format!("{tname}: {saved_text}{}", crate::lang::tr_ui(condition.name())));
                    }
                    SpellPart::Stabilize => self.stabilize(t, ""),
                    SpellPart::Cure { conditions: list, disease } => {
                        let ended = self.who_conditions_mut(t).and_then(|l| {
                            let c = list.iter().copied().find(|c| conditions::has(l, *c))?;
                            conditions::remove(l, c);
                            Some(c)
                        });
                        if let Some(c) = ended {
                            self.fight_log(format!("{tname}: {} {}", crate::lang::tr_ui("no longer"), crate::lang::tr_ui(c.name())));
                        } else if *disease && t == Who::Player {
                            self.cure_one_disease();
                        }
                    }
                    SpellPart::Buff { buff } => {
                        if (save.is_some() || attack.is_some()) && !landed {
                            continue;
                        }
                        if let Some(l) = self.who_conditions_mut(t) {
                            conditions::apply_buff(l, buff.clone(), secs, &source);
                        }
                    }
                    // (A willing ally or the caster: no save.)
                    SpellPart::Shape { max_cr } => {
                        let ally = t == caster || self.fight.as_ref().is_some_and(|c| c.fighter(t).map(|f| f.side) == c.fighter(caster).map(|f| f.side));
                        if !save_text.is_empty() && damage.is_none() && !ally {
                            self.fight_log(format!("{tname}: {save_text}"));
                        }
                        if (save.is_some() || attack.is_some()) && !landed && !ally {
                            continue;
                        }
                        let secs = if d.duration_rounds == 0 { 0.0 } else { secs };
                        self.spell_shape(caster, t, max_cr, &source, secs);
                    }
                    _ => {}
                }
            }
        }
        // Parts that act once, not per target.
        for part in &d.parts {
            match part {
                SpellPart::Light { radius } => {
                    let secs = if d.duration_rounds == 0 { 3600.0 } else { d.duration_rounds as f32 * conditions::ROUND };
                    if caster == Who::Player {
                        self.player_light_for(*radius, secs, crate::events::spell_light_color(), false);
                    } else {
                        let p = self.who_pos(caster);
                        self.light_at(p, *radius, secs, crate::events::spell_light_color(), false);
                    }
                }
                SpellPart::Unlock => self.knock(),
                SpellPart::Summon { creature, count } => {
                    let secs = if d.duration_rounds == 0 { 0.0 } else { spell_secs(d) };
                    let source = self.spell_source(caster, d);
                    let at = point.unwrap_or_else(|| self.who_tile(caster));
                    self.summon(caster, at, creature, *count, &source, secs);
                }
                SpellPart::Script { function } => {
                    self.sync_script_player();
                    let t = point.unwrap_or_else(|| self.who_tile(targets.first().copied().unwrap_or(caster)));
                    let key = if caster == Who::Player { "player".to_string() } else { self.who_key(caster) };
                    let args = vec![rhai::Dynamic::from(key), rhai::Dynamic::from(t.0 as f64 + 0.5), rhai::Dynamic::from(t.1 as f64 + 0.5), rhai::Dynamic::from(slot as i64)];
                    let actions = self.scripts.call_args(&mut self.world, function, args, None);
                    self.runner.push(&actions, &crate::events::runner::ActionCtx::default());
                }
                _ => {}
            }
        }
    }

    /// Where a spell's effect comes from (cover is counted from there): the point of an area
    /// spread around a point (a Fireball's), else the caster.
    pub(super) fn spell_origin(&self, caster: Who, d: &SpellDef, point: Option<TilePos>) -> TilePos {
        match (d.area, point) {
            (Some(a), Some(p)) if d.range != SpellRange::Caster && matches!(a.shape, Shape::Sphere | Shape::Cylinder) => p,
            _ => self.who_tile(caster),
        }
    }

    /// Temporary hit points: in a fight the fighter's (the larger counts), else nothing to keep.
    fn give_temp_hp(&mut self, who: Who, n: i32) {
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.fx.temp_hp = f.fx.temp_hp.max(n);
        }
    }

    /// Knock: the nearest locked door, chest or gate within two tiles of the hero opens.
    fn knock(&mut self) {
        let ts = self.ts();
        let p = self.player.pos;
        let near = (0..self.loaded.entities.len()).filter(|&i| self.loaded.entities[i].active && self.active_lock(i).is_some()).min_by(|a, b| {
            let (da, db) = (self.loaded.entities[*a].body.pos.distance(p), self.loaded.entities[*b].body.pos.distance(p));
            da.total_cmp(&db)
        });
        match near.filter(|i| self.loaded.entities[*i].body.pos.distance(p) <= 2.5 * ts) {
            Some(i) => {
                let lock = self.active_lock(i).unwrap();
                self.audio.play(&self.project.config.sfx.unlock);
                self.apply_unlock(i, &lock.on_unlock);
                let uid = self.loaded.entities[i].uid;
                self.fire(crate::events::GameEvent::Unlock, crate::runtime::Source::Entity(uid));
            }
            None => self.ui.toast(crate::lang::tr_ui("Nothing locked nearby").into_owned()),
        }
    }

    /// Damage while concentrating: a CON save (DC 10 or half the damage) or the spell ends.
    pub(super) fn concentration_check(&mut self, who: Who, damage: i32) {
        if damage <= 0 || self.concentration_of(who).is_none() {
            return;
        }
        let dc = (damage / 2).max(10);
        let ok = self.who_check(who, "con", dc, true).is_some_and(|r| r.success == Some(true));
        if !ok {
            let name = self.who_name(who);
            self.fight_log(format!("{name}: {}", crate::lang::tr_ui("loses concentration")));
            self.end_concentration(who);
        }
    }

    /// A scroll read in a fight: the spell aims like *Cast* (the action; no slot).
    pub(in crate::runtime) fn scroll_in_fight(&mut self, spell: &str) -> bool {
        let Some(c) = &self.fight else { return false };
        if c.current().who != Who::Player || c.current().ai || c.turn.action {
            self.ui.toast(crate::lang::tr_ui("Action used").into_owned());
            return false;
        }
        let level = self.project.db.spell(spell).map_or(1, |d| d.level);
        if let Some(c) = &mut self.fight {
            c.scroll = Some(spell.to_string());
        }
        self.aim_spell(spell, level);
        // (The scroll is used up when the spell is cast.)
        false
    }

    /// The cursor aims spell `id` at slot level `slot`: on the caster for its own spells, the
    /// nearest ally for healing and buffs, the nearest foe otherwise.
    pub(super) fn aim_spell(&mut self, id: &str, slot: u32) {
        let Some(c) = &self.fight else { return };
        let me = c.current().clone();
        let here = self.who_tile(me.who);
        let d = self.project.db.spell(id).cloned();
        let selfish = d.as_ref().is_some_and(|d| d.range == SpellRange::Caster);
        let friendly = d.as_ref().is_some_and(|d| d.heals() || (!d.offensive() && d.parts.iter().any(|p| matches!(p, SpellPart::Buff { .. } | SpellPart::TempHp { .. }))));
        let near = c.order.iter().filter(|f| !f.out && (friendly || f.who != me.who) && ((f.side == me.side) == friendly)).map(|f| self.who_tile(f.who)).min_by_key(|t| t.0.abs_diff(here.0).max(t.1.abs_diff(here.1)));
        let c = self.fight.as_mut().unwrap();
        c.aim = super::Aim::Cast(id.to_string(), slot);
        c.cursor = if selfish { here } else { near.unwrap_or(here) };
    }

    /// Confirm with a spell aimed: cast it at the cursor (a creature there, else the tile for an
    /// area or the caster's own spells).
    pub(super) fn confirm_spell(&mut self, id: &str, slot: u32) -> Result<(), &'static str> {
        let c = self.fight.as_ref().ok_or("No fight")?;
        let (cursor, me) = (c.cursor, c.current().who);
        let d = self.project.db.spell(id).cloned().ok_or("No such spell")?;
        let summons = d.parts.iter().any(|p| matches!(p, SpellPart::Summon { .. }));
        let at = if d.area.is_some() || d.range == SpellRange::Caster || (summons && self.who_at(cursor).is_none()) {
            CastAt::Tile(cursor)
        } else {
            CastAt::On(self.who_at(cursor).ok_or("No one there")?)
        };
        let scroll = self.fight.as_ref().and_then(|c| c.scroll.clone()).filter(|s| s == id);
        if let Some(r) = self.cast_item_spell(id, at) {
            return r;
        }
        self.cast_spell_in_fight(id, slot, at, scroll.is_some())?;
        if scroll.is_some() {
            if let Some(item) = self.project.db.items.iter().find(|i| i.teaches == id && self.world.item_count(&i.id) > 0).map(|i| i.id.clone()) {
                self.world.take_item(&item, 1);
            }
            if let Some(c) = &mut self.fight {
                c.scroll = None;
            }
        }
        let _ = me;
        Ok(())
    }

    /// A spell from an item picked in the Cast window (`Combat::item_cast`, when it is `id`): no
    /// slot (the action), the item's DC while it resolves, then its charges. None = not one.
    pub(super) fn cast_item_spell(&mut self, id: &str, at: CastAt) -> Option<Result<(), &'static str>> {
        let cast = self.fight.as_ref().and_then(|c| c.item_cast.clone()).filter(|c| c.spell.spell == id)?;
        let me = self.fight.as_ref()?.current().who;
        self.casting_item = Some(cast.clone());
        let r = self.cast_spell_in_fight(id, cast.slot, at, true);
        self.casting_item = None;
        if let Err(e) = r {
            return Some(Err(e));
        }
        if let Some(c) = &mut self.fight {
            c.item_cast = None;
        }
        Some(if cast.charges > 0 { self.spend_charges(me, &cast.item, cast.charges) } else { Ok(()) })
    }

    /// The line above the action bar while a spell is aimed: its name, slot, area, save or
    /// attack, and how many it hits under the cursor.
    pub(super) fn spell_aim_line(&self, id: &str, slot: u32) -> String {
        let Some(d) = self.project.db.spell(id) else { return id.to_string() };
        let Some(c) = &self.fight else { return String::new() };
        let me = c.current().who;
        let mut s = crate::lang::tr(&d.name);
        if slot >= 1 {
            s += &format!("  {} {slot}", crate::lang::tr_ui("slot"));
        }
        if let Some(a) = d.area {
            s += &format!("  {}", a.label());
        }
        if let Some((ab, _)) = d.parts.iter().find_map(|p| if let SpellPart::Save { ability, half } = p { Some((*ability, *half)) } else { None }) {
            s += &format!("  {} DC {}", ab.short(), self.spell_dc(me));
        } else if d.parts.iter().any(|p| matches!(p, SpellPart::Attack { .. })) {
            s += &format!("  {:+}", self.spell_attack_bonus(me));
        }
        if c.scroll.as_deref() == Some(id) {
            s += &format!("  ({})", crate::lang::tr_ui("scroll"));
        }
        s
    }

    /// The tiles a spell aimed now would cover (the overlay).
    pub(super) fn aimed_area(&self) -> Vec<TilePos> {
        let Some(c) = &self.fight else { return Vec::new() };
        let super::Aim::Cast(id, _) = &c.aim else { return Vec::new() };
        let Some(d) = self.project.db.spell(id) else { return Vec::new() };
        if d.area.is_none() {
            return Vec::new();
        }
        self.area_tiles(c.current().who, d, c.cursor)
    }

    /// A Bless / Bane die rolled for an attack roll (`attack`) or a saving throw: what the buffs
    /// add (negative for Bane).
    pub(in crate::runtime) fn buff_roll(&self, who: Who, attack: bool) -> i32 {
        let roll = |d: &str| crate::srd::Expr::parse(d).map(|e| e.roll(&crate::srd::dice::NoMods).total).unwrap_or(0) as i32;
        conditions::buffs(self.who_conditions(who))
            .map(|b| match b {
                conditions::Buff::AttackDie { dice, minus } if attack => roll(dice) * if *minus { -1 } else { 1 },
                conditions::Buff::SaveDie { dice, minus } if !attack => roll(dice) * if *minus { -1 } else { 1 },
                _ => 0,
            })
            .sum()
    }

    /// Armor class with the buffs: Shield of Faith's and Shield's bonus, Mage Armor's base + DEX
    /// (unarmored), Haste's +2.
    pub(in crate::runtime) fn buff_ac(&self, who: Who, ac: i32) -> i32 {
        let dex = match who {
            Who::Player => self.world.abilities.modifier(Ability::Dex),
            w => self.who_block(w).map_or(0, |b| b.abilities.modifier(Ability::Dex)),
        };
        let unarmored = who != Who::Player || self.world.equipment.item(crate::defs::EquipSlot::Body).is_none();
        let mut out = ac;
        for b in conditions::buffs(self.who_conditions(who)) {
            match b {
                conditions::Buff::AcBase { base } if unarmored => out = out.max(base + dex),
                _ => {}
            }
        }
        for b in conditions::buffs(self.who_conditions(who)) {
            match b {
                conditions::Buff::Ac { bonus } => out += bonus,
                conditions::Buff::Haste => out += 2,
                _ => {}
            }
        }
        out
    }

    /// Speed with the buffs (Longstrider, Ray of Frost's slowing; Haste doubles it).
    pub(in crate::runtime) fn buff_speed(&self, who: Who, speed: u32) -> u32 {
        let mut s = speed as i32;
        let mut haste = false;
        for b in conditions::buffs(self.who_conditions(who)) {
            match b {
                conditions::Buff::Speed { feet } => s += feet,
                conditions::Buff::Haste => haste = true,
                _ => {}
            }
        }
        (s.max(0) * if haste { 2 } else { 1 }) as u32
    }

    /// Damage types the buffs resist (Stoneskin, Protection from Poison).
    pub(in crate::runtime) fn buff_resistances(&self, who: Who) -> Vec<String> {
        conditions::buffs(self.who_conditions(who)).filter_map(|b| if let conditions::Buff::Resistance { types } = b { Some(types.clone()) } else { None }).flatten().collect()
    }

    /// Mode for a saving throw from a spell buff on the saver (Bless / Bane add or take a die;
    /// Haste: advantage on DEX).
    pub(super) fn buff_save_mode(&self, who: Who, what: &str) -> Mode {
        let haste = conditions::buffs(self.who_conditions(who)).any(|b| matches!(b, conditions::Buff::Haste));
        // (A save against poison: Antitoxin, Protection from Poison, a dwarf's resilience.)
        let poison = if self.poison_save { self.poison_save_mode(who) } else { Mode::Normal };
        if haste && (what.eq_ignore_ascii_case("dex") || what.eq_ignore_ascii_case("save:dex")) {
            Mode::Advantage
        } else {
            poison
        }
    }
}
