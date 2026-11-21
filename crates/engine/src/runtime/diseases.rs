//! The SRD's diseases in play (`diseases.rs`): the hero catches one (a monster's bite, *Expose to
//! disease*, `infect()`) after its saving throw; its symptoms show after its onset (exhaustion, a
//! condition, failing sight); it blocks or halves natural healing, takes hit point maximum every
//! 24 hours, throws a fit when the hero is hurt, and runs its course over long rests until it is
//! over or cured (*Cure disease*, a Cure spell part, `cure_disease()`).

use super::Game;
use crate::diseases::{ActiveDisease, Course, DiseaseDef, RestOutcome, SIGHT_BLIND};
use crate::srd::conditions::{self, Condition, ConditionSpec};
use crate::srd::Mode;

impl Game {
    fn disease_def(&self, id: &str) -> Option<DiseaseDef> {
        self.project.db.disease(id).cloned()
    }

    /// The hero meets disease `id`: its saving throw (when `save` and it has a DC), then it waits
    /// for its onset. Nothing when the hero has it already.
    pub(super) fn expose_disease(&mut self, id: &str, save: bool) {
        let Some(def) = self.disease_def(id) else {
            eprintln!("[events] Expose to disease: no disease '{id}'");
            return;
        };
        if self.world.diseases.iter().any(|d| d.id == id) {
            return;
        }
        if save && def.dc > 0 && self.world.check(def.save.id(), def.dc, Mode::Normal, true).is_some_and(|r| r.success == Some(true)) {
            return;
        }
        self.catch_disease(&def);
    }

    /// The hero has caught `def` (a failed save): it waits for its onset.
    pub(super) fn catch_disease(&mut self, def: &DiseaseDef) {
        if self.world.diseases.iter().any(|d| d.id == def.id) {
            return;
        }
        let rolled = if def.onset.is_empty() { 0 } else { crate::srd::Expr::parse(&def.onset).map(|e| e.roll(&crate::srd::dice::NoMods).total as i32).unwrap_or(0) };
        let onset = self.world.total_hours() + def.onset_hours(rolled);
        self.world.diseases.push(ActiveDisease::new(def, onset));
        // (No onset: the symptoms show at once.)
        self.update_diseases();
    }

    /// Ends the hero's disease `id` ("" = every one): its condition, blindness and lost hit
    /// point maximum go; its exhaustion stays for rests to take away.
    pub(super) fn cure_disease(&mut self, id: &str) -> bool {
        let (gone, kept): (Vec<ActiveDisease>, Vec<ActiveDisease>) = std::mem::take(&mut self.world.diseases).into_iter().partition(|d| id.is_empty() || d.id == id);
        self.world.diseases = kept;
        for a in &gone {
            self.end_symptoms(a);
            let name = self.disease_def(&a.id).map_or(a.id.clone(), |d| crate::lang::tr(&d.name));
            if a.sick {
                self.ui.toast(crate::lang::tr_ui("You are cured of {}.").replacen("{}", &name, 1));
            }
        }
        !gone.is_empty()
    }

    /// One disease of the hero's ends (a Cure spell part): the sick one first.
    pub(super) fn cure_one_disease(&mut self) -> bool {
        let id = self.world.diseases.iter().find(|d| d.sick).or(self.world.diseases.first()).map(|d| d.id.clone());
        id.is_some_and(|id| self.cure_disease(&id))
    }

    fn end_symptoms(&mut self, a: &ActiveDisease) {
        let Some(def) = self.disease_def(&a.id) else { return };
        if let Some(c) = def.condition.filter(|_| a.sick) {
            conditions::remove(&mut self.world.effects, c);
        }
        if def.sight && a.stage + 1 >= SIGHT_BLIND {
            conditions::remove(&mut self.world.effects, Condition::Blinded);
        }
    }

    /// Every frame (cheap): onsets, and the hit point maximum a disease takes every 24 hours.
    pub(super) fn update_diseases(&mut self) {
        if self.world.diseases.is_empty() {
            return;
        }
        let now = self.world.total_hours();
        for k in 0..self.world.diseases.len() {
            let a = self.world.diseases[k].clone();
            let Some(def) = self.disease_def(&a.id) else { continue };
            if !a.sick && now >= a.onset_at {
                self.world.diseases[k].sick = true;
                self.world.diseases[k].last_loss = Some(now);
                self.ui.toast(crate::lang::tr_ui("You feel sick: {}.").replacen("{}", &crate::lang::tr(&def.name), 1));
                if def.exhaustion > 0 {
                    conditions::apply(&mut self.world.effects, &ConditionSpec { level: def.exhaustion, ..ConditionSpec::new(Condition::Exhaustion, 0.0) });
                }
                if let Some(c) = def.condition {
                    self.apply_player_effect(&ConditionSpec::new(c, 0.0));
                }
                continue;
            }
            // Every 24 hours the hit point maximum drops; at 0 the hero dies.
            if a.sick && !def.max_hp_loss.is_empty() && a.last_loss.is_some_and(|t| now - t >= 24.0) {
                let lost = crate::srd::Expr::parse(&def.max_hp_loss).map(|e| e.roll(&crate::srd::dice::NoMods).total as i32).unwrap_or(0).max(1);
                let before = self.player_max_hp() + self.disease_hp_lost();
                let d = &mut self.world.diseases[k];
                d.hp_lost += lost;
                d.last_loss = d.last_loss.map(|t| t + 24.0);
                let left = before - self.disease_hp_lost();
                self.ui.toast(crate::lang::tr_ui("{} weakens you: -{} hit point maximum.").replacen("{}", &crate::lang::tr(&def.name), 1).replacen("{}", &lost.to_string(), 1));
                if left <= 0 {
                    self.world.player_hp = Some(0);
                    self.player_defeated();
                    return;
                }
                let max = self.player_max_hp();
                if self.hp() > max {
                    self.world.player_hp = Some(max);
                }
            }
        }
    }

    /// After a long rest: each sick disease's course (a save against its DC, or worse sight).
    pub(super) fn rest_diseases(&mut self) {
        for k in (0..self.world.diseases.len()).rev() {
            let mut a = self.world.diseases[k].clone();
            let Some(def) = self.disease_def(&a.id) else { continue };
            if !a.sick || def.course == Course::None {
                continue;
            }
            let saved = match def.course {
                Course::ShrinkingDc { .. } | Course::Exhaustion => self.world.check(def.save.id(), a.dc.max(1), Mode::Normal, true).map(|r| r.success == Some(true)),
                _ => None,
            };
            let shrink = match &def.course {
                Course::ShrinkingDc { dice } => crate::srd::Expr::parse(dice).map(|e| e.roll(&crate::srd::dice::NoMods).total as i32).unwrap_or(1),
                _ => 0,
            };
            let out = a.rest(&def, saved, shrink, conditions::exhaustion(&self.world.effects));
            let name = crate::lang::tr(&def.name);
            match out {
                RestOutcome::Cured => {
                    if def.course == Course::Exhaustion {
                        conditions::lower_exhaustion(&mut self.world.effects, 1);
                    }
                    self.world.diseases[k] = a;
                    let id = def.id.clone();
                    self.cure_disease(&id);
                    continue;
                }
                RestOutcome::Better => {
                    if def.course == Course::Exhaustion {
                        conditions::lower_exhaustion(&mut self.world.effects, 1);
                    }
                    self.ui.toast(crate::lang::tr_ui("{} eases.").replacen("{}", &name, 1));
                }
                RestOutcome::Worse => {
                    if def.course == Course::Exhaustion {
                        conditions::apply(&mut self.world.effects, &ConditionSpec::new(Condition::Exhaustion, 0.0));
                    }
                    if def.sight && a.sight_penalty(&def) >= SIGHT_BLIND {
                        self.apply_player_effect(&ConditionSpec::new(Condition::Blinded, 0.0));
                    }
                    self.ui.toast(crate::lang::tr_ui("{} gets worse.").replacen("{}", &name, 1));
                }
                RestOutcome::Stays => {}
            }
            self.world.diseases[k] = a;
        }
    }

    fn sick_with(&self) -> Vec<DiseaseDef> {
        self.world.diseases.iter().filter(|d| d.sick).filter_map(|d| self.disease_def(&d.id)).collect()
    }

    /// A disease keeps its exhaustion: long rests don't lower it.
    pub(super) fn disease_keeps_exhaustion(&self) -> bool {
        self.sick_with().iter().any(|d| d.keeps_exhaustion)
    }

    /// Long rests give no hit points.
    pub(super) fn disease_blocks_rest_healing(&self) -> bool {
        self.sick_with().iter().any(|d| d.no_natural_healing || d.no_rest_healing)
    }

    /// What hit dice heal: all, half (Sewer Plague) or nothing (only magic heals).
    pub(super) fn disease_hit_dice_heal(&self, gain: i32) -> i32 {
        let sick = self.sick_with();
        if sick.iter().any(|d| d.no_natural_healing) {
            0
        } else if sick.iter().any(|d| d.half_hit_dice) {
            gain / 2
        } else {
            gain
        }
    }

    /// The hit point maximum the hero's diseases took.
    pub(super) fn disease_hp_lost(&self) -> i32 {
        self.world.diseases.iter().map(|d| d.hp_lost).sum()
    }

    /// Failing sight's penalty to attack rolls and checks by sight.
    pub(super) fn disease_sight_penalty(&self) -> i32 {
        self.world.diseases.iter().filter_map(|a| self.project.db.disease(&a.id).map(|d| a.sight_penalty(d))).max().unwrap_or(0)
    }

    /// The hero was hurt: a disease's fit (Cackle Fever's laughter) unless a save at its DC.
    pub(super) fn disease_fit(&mut self) {
        if self.in_disease_fit {
            return;
        }
        let fits: Vec<(DiseaseDef, i32)> = self.world.diseases.iter().filter(|a| a.sick).filter_map(|a| self.disease_def(&a.id).filter(|d| d.when_hurt.is_some()).map(|d| (d, a.dc))).collect();
        for (def, dc) in fits {
            let Some(w) = def.when_hurt.clone() else { continue };
            if self.world.check(def.save.id(), dc.max(1), Mode::Normal, true).is_some_and(|r| r.success == Some(true)) {
                continue;
            }
            // (Its own damage doesn't start another fit.)
            self.in_disease_fit = true;
            self.ui.toast(crate::lang::tr_ui("{} seizes you!").replacen("{}", &crate::lang::tr(&def.name), 1));
            if !w.damage.is_empty() {
                let n = crate::srd::Expr::parse(&w.damage).map(|e| e.roll(&crate::srd::dice::NoMods).total as i32).unwrap_or(0);
                let from = self.player.pos;
                self.hurt_player_by(n.max(1), from, w.damage_type);
            }
            if let Some(c) = w.condition.filter(|_| self.hp() > 0) {
                self.apply_player_effect(&ConditionSpec::new(c, w.seconds));
            }
            self.in_disease_fit = false;
        }
    }

    /// `diseases=` for status: `id:onset@HOURS` (incubating) or `id:sick:dc/stage/-hp`.
    pub(super) fn diseases_status(&self) -> String {
        if self.world.diseases.is_empty() {
            return "-".into();
        }
        let now = self.world.total_hours();
        self.world.diseases.iter().map(|d| if d.sick { format!("{}:sick:{}/{}/-{}", d.id, d.dc, d.stage, d.hp_lost) } else { format!("{}:onset@{:.1}", d.id, (d.onset_at - now).max(0.0)) }).collect::<Vec<_>>().join(",")
    }
}
