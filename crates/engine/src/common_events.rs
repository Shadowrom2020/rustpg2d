//! Common events (`defs/common_events.ron`): named action lists, picked in the editor instead of
//! written as script functions. *Call common event* / `call_common(id)` runs one; with a trigger
//! it also runs by itself: *parallel* (again and again, beside play) or *autorun* (holding the
//! player while it runs) as long as a flag is set. `runtime/common.rs` runs them.

use serde::{Deserialize, Serialize};

use crate::events::Action;

/// When a common event runs by itself.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum CommonTrigger {
    /// Only when called.
    #[default]
    Call,
    /// Again and again while `flag` is set, beside play (its own action queue).
    Parallel { flag: String },
    /// Again and again while `flag` is set, holding the player while it runs (clear the flag in
    /// its actions to end it).
    Autorun { flag: String },
}

impl CommonTrigger {
    pub fn label(&self) -> &'static str {
        match self {
            CommonTrigger::Call => "Only when called",
            CommonTrigger::Parallel { .. } => "Parallel while flag",
            CommonTrigger::Autorun { .. } => "Autorun while flag",
        }
    }

    /// The flag it runs while set ("" for *Call*).
    pub fn flag(&self) -> &str {
        match self {
            CommonTrigger::Call => "",
            CommonTrigger::Parallel { flag } | CommonTrigger::Autorun { flag } => flag,
        }
    }
}

/// A common event.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CommonEventDef {
    pub id: String,
    /// Name shown in the editor.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub trigger: CommonTrigger,
    #[serde(default)]
    pub actions: Vec<Action>,
}

impl CommonEventDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> Self {
        CommonEventDef { id: id.to_string(), name: id.to_string(), ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_common_event_round_trips() {
        let d = CommonEventDef {
            id: "reward".into(),
            name: "Quest reward".into(),
            trigger: CommonTrigger::Parallel { flag: "rain_dance".into() },
            actions: vec![Action::Wait { seconds: 1.0 }, Action::RunScript { function: "quest_thanks".into() }],
        };
        let text = ron::to_string(&d).unwrap();
        assert_eq!(ron::from_str::<CommonEventDef>(&text).unwrap(), d);
        assert_eq!(ron::from_str::<CommonEventDef>(r#"(id: "x")"#).unwrap().trigger, CommonTrigger::Call);
        assert_eq!(d.trigger.flag(), "rain_dance");
    }
}
