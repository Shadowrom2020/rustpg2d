//! Minigames that pause the world: fishing (wait for a bite, strike in time, then reel: hold to
//! pull in, let go before the line snaps). Keyboard, gamepad and autoplay play it with Confirm /
//! Interact and Cancel. (The lockpicking game of 0.27 and older became a DEX check: `locks.rs`.)

use gfx::prelude::*;

use super::skin;

use super::input::{self, Act};
use super::{ui::panel, Game};
use crate::events::runner::ActionHost;
use crate::defs::EquipSlot;

use super::text::{draw_text, measure_text};

/// Seconds the result stays on screen.
const RESULT_TIME: f32 = 0.8;
/// Fishing: the bite window and how long the fish fights.
const BITE_TIME: f32 = 0.6;
const REEL_TIME: f32 = 10.0;

#[derive(Clone, Debug, PartialEq)]
enum FishPhase {
    /// Seconds until the bite.
    Wait(f32),
    /// Seconds left to strike.
    Bite(f32),
    Reel,
}

#[derive(Clone, Debug)]
pub(super) struct Fishing {
    /// The fishing spot zone.
    zone: u32,
    phase: FishPhase,
    /// 0..1: 1 = the line snaps.
    tension: f32,
    /// 0..1: 1 = landed.
    progress: f32,
    time: f32,
    /// Phase of the fish's pull.
    pull: f32,
    /// Where the bobber floats (world pixels): on the spot's water in front of the player.
    bobber: Vec2,
}

#[derive(Clone, Debug)]
pub(super) enum Kind {
    Fishing(Fishing),
}

/// A running minigame and, once it ended, its result (shown briefly).
#[derive(Clone, Debug)]
pub(super) struct Minigame {
    game: Kind,
    /// Ended: (won, seconds shown, message).
    done: Option<(bool, f32, String)>,
    seed: u32,
}

impl Minigame {
    fn rand(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// A seed: fixed for automated play-throughs (their timing must repeat), random otherwise.
fn seed() -> u32 {
    if std::env::var_os("RUSTPG2D_AUTOPLAY").is_some() {
        0x2545_f491
    } else {
        gfx::rand::rand() | 1
    }
}

impl Game {
    pub(super) fn minigame_open(&self) -> bool {
        self.minigame.is_some()
    }

    /// Casts the line at fishing spot `zone`.
    pub(super) fn start_fishing(&mut self, zone: u32, bobber: Vec2) {
        let mut m = Minigame { game: Kind::Fishing(Fishing { zone, phase: FishPhase::Wait(0.0), tension: 0.0, progress: 0.0, time: 0.0, pull: 0.0, bobber }), done: None, seed: seed() };
        let wait = 1.0 + m.rand() * 3.0;
        let pull = m.rand() * std::f32::consts::TAU;
        let Kind::Fishing(f) = &mut m.game;
        f.phase = FishPhase::Wait(wait);
        f.pull = pull;
        self.minigame = Some(m);
        self.player.moving = false;
    }

    /// Runs the open minigame. Returns true while one is open (the world is paused).
    pub(super) fn update_minigame(&mut self, dt: f32) -> bool {
        let Some(mut m) = self.minigame.take() else { return false };
        if let Some((won, t, msg)) = &mut m.done {
            *t += dt;
            if *t >= RESULT_TIME {
                let (won, msg) = (*won, msg.clone());
                self.finish_minigame(m.game, won, msg);
                return true;
            }
            self.minigame = Some(m);
            return true;
        }
        let press = input::any_act_pressed(&[Act::Confirm, Act::Attack]);
        let hold = input::act_down(Act::Confirm) || input::act_down(Act::Attack);
        if input::act_pressed(Act::Cancel) {
            let msg = match m.game {
                Kind::Fishing(_) => "You reel in the line.",
            };
            m.done = Some((false, RESULT_TIME, msg.into()));
            self.minigame = Some(m);
            return true;
        }
        let mut sound: Option<String> = None;
        match &mut m.game {
            Kind::Fishing(f) => {
                f.time += dt;
                match f.phase {
                    FishPhase::Wait(t) => {
                        if press {
                            m.done = Some((false, 0.0, "Too soon - nothing on the hook.".into()));
                        } else if t - dt <= 0.0 {
                            f.phase = FishPhase::Bite(BITE_TIME);
                            sound = Some(self.project.config.sfx.hit.clone());
                        } else {
                            f.phase = FishPhase::Wait(t - dt);
                        }
                    }
                    FishPhase::Bite(t) => {
                        if press {
                            f.phase = FishPhase::Reel;
                            f.time = 0.0;
                        } else if t - dt <= 0.0 {
                            m.done = Some((false, 0.0, "It got away.".into()));
                        } else {
                            f.phase = FishPhase::Bite(t - dt);
                        }
                    }
                    FishPhase::Reel => {
                        // The fish pulls in waves; holding raises the tension (and brings it in
                        // while the line holds), letting go eases it.
                        let pull = 0.5 + 0.5 * (f.time * 2.3 + f.pull).sin();
                        if hold {
                            f.tension += (0.35 + 0.55 * pull) * dt;
                            if f.tension < 0.9 {
                                f.progress += 0.3 * dt;
                            }
                        } else {
                            f.tension -= 0.8 * dt;
                            f.progress -= 0.03 * dt;
                        }
                        f.tension = f.tension.clamp(0.0, 1.2);
                        f.progress = f.progress.clamp(0.0, 1.0);
                        if f.tension >= 1.0 {
                            m.done = Some((false, 0.0, "Snap! The line broke.".into()));
                        } else if f.progress >= 1.0 {
                            m.done = Some((true, 0.0, String::new()));
                        } else if f.time >= REEL_TIME {
                            m.done = Some((false, 0.0, "It got away.".into()));
                        }
                    }
                }
            }
        }
        if let Some(s) = sound {
            self.audio.play(&s);
        }
        self.minigame = Some(m);
        true
    }

    /// Applies the result of a finished minigame.
    fn finish_minigame(&mut self, game: Kind, won: bool, msg: String) {
        match game {
            Kind::Fishing(f) => {
                if !won {
                    if !msg.is_empty() {
                        self.ui.toast(msg);
                    }
                    return;
                }
                let table = self.loaded.map.zones.iter().find(|z| z.uid == f.zone).and_then(|z| z.loot.clone()).unwrap_or_default();
                match pick_catch(&table, gfx::rand::gen_range(0.0, 1.0)) {
                    Some((item, n)) => {
                        let lost = self.world.give_item(&item, n);
                        self.item_given(&item, n - lost, lost);
                    }
                    None => self.ui.toast("Nothing on the hook.".into()),
                }
            }
        }
    }

    /// For `status()`: "fishing:wait|bite|reel(t=,p=)" /
    /// "...:done(won)", "-" without one.
    pub(super) fn minigame_status(&self) -> String {
        let Some(m) = &self.minigame else { return "-".into() };
        let what = match &m.game {
            Kind::Fishing(f) => match f.phase {
                FishPhase::Wait(_) => "fishing:wait".into(),
                FishPhase::Bite(_) => "fishing:bite".into(),
                FishPhase::Reel => format!("fishing:reel(t={:.2},p={:.2})", f.tension, f.progress),
            },
        };
        match &m.done {
            Some((won, _, _)) => format!("{what}:done({won})"),
            None => what,
        }
    }

    /// For automated play-throughs: is the minigame in state `what` right now? "bite" (fishing),
    /// "reel_low" (tension below 0.6).
    pub fn minigame_probe(&self, what: &str) -> bool {
        let Some(m) = self.minigame.as_ref().filter(|m| m.done.is_none()) else { return false };
        match (&m.game, what) {
            (Kind::Fishing(f), "bite") => matches!(f.phase, FishPhase::Bite(_)),
            (Kind::Fishing(f), "reel_low") => f.phase == FishPhase::Reel && f.tension < 0.6,
            _ => false,
        }
    }

    pub(super) fn draw_minigame(&self) {
        let Some(m) = &self.minigame else { return };
        let (sw, sh) = (screen_width(), screen_height());
        match &m.game {
            Kind::Fishing(f) => {
                // The line and the bobber in front of the player.
                let cam = self.camera();
                let ts = self.ts();
                let dir = Vec2::from(self.player.facing.dir());
                // From the rod's tip (or the hand when no rod is held).
                // (The rod's tip and the hand stand with the player: their offsets as the upright
                // drawing shows them.)
                let v = self.view();
                let (feet, u) = (self.player.pos, v.upright_scale());
                let hand = if self.world.equipment.tool.is_some() {
                    v.to_screen(feet) + (self.rod_tip() - feet) * u
                } else {
                    v.above(feet, ts * 0.8) + crate::render::iso::lay(dir * ts * 0.3) * u
                };
                let bob_world = f.bobber;
                let bite = matches!(f.phase, FishPhase::Bite(_) | FishPhase::Reel);
                let dip = if bite { (get_time() as f32 * 18.0).sin() * 3.0 + 3.0 } else { (get_time() as f32 * 3.0).sin() * 1.5 };
                let bob = cam.world_to_screen(bob_world) + vec2(0.0, dip);
                draw_line(hand.x, hand.y, bob.x, bob.y, 1.5, Color::new(0.9, 0.9, 0.9, 0.8));
                draw_circle(bob.x, bob.y, 6.0, Color::new(0.9, 0.15, 0.1, 1.0));
                draw_circle(bob.x, bob.y - 3.0, 3.5, WHITE);
                if bite {
                    draw_circle_lines(bob.x, bob.y + 4.0, 12.0 + dip, 1.5, Color::new(0.8, 0.9, 1.0, 0.7));
                }
                // The panel at the bottom.
                let (w, h) = (500.0, 120.0);
                let (x, y) = (((sw - w) / 2.0).round(), (sh - h - 110.0).round());
                panel(x, y, w, h);
                let (title, hint) = match f.phase {
                    FishPhase::Wait(_) => ("Waiting for a bite...", "Wait for the bobber to dip   {key:cancel}: reel in"),
                    FishPhase::Bite(_) => ("A bite! Strike now!", "{key:confirm}: strike"),
                    FishPhase::Reel => ("Reel it in!", "Hold {key:confirm} to reel, let go before the line turns red"),
                };
                draw_text(title, x + 18.0, y + 32.0, 24.0, if bite { skin::highlight() } else { skin::normal() });
                if f.phase == FishPhase::Reel {
                    let bar = |by: f32, v: f32, c: Color, label: &str| {
                        draw_text(label, x + 18.0, by + 14.0, 16.0, skin::dim());
                        let r = Rect::new(x + 90.0, by, w - 110.0, 16.0);
                        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.1, 0.12, 1.0));
                        draw_rectangle(r.x, r.y, r.w * v.clamp(0.0, 1.0), r.h, c);
                        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, skin::line(1.0));
                    };
                    let tc = if f.tension > 0.85 { Color::new(0.9, 0.2, 0.15, 1.0) } else if f.tension > 0.6 { Color::new(0.95, 0.7, 0.2, 1.0) } else { Color::new(0.4, 0.8, 0.4, 1.0) };
                    bar(y + 46.0, f.tension, tc, "Tension");
                    bar(y + 70.0, f.progress, Color::new(0.35, 0.6, 0.95, 1.0), "Reeled");
                }
                super::glyphs::draw_hint_in(hint, x + 18.0, skin::hint_baseline(y + h), 16.0, w - 36.0, skin::disabled());
            }
        }
        if let Some((won, _, msg)) = &m.done {
            if !msg.is_empty() {
                let tw = measure_text(msg, None, 26, 1.0).width;
                let c = if *won { Color::new(0.6, 0.95, 0.6, 1.0) } else { Color::new(1.0, 0.6, 0.5, 1.0) };
                draw_rectangle((sw - tw) / 2.0 - 12.0, sh * 0.3 - 30.0, tw + 24.0, 42.0, Color::new(0.0, 0.0, 0.0, 0.7));
                draw_text(msg, (sw - tw) / 2.0, sh * 0.3, 26.0, c);
            }
        }
    }
}

impl Game {
    /// Interact inside a fishing spot (nothing else in front): casts the line, if the player
    /// has the rod it needs.
    pub(super) fn try_fish(&mut self) {
        let Some((zone, rod, bobber)) = self.fishing_target() else { return };
        if self.ui.dialog_open() || self.runner.busy() {
            return; // a zone trigger had something to say first
        }
        if !rod.is_empty() && self.world.equipment.item(EquipSlot::Tool) != Some(rod.as_str()) {
            let name = self.item_name(&rod);
            // In the bag: it goes into the Tool slot (what was there goes into the bag).
            if self.world.inventory.count(&rod) == 0 {
                self.ui.toast(format!("You need a {name} to fish here."));
                return;
            }
            if self.project.db.item(&rod).and_then(|d| d.equip_slot()) != Some(EquipSlot::Tool) || !self.equip_item(&rod) {
                self.ui.toast(format!("Put the {name} into the Tool slot to fish here."));
                return;
            }
        }
        self.start_fishing(zone, bobber);
    }

    /// The fishing spot the player faces: a `FishingSpot` zone (the water) within three tiles
    /// straight ahead. Returns the zone, the item it needs and where the bobber lands: two
    /// tiles out, or the farthest of the spot's tiles that is closer.
    pub(super) fn fishing_target(&self) -> Option<(u32, String, Vec2)> {
        let ts = self.ts();
        let here = self.tile_of(self.player.pos);
        let (dx, dy) = self.player.facing.dir();
        let ahead = |k: i32| (here.0 + dx as i32 * k, here.1 + dy as i32 * k);
        let spot_at = |t: (i32, i32)| {
            self.loaded.map.zones.iter().find(|z| {
                matches!(z.kind, crate::map::ZoneKind::FishingSpot { .. })
                    && t.0 >= z.x as i32
                    && t.1 >= z.y as i32
                    && t.0 < (z.x + z.w) as i32
                    && t.1 < (z.y + z.h) as i32
            })
        };
        // Standing in the water (swimming, later) doesn't count: the spot starts ahead.
        let first = (1..=3).find(|&k| spot_at(ahead(k)).is_some())?;
        let z = spot_at(ahead(first))?;
        let last = (first..=2.max(first)).take_while(|&k| spot_at(ahead(k)).is_some_and(|o| o.uid == z.uid)).last().unwrap_or(first);
        let t = ahead(last);
        let rod = match &z.kind {
            crate::map::ZoneKind::FishingSpot { rod } => rod.clone(),
            _ => String::new(),
        };
        Some((z.uid, rod, vec2((t.0 as f32 + 0.5) * ts, (t.1 as f32 + 0.55) * ts)))
    }
}

impl Game {
    /// The tool the player holds instead of the weapon: the rod while fishing.
    pub(super) fn player_tool_in_use(&self) -> Option<String> {
        let fishing = matches!(self.minigame.as_ref().map(|m| &m.game), Some(Kind::Fishing(_)));
        fishing.then(|| self.world.equipment.tool.as_ref().map(|t| t.item.clone())).flatten()
    }

    /// Where the fishing line leaves the rod (world pixels): about the tip of a rod held in the
    /// right hand of the idle frame (32 x 48 character layout, scaled to the tile size).
    fn rod_tip(&self) -> Vec2 {
        // The tip of the rod in the hand, else about where a held rod's tip is.
        let tool = self.player_tool_in_use();
        let pose = (self.player.facing, self.player_anim());
        let gear = self.player_gear();
        let tip = self.player_appearance().zip(tool.as_deref()).and_then(|(a, t)| self.held_tip(&a, self.player.pos, &gear, pose, t));
        tip.unwrap_or_else(|| super::activities::rod_tip(self.player.pos, self.player.facing, self.ts() / 32.0))
    }
}

/// One catch from a fishing spot's table: entries are weighted by their chance (always one
/// catch when the table has any), the count between min and max. `r` = random 0..1.
fn pick_catch(table: &[crate::defs::LootEntry], r: f32) -> Option<(String, u32)> {
    let entries: Vec<&crate::defs::LootEntry> = table.iter().filter(|e| !e.item.is_empty() && e.chance > 0.0).collect();
    let total: f32 = entries.iter().map(|e| e.chance).sum();
    let mut at = r.clamp(0.0, 0.9999) * total;
    for e in entries {
        if at < e.chance {
            let n = e.min.max(1) + ((r * 7919.0).fract() * (e.max.max(e.min.max(1)) - e.min.max(1) + 1) as f32) as u32;
            return Some((e.item.clone(), n.min(e.max.max(e.min.max(1)))));
        }
        at -= e.chance;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::LootEntry;

    #[test]
    fn catches_are_weighted() {
        let table = vec![LootEntry::new("trout", 75.0, 1, 1), LootEntry::new("boot", 25.0, 1, 1)];
        assert_eq!(pick_catch(&table, 0.1).unwrap().0, "trout");
        assert_eq!(pick_catch(&table, 0.74).unwrap().0, "trout");
        assert_eq!(pick_catch(&table, 0.8).unwrap().0, "boot");
        assert_eq!(pick_catch(&table, 1.0).unwrap().0, "boot");
        assert!(pick_catch(&[], 0.5).is_none());
        let many = vec![LootEntry::new("fish", 10.0, 2, 4)];
        for k in 0..20 {
            let (_, n) = pick_catch(&many, k as f32 / 20.0).unwrap();
            assert!((2..=4).contains(&n));
        }
    }
}
