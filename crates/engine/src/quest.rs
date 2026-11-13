//! Quests: definitions (title, description, stages), the player's progress, and the
//! operations / checks used by the "Quest" action and condition.

use serde::{Deserialize, Serialize};

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
