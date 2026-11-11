//! Visible activities: characters seen fishing, hoeing, hammering, chopping, mining, sweeping
//! or reading at their spot (`ScheduleStop::activity` at a schedule stop, `EntityInstance::
//! activity` all day). They stay put facing their way with the tool in hand and work in place;
//! small effects (the fishing line and bobber, soil specks, sparks, chips, dust, a book) are
//! pure functions of the time, so nothing is saved. Talking to one pauses it for a moment (it
//! turns to the player), then it goes back to work.

use macroquad::prelude::*;

use super::Game;
use crate::entity::{Activity, ActivityKind, Facing};

/// Seconds a talk interrupts the work.
pub(super) const TALK_PAUSE: f32 = 5.0;
/// Seconds per work stroke (hoe, hammer, axe, pickaxe, broom).
fn stroke(kind: ActivityKind) -> f32 {
    match kind {
        ActivityKind::Craft => 1.1,
        ActivityKind::Farm | ActivityKind::Chop | ActivityKind::Mine => 1.5,
        ActivityKind::Sweep => 0.9,
        ActivityKind::Fish | ActivityKind::Read => 0.0,
    }
}

impl Game {
    /// What character `i` is doing now (and the way it faces), if anything: at a schedule stop
    /// it has reached, or all day without a schedule; not while walking, on furniture or just
    /// talked to.
    pub(super) fn current_activity(&self, i: usize) -> Option<(Activity, Facing)> {
        if self.loaded.entities[i].act_pause > 0.0 {
            return None;
        }
        self.activity_here(i)
    }

    /// The activity character `i` has at its spot now, also while a talk pauses it.
    fn activity_here(&self, i: usize) -> Option<(Activity, Facing)> {
        let e = &self.loaded.entities[i];
        if !e.active || e.away || e.npc_seat.is_some() || e.companion.is_some() {
            return None;
        }
        match e.schedule.as_ref().filter(|s| s.on) {
            Some(s) => {
                let stop = s.current.and_then(|k| s.stops.get(k)).filter(|_| s.arrived)?;
                Some((stop.activity.clone()?, stop.facing.unwrap_or(e.act_facing)))
            }
            None => e.activity.clone().map(|a| (a, e.act_facing)),
        }
    }

    /// Keeps character `i` at work: still, facing its way; the stroke sounds near the player.
    /// Returns true if it is busy with an activity (nothing else moves it).
    pub(super) fn activity_step(&mut self, i: usize, dt: f32) -> bool {
        {
            let e = &mut self.loaded.entities[i];
            e.act_pause = (e.act_pause - dt).max(0.0);
        }
        // Paused by a talk: it waits where it is (facing the player).
        if self.loaded.entities[i].act_pause > 0.0 {
            return self.activity_here(i).is_some();
        }
        let Some((a, facing)) = self.current_activity(i) else { return false };
        let ts = self.ts();
        let e = &mut self.loaded.entities[i];
        e.body.facing = facing;
        e.body.moving = false;
        let period = stroke(a.kind);
        if period > 0.0 {
            let cycle = ((get_time() as f32 + (e.uid % 1024) as f32 * 0.37) / period) as u32;
            let near = e.body.pos.distance(self.player.pos) < ts * 4.0;
            if cycle != e.act_cycle {
                e.act_cycle = cycle;
                if near && matches!(a.kind, ActivityKind::Craft | ActivityKind::Chop | ActivityKind::Mine) {
                    self.audio.play(&self.project.config.sfx.hit);
                }
            }
        }
        true
    }

    /// The walk column character `i` is drawn with while working (strokes swing the arms).
    pub(super) fn activity_frame(&self, i: usize) -> Option<u32> {
        let (a, _) = self.current_activity(i)?;
        let period = stroke(a.kind);
        if period <= 0.0 {
            return Some(0);
        }
        let e = &self.loaded.entities[i];
        let u = ((get_time() as f32 + (e.uid % 1024) as f32 * 0.37) / period).fract();
        Some(if u < 0.25 { 1 } else if u < 0.5 { 2 } else { 0 })
    }

    /// The effects of the characters at work (world space, after the entities).
    pub(super) fn draw_activities(&self) {
        let (ts, t) = (self.ts(), get_time() as f32);
        let k = ts / 32.0;
        for i in 0..self.loaded.entities.len() {
            let Some((a, facing)) = self.current_activity(i) else { continue };
            let e = &self.loaded.entities[i];
            let dir = Vec2::from(facing.dir());
            let seed = (e.uid % 1024) as f32 * 0.37;
            // Where the strokes land: in front, or (facing up, the body in the way) beyond the head.
            let front = if facing == Facing::Up && a.kind == ActivityKind::Craft { e.body.pos - vec2(0.0, 50.0 * k) } else { e.body.pos + dir * ts * 0.75 - vec2(0.0, 4.0 * k) };
            match a.kind {
                ActivityKind::Fish => {
                    let tip = rod_tip(e.body.pos, facing, k);
                    let bob = e.body.pos + dir * ts * 1.7 + vec2(0.0, (t * 2.5 + seed).sin() * 1.5 * k);
                    draw_line(tip.x, tip.y, bob.x, bob.y, 1.0 * k, Color::new(0.9, 0.9, 0.9, 0.8));
                    draw_circle(bob.x, bob.y, 3.0 * k, Color::new(0.9, 0.15, 0.1, 1.0));
                    draw_circle(bob.x, bob.y - 1.5 * k, 1.8 * k, WHITE);
                    // Now and then a ring on the water.
                    let u = ((t + seed) / 6.0).fract();
                    if u < 0.3 {
                        draw_circle_lines(bob.x, bob.y + 2.0 * k, (4.0 + u * 30.0) * k, 1.0 * k, Color::new(0.85, 0.95, 1.0, 0.7 * (1.0 - u / 0.3)));
                    }
                }
                ActivityKind::Read => {
                    // An open book held in front.
                    let c = e.body.pos - vec2(0.0, 23.0 * k) + dir * 4.0 * k;
                    if facing != Facing::Up {
                        draw_rectangle(c.x - 8.0 * k, c.y - 5.0 * k, 16.0 * k, 9.0 * k, Color::new(0.08, 0.06, 0.1, 1.0));
                        draw_rectangle(c.x - 7.5 * k, c.y - 4.5 * k, 15.0 * k, 8.0 * k, Color::new(0.5, 0.18, 0.12, 1.0));
                        draw_rectangle(c.x - 6.5 * k, c.y - 4.0 * k, 6.0 * k, 6.5 * k, Color::new(0.97, 0.95, 0.88, 1.0));
                        draw_rectangle(c.x + 0.5 * k, c.y - 4.0 * k, 6.0 * k, 6.5 * k, Color::new(0.93, 0.9, 0.82, 1.0));
                        // Lines of text, and a page turning now and then.
                        for l in 0..3 {
                            let y = c.y - 2.5 * k + l as f32 * 1.8 * k;
                            draw_line(c.x - 5.5 * k, y, c.x - 1.5 * k, y, 0.6 * k, Color::new(0.4, 0.4, 0.45, 0.8));
                            draw_line(c.x + 1.5 * k, y, c.x + 5.5 * k, y, 0.6 * k, Color::new(0.4, 0.4, 0.45, 0.8));
                        }
                        let u = ((t + seed) / 7.0).fract();
                        if u < 0.08 {
                            let w = 6.0 * k * (1.0 - u / 0.04).abs();
                            draw_rectangle(c.x, c.y - 4.5 * k, w, 7.0 * k, Color::new(1.0, 0.98, 0.92, 1.0));
                        }
                    }
                }
                kind => {
                    // Particles thrown up at each stroke's impact in front.
                    let period = stroke(kind).max(0.1);
                    let u = ((t + seed) / period).fract();
                    if u > 0.45 {
                        continue;
                    }
                    let (col, n, rise) = match kind {
                        ActivityKind::Farm => (Color::new(0.62, 0.45, 0.28, 1.0), 5, 18.0),
                        ActivityKind::Craft => (Color::new(1.0, 0.8, 0.25, 1.0), 6, 22.0),
                        ActivityKind::Chop => (Color::new(0.65, 0.45, 0.22, 1.0), 4, 16.0),
                        ActivityKind::Mine => (Color::new(0.6, 0.6, 0.62, 1.0), 4, 16.0),
                        _ => (Color::new(0.75, 0.72, 0.65, 0.7), 3, 6.0),
                    };
                    let fade = 1.0 - u / 0.45;
                    for p in 0..n {
                        let ang = -std::f32::consts::FRAC_PI_2 + (p as f32 - n as f32 / 2.0) * 0.45;
                        let d = u * 40.0 * k;
                        let pos = front + Vec2::from_angle(ang) * d + vec2(0.0, u * u * rise * 6.0 * k - rise * u * k);
                        if kind == ActivityKind::Sweep {
                            draw_circle(pos.x, pos.y, (2.0 + u * 6.0) * k, Color::new(col.r, col.g, col.b, col.a * fade));
                        } else {
                            draw_rectangle(pos.x, pos.y, 2.2 * k, 2.2 * k, Color::new(col.r, col.g, col.b, fade));
                        }
                    }
                }
            }
        }
    }

    /// `status()`: characters at work now (`name:kind@x,y`).
    pub(super) fn activities_status(&self) -> Vec<String> {
        (0..self.loaded.entities.len())
            .filter_map(|i| {
                let (a, _) = self.current_activity(i)?;
                let e = &self.loaded.entities[i];
                let t = self.tile_of(e.body.pos);
                Some(format!("{}:{}@{},{}", if e.name.is_empty() { &e.def } else { &e.name }, a.kind.id(), t.0, t.1))
            })
            .collect()
    }
}

/// Where a fishing line leaves a rod held by a character standing at `feet` facing `facing`
/// (32 x 48 character layout; `k` = tile size / 32).
pub(super) fn rod_tip(feet: Vec2, facing: Facing, k: f32) -> Vec2 {
    let off = match facing {
        Facing::Down => vec2(-13.0, 0.0),
        Facing::Up => vec2(14.0, -42.0),
        Facing::Right => vec2(13.0, -36.0),
        Facing::Left => vec2(-13.0, -36.0),
    };
    feet + off * k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{EntityInstance, EntityKind, ScheduleStop};

    #[test]
    fn activities_in_ron() {
        // Stops and placed characters from before activities have none.
        let s: ScheduleStop = ron::from_str("(hour: 7.0, x: 1, y: 2)").unwrap();
        assert!(s.activity.is_none());
        let mut e = EntityInstance::new(3, EntityKind::Character, "villager", 1.0, 2.0);
        e.activity = Some(Activity { kind: ActivityKind::Fish, tool: "fishing_rod".into() });
        let back: EntityInstance = ron::from_str(&ron::to_string(&e).unwrap()).unwrap();
        assert_eq!(back.activity, e.activity);
        assert_eq!(ActivityKind::ALL.iter().map(|k| k.id()).collect::<Vec<_>>(), ["fish", "farm", "craft", "chop", "mine", "sweep", "read"]);
        // The rod's tip is above the feet and to the side it faces.
        let tip = rod_tip(vec2(100.0, 100.0), Facing::Right, 1.0);
        assert!(tip.x > 100.0 && tip.y < 100.0);
    }
}
