//! Timers (*Start timer* / *Stop timer*, `timer_start`): named countdowns, shown on screen or not,
//! that run a script function or fire a custom event when they run out. They pause while a menu
//! or a dialog is open and are saved in `WorldState::timers`.

use serde::{Deserialize, Serialize};

/// A running timer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    /// Seconds left.
    pub left: f32,
    /// Seconds it started with.
    pub total: f32,
    /// Shown at the top of the screen (minutes and seconds).
    #[serde(default)]
    pub shown: bool,
    /// The script function it runs when it runs out ("" = none).
    #[serde(default)]
    pub function: String,
    /// The custom event it fires when it runs out ("" = none).
    #[serde(default)]
    pub event: String,
}

impl Timer {
    pub fn new(seconds: f32, shown: bool, function: &str, event: &str) -> Self {
        Timer { left: seconds.max(0.0), total: seconds.max(0.0), shown, function: function.to_string(), event: event.to_string() }
    }
}

/// "1:05" for 65 seconds (rounded up, so a timer shows 0:00 only when it has run out).
pub fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Counts every timer down by `dt`; returns the ones that ran out (taken away), by name.
pub fn tick(timers: &mut std::collections::BTreeMap<String, Timer>, dt: f32) -> Vec<(String, Timer)> {
    let mut ended = Vec::new();
    for (name, t) in timers.iter_mut() {
        t.left -= dt;
        if t.left <= 0.0 {
            t.left = 0.0;
            ended.push((name.clone(), t.clone()));
        }
    }
    for (name, _) in &ended {
        timers.remove(name);
    }
    ended
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timers_count_down_and_end() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("soup".to_string(), Timer::new(2.0, true, "soup_cold", ""));
        m.insert("long".to_string(), Timer::new(60.0, false, "", "bell"));
        assert!(tick(&mut m, 1.5).is_empty());
        assert_eq!(clock(m["soup"].left), "0:01");
        let ended = tick(&mut m, 1.0);
        assert_eq!(ended.iter().map(|e| (e.0.as_str(), e.1.function.as_str())).collect::<Vec<_>>(), [("soup", "soup_cold")]);
        assert_eq!(m.keys().collect::<Vec<_>>(), ["long"]);
        assert_eq!(clock(65.0), "1:05");
        let text = ron::to_string(&m).unwrap();
        assert_eq!(ron::from_str::<std::collections::BTreeMap<String, Timer>>(&text).unwrap(), m);
    }
}
