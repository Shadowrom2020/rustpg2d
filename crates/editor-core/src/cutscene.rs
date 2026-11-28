//! A cut-scene looked at in time: where its actors are and what the camera frames at a
//! moment (worked out from the cues: walks at the walker's speed, pans over their seconds;
//! a dialog that holds the scene in the game takes no time here), what a cue's box says, the
//! actions a track offers, and how cues that would overlap stack in lanes.

use gfx::prelude::{vec2, Vec2};
use rustpg2d_engine::camera::CameraAim;
use rustpg2d_engine::cutscenes::{Cue, CutsceneDef, TrackKind};
use rustpg2d_engine::events::Action;
use rustpg2d_engine::map::Map;

use crate::workspace::Workspace;

/// Walking speed when the walker's definition is unknown (world px per second).
const WALK: f32 = 120.0;

/// Where things are at a moment of a cut-scene, as the editor previews it.
#[derive(Debug, Default, PartialEq)]
pub struct Preview {
    /// Each actor track's character: its name, where it stands (tile, fractions while walking),
    /// whether it is walking.
    pub actors: Vec<(String, Vec2, bool)>,
    /// The camera's middle (tile) and zoom; None = on the player as usual.
    pub camera: Option<(Vec2, f32)>,
}

/// The tile `target` starts on: a character of that name on the map (the player: the map's
/// `start` spawn, else none until a cue places them).
fn start_of(map: &Map, target: &str) -> Option<Vec2> {
    if target == "player" {
        return map.zones.iter().find(|z| z.name == "start").map(|z| vec2(z.x as f32, z.y as f32));
    }
    let ts = rustpg2d_engine::project::TILE_SIZE as f32;
    map.entities.iter().find(|e| e.name == target).map(|e| vec2((e.x / ts).floor(), ((e.y - 1.0) / ts).floor()))
}

/// Where a cut-scene has everyone at `t` seconds. The cues are taken at their times (a dialog
/// that holds the scene in the game takes no time here): walks go at `speed(target)` world
/// pixels per second, pans over their seconds.
pub fn preview_at(d: &CutsceneDef, map: &Map, t: f32, speed: &dyn Fn(&str) -> f32) -> Preview {
    let ts = rustpg2d_engine::project::TILE_SIZE as f32;
    let mut out = Preview::default();
    let pos_of = |name: &str, p: &Preview| p.actors.iter().find(|a| a.0 == name).map(|a| a.1);
    for (_, c) in d.cues().into_iter().filter(|(_, c)| c.at <= t + 1e-4) {
        let since = t - c.at;
        match &c.action {
            Action::Place { target, x, y } => set(&mut out.actors, target, vec2(*x as f32, *y as f32), false),
            Action::WalkTo { target, x, y } => {
                let to = vec2(*x as f32, *y as f32);
                match pos_of(target, &out).or_else(|| start_of(map, target)) {
                    Some(from) => {
                        let secs = from.distance(to) * ts / speed(target).max(1.0);
                        let k = if secs <= 0.0 { 1.0 } else { (since / secs).min(1.0) };
                        set(&mut out.actors, target, from.lerp(to, k), k < 1.0);
                    }
                    None => set(&mut out.actors, target, to, false),
                }
            }
            Action::Approach { target, to } => {
                if let (Some(a), Some(b)) = (pos_of(target, &out).or_else(|| start_of(map, target)), pos_of(to, &out).or_else(|| start_of(map, to))) {
                    let off = if (a - b).x.abs() >= (a - b).y.abs() { vec2((a - b).x.signum(), 0.0) } else { vec2(0.0, (a - b).y.signum()) };
                    let dest = b + if off == Vec2::ZERO { vec2(0.0, 1.0) } else { off };
                    let secs = a.distance(dest) * ts / speed(target).max(1.0);
                    let k = if secs <= 0.0 { 1.0 } else { (since / secs).min(1.0) };
                    set(&mut out.actors, target, a.lerp(dest, k), k < 1.0);
                }
            }
            Action::Camera { aim, seconds, zoom, .. } => {
                let to = match aim {
                    CameraAim::Tile { x, y } => Some(vec2(*x as f32, *y as f32)),
                    CameraAim::Player => pos_of("player", &out).or_else(|| start_of(map, "player")),
                    CameraAim::Follow { target } => pos_of(target, &out).or_else(|| start_of(map, target)),
                };
                if let Some(to) = to {
                    let from = out.camera.map(|c| c.0).or_else(|| pos_of("player", &out)).or_else(|| start_of(map, "player")).unwrap_or(to);
                    let k = if *seconds <= 0.0 { 1.0 } else { (since / seconds).min(1.0) };
                    let z0 = out.camera.map_or(1.0, |c| c.1);
                    out.camera = Some((from.lerp(to, k), z0 + (zoom.max(0.1) - z0) * k));
                }
            }
            _ => {}
        }
    }
    // Actor tracks whose character hasn't moved yet stand where the map has them.
    for tr in d.tracks.iter().filter(|t| t.kind == TrackKind::Actor && !t.target.is_empty()) {
        if pos_of(&tr.target, &out).is_none() {
            if let Some(p) = start_of(map, &tr.target) {
                set(&mut out.actors, &tr.target, p, false);
            }
        }
    }
    out
}

fn set(actors: &mut Vec<(String, Vec2, bool)>, name: &str, p: Vec2, walking: bool) {
    match actors.iter_mut().find(|a| a.0 == name) {
        Some(a) => (a.1, a.2) = (p, walking),
        None => actors.push((name.to_string(), p, walking)),
    }
}

/// A short line for a cue's box.
pub fn cue_label(a: &Action) -> String {
    let short = |s: &str| if s.chars().count() > 18 { format!("{}…", s.chars().take(17).collect::<String>()) } else { s.to_string() };
    match a {
        Action::WalkTo { x, y, .. } => format!("walk {x},{y}"),
        Action::Place { x, y, .. } => format!("at {x},{y}"),
        Action::Approach { to, .. } => format!("to {to}"),
        Action::Face { facing, .. } => format!("face {facing:?}").to_lowercase(),
        Action::Emote { emotion, .. } => emotion.clone(),
        Action::Dialog { speaker, text } => short(&if speaker.is_empty() { text.clone() } else { format!("{speaker}: {text}") }),
        Action::Camera { aim, .. } => match aim {
            CameraAim::Player => "camera: player".into(),
            CameraAim::Tile { x, y } => format!("camera {x},{y}"),
            CameraAim::Follow { target } => format!("follow {target}"),
        },
        Action::PlaySound { sound, .. } | Action::PlayMusic { sound, .. } | Action::PlayMusicEffect { sound, .. } => sound.clone(),
        Action::ShowPicture { layer, sheet, .. } => format!("{layer}: {sheet}"),
        Action::MovePicture { layer, .. } => format!("move {layer}"),
        Action::ErasePicture { layer } => format!("erase {layer}"),
        Action::Wait { seconds } => format!("wait {seconds:.1}"),
        other => other.label().to_lowercase(),
    }
}

/// The actions a track of `kind` offers for new cues (with the actor filled in).
pub fn offered(kind: TrackKind, target: &str) -> Vec<Action> {
    Action::templates()
        .into_iter()
        .filter(|a| kind.fits(a))
        .map(|mut a| {
            if kind == TrackKind::Actor {
                if let Action::WalkTo { target: t, .. } | Action::Place { target: t, .. } | Action::Face { target: t, .. } | Action::Emote { target: t, .. } | Action::Bark { target: t, .. } | Action::MoveRoute { target: t, .. } | Action::Approach { target: t, .. } | Action::Mood { target: t, .. } = &mut a {
                    *t = target.to_string();
                }
            }
            a
        })
        .collect()
}

/// The width of a cue's block on the timeline (pixels).
pub fn cue_width(pps: f32) -> f32 {
    96.0_f32.min(pps * 2.0).max(34.0)
}

/// Each cue's lane within its track, so blocks that would overlap stack instead (cues close
/// together, like dialog lines that wait for each other), and the number of lanes.
pub fn lanes(cues: &[Cue], pps: f32) -> (Vec<usize>, usize) {
    let w = cue_width(pps);
    let mut order: Vec<usize> = (0..cues.len()).collect();
    order.sort_by(|a, b| cues[*a].at.total_cmp(&cues[*b].at).then(a.cmp(b)));
    let mut ends: Vec<f32> = Vec::new();
    let mut lane = vec![0; cues.len()];
    for k in order {
        let x = cues[k].at * pps;
        let l = ends.iter().position(|e| *e <= x + 0.5).unwrap_or(ends.len());
        if l == ends.len() {
            ends.push(0.0);
        }
        ends[l] = x + w + 2.0;
        lane[k] = l;
    }
    (lane, ends.len().max(1))
}

impl Workspace {
    /// The map cut-scene `id` plays on (the start map when it names none).
    pub fn cutscene_map(&self, id: &str) -> Option<String> {
        let d = self.project.db.cutscene(id)?;
        Some(if d.map.is_empty() { self.project.config.start_map.clone() } else { d.map.clone() })
    }

    /// Where cut-scene `id` has everyone at `t` seconds, on its map as it is now (open: as it
    /// is being worked on).
    pub fn cutscene_preview(&self, id: &str, t: f32) -> Option<Preview> {
        let d = self.project.db.cutscene(id)?;
        let name = self.cutscene_map(id)?;
        let loaded;
        let map = match self.open_maps().into_iter().find(|doc| doc.map.name == name) {
            Some(doc) => &doc.map,
            None => {
                loaded = self.project.load_map(&name).ok()?;
                &loaded
            }
        };
        let db = &self.project.db;
        let speed = |who: &str| {
            let def = if who == "player" { self.project.config.player.clone() } else { map.entities.iter().find(|e| e.name == who).map(|e| e.def.clone()).unwrap_or_default() };
            db.character(&def).map_or(WALK, |c| c.speed)
        };
        Some(preview_at(d, map, t, &speed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::cutscenes::CutsceneTrack;

    #[test]
    fn cues_close_together_stack_in_lanes() {
        let at = |t: f32| Cue { at: t, wait: true, action: Action::Wait { seconds: 0.0 } };
        // 50 px a second, blocks of 96 px: 7.5 / 7.6 / 7.7 overlap, 10.0 is clear of the first.
        let (lane, n) = lanes(&[at(7.6), at(7.5), at(10.0), at(7.7)], 50.0);
        assert_eq!((lane, n), (vec![1, 0, 0, 2], 3));
        assert_eq!(lanes(&[], 50.0).1, 1);
    }

    fn map() -> Map {
        let mut m = Map::new("village", 40, 30);
        let mut guard = rustpg2d_engine::entity::EntityInstance::new(1, rustpg2d_engine::entity::EntityKind::Character, "guard", 10.0 * 32.0 + 16.0, 5.0 * 32.0 + 31.0);
        guard.name = "guard".into();
        m.entities.push(guard);
        m
    }

    #[test]
    fn the_preview_walks_actors_and_pans_the_camera() {
        let d = CutsceneDef {
            id: "intro".into(),
            tracks: vec![
                CutsceneTrack { kind: TrackKind::Actor, target: "player".into(), cues: vec![Cue { at: 0.0, wait: true, action: Action::Place { target: "player".into(), x: 0, y: 5 } }, Cue { at: 1.0, wait: true, action: Action::WalkTo { target: "player".into(), x: 4, y: 5 } }] },
                CutsceneTrack { kind: TrackKind::Actor, target: "guard".into(), cues: vec![] },
                CutsceneTrack { kind: TrackKind::Camera, target: String::new(), cues: vec![Cue { at: 0.0, wait: false, action: Action::Camera { aim: CameraAim::Tile { x: 20, y: 5 }, seconds: 2.0, easing: Default::default(), zoom: 1.0, wait: true } }] },
            ],
            ..Default::default()
        };
        // 4 tiles at 64 px/s = 2 s; at 2 s the player is halfway, the camera there.
        let p = preview_at(&d, &map(), 2.0, &|_| 64.0);
        let player = p.actors.iter().find(|a| a.0 == "player").unwrap();
        assert!((player.1.x - 2.0).abs() < 1e-3 && player.2, "{player:?}");
        assert_eq!(p.actors.iter().find(|a| a.0 == "guard").map(|a| a.1), Some(vec2(10.0, 5.0)));
        let (cam, zoom) = p.camera.unwrap();
        assert!((cam.x - 20.0).abs() < 1e-3 && (zoom - 1.0).abs() < 1e-3);
        let early = preview_at(&d, &map(), 1.0, &|_| 64.0);
        assert!((early.camera.unwrap().0.x - 10.0).abs() < 1e-3, "halfway from the player's spot to tile 20");
    }
}
