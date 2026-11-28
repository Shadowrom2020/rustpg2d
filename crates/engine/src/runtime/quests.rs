//! Quests described in the quest editor (`QuestDef::giver`, `offer`, `steps`, `rewards`): the
//! giver offers them when talked to, Talk steps are done by talking to their character, Reach
//! steps by arriving at the stage's marker target, Conditions steps as soon as their conditions
//! hold; each step's actions run when it starts and when it is done, and completing a quest
//! gives its rewards. Everything is played as ordinary actions on the event runner (dialogs,
//! choices, the Quest action), so quests moved on by triggers and scripts work the same.

use gfx::prelude::*;

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
    /// the reward actions). Run right after the Quest action, with the giver as
    /// owner when it is here.
    pub(super) fn quest_follow_up(&mut self, quest: &str, before: Option<(u32, QuestStatus)>) -> Vec<(Action, ActionCtx)> {
        let Some(def) = self.project.db.quest(quest).cloned() else { return Vec::new() };
        let Some(now) = self.world.quest(quest).cloned() else { return Vec::new() };
        let acts = def.follow_up(before, &now);
        let ctx = ActionCtx { owner: self.quest_owner(&def.giver), item: String::new() };
        acts.into_iter().map(|a| (a, ctx.clone())).collect()
    }

    /// The quests' objects on this map (`QuestDef::objects`): hidden before their quest is
    /// started; shown once while it is active (recorded in `fired`, so items taken afterwards
    /// stay taken and props changed stay changed); never shown, they stay hidden once it ended.
    pub(super) fn apply_quest_objects(&mut self) {
        let map = self.loaded.map.name.clone();
        if !self.project.db.quests.iter().any(|q| q.objects.iter().any(|o| o.map.trim() == map)) {
            return;
        }
        let quests = self.project.db.quests.clone();
        let mut changed = false;
        for q in &quests {
            for o in q.objects.iter().filter(|o| o.map.trim() == map && !o.name.trim().is_empty()) {
                let name = o.name.trim();
                let key = format!("quest_object/{}/{map}/{name}", q.id);
                if self.world.fired.contains(&key) {
                    continue;
                }
                let show = self.world.quest(&q.id).is_some_and(|p| p.status == QuestStatus::Active);
                for i in 0..self.loaded.entities.len() {
                    let e = &mut self.loaded.entities[i];
                    if e.name != name || e.active == show {
                        continue;
                    }
                    e.active = show;
                    e.hidden = !show;
                    e.away = false;
                    self.persist(i);
                    changed = true;
                }
                if show {
                    self.world.fired.insert(key);
                }
            }
        }
        if changed {
            self.rebuild_occupancy();
        }
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
        // Talk steps whose conditions hold (their markers lead to the character, the others to
        // what to fetch first).
        let quests = self.project.db.quests.clone();
        let mut ready = Vec::new();
        for q in &quests {
            let Some(p) = self.world.quest(&q.id).filter(|p| p.status == QuestStatus::Active).cloned() else { continue };
            if let Some(StepGoal::Talk { when, .. }) = q.step(p.stage).map(|s| &s.done) {
                let when = when.clone();
                if conditions_met(self, &when, &ActionCtx::default()) {
                    ready.push(q.id.clone());
                }
            }
        }
        self.talk_ready = ready;
        self.refresh_marker_hops();
    }

    /// The maps a map's exits (portals, transitions, edges) lead to; read once per map.
    fn links_of(&mut self, name: &str) -> Vec<String> {
        fn links(m: &crate::map::Map) -> Vec<String> {
            let zones = m.zones.iter().filter_map(|z| match &z.kind {
                crate::map::ZoneKind::Portal { target_map, .. } | crate::map::ZoneKind::Transition { target_map, .. } => Some(target_map.clone()),
                _ => None,
            });
            let edges = crate::map::Edge::ALL.into_iter().filter_map(|e| m.edges.get(e).map(|l| l.target_map.clone()));
            let mut v: Vec<String> = zones.chain(edges).filter(|t| !t.is_empty()).collect();
            v.sort();
            v.dedup();
            v
        }
        if name == self.loaded.map.name {
            return links(&self.loaded.map);
        }
        if let Some(v) = self.map_links.get(name) {
            return v.clone();
        }
        let v = match self.cache.iter().find(|m| m.name == name) {
            Some(m) => links(m),
            None => self.project.load_map(name).map(|m| links(&m)).unwrap_or_default(),
        };
        self.map_links.insert(name.to_string(), v.clone());
        v
    }

    /// For every map an active quest leads to and this map has no exit to: the first map on the
    /// shortest way there (its exit gets the marker).
    fn refresh_marker_hops(&mut self) {
        let here = self.loaded.map.name.clone();
        let mut wanted: Vec<String> = Vec::new();
        for q in &self.project.db.quests {
            let Some(p) = self.world.quest(&q.id).filter(|p| p.status == QuestStatus::Active) else { continue };
            let first = q.step(p.stage).map(|s| &s.first_to).filter(|t| !t.is_empty());
            for t in q.target_of(p.stage).into_iter().chain(first) {
                let m = t.map.trim().to_string();
                if !m.is_empty() && m != here && !wanted.contains(&m) {
                    wanted.push(m);
                }
            }
        }
        let direct = self.links_of(&here);
        self.marker_hops.clear();
        for goal in wanted.into_iter().filter(|g| !direct.contains(g)) {
            // Breadth first over the maps, remembering the first hop of each path.
            let mut seen: Vec<String> = vec![here.clone()];
            let mut queue: std::collections::VecDeque<(String, String)> = direct.iter().map(|d| (d.clone(), d.clone())).collect();
            seen.extend(direct.iter().cloned());
            while let Some((map, hop)) = queue.pop_front() {
                if map == goal {
                    self.marker_hops.insert(goal.clone(), hop);
                    break;
                }
                for next in self.links_of(&map) {
                    if !seen.contains(&next) && seen.len() < 256 {
                        seen.push(next.clone());
                        queue.push_back((next, hop.clone()));
                    }
                }
            }
        }
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
            let _ = cam;
            let p = self.view().above(e.body.pos, h + ts * 0.25);
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
