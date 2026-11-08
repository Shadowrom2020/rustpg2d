//! Minigames that pause the world: lockpicking (three pins: press when the sweeping marker is
//! in the sweet spot; three misses and the lock holds) and fishing (wait for a bite, strike
//! in time, then reel: hold to pull in, let go before the line snaps). Keyboard, gamepad and
//! autoplay play them with Confirm / Interact and Cancel.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::{ui::panel, Game, Source};
use crate::events::runner::ActionHost;
use crate::defs::EquipSlot;

use crate::events::GameEvent;

/// Misses allowed before a lock holds.
const MAX_MISSES: u32 = 3;
const PINS: usize = 3;
/// Seconds the result stays on screen.
const RESULT_TIME: f32 = 0.8;
/// Fishing: the bite window and how long the fish fights.
const BITE_TIME: f32 = 0.6;
const REEL_TIME: f32 = 10.0;

/// What a picked lock opens.
#[derive(Clone, Debug)]
pub(super) enum PickTarget {
    /// A locked prop (entity uid).
    Entity(u32),
    /// The *Lockpick* action / `lockpick(difficulty, var)`: 1 or 0 goes into the variable.
    Var(String),
}

#[derive(Clone, Debug)]
pub(super) struct Lockpick {
    target: PickTarget,
    pin: usize,
    /// Marker position 0..1 and direction.
    pos: f32,
    dir: f32,
    /// Bar widths per second.
    speed: f32,
    /// Sweet spot centre per pin and its half width.
    spots: [f32; PINS],
    half: f32,
    misses: u32,
    /// Red flash after a miss / green after a pin.
    flash: (f32, bool),
}

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
    Lockpick(Lockpick),
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
        macroquad::rand::rand() | 1
    }
}

impl Game {
    pub(super) fn minigame_open(&self) -> bool {
        self.minigame.is_some()
    }

    /// Starts the lockpicking game. `level_gap` = the lock's difficulty minus the player's
    /// lockpicking level (-1 = easy … 3 = very hard).
    pub(super) fn start_lockpick(&mut self, target: PickTarget, level_gap: i32) {
        let g = level_gap.clamp(-1, 3) as f32;
        let mut m = Minigame {
            game: Kind::Lockpick(Lockpick { target, pin: 0, pos: 0.0, dir: 1.0, speed: 0.9 + 0.35 * g, spots: [0.5; PINS], half: (0.14 - 0.03 * g).max(0.05), misses: 0, flash: (0.0, true) }),
            done: None,
            seed: seed(),
        };
        let spots: Vec<f32> = (0..PINS).map(|_| 0.2 + m.rand() * 0.6).collect();
        if let Kind::Lockpick(l) = &mut m.game {
            l.spots.copy_from_slice(&spots);
        }
        self.minigame = Some(m);
        self.audio.play(&self.project.config.sfx.open_container);
    }

    /// Casts the line at fishing spot `zone`.
    pub(super) fn start_fishing(&mut self, zone: u32, bobber: Vec2) {
        let mut m = Minigame { game: Kind::Fishing(Fishing { zone, phase: FishPhase::Wait(0.0), tension: 0.0, progress: 0.0, time: 0.0, pull: 0.0, bobber }), done: None, seed: seed() };
        let wait = 1.0 + m.rand() * 3.0;
        let pull = m.rand() * std::f32::consts::TAU;
        if let Kind::Fishing(f) = &mut m.game {
            f.phase = FishPhase::Wait(wait);
            f.pull = pull;
        }
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
                Kind::Lockpick(_) => "You leave the lock alone.",
                Kind::Fishing(_) => "You reel in the line.",
            };
            m.done = Some((false, RESULT_TIME, msg.into()));
            self.minigame = Some(m);
            return true;
        }
        let mut sound: Option<String> = None;
        match &mut m.game {
            Kind::Lockpick(l) => {
                l.flash.0 = (l.flash.0 - dt).max(0.0);
                l.pos += l.dir * l.speed * dt;
                if l.pos >= 1.0 {
                    l.pos = 2.0 - l.pos;
                    l.dir = -1.0;
                } else if l.pos <= 0.0 {
                    l.pos = -l.pos;
                    l.dir = 1.0;
                }
                if press {
                    if (l.pos - l.spots[l.pin]).abs() <= l.half {
                        l.pin += 1;
                        l.flash = (0.25, true);
                        sound = Some(self.project.config.sfx.pickup.clone());
                        if l.pin >= PINS {
                            m.done = Some((true, 0.0, "The lock clicks open.".into()));
                            sound = Some(self.project.config.sfx.unlock.clone());
                        }
                    } else {
                        l.misses += 1;
                        l.flash = (0.3, false);
                        sound = Some(self.project.config.sfx.hit.clone());
                        if l.misses >= MAX_MISSES {
                            m.done = Some((false, 0.0, "The pick slips - the lock holds.".into()));
                        }
                    }
                }
            }
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
            Kind::Lockpick(l) => match l.target {
                PickTarget::Entity(uid) => {
                    if !msg.is_empty() {
                        self.ui.toast(msg);
                    }
                    let Some(i) = self.loaded.entities.iter().position(|e| e.uid == uid) else { return };
                    if won {
                        if let Some(lock) = self.active_lock(i) {
                            self.apply_unlock(i, &lock.on_unlock);
                            self.fire(GameEvent::Unlock, Source::Entity(uid));
                            if self.loaded.entities[i].active {
                                self.fire(GameEvent::Interact, Source::Entity(uid));
                                self.try_open_container(i);
                            }
                        }
                    }
                }
                PickTarget::Var(var) => {
                    if !msg.is_empty() {
                        self.ui.toast(msg);
                    }
                    if !var.is_empty() {
                        self.world.set_var(&var, i64::from(won));
                    }
                }
            },
            Kind::Fishing(f) => {
                if !won {
                    if !msg.is_empty() {
                        self.ui.toast(msg);
                    }
                    return;
                }
                let table = self.loaded.map.zones.iter().find(|z| z.uid == f.zone).and_then(|z| z.loot.clone()).unwrap_or_default();
                match pick_catch(&table, macroquad::rand::gen_range(0.0, 1.0)) {
                    Some((item, n)) => {
                        let lost = self.world.give_item(&item, n);
                        self.item_given(&item, n - lost, lost);
                    }
                    None => self.ui.toast("Nothing on the hook.".into()),
                }
            }
        }
    }

    /// For `status()`: "lockpick:pin=N/3,miss=M,pos=P,spot=S" / "fishing:wait|bite|reel(t=,p=)" /
    /// "...:done(won)", "-" without one.
    pub(super) fn minigame_status(&self) -> String {
        let Some(m) = &self.minigame else { return "-".into() };
        let what = match &m.game {
            Kind::Lockpick(l) => format!("lockpick:pin={}/{PINS},miss={},pos={:.2},spot={:.2}", l.pin, l.misses, l.pos, l.spots.get(l.pin).copied().unwrap_or(0.0)),
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

    /// For automated play-throughs: is the minigame in state `what` right now? "in_spot" /
    /// "out_spot" (the lockpick marker), "bite" (fishing), "reel_low" (tension below 0.6).
    pub fn minigame_probe(&self, what: &str) -> bool {
        let Some(m) = self.minigame.as_ref().filter(|m| m.done.is_none()) else { return false };
        match (&m.game, what) {
            (Kind::Lockpick(l), "in_spot") => (l.pos - l.spots[l.pin.min(PINS - 1)]).abs() <= l.half * 0.6,
            (Kind::Lockpick(l), "out_spot") => (l.pos - l.spots[l.pin.min(PINS - 1)]).abs() > l.half * 2.0,
            (Kind::Fishing(f), "bite") => matches!(f.phase, FishPhase::Bite(_)),
            (Kind::Fishing(f), "reel_low") => f.phase == FishPhase::Reel && f.tension < 0.6,
            _ => false,
        }
    }

    pub(super) fn draw_minigame(&self) {
        let Some(m) = &self.minigame else { return };
        let (sw, sh) = (screen_width(), screen_height());
        match &m.game {
            Kind::Lockpick(l) => {
                let (w, h) = (500.0, 200.0);
                let (x, y) = (((sw - w) / 2.0).round(), ((sh - h) / 2.0).round());
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.35));
                panel(x, y, w, h);
                draw_text("Picking the lock", x + 18.0, y + 34.0, 26.0, GOLD);
                // Pins: set ones gold, the current one outlined.
                for k in 0..PINS {
                    let px = x + w - 30.0 - (PINS - k) as f32 * 30.0;
                    let set = k < l.pin;
                    draw_rectangle(px, y + 16.0, 18.0, 22.0, if set { GOLD } else { Color::new(0.2, 0.2, 0.25, 1.0) });
                    if k == l.pin {
                        draw_rectangle_lines(px - 2.0, y + 14.0, 22.0, 26.0, 2.0, WHITE);
                    }
                }
                // The bar with the sweet spot and the marker.
                let bar = Rect::new(x + 24.0, y + 80.0, w - 48.0, 30.0);
                let flash = if l.flash.0 > 0.0 { if l.flash.1 { Color::new(0.3, 0.6, 0.3, 1.0) } else { Color::new(0.6, 0.2, 0.2, 1.0) } } else { Color::new(0.1, 0.1, 0.12, 1.0) };
                draw_rectangle(bar.x, bar.y, bar.w, bar.h, flash);
                if l.pin < PINS {
                    let c = l.spots[l.pin];
                    draw_rectangle(bar.x + (c - l.half) * bar.w, bar.y, 2.0 * l.half * bar.w, bar.h, Color::new(0.35, 0.8, 0.35, 0.85));
                }
                draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 2.0, Color::new(0.85, 0.8, 0.6, 1.0));
                let mx = bar.x + l.pos * bar.w;
                draw_rectangle(mx - 2.0, bar.y - 8.0, 4.0, bar.h + 16.0, WHITE);
                // Misses left.
                let left = MAX_MISSES.saturating_sub(l.misses);
                draw_text(format!("Picks left: {left}"), x + 24.0, y + 140.0, 20.0, if left <= 1 { Color::new(1.0, 0.55, 0.5, 1.0) } else { LIGHTGRAY });
                draw_text("E / Space: set the pin while the marker is in the green   Esc: give up", x + 24.0, y + h - 18.0, 16.0, GRAY);
            }
            Kind::Fishing(f) => {
                // The line and the bobber in front of the player.
                let cam = self.camera();
                let ts = self.ts();
                let dir = Vec2::from(self.player.facing.dir());
                // From the rod's tip (or the hand when no rod is held).
                let hand = if self.world.equipment.tool.is_some() { cam.world_to_screen(self.rod_tip()) } else { cam.world_to_screen(self.player.pos - vec2(0.0, ts * 0.8) + dir * ts * 0.3) };
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
                    FishPhase::Wait(_) => ("Waiting for a bite...", "Wait for the bobber to dip   Esc: reel in"),
                    FishPhase::Bite(_) => ("A bite! Strike now!", "E / Space: strike"),
                    FishPhase::Reel => ("Reel it in!", "Hold E / Space to reel, let go before the line turns red"),
                };
                draw_text(title, x + 18.0, y + 32.0, 24.0, if bite { GOLD } else { WHITE });
                if f.phase == FishPhase::Reel {
                    let bar = |by: f32, v: f32, c: Color, label: &str| {
                        draw_text(label, x + 18.0, by + 14.0, 16.0, LIGHTGRAY);
                        let r = Rect::new(x + 90.0, by, w - 110.0, 16.0);
                        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.1, 0.12, 1.0));
                        draw_rectangle(r.x, r.y, r.w * v.clamp(0.0, 1.0), r.h, c);
                        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color::new(0.85, 0.8, 0.6, 1.0));
                    };
                    let tc = if f.tension > 0.85 { Color::new(0.9, 0.2, 0.15, 1.0) } else if f.tension > 0.6 { Color::new(0.95, 0.7, 0.2, 1.0) } else { Color::new(0.4, 0.8, 0.4, 1.0) };
                    bar(y + 46.0, f.tension, tc, "Tension");
                    bar(y + 70.0, f.progress, Color::new(0.35, 0.6, 0.95, 1.0), "Reeled");
                }
                draw_text(hint, x + 18.0, y + h - 12.0, 16.0, GRAY);
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
    /// What the player is drawn with: while fishing, the tool (the rod) is in the right hand
    /// instead of the weapon.
    pub(super) fn player_equipment_drawn(&self) -> std::borrow::Cow<'_, crate::state::Equipment> {
        let fishing = matches!(self.minigame.as_ref().map(|m| &m.game), Some(Kind::Fishing(_)));
        match (&self.world.equipment.tool, fishing) {
            (Some(tool), true) => {
                let mut eq = self.world.equipment.clone();
                eq.weapon = Some(tool.clone());
                std::borrow::Cow::Owned(eq)
            }
            _ => std::borrow::Cow::Borrowed(&self.world.equipment),
        }
    }

    /// Where the fishing line leaves the rod (world pixels): about the tip of a rod held in the
    /// right hand of the idle frame (32 x 48 character layout, scaled to the tile size).
    fn rod_tip(&self) -> Vec2 {
        super::activities::rod_tip(self.player.pos, self.player.facing, self.ts() / 32.0)
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
