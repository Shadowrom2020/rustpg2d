//! Quests: definitions (title, description, stages; who gives them and when, what the giver
//! says, how each step is done, the rewards), the player's progress, and the operations /
//! checks used by the "Quest" action and condition. `runtime/quests.rs` plays the described parts.

use serde::{Deserialize, Serialize};

use crate::events::{Action, Condition};
use crate::state::WorldState;

/// A quest as defined in `defs/quests.ron`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QuestDef {
    pub id: String,
    pub title: String,
    /// Shown under the title in the journal.
    #[serde(default)]
    pub description: String,
    /// One objective per stage, shown in order as the quest advances. `{name}` shows the
    /// value of variable `name` (e.g. "Defeat slimes ({slimes}/5)").
    #[serde(default)]
    pub stages: Vec<String>,
    /// Experience given when the quest is completed (with experience and levels on).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub xp: u32,
    /// Where each stage leads (by index; missing or empty = no quest marker for that stage).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<QuestTarget>,
    /// The character definition that offers the quest and takes it back ("" = started by
    /// triggers or scripts).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub giver: String,
    /// The giver's name in its dialogs ("" = its character's name): e.g. "Tilda" for a placed
    /// merchant whose definition is called "Merchant".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub giver_name: String,
    /// When the giver offers it (besides not being started yet): e.g. another quest completed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Condition>,
    /// What the giver says when offering it.
    #[serde(default, skip_serializing_if = "QuestOffer::is_empty")]
    pub offer: QuestOffer,
    /// How each stage is done (by index like `targets`; missing = moved on by triggers / scripts).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<QuestStep>,
    /// Given when the quest is completed (besides `xp`).
    #[serde(default, skip_serializing_if = "QuestRewards::is_empty")]
    pub rewards: QuestRewards,
    /// Placed items, props and characters (map + name) that belong to the quest: hidden until
    /// it is started, then shown once while it is active (taken items stay taken).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<QuestTarget>,
}

/// What the giver says when offering a quest: `lines` (one dialog box each), then `question`
/// with the options `accept` / `decline` and the lines after each answer. Without a question the
/// quest starts after the lines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuestOffer {
    pub lines: Vec<String>,
    pub question: String,
    pub accept: String,
    pub decline: String,
    pub accepted: Vec<String>,
    pub declined: Vec<String>,
}

impl QuestOffer {
    pub fn is_empty(&self) -> bool {
        *self == QuestOffer::default()
    }
}

/// How one stage of a quest is done, and what happens around it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuestStep {
    pub done: StepGoal,
    /// What the giver says while this step is current ("" = nothing: the giver's own triggers).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub giver_line: String,
    /// Run when the step begins (the first one when the quest starts).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub on_start: Vec<Action>,
    /// Run when the step is done.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub on_done: Vec<Action>,
    /// A Talk step whose `when` doesn't hold yet leads here instead of to its character (what to
    /// fetch or make first: the cart with the flour, the oven); empty = no marker until then.
    #[serde(skip_serializing_if = "QuestTarget::is_empty")]
    pub first_to: QuestTarget,
}

impl QuestStep {
    pub fn is_empty(&self) -> bool {
        *self == QuestStep::default()
    }
}

/// When a quest step is done.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum StepGoal {
    /// Quest actions or scripts move the quest on.
    #[default]
    Manual,
    /// As soon as the conditions hold (has an item, defeated so many, a variable, a flag…).
    Conditions(Vec<Condition>),
    /// The player talks to `character` (a placed character's name or a character definition;
    /// "" = the quest's giver) while `when` holds (e.g. carrying what is handed in): it says
    /// `lines`, then the step is done.
    Talk {
        #[serde(default)]
        character: String,
        #[serde(default)]
        lines: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        when: Vec<Condition>,
        /// Its name in the dialog ("" = the giver's for the giver, else its character's name).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        speaker: String,
    },
    /// The player arrives at the step's marker target: enters the zone, or interacts with the
    /// character or prop of that name on its map.
    Reach,
}

impl StepGoal {
    pub fn label(&self) -> &'static str {
        match self {
            StepGoal::Manual => "triggers or scripts move it on",
            StepGoal::Conditions(_) => "these conditions hold",
            StepGoal::Talk { .. } => "the player talks to",
            StepGoal::Reach => "the player reaches the marker target",
        }
    }
}

/// An item given as a quest reward.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RewardItem {
    pub item: String,
    #[serde(default = "one")]
    pub count: u32,
}

fn one() -> u32 {
    1
}

/// What completing a quest gives (its `xp` is given too).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuestRewards {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<RewardItem>,
    /// Anything else: Run script (e.g. friendship with the giver), Change standing…
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
}

impl QuestRewards {
    pub fn is_empty(&self) -> bool {
        *self == QuestRewards::default()
    }
}

/// Where a quest stage leads, for its quest marker: a placed character, prop or zone by name on a
/// map.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QuestTarget {
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub name: String,
}

impl QuestTarget {
    pub fn is_empty(&self) -> bool {
        self.map.trim().is_empty() || self.name.trim().is_empty()
    }
}

impl QuestDef {
    /// The target of stage `stage`, if it has one.
    pub fn target_of(&self, stage: u32) -> Option<&QuestTarget> {
        self.targets.get(stage as usize).filter(|t| !t.is_empty())
    }

    /// How stage `stage` is done (Manual when not described).
    pub fn step(&self, stage: u32) -> Option<&QuestStep> {
        self.steps.get(stage as usize)
    }

    /// The character a Talk step at `stage` waits for (a placed name or definition id; the giver
    /// for "").
    pub fn talk_to(&self, stage: u32) -> Option<&str> {
        match &self.step(stage)?.done {
            StepGoal::Talk { character, .. } => Some(if character.trim().is_empty() { self.giver.as_str() } else { character.as_str() }).filter(|c| !c.is_empty()),
            _ => None,
        }
    }

    /// Whether the quest is described in the quest editor (given by a character).
    pub fn has_giver(&self) -> bool {
        !self.giver.trim().is_empty()
    }

    /// The Quest action that finishes stage `stage`: on to the next one, or completing the quest
    /// after the last.
    pub fn step_done_action(&self, stage: u32) -> Action {
        let op = if stage as usize + 1 >= self.stages.len() { QuestOp::Complete } else { QuestOp::Advance };
        Action::Quest { quest: self.id.clone(), op, stage: 0 }
    }

    /// Talking to the character of the Talk step at `stage` (said by `speaker`): its lines, then
    /// the step is done.
    pub fn talk_actions(&self, stage: u32, speaker: &str) -> Vec<Action> {
        let lines = match self.step(stage).map(|s| &s.done) {
            Some(StepGoal::Talk { lines, .. }) => lines.as_slice(),
            _ => &[],
        };
        let mut acts = dialog_lines(speaker, lines);
        acts.push(self.step_done_action(stage));
        acts
    }

    /// The giver (`speaker`) offering the quest: its lines, then the question with the options
    /// (`yes` / `no` when the offer names none) and the lines after each answer; accepting starts
    /// the quest. Without a question the quest starts after the lines.
    pub fn offer_actions(&self, speaker: &str, yes: &str, no: &str) -> Vec<Action> {
        let o = &self.offer;
        let mut acts = dialog_lines(speaker, &o.lines);
        let mut accepted = vec![Action::Quest { quest: self.id.clone(), op: QuestOp::Start, stage: 0 }];
        accepted.extend(dialog_lines(speaker, &o.accepted));
        if o.question.trim().is_empty() {
            acts.extend(accepted);
            return acts;
        }
        let pick = |s: &str, default: &str| if s.trim().is_empty() { default.to_string() } else { s.to_string() };
        acts.push(Action::Choice { speaker: speaker.to_string(), text: o.question.clone(), options: vec![pick(&o.accept, yes), pick(&o.decline, no)], var: OFFER_VAR.into() });
        acts.push(Action::If {
            conditions: vec![Condition { cond: crate::events::Cond::Var { name: OFFER_VAR.into(), op: crate::events::CmpOp::Eq, value: 0 }, negate: false }],
            then: accepted,
            otherwise: dialog_lines(speaker, &o.declined),
        });
        acts
    }

    /// What follows the quest's change from `before` (stage, status; None = not started) to
    /// `now`: the old step's done actions (not when it failed), the new step's start actions,
    /// and on completion the rewards (items, the reward actions).
    pub fn follow_up(&self, before: Option<(u32, QuestStatus)>, now: &QuestProgress) -> Vec<Action> {
        let mut acts: Vec<Action> = Vec::new();
        if let Some((stage, QuestStatus::Active)) = before {
            let moved = now.stage != stage || now.status == QuestStatus::Completed;
            if moved && now.status != QuestStatus::Failed {
                acts.extend(self.step(stage).map_or(&[][..], |s| &s.on_done).iter().cloned());
            }
        }
        let started = before.is_none_or(|(s, st)| st != QuestStatus::Active || s != now.stage);
        if now.status == QuestStatus::Active && started {
            acts.extend(self.step(now.stage).map_or(&[][..], |s| &s.on_start).iter().cloned());
        }
        if now.status == QuestStatus::Completed && before.is_some_and(|(_, st)| st != QuestStatus::Completed) {
            let r = &self.rewards;
            acts.extend(r.items.iter().filter(|i| !i.item.is_empty() && i.count > 0).map(|i| Action::GiveItem { item: i.item.clone(), count: i.count }));
            acts.extend(r.actions.iter().cloned());
        }
        acts
    }
}

/// The variable an offer's question answers into (0 = accept).
pub const OFFER_VAR: &str = "quest_offer_answer";

/// One dialog box per non-empty line, said by `speaker`.
pub fn dialog_lines(speaker: &str, lines: &[String]) -> Vec<Action> {
    lines.iter().filter(|l| !l.trim().is_empty()).map(|l| Action::Dialog { speaker: speaker.to_string(), text: l.clone() }).collect()
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestStatus {
    #[default]
    Active,
    Completed,
    Failed,
}

/// The player's progress in one quest (only quests that were started have one).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QuestProgress {
    /// Current stage (index into the definition's stages).
    pub stage: u32,
    pub status: QuestStatus,
    /// Order in which quests were started / last changed (newest first in the journal).
    #[serde(default)]
    pub order: u64,
}

/// What a "Quest" action does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestOp {
    /// Starts the quest at stage 0 (nothing happens if it was already started).
    Start,
    /// Goes to the next stage (starts the quest if needed).
    Advance,
    /// Goes to stage `stage` (starts the quest if needed).
    SetStage,
    Complete,
    Fail,
}

impl QuestOp {
    pub const ALL: [QuestOp; 5] = [QuestOp::Start, QuestOp::Advance, QuestOp::SetStage, QuestOp::Complete, QuestOp::Fail];
    pub fn label(self) -> &'static str {
        match self {
            QuestOp::Start => "start",
            QuestOp::Advance => "advance to the next stage",
            QuestOp::SetStage => "set stage",
            QuestOp::Complete => "complete",
            QuestOp::Fail => "fail",
        }
    }
}

/// What a "Quest" condition checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestCheck {
    NotStarted,
    /// Started and neither completed nor failed.
    Active,
    Completed,
    Failed,
    /// Active and at exactly stage `stage`.
    AtStage,
    /// Started and at stage `stage` or later (completed counts as past every stage).
    StageAtLeast,
}

impl QuestCheck {
    pub const ALL: [QuestCheck; 6] = [QuestCheck::NotStarted, QuestCheck::Active, QuestCheck::Completed, QuestCheck::Failed, QuestCheck::AtStage, QuestCheck::StageAtLeast];
    pub fn label(self) -> &'static str {
        match self {
            QuestCheck::NotStarted => "not started",
            QuestCheck::Active => "active",
            QuestCheck::Completed => "completed",
            QuestCheck::Failed => "failed",
            QuestCheck::AtStage => "active at stage",
            QuestCheck::StageAtLeast => "at stage or later",
        }
    }
    pub fn uses_stage(self) -> bool {
        matches!(self, QuestCheck::AtStage | QuestCheck::StageAtLeast)
    }
}

/// What an operation changed (for messages).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestChange {
    Started,
    Updated,
    Completed,
    Failed,
}

impl WorldState {
    pub fn quest(&self, id: &str) -> Option<&QuestProgress> {
        self.quests.get(id)
    }

    /// Applies a quest operation. Returns what changed (None = nothing).
    pub fn apply_quest(&mut self, id: &str, op: QuestOp, stage: u32) -> Option<QuestChange> {
        if id.is_empty() {
            return None;
        }
        let order = self.quests.values().map(|q| q.order).max().map_or(1, |m| m + 1);
        let started = self.quests.contains_key(id);
        let q = self.quests.entry(id.to_string()).or_insert(QuestProgress { stage: 0, status: QuestStatus::Active, order });
        let before = q.clone();
        match op {
            QuestOp::Start => {}
            QuestOp::Advance if started => q.stage += 1,
            QuestOp::Advance => {}
            QuestOp::SetStage => q.stage = stage,
            QuestOp::Complete => q.status = QuestStatus::Completed,
            QuestOp::Fail => q.status = QuestStatus::Failed,
        }
        // Finished quests don't change stage or come back.
        if before.status != QuestStatus::Active && started {
            *q = before;
            return None;
        }
        if started && *q == before {
            return None;
        }
        q.order = order;
        Some(match (started, q.status) {
            (_, QuestStatus::Completed) => QuestChange::Completed,
            (_, QuestStatus::Failed) => QuestChange::Failed,
            (false, _) => QuestChange::Started,
            (true, _) => QuestChange::Updated,
        })
    }

    pub fn check_quest(&self, id: &str, check: QuestCheck, stage: u32) -> bool {
        let q = self.quests.get(id);
        match check {
            QuestCheck::NotStarted => q.is_none(),
            QuestCheck::Active => q.is_some_and(|q| q.status == QuestStatus::Active),
            QuestCheck::Completed => q.is_some_and(|q| q.status == QuestStatus::Completed),
            QuestCheck::Failed => q.is_some_and(|q| q.status == QuestStatus::Failed),
            QuestCheck::AtStage => q.is_some_and(|q| q.status == QuestStatus::Active && q.stage == stage),
            QuestCheck::StageAtLeast => q.is_some_and(|q| q.status == QuestStatus::Completed || q.stage >= stage),
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quest as the quest editor describes it: the elder offers it after another quest, a
    /// Conditions step, a Talk step back to him, rewards.
    fn designed() -> QuestDef {
        let say = |t: &str| Action::Dialog { speaker: String::new(), text: t.into() };
        QuestDef {
            id: "ashes".into(),
            title: "Ashes".into(),
            stages: vec!["Find the ashes".into(), "Bring them to the elder".into()],
            giver: "elder".into(),
            requires: vec![Condition { cond: crate::events::Cond::Quest { quest: "lights".into(), check: QuestCheck::Completed, stage: 0 }, negate: false }],
            offer: QuestOffer { lines: vec!["The shrine left ashes.".into()], question: "Will you look?".into(), accept: "Yes".into(), decline: String::new(), accepted: vec!["Thank you.".into()], declined: vec!["Pity.".into()] },
            steps: vec![
                QuestStep { done: StepGoal::Conditions(vec![]), on_start: vec![say("start 0")], on_done: vec![say("done 0")], ..Default::default() },
                QuestStep {
                    done: StepGoal::Talk { character: String::new(), lines: vec!["You found them!".into()], when: vec![], speaker: String::new() },
                    giver_line: "Any luck?".into(),
                    first_to: QuestTarget { map: "forest".into(), name: "shrine".into() },
                    ..Default::default()
                },
            ],
            rewards: QuestRewards { items: vec![RewardItem { item: "coin".into(), count: 25 }], actions: vec![Action::RunScript { function: "quest_thanks".into() }] },
            ..Default::default()
        }
    }

    #[test]
    fn designed_quests_load_save_and_old_ones_stay_plain() {
        // A quest from before the quest designer: none of the new fields, written back the same.
        let old = r#"(id: "q", title: "Q", stages: ["a"], xp: 5)"#;
        let q: QuestDef = ron::from_str(old).unwrap();
        assert!(!q.has_giver() && q.steps.is_empty() && q.rewards.is_empty() && q.offer.is_empty());
        let again = ron::to_string(&q).unwrap();
        assert!(!again.contains("giver") && !again.contains("offer") && !again.contains("rewards"), "{again}");
        // The new fields survive a round trip.
        let d = designed();
        let back: QuestDef = ron::from_str(&ron::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn talk_steps_wait_for_their_character_or_the_giver() {
        let d = designed();
        assert_eq!(d.talk_to(0), None);
        assert_eq!(d.talk_to(1), Some("elder"));
        let acts = d.talk_actions(1, "Rowan");
        assert_eq!(acts[0], Action::Dialog { speaker: "Rowan".into(), text: "You found them!".into() });
        // The last step completes the quest; earlier ones advance it.
        assert_eq!(acts[1], Action::Quest { quest: "ashes".into(), op: QuestOp::Complete, stage: 0 });
        assert_eq!(d.step_done_action(0), Action::Quest { quest: "ashes".into(), op: QuestOp::Advance, stage: 0 });
    }

    #[test]
    fn the_offer_asks_and_starts_the_quest_when_accepted() {
        let acts = designed().offer_actions("Rowan", "Yes", "No");
        assert!(matches!(&acts[0], Action::Dialog { text, .. } if text == "The shrine left ashes."));
        let Action::Choice { options, var, .. } = &acts[1] else { panic!("{:?}", acts[1]) };
        // An empty option takes the default.
        assert_eq!((options.as_slice(), var.as_str()), (&["Yes".to_string(), "No".to_string()][..], OFFER_VAR));
        let Action::If { then, otherwise, .. } = &acts[2] else { panic!() };
        assert_eq!(then[0], Action::Quest { quest: "ashes".into(), op: QuestOp::Start, stage: 0 });
        assert!(matches!(&otherwise[0], Action::Dialog { text, .. } if text == "Pity."));
        // Without a question: the lines, then it starts.
        let mut d = designed();
        d.offer.question.clear();
        let acts = d.offer_actions("Rowan", "Yes", "No");
        assert_eq!(acts[1], Action::Quest { quest: "ashes".into(), op: QuestOp::Start, stage: 0 });
    }

    #[test]
    fn steps_run_their_actions_and_completion_gives_the_rewards() {
        let d = designed();
        let texts = |acts: Vec<Action>| acts.iter().map(|a| match a { Action::Dialog { text, .. } => text.clone(), other => other.label().to_string() }).collect::<Vec<_>>();
        let p = |stage, status| QuestProgress { stage, status, order: 0 };
        // Started: the first step's start actions.
        assert_eq!(texts(d.follow_up(None, &p(0, QuestStatus::Active))), ["start 0"]);
        // On to stage 1: the first step's done actions.
        assert_eq!(texts(d.follow_up(Some((0, QuestStatus::Active)), &p(1, QuestStatus::Active))), ["done 0"]);
        // Completed: the rewards (items, the reward actions).
        assert_eq!(texts(d.follow_up(Some((1, QuestStatus::Active)), &p(1, QuestStatus::Completed))), ["Give item", "Run script"]);
        // Failed: nothing.
        assert!(d.follow_up(Some((0, QuestStatus::Active)), &p(0, QuestStatus::Failed)).is_empty());
    }

    #[test]
    fn quest_progress() {
        let mut w = WorldState::default();
        assert!(w.check_quest("q", QuestCheck::NotStarted, 0));
        assert_eq!(w.apply_quest("q", QuestOp::Start, 0), Some(QuestChange::Started));
        assert_eq!(w.apply_quest("q", QuestOp::Start, 0), None);
        assert!(w.check_quest("q", QuestCheck::AtStage, 0));
        assert_eq!(w.apply_quest("q", QuestOp::Advance, 0), Some(QuestChange::Updated));
        assert!(w.check_quest("q", QuestCheck::StageAtLeast, 1) && !w.check_quest("q", QuestCheck::AtStage, 0));
        assert_eq!(w.apply_quest("q", QuestOp::Complete, 0), Some(QuestChange::Completed));
        // Finished quests stay finished.
        assert_eq!(w.apply_quest("q", QuestOp::Advance, 0), None);
        assert_eq!(w.apply_quest("q", QuestOp::Fail, 0), None);
        assert!(w.check_quest("q", QuestCheck::Completed, 0) && w.check_quest("q", QuestCheck::StageAtLeast, 9));
        // Setting a stage starts a quest.
        assert_eq!(w.apply_quest("r", QuestOp::SetStage, 2), Some(QuestChange::Started));
        assert_eq!(w.quest("r").unwrap().stage, 2);
        assert!(w.quest("r").unwrap().order > w.quest("q").unwrap().order);
    }

    #[test]
    fn expands_text() {
        let mut w = WorldState { player_name: "Ada".into(), ..Default::default() };
        w.set_var("slimes", 3);
        assert_eq!(w.expand_text("Slimes ({slimes}/5) {} {a b} {"), "Slimes (3/5) {} {a b} {");
        assert_eq!(w.expand_text("Thank you, {sir|madam} {player}! {He|She} said."), "Thank you, sir Ada! He said.");
        w.player_sex = crate::state::Sex::Female;
        assert_eq!(w.expand_text("{sir|madam} {player}, {|ms. }x"), "madam Ada, ms. x");
        // An action's key (the keyboard: no gamepad used in tests); unknown actions stay as written.
        assert_eq!(w.expand_text("Press {key:interact} to talk, {key:journal} for the journal. {key:nope}"), "Press E to talk, J for the journal. {key:nope}");
    }
}
