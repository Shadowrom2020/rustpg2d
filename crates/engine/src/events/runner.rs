//! Sequential action execution with support for blocking actions.

use std::collections::VecDeque;

use super::{Action, Cond, Condition, VarOp};
use crate::state::WorldState;

/// Context an action runs in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionCtx {
    /// Entity uid that owns the trigger (`"self"` in targets).
    pub owner: Option<u32>,
    /// The item of the *Use item* event that fired the trigger ("" for other events;
    /// `used_item()` in scripts).
    pub item: String,
}

/// Result of a host executing an action.
pub enum Step {
    /// Done, continue with the next action.
    Continue,
    /// Wait until `ActionHost::blocked` returns false.
    Block,
    /// Run these actions next (before the rest of the queue).
    Insert(Vec<(Action, ActionCtx)>),
}

/// Implemented by the game: executes world-specific actions.
pub trait ActionHost {
    fn world(&mut self) -> &mut WorldState;
    fn execute(&mut self, action: &Action, ctx: &ActionCtx) -> Step;
    /// Whether the last blocking action is still in progress.
    fn blocked(&mut self) -> bool;
    fn script_condition(&mut self, function: &str, ctx: &ActionCtx) -> bool;
    /// Current state name of a prop (for "Prop state" conditions).
    fn prop_state(&mut self, _target: &str, _ctx: &ActionCtx) -> Option<String> {
        None
    }
    /// "sit" / "lie" / "stand" of a character (or "player") on furniture, "" otherwise.
    fn pose(&mut self, _target: &str, _ctx: &ActionCtx) -> String {
        String::new()
    }
    /// The player's standing with a faction (the saved one, else where it starts).
    fn standing(&mut self, faction: &str) -> i32 {
        self.world().standing.get(faction).copied().unwrap_or(0)
    }
    /// Called after `GiveItem`: `count` were added, `lost` didn't fit (for messages and sounds).
    fn item_given(&mut self, _item: &str, _count: u32, _lost: u32) {}
}

enum Waiting {
    None,
    Timer(f32),
    Host,
}

pub struct ActionRunner {
    queue: VecDeque<(Action, ActionCtx)>,
    waiting: Waiting,
}

impl Default for ActionRunner {
    fn default() -> Self {
        ActionRunner { queue: VecDeque::new(), waiting: Waiting::None }
    }
}

/// Safety valve against triggers that emit each other forever.
const MAX_STEPS_PER_FRAME: usize = 10_000;

impl ActionRunner {
    pub fn push(&mut self, actions: &[Action], ctx: &ActionCtx) {
        self.queue.extend(actions.iter().cloned().map(|a| (a, ctx.clone())));
    }

    /// Adds the queued actions of `other` after this runner's own.
    pub fn append(&mut self, other: ActionRunner) {
        self.queue.extend(other.queue);
    }

    fn push_front(&mut self, items: Vec<(Action, ActionCtx)>) {
        for item in items.into_iter().rev() {
            self.queue.push_front(item);
        }
    }

    /// True while a sequence is running (gameplay input should be paused).
    pub fn busy(&self) -> bool {
        !self.queue.is_empty() || !matches!(self.waiting, Waiting::None)
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.waiting = Waiting::None;
    }

    pub fn update(&mut self, host: &mut dyn ActionHost, dt: f32) {
        match &mut self.waiting {
            Waiting::None => {}
            Waiting::Timer(t) => {
                *t -= dt;
                if *t > 0.0 {
                    return;
                }
                self.waiting = Waiting::None;
            }
            Waiting::Host => {
                if host.blocked() {
                    return;
                }
                self.waiting = Waiting::None;
            }
        }

        for _ in 0..MAX_STEPS_PER_FRAME {
            let Some((action, ctx)) = self.queue.pop_front() else { return };
            match &action {
                Action::Wait { seconds } => {
                    self.waiting = Waiting::Timer(*seconds);
                    return;
                }
                Action::If { conditions, then, otherwise } => {
                    let met = conditions_met(host, conditions, &ctx);
                    let branch = if met { then } else { otherwise };
                    self.push_front(branch.iter().cloned().map(|a| (a, ctx.clone())).collect());
                }
                Action::SetFlag { name, value } => host.world().set_flag(name, *value),
                Action::SetVar { name, op, value } => {
                    let w = host.world();
                    let v = match op {
                        VarOp::Set => *value,
                        VarOp::Add => w.var(name) + value,
                    };
                    w.set_var(name, v);
                }
                Action::GiveItem { item, count } => {
                    let lost = host.world().give_item(item, *count);
                    host.item_given(item, *count - lost, lost);
                }
                Action::TakeItem { item, count } => {
                    host.world().take_item(item, *count);
                }
                _ => match host.execute(&action, &ctx) {
                    Step::Continue => {}
                    Step::Block => {
                        self.waiting = Waiting::Host;
                        return;
                    }
                    Step::Insert(items) => self.push_front(items),
                },
            }
        }
        eprintln!("[events] action runner hit {MAX_STEPS_PER_FRAME} steps in one frame; possible event loop");
    }
}

/// Evaluates conditions, calling into the host for script conditions.
pub fn conditions_met(host: &mut dyn ActionHost, conds: &[Condition], ctx: &ActionCtx) -> bool {
    conds.iter().all(|c| match &c.cond {
        Cond::Script { function } => host.script_condition(function, ctx) != c.negate,
        Cond::PropState { target, state } => (host.prop_state(target, ctx).as_deref() == Some(state.as_str())) != c.negate,
        Cond::Pose { target, pose } => (host.pose(target, ctx) == *pose) != c.negate,
        Cond::FactionStanding { faction, op, value } => op.apply(host.standing(faction) as i64, *value) != c.negate,
        _ => c.eval(host.world(), &mut |_| false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        world: WorldState,
        log: Vec<String>,
        dialog_open: bool,
    }

    impl ActionHost for Host {
        fn world(&mut self) -> &mut WorldState {
            &mut self.world
        }
        fn execute(&mut self, action: &Action, ctx: &ActionCtx) -> Step {
            match action {
                Action::Dialog { text, .. } => {
                    self.log.push(format!("dialog:{text}"));
                    self.dialog_open = true;
                    Step::Block
                }
                Action::Emit { name } => {
                    self.log.push(format!("emit:{name}"));
                    Step::Insert(vec![(Action::SetFlag { name: name.clone(), value: true }, ctx.clone())])
                }
                other => {
                    self.log.push(other.label().to_string());
                    Step::Continue
                }
            }
        }
        fn blocked(&mut self) -> bool {
            self.dialog_open
        }
        fn script_condition(&mut self, f: &str, _: &ActionCtx) -> bool {
            f == "yes"
        }
    }

    #[test]
    fn sequencing_blocks_and_branches() {
        let mut host = Host::default();
        let mut r = ActionRunner::default();
        let ctx = ActionCtx::default();
        r.push(
            &[
                Action::GiveItem { item: "key".into(), count: 1 },
                Action::Dialog { speaker: String::new(), text: "one".into() },
                Action::If {
                    conditions: vec![Condition::new(Cond::HasItem { item: "key".into(), count: 1 })],
                    then: vec![Action::Emit { name: "opened".into() }],
                    otherwise: vec![Action::Dialog { speaker: String::new(), text: "locked".into() }],
                },
                Action::Wait { seconds: 1.0 },
                Action::Dialog { speaker: String::new(), text: "two".into() },
            ],
            &ctx,
        );
        r.update(&mut host, 0.016);
        assert_eq!(host.log, vec!["dialog:one"]);
        assert!(r.busy());
        r.update(&mut host, 0.016); // still blocked by dialog
        assert_eq!(host.log.len(), 1);
        host.dialog_open = false;
        r.update(&mut host, 0.016); // runs If -> emit, then hits Wait
        assert_eq!(host.log, vec!["dialog:one", "emit:opened"]);
        assert!(host.world.flag("opened"));
        r.update(&mut host, 0.5);
        assert_eq!(host.log.len(), 2);
        r.update(&mut host, 0.6);
        assert_eq!(host.log.last().unwrap(), "dialog:two");
        host.dialog_open = false;
        r.update(&mut host, 0.016);
        assert!(!r.busy());
        assert_eq!(host.world.item_count("key"), 1);
    }

    #[test]
    fn script_conditions_use_host() {
        let mut host = Host::default();
        let mut r = ActionRunner::default();
        r.push(
            &[Action::If {
                conditions: vec![Condition::new(Cond::Script { function: "yes".into() })],
                then: vec![Action::SetFlag { name: "a".into(), value: true }],
                otherwise: vec![Action::SetFlag { name: "b".into(), value: true }],
            }],
            &ActionCtx::default(),
        );
        r.update(&mut host, 0.0);
        assert!(host.world.flag("a") && !host.world.flag("b"));
    }
}
