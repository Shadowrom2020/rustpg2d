//! *Camera*: pans and zooms the game camera to a tile, a character it then follows, or back to
//! the player, over some seconds with an easing. The camera stays where an event put it while
//! the event (or cut-scene) runs; once it is over and the last move has arrived, it glides back
//! to the player. Not saved; a map change returns it at once.

use macroquad::prelude::*;

use crate::camera::CameraAim;
use crate::pictures::Easing;

use super::Game;

/// How long the camera takes to come back to the player by itself.
const RETURN_SECONDS: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Aim {
    Player,
    /// A point of the map (pixels).
    Point(Vec2),
    /// A placed entity (uid), followed.
    Entity(u32),
}

pub(super) struct CamMove {
    from: Vec2,
    from_zoom: f32,
    aim: Aim,
    zoom: f32,
    t: f32,
    seconds: f32,
    easing: Easing,
}

impl Game {
    /// Where the camera looks when nothing moved it: just above the player's feet.
    fn player_focus(&self) -> Vec2 {
        vec2(self.player.pos.x, self.player.pos.y - 8.0)
    }

    fn aim_point(&self, aim: Aim) -> Vec2 {
        match aim {
            Aim::Player => self.player_focus(),
            Aim::Point(p) => p,
            Aim::Entity(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map_or_else(|| self.player_focus(), |e| vec2(e.body.pos.x, e.body.pos.y - 8.0)),
        }
    }

    /// The point the camera looks at and its zoom (1 = usual) this frame.
    pub(super) fn camera_focus(&self) -> (Vec2, f32) {
        match &self.cam {
            None => (self.player_focus(), 1.0),
            Some(c) => {
                let k = if c.seconds <= 0.0 { 1.0 } else { c.easing.apply((c.t / c.seconds).clamp(0.0, 1.0)) };
                (c.from.lerp(self.aim_point(c.aim), k), c.from_zoom + (c.zoom - c.from_zoom) * k)
            }
        }
    }

    /// *Camera*: false when its character isn't on the map (the camera stays).
    pub(super) fn start_camera(&mut self, aim: &CameraAim, owner: Option<u32>, seconds: f32, easing: Easing, zoom: f32) -> bool {
        let aim = match aim {
            CameraAim::Player => Aim::Player,
            CameraAim::Tile { x, y } => {
                let ts = self.ts();
                Aim::Point(vec2((*x as f32 + 0.5) * ts, (*y as f32 + 0.5) * ts))
            }
            CameraAim::Follow { target } if target == "player" => Aim::Player,
            CameraAim::Follow { target } => match self.entity_index(target, owner) {
                Some(i) => Aim::Entity(self.loaded.entities[i].uid),
                None => {
                    eprintln!("[camera] Camera: no character '{target}' here");
                    return false;
                }
            },
        };
        self.move_camera(aim, seconds, easing, zoom);
        true
    }

    fn move_camera(&mut self, aim: Aim, seconds: f32, easing: Easing, zoom: f32) {
        let (from, from_zoom) = self.camera_focus();
        self.cam = Some(CamMove { from, from_zoom, aim, zoom: if zoom > 0.0 { zoom.clamp(0.25, 4.0) } else { 1.0 }, t: 0.0, seconds: seconds.max(0.0), easing });
    }

    /// Advances the move; hands the camera back to the player once the event that moved it is
    /// over (and the move has arrived), and forgets it when it is back.
    pub(super) fn update_camera(&mut self, dt: f32) {
        let Some(c) = &mut self.cam else { return };
        c.t += dt;
        let arrived = c.t >= c.seconds;
        let home = c.aim == Aim::Player && (c.zoom - 1.0).abs() < 0.001;
        if arrived && home {
            self.cam = None;
        } else if arrived && !self.runner.busy() && !self.cutscene_active() {
            self.move_camera(Aim::Player, RETURN_SECONDS, Easing::InOut, 1.0);
        }
    }

    /// Back on the player at once (a new map, loading).
    pub(super) fn reset_camera(&mut self) {
        self.cam = None;
    }

    /// For `status()`: `player`, `tile:X,Y`, `follow:NAME`, plus `@ZOOMx` when zoomed and `>`
    /// while moving.
    /// `view=` for `status`: `iso:ZOOM@x,y` (the isometric view's zoom and the tile in its middle),
    /// then ` walls=DRAWN/CUT` (walls and counters drawn as blocks last frame, cut down in front of
    /// the hero).
    pub(super) fn view_status(&self) -> String {
        let v = self.view();
        let ts = self.ts();
        let (n, c) = self.blocks_drawn.get();
        format!("iso:{:.2}@{},{} walls={n}/{c}", v.scale, (v.target.x / ts).floor(), (v.target.y / ts).floor())
    }

    pub(super) fn camera_status(&self) -> String {
        let Some(c) = &self.cam else { return "player".into() };
        let ts = self.ts();
        let what = match c.aim {
            Aim::Player => "player".to_string(),
            Aim::Point(p) => format!("tile:{},{}", (p.x / ts).floor(), (p.y / ts).floor()),
            Aim::Entity(uid) => format!("follow:{}", self.loaded.entities.iter().find(|e| e.uid == uid).map_or(uid.to_string(), |e| if e.name.is_empty() { uid.to_string() } else { e.name.clone() })),
        };
        let zoom = if (c.zoom - 1.0).abs() > 0.001 { format!("@{}x", c.zoom) } else { String::new() };
        format!("{what}{zoom}{}", if c.t < c.seconds { ">" } else { "" })
    }
}
