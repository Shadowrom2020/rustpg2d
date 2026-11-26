//! Cut-scene assets (`defs/cutscenes.ron`): a cut-scene as tracks of timed cues instead of a
//! script function. A track belongs to an actor (walking, facing, emotions, barks, routes), the
//! camera, dialogs, sound and music, or the screen (pictures, tints, fades); each cue is an
//! ordinary [`Action`] at a time in seconds.
//!
//! *Play cutscene* plays one by its id (a script function of that name otherwise): the cues are
//! turned into one list of actions with waits between them ([`CutsceneDef::actions`]). A cue that
//! *waits* holds the scene until it is done (a dialog until it is closed, a walk until the
//! walker arrives), so the cues after it come that much later; one that doesn't runs alongside
//! ([`Action::Parallel`]).

use serde::{Deserialize, Serialize};

use crate::events::Action;

/// A cut-scene asset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CutsceneDef {
    pub id: String,
    /// Name shown in the editor.
    #[serde(default)]
    pub name: String,
    /// The map it is made for: the editor previews it there (the game plays it wherever it is
    /// called).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub map: String,
    #[serde(default)]
    pub tracks: Vec<CutsceneTrack>,
}

/// One row of the timeline.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CutsceneTrack {
    #[serde(default)]
    pub kind: TrackKind,
    /// An actor track's character: "player" or a character's name on the map.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(default)]
    pub cues: Vec<Cue>,
}

/// What a track holds (the editor offers the matching actions).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum TrackKind {
    #[default]
    Actor,
    Camera,
    Dialog,
    Sound,
    Screen,
}

impl TrackKind {
    pub const ALL: [TrackKind; 5] = [TrackKind::Actor, TrackKind::Camera, TrackKind::Dialog, TrackKind::Sound, TrackKind::Screen];

    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Actor => "Actor",
            TrackKind::Camera => "Camera",
            TrackKind::Dialog => "Dialog",
            TrackKind::Sound => "Sound and music",
            TrackKind::Screen => "Pictures and screen",
        }
    }

    /// Whether `a` belongs on a track of this kind (the editor's choice of new cues).
    pub fn fits(self, a: &Action) -> bool {
        match self {
            TrackKind::Actor => matches!(
                a,
                Action::WalkTo { .. } | Action::Approach { .. } | Action::Place { .. } | Action::Face { .. } | Action::Emote { .. } | Action::Bark { .. } | Action::MoveRoute { .. } | Action::Mood { .. } | Action::SetActive { .. }
            ),
            TrackKind::Camera => matches!(a, Action::Camera { .. } | Action::Shake { .. }),
            TrackKind::Dialog => matches!(a, Action::Dialog { .. } | Action::Choice { .. } | Action::Wait { .. } | Action::RunScript { .. }),
            TrackKind::Sound => matches!(a, Action::PlaySound { .. } | Action::PlayMusic { .. } | Action::PlayMusicEffect { .. }),
            TrackKind::Screen => matches!(
                a,
                Action::ShowPicture { .. } | Action::MovePicture { .. } | Action::ErasePicture { .. } | Action::TintScreen { .. } | Action::FlashScreen { .. } | Action::FadeOut { .. } | Action::FadeIn { .. } | Action::Dim { .. } | Action::Effect { .. } | Action::SetWeather { .. }
            ),
        }
    }

    /// Whether a new cue of `a` on this track waits by default (dialogs and walks do, the camera,
    /// sounds and pictures run alongside).
    pub fn waits_by_default(self, a: &Action) -> bool {
        matches!(a, Action::Dialog { .. } | Action::Choice { .. } | Action::WalkTo { .. } | Action::Approach { .. } | Action::Wait { .. } | Action::FadeOut { .. } | Action::FadeIn { .. })
    }
}

/// An action at a time of the cut-scene.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Cue {
    /// Seconds from the start (a cue that waits holds the scene: the ones after it come later).
    #[serde(default)]
    pub at: f32,
    /// Holds the scene until it is done; otherwise it runs alongside.
    #[serde(default = "yes")]
    pub wait: bool,
    pub action: Action,
}

fn yes() -> bool {
    true
}

impl CutsceneDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> Self {
        CutsceneDef { id: id.to_string(), name: id.to_string(), ..Default::default() }
    }

    /// Every cue in playing order: by time, then by track, then as listed.
    pub fn cues(&self) -> Vec<(usize, &Cue)> {
        let mut all: Vec<(usize, usize, &Cue)> = self.tracks.iter().enumerate().flat_map(|(t, tr)| tr.cues.iter().enumerate().map(move |(k, c)| (t, k, c))).collect();
        all.sort_by(|a, b| a.2.at.total_cmp(&b.2.at).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        all.into_iter().map(|(t, _, c)| (t, c)).collect()
    }

    /// Where the last cue starts (seconds).
    pub fn length(&self) -> f32 {
        self.tracks.iter().flat_map(|t| &t.cues).map(|c| c.at).fold(0.0, f32::max)
    }

    /// The actions that play it from `from` seconds on: what the cues before `from` leave behind
    /// happens at once (walkers stand where they were going, the camera is there, pictures
    /// shown; dialogs and sounds are left out), then the cues with waits between them.
    pub fn actions(&self, from: f32) -> Vec<Action> {
        let mut out = Vec::new();
        let cues = self.cues();
        let start = from.max(0.0);
        for (_, c) in cues.iter().filter(|(_, c)| c.at < start - 1e-4) {
            if let Some(a) = catch_up(&c.action) {
                out.push(Action::Parallel { actions: vec![a] });
            }
        }
        let mut t = start;
        for (_, c) in cues.iter().filter(|(_, c)| c.at >= start - 1e-4) {
            if c.at > t + 1e-4 {
                out.push(Action::Wait { seconds: c.at - t });
                t = c.at;
            }
            let a = with_wait(c.action.clone(), true);
            out.push(if c.wait { a } else { Action::Parallel { actions: vec![a] } });
        }
        out
    }
}

/// `a` with its own *wait* option (where it has one) set to `wait`.
fn with_wait(mut a: Action, on: bool) -> Action {
    match &mut a {
        Action::Camera { wait, .. } | Action::MovePicture { wait, .. } | Action::TintScreen { wait, .. } | Action::FadeOut { wait, .. } | Action::FadeIn { wait, .. } | Action::MoveRoute { wait, .. } | Action::Emote { wait, .. } => *wait = on,
        _ => {}
    }
    a
}

/// What a cue the scene starts after leaves behind, done at once: a walk ends where it was
/// going, a pan is there, a picture's move is over. Dialogs, sounds, emotions and other
/// moments are left out (None).
fn catch_up(a: &Action) -> Option<Action> {
    Some(match a.clone() {
        Action::WalkTo { target, x, y } => Action::Place { target, x, y },
        Action::Camera { aim, zoom, easing, .. } => Action::Camera { aim, seconds: 0.0, easing, zoom, wait: false },
        Action::MovePicture { layer, to, easing, .. } => Action::MovePicture { layer, to, seconds: 0.0, easing, wait: false },
        Action::TintScreen { color, .. } => Action::TintScreen { color, seconds: 0.0, wait: false },
        Action::Dim { amount, .. } => Action::Dim { amount, seconds: 0.0 },
        Action::FadeOut { color, .. } => Action::FadeOut { seconds: 0.0, color, wait: false },
        Action::FadeIn { .. } => Action::FadeIn { seconds: 0.0, wait: false },
        Action::Dialog { .. }
        | Action::Choice { .. }
        | Action::Emote { .. }
        | Action::Bark { .. }
        | Action::PlaySound { .. }
        | Action::PlayMusicEffect { .. }
        | Action::Wait { .. }
        | Action::Shake { .. }
        | Action::FlashScreen { .. }
        | Action::Effect { .. }
        | Action::MoveRoute { .. }
        | Action::ScrollText { .. } => return None,
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::CameraAim;

    fn cue(at: f32, wait: bool, action: Action) -> Cue {
        Cue { at, wait, action }
    }

    fn scene() -> CutsceneDef {
        let walk = Action::WalkTo { target: "player".into(), x: 5, y: 30 };
        let pan = Action::Camera { aim: CameraAim::Tile { x: 34, y: 22 }, seconds: 4.5, easing: Default::default(), zoom: 1.0, wait: true };
        let line = Action::Dialog { speaker: "Guard".into(), text: "Halt!".into() };
        CutsceneDef {
            id: "intro".into(),
            tracks: vec![
                CutsceneTrack { kind: TrackKind::Actor, target: "player".into(), cues: vec![cue(0.0, true, Action::Place { target: "player".into(), x: 0, y: 30 }), cue(6.0, true, walk)] },
                CutsceneTrack { kind: TrackKind::Camera, cues: vec![cue(0.0, false, pan)], ..Default::default() },
                CutsceneTrack { kind: TrackKind::Dialog, cues: vec![cue(7.5, true, line)], ..Default::default() },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn cues_become_actions_with_waits_and_side_runs() {
        let a = scene().actions(0.0);
        // The place, the pan alongside (its own wait on: the side run waits for it), 6 s, the
        // walk, 1.5 s, the line.
        assert!(matches!(&a[0], Action::Place { .. }));
        assert!(matches!(&a[1], Action::Parallel { actions } if matches!(&actions[0], Action::Camera { wait: true, .. })));
        assert!(matches!(a[2], Action::Wait { seconds } if (seconds - 6.0).abs() < 1e-4));
        assert!(matches!(&a[3], Action::WalkTo { .. }));
        assert!(matches!(a[4], Action::Wait { seconds } if (seconds - 1.5).abs() < 1e-4));
        assert!(matches!(&a[5], Action::Dialog { .. }));
        assert_eq!(a.len(), 6);
        assert!((scene().length() - 7.5).abs() < 1e-4);
    }

    #[test]
    fn playing_from_a_moment_catches_up_without_dialogs() {
        let a = scene().actions(7.0);
        // Before 7 s: the place, the pan (at once) and the walk (as a place); then 0.5 s and the line.
        let caught: Vec<&Action> = a.iter().take_while(|x| matches!(x, Action::Parallel { .. })).collect();
        assert_eq!(caught.len(), 3);
        assert!(matches!(&caught[1], Action::Parallel { actions } if matches!(&actions[0], Action::Camera { seconds, .. } if *seconds == 0.0)));
        assert!(matches!(&caught[2], Action::Parallel { actions } if matches!(&actions[0], Action::Place { x: 5, y: 30, .. })));
        assert!(matches!(a[3], Action::Wait { seconds } if (seconds - 0.5).abs() < 1e-4));
        assert!(matches!(&a[4], Action::Dialog { .. }));
    }

    #[test]
    fn a_cutscene_round_trips_and_old_fields_default() {
        let d = scene();
        let text = ron::to_string(&d).unwrap();
        assert_eq!(ron::from_str::<CutsceneDef>(&text).unwrap(), d);
        let bare: CutsceneDef = ron::from_str(r#"(id: "x", tracks: [(cues: [(action: Wait(seconds: 1.0))])])"#).unwrap();
        assert_eq!((bare.tracks[0].kind, bare.tracks[0].cues[0].wait, bare.tracks[0].cues[0].at), (TrackKind::Actor, true, 0.0));
    }
}
