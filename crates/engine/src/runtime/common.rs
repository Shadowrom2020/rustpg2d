//! Common events (`common_events.rs`): *Call common event* runs one's actions in place; parallel
//! ones run in their own action queue, again and again while their flag is set (stopping at once
//! when it is cleared); autorun ones are queued into the main queue whenever it is idle while
//! their flag is set, so the player waits until they clear it.

use crate::common_events::CommonTrigger;
use crate::events::runner::ActionCtx;
use crate::events::Action;

use super::Game;

impl Game {
    /// *Call common event*: its actions, to run next with the caller's context.
    pub(super) fn common_actions(&self, id: &str, ctx: &ActionCtx) -> Vec<(Action, ActionCtx)> {
        match self.project.db.common_event(id) {
            Some(d) => d.actions.iter().map(|a| (a.clone(), ctx.clone())).collect(),
            None => {
                eprintln!("[events] Call common event: no common event '{id}'");
                vec![]
            }
        }
    }

    /// Runs the parallel common events and queues autorun ones (once a frame, not on the title).
    pub(super) fn update_common_events(&mut self, dt: f32) {
        if self.project.db.common_events.is_empty() || self.on_title() {
            return;
        }
        let mut parallel = Vec::new();
        let mut autorun = None;
        for d in &self.project.db.common_events {
            match &d.trigger {
                CommonTrigger::Call => {}
                CommonTrigger::Parallel { flag } => {
                    if !flag.is_empty() && self.world.flag(flag) {
                        parallel.push(d.id.clone());
                    }
                }
                CommonTrigger::Autorun { flag } => {
                    if autorun.is_none() && !flag.is_empty() && self.world.flag(flag) {
                        autorun = Some(d.actions.clone());
                    }
                }
            }
        }
        // A cleared flag stops its event at once.
        self.common_runs.retain(|id, _| parallel.contains(id));
        for id in parallel {
            let mut runner = self.common_runs.remove(&id).unwrap_or_default();
            if !runner.busy() {
                if let Some(d) = self.project.db.common_event(&id) {
                    if d.actions.is_empty() {
                        continue;
                    }
                    runner.push(&d.actions, &ActionCtx::default());
                }
            }
            runner.update(self, dt);
            // A parallel event that cleared its own flag is gone next frame.
            self.common_runs.insert(id, runner);
        }
        if let Some(actions) = autorun {
            if !self.runner.busy() && !actions.is_empty() && !self.modal() {
                self.runner.push(&actions, &ActionCtx::default());
            }
        }
    }

    /// Loading a game restarts them from their first action (they keep running across maps).
    /// Queues started by *Run alongside* end.
    pub(super) fn reset_common_events(&mut self) {
        self.common_runs.clear();
        self.side_runs.clear();
    }

    /// *Run alongside*: steps the queues it started; finished ones go.
    pub(super) fn update_side_runs(&mut self, dt: f32) {
        if self.side_runs.is_empty() {
            return;
        }
        let mut runs = std::mem::take(&mut self.side_runs);
        for r in &mut runs {
            r.update(self, dt);
        }
        runs.retain(|r| r.busy());
        // (Started while they ran: by a cue of their own.)
        runs.append(&mut self.side_runs);
        self.side_runs = runs;
    }

    /// For `status()`: the common events running by themselves now, `parallel:ID` / `autorun:ID`.
    pub(super) fn common_status(&self) -> Vec<String> {
        let mut out: Vec<String> = self.common_runs.keys().map(|id| format!("parallel:{id}")).collect();
        for d in &self.project.db.common_events {
            if let CommonTrigger::Autorun { flag } = &d.trigger {
                if !flag.is_empty() && self.world.flag(flag) {
                    out.push(format!("autorun:{}", d.id));
                }
            }
        }
        out
    }
}
