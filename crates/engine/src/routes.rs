//! Move routes (*Move route*): a character (or the player) walks a list of steps one by one —
//! tiles in a direction, toward or away from the player, turns, waits, jumps, speed changes,
//! animation clips, sounds, walking through walls, scripts. `runtime/routes.rs` runs them.

use serde::{Deserialize, Serialize};

use crate::entity::Facing;

/// One step of a route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RouteStep {
    /// Walks `tiles` tiles in a direction.
    Move { dir: Facing, tiles: u32 },
    /// Walks `tiles` tiles toward the player (a tile at a time, along the longer axis).
    Toward { tiles: u32 },
    /// Walks `tiles` tiles away from the player.
    Away { tiles: u32 },
    /// Turns to a direction.
    Turn { dir: Facing },
    /// Turns to face the player.
    TurnToPlayer,
    /// Turns to a random direction.
    TurnRandom,
    /// Waits.
    Wait { seconds: f32 },
    /// Jumps `dx`, `dy` tiles (0, 0 = on the spot), over anything in between.
    Jump { dx: i32, dy: i32 },
    /// Walks at this part of its usual speed from now on (1 = usual, 2 = twice as fast).
    Speed { speed: f32 },
    /// Plays an animation clip of its 3D model ("wave", "craft"…) for some seconds.
    Anim { clip: String, seconds: f32 },
    /// Plays a sound.
    Sound { sound: String },
    /// Walks through walls and characters (on) or not (off).
    Through { on: bool },
    /// Runs a script function.
    Script { function: String },
}

impl RouteStep {
    /// One of each, for the editor's buttons.
    pub fn templates() -> Vec<RouteStep> {
        vec![
            RouteStep::Move { dir: Facing::Down, tiles: 1 },
            RouteStep::Toward { tiles: 1 },
            RouteStep::Away { tiles: 1 },
            RouteStep::Turn { dir: Facing::Down },
            RouteStep::TurnToPlayer,
            RouteStep::TurnRandom,
            RouteStep::Wait { seconds: 0.5 },
            RouteStep::Jump { dx: 0, dy: 0 },
            RouteStep::Speed { speed: 1.0 },
            RouteStep::Anim { clip: String::new(), seconds: 1.0 },
            RouteStep::Sound { sound: String::new() },
            RouteStep::Through { on: true },
            RouteStep::Script { function: String::new() },
        ]
    }

    /// The button's name.
    pub fn label(&self) -> &'static str {
        match self {
            RouteStep::Move { .. } => "Move",
            RouteStep::Toward { .. } => "Toward player",
            RouteStep::Away { .. } => "Away from player",
            RouteStep::Turn { .. } => "Turn",
            RouteStep::TurnToPlayer => "Face player",
            RouteStep::TurnRandom => "Turn at random",
            RouteStep::Wait { .. } => "Wait",
            RouteStep::Jump { .. } => "Jump",
            RouteStep::Speed { .. } => "Speed",
            RouteStep::Anim { .. } => "Animation",
            RouteStep::Sound { .. } => "Sound",
            RouteStep::Through { .. } => "Through walls",
            RouteStep::Script { .. } => "Script",
        }
    }

    /// A short line for lists and `status` ("move right 3", "wait 0.5").
    pub fn summary(&self) -> String {
        let d = |f: &Facing| format!("{f:?}").to_lowercase();
        match self {
            RouteStep::Move { dir, tiles } => format!("move {} {tiles}", d(dir)),
            RouteStep::Toward { tiles } => format!("toward {tiles}"),
            RouteStep::Away { tiles } => format!("away {tiles}"),
            RouteStep::Turn { dir } => format!("turn {}", d(dir)),
            RouteStep::TurnToPlayer => "face player".into(),
            RouteStep::TurnRandom => "turn random".into(),
            RouteStep::Wait { seconds } => format!("wait {seconds}"),
            RouteStep::Jump { dx, dy } => format!("jump {dx},{dy}"),
            RouteStep::Speed { speed } => format!("speed {speed}"),
            RouteStep::Anim { clip, seconds } => format!("anim {clip} {seconds}"),
            RouteStep::Sound { sound } => format!("sound {sound}"),
            RouteStep::Through { on } => format!("through {}", if *on { "on" } else { "off" }),
            RouteStep::Script { function } => format!("script {function}"),
        }
    }

    /// Parses `summary()` back (scripts' `move_route(target, ["move right 3", …])`).
    pub fn parse(s: &str) -> Option<RouteStep> {
        let w: Vec<&str> = s.split_whitespace().collect();
        let n = |k: usize, d: f32| w.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
        let dir = |k: usize| w.get(k).and_then(|v| facing(v));
        Some(match *w.first()? {
            "move" => RouteStep::Move { dir: dir(1)?, tiles: n(2, 1.0).max(0.0) as u32 },
            "toward" => RouteStep::Toward { tiles: n(1, 1.0).max(0.0) as u32 },
            "away" => RouteStep::Away { tiles: n(1, 1.0).max(0.0) as u32 },
            "turn" if w.get(1) == Some(&"random") => RouteStep::TurnRandom,
            "turn" => RouteStep::Turn { dir: dir(1)? },
            "face" => RouteStep::TurnToPlayer,
            "wait" => RouteStep::Wait { seconds: n(1, 0.5) },
            "jump" => {
                let (dx, dy) = w.get(1).and_then(|v| v.split_once(',')).map_or((0, 0), |(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(0)));
                RouteStep::Jump { dx, dy }
            }
            "speed" => RouteStep::Speed { speed: n(1, 1.0) },
            "anim" => RouteStep::Anim { clip: w.get(1)?.to_string(), seconds: n(2, 1.0) },
            "sound" => RouteStep::Sound { sound: w.get(1)?.to_string() },
            "through" => RouteStep::Through { on: w.get(1) != Some(&"off") },
            "script" => RouteStep::Script { function: w.get(1)?.to_string() },
            _ => return None,
        })
    }
}

/// "up", "down", "left", "right" (any case).
pub fn facing(s: &str) -> Option<Facing> {
    match s.trim().to_lowercase().as_str() {
        "up" => Some(Facing::Up),
        "down" => Some(Facing::Down),
        "left" => Some(Facing::Left),
        "right" => Some(Facing::Right),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_read_back_from_their_summary() {
        for s in RouteStep::templates().into_iter().chain([RouteStep::Move { dir: Facing::Right, tiles: 3 }, RouteStep::Jump { dx: -1, dy: 2 }, RouteStep::Anim { clip: "wave".into(), seconds: 2.0 }, RouteStep::Sound { sound: "blip".into() }, RouteStep::Script { function: "f".into() }]) {
            let empty_name = matches!(&s, RouteStep::Anim { clip, .. } if clip.is_empty()) || matches!(&s, RouteStep::Sound { sound } if sound.is_empty()) || matches!(&s, RouteStep::Script { function } if function.is_empty());
            if empty_name {
                continue;
            }
            assert_eq!(RouteStep::parse(&s.summary()), Some(s.clone()), "{}", s.summary());
        }
        assert_eq!(RouteStep::parse("fly away"), None);
    }
}
