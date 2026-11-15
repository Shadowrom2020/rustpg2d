//! Quests described in the quest editor (`QuestDef::giver`, `offer`, `steps`, `rewards`): the
//! giver offers them when talked to, Talk steps are done by talking to their character, Reach
//! steps by arriving at the stage's marker target, Conditions steps as soon as their conditions
//! hold; each step's actions run when it starts and when it is done, and completing a quest
//! gives its rewards. Everything is played as ordinary actions on the event runner (dialogs,
//! choices, the Quest action), so quests moved on by triggers and scripts work the same.

use macroquad::prelude::*;

use super::{conditions_met, ActionCtx, Game};
use crate::entity::EntityKind;
use crate::events::Action;
use crate::quest::{QuestDef, QuestStatus, StepGoal};
/// Seconds between checks of Conditions steps.
const CHECK_EVERY: f32 = 0.25;

impl Game {
    /// The speaker name of a character definition (its name, else the id).
    fn quest_speaker(&self, def: &str) -> String {
        self.project.db.character(def).map_or(def.to_string(), |d| d.name.clone())
    }

    /// The runtime uid of a character of definition `def` on this map (the quest's giver for
    /// step and reward actions: emotes, `{self}`, `self_name()`).
    fn quest_owner(&self, who: &str) -> Option<u32> {
        (!who.is_empty()).then(|| self.loaded.entities.iter().find(|e| e.active && e.kind == EntityKind::Character && (e.name == who || e.def == who)).map(|e| e.uid)).flatten()
    }

    /// Whether quest `def` can be offered now: given by someone, not started, its conditions met.
    fn quest_offerable(&mut self, def: &QuestDef, owner: Option<u32>) -> bool {
        def.has_giver() && self.world.quest(&def.id).is_none() && conditions_met(self, &def.requires, &ActionCtx { owner, item: String::new() })
    }

    /// Whether a Talk step's own conditions hold (e.g. carrying what is handed in).
    fn talk_allowed(&mut self, q: &QuestDef, stage: u32, uid: u32) -> bool {
        match q.step(stage).map(|s| &s.done) {
            Some(StepGoal::Talk { when, .. }) => {
                let when = when.clone();
                conditions_met(self, &when, &ActionCtx { owner: Some(uid), item: String::new() })
            }
            _ => true,
        }
    }

    /// Talking to the character at entity index `i` (after its own Interact triggers): a Talk
    /// step waiting for it, else (when `awake`: not lying in bed) a quest it offers or what it
    /// says about its active quest's current step. Queues that; false = nothing to say.
    pub(super) fn quest_talk(&mut self, i: usize, awake: bool) -> bool {
        let (uid, def_id, name) = {
            let e = &self.loaded.entities[i];
            if e.kind != EntityKind::Character {
                return false;
            }
            (e.uid, e.def.clone(), e.name.clone())
        };
        // A quest's giver or talk character: this one's placed name or its definition.
        let is_me = |who: &str| !who.is_empty() && (who == name || who == def_id);
        let ctx = ActionCtx { owner: Some(uid), item: String::new() };
        let own_name = self.quest_speaker(&def_id);
        // What the quest calls this character in its dialogs.
        let giver_speaker = |q: &QuestDef| if q.giver_name.trim().is_empty() { own_name.clone() } else { q.giver_name.clone() };
        let quests = self.project.db.quests.clone();
        // 1. A current Talk step waiting for this character.
        for q in &quests {
            let Some(p) = self.world.quest(&q.id).filter(|p| p.status == QuestStatus::Active).cloned() else { continue };
            if !q.talk_to(p.stage).is_some_and(is_me) || !self.talk_allowed(q, p.stage, uid) {
                continue;
            }
            let speaker = match q.step(p.stage).map(|s| &s.done) {
                Some(StepGoal::Talk { speaker, .. }) if !speaker.trim().is_empty() => speaker.clone(),
                Some(StepGoal::Talk { character, .. }) if character.trim().is_empty() || is_me(&q.giver) => giver_speaker(q),
                _ => own_name.clone(),
            };
            let acts = q.talk_actions(p.stage, &speaker);
            self.runner.push(&acts, &ctx);
            return true;
        }
        if !awake {
            return false;
        }
        // 2. A quest it gives that can be offered now.
        for q in &quests {
            if !is_me(&q.giver) || !self.quest_offerable(q, Some(uid)) {
                continue;
            }
            let acts = q.offer_actions(&giver_speaker(q), &crate::lang::tr_ui("Yes"), &crate::lang::tr_ui("No"));
            if acts.is_empty() {
                continue;
            }
            self.runner.push(&acts, &ctx);
            return true;
        }
        // 3. What it says while one of its quests is under way.
        for q in &quests {
            if !is_me(&q.giver) {
                continue;
            }
            let Some(p) = self.world.quest(&q.id).filter(|p| p.status == QuestStatus::Active) else { continue };
            let Some(line) = q.step(p.stage).map(|s| s.giver_line.clone()).filter(|l| !l.trim().is_empty()) else { continue };
            self.runner.push(&[Action::Dialog { speaker: giver_speaker(q), text: line }], &ctx);
            return true;
        }
        false
    }

    /// The player arrived at something named `name` on this map (entered a zone, interacted
    /// with a character or prop): Reach steps whose marker target it is are done.
    pub(super) fn quest_reach(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        let map = self.loaded.map.name.clone();
        let quests = self.project.db.quests.clone();
        for q in &quests {
            let Some(p) = self.world.quest(&q.id).filter(|p| p.status == QuestStatus::Active).cloned() else { continue };
            if !matches!(q.step(p.stage).map(|s| &s.done), Some(StepGoal::Reach)) {
                continue;
            }
            if q.target_of(p.stage).is_some_and(|t| t.map.trim() == map && t.name.trim() == name) {
                let ctx = ActionCtx { owner: self.quest_owner(&q.giver), item: String::new() };
                self.runner.push(&[q.step_done_action(p.stage)], &ctx);
            }
        }
    }

    /// Every `CHECK_EVERY` seconds while nothing else runs: Conditions steps whose conditions
    /// hold are done.
    pub(super) fn check_quest_steps(&mut self, dt: f32) {
        self.quest_check_timer -= dt;
        if self.quest_check_timer > 0.0 || self.runner.busy() || self.modal() {
            return;
        }
        self.quest_check_timer = CHECK_EVERY;
        let quests = self.project.db.quests.clone();
        for q in &quests {
            let Some(p) = self.world.quest(&q.id).filter(|p| p.status == QuestStatus::Active).cloned() else { continue };
            let Some(StepGoal::Conditions(conds)) = q.step(p.stage).map(|s| &s.done) else { continue };
            let ctx = ActionCtx { owner: self.quest_owner(&q.giver), item: String::new() };
            if conditions_met(self, conds, &ctx) {
                self.runner.push(&[q.step_done_action(p.stage)], &ctx);
                return; // one at a time: the next check sees the new stage
            }
        }
    }

    /// What follows a quest's change from `before` (stage, status) to its state now: the old
    /// step's done actions, the new step's start actions, and on completion the rewards (items,
    /// skill points, the reward actions). Run right after the Quest action, with the giver as
    /// owner when it is here.
    pub(super) fn quest_follow_up(&mut self, quest: &str, before: Option<(u32, QuestStatus)>) -> Vec<(Action, ActionCtx)> {
        let Some(def) = self.project.db.quest(quest).cloned() else { return Vec::new() };
        let Some(now) = self.world.quest(quest).cloned() else { return Vec::new() };
        let acts = def.follow_up(before, &now);
        let ctx = ActionCtx { owner: self.quest_owner(&def.giver), item: String::new() };
        acts.into_iter().map(|a| (a, ctx.clone())).collect()
    }

    /// The characters on this map with a quest to offer now (entity uid, quest id), for the
    /// `!` markers and `status`.
    pub(super) fn quest_offers(&mut self) -> Vec<(u32, String)> {
        let givers: Vec<(u32, String, String)> = self.loaded.entities.iter().filter(|e| e.active && e.kind == EntityKind::Character).map(|e| (e.uid, e.def.clone(), e.name.clone())).collect();
        if givers.is_empty() || !self.project.db.quests.iter().any(|q| q.has_giver()) {
            return Vec::new();
        }
        let quests = self.project.db.quests.clone();
        let mut out = Vec::new();
        for (uid, def, name) in givers {
            if let Some(q) = quests.iter().find(|q| (q.giver == def || q.giver == name) && self.quest_offerable(q, Some(uid))) {
                out.push((uid, q.id.clone()));
            }
        }
        out
    }

    /// Refreshes the offers the `!` markers and `status` show (every half second while no event
    /// runs; a running event keeps the last ones hidden).
    pub(super) fn refresh_quest_offers(&mut self, dt: f32) {
        self.offer_check_timer -= dt;
        if self.offer_check_timer > 0.0 || self.runner.busy() {
            return;
        }
        self.offer_check_timer = 0.5;
        self.offer_cache = self.quest_offers();
    }

    /// `offers=` for `status`: quest@giver for every quest a character here offers now.
    pub(super) fn offers_status(&self) -> Vec<String> {
        self.offer_cache.iter().map(|(uid, q)| format!("{q}@{}", self.loaded.entities.iter().find(|e| e.uid == *uid).map_or(String::new(), |e| e.name.clone()))).collect()
    }

    /// A "!" over each character with a quest to offer (screen space; `cam` = the world camera).
    pub(super) fn draw_offer_marks(&self, cam: &Camera2D) {
        if !self.settings.quest_markers || self.on_title() || self.cutscene_active() || self.runner.busy() {
            return;
        }
        let t = get_time() as f32;
        let ts = self.ts();
        for (k, (uid, _)) in self.offer_cache.iter().enumerate() {
            let Some(e) = self.loaded.entities.iter().find(|e| e.uid == *uid && e.active) else { continue };
            let look = self.look(&e.appearance);
            let h = self.project.db.model(&look.model).map_or(ts * 1.4, |m| m.height) * self.look_size(&e.appearance);
            let p = cam.world_to_screen(e.body.pos - vec2(0.0, h + ts * 0.25));
            let bob = (t * 3.0 + k as f32).sin() * 3.0;
            let (x, y) = (p.x - 5.0, p.y - 2.0 + bob);
            let gold = Color::new(1.0, 0.82, 0.25, 1.0);
            let rim = Color::new(0.15, 0.1, 0.05, 1.0);
            for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)] {
                super::text::draw_text("!", x + dx, y + dy, 34.0, rim);
            }
            super::text::draw_text("!", x, y, 34.0, gold);
        }
    }
}
