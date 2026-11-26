//! What a test-play starts with: the hero's level and class, items, flags, quests, the hour, the party.

use serde::{Deserialize, Serialize};

/// What a test-play starts with (besides the place).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestSetup {
    /// The hero's level (0 or 1 = a new hero's).
    pub level: u32,
    /// The hero's class id (empty = none, as a new game without the creator).
    pub class: String,
    /// Items in the bag: (item id, count).
    pub items: Vec<(String, u32)>,
    pub flags: Vec<String>,
    pub vars: Vec<(String, i64)>,
    /// Quests started: (quest id, stage, completed).
    pub quests: Vec<(String, u32, bool)>,
    /// The hour (None = the project's start hour) and the day.
    pub hour: Option<f32>,
    pub day: u32,
    /// Party members: names of placed characters (on any map) who join as companions.
    pub party: Vec<String>,
}

impl TestSetup {
    /// For `status` (`testsetup=`): level/class, then counts.
    pub fn summary(&self) -> String {
        format!(
            "L{}{}:items{}:flags{}:vars{}:quests{}:party{}{}",
            self.level.max(1),
            if self.class.is_empty() { String::new() } else { format!("/{}", self.class) },
            self.items.len(),
            self.flags.len(),
            self.vars.len(),
            self.quests.len(),
            self.party.len(),
            self.hour.map_or(String::new(), |h| format!("@{h:.1}h/d{}", self.day)),
        )
    }
}
