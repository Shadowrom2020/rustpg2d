//! The SRD's traps in play (`srd/traps.rs`): trap zones (`ZoneKind::Trap`) and trapped props
//! (`PropDef::trap`). The hero notices one within 2 tiles by passive Perception or Investigation
//! (once a second) or by a Search; E on a found one disarms it (DEX with Thieves' Tools; failing
//! by 5 or more springs it); walking into an armed zone or using an armed prop springs it: an
//! attack roll or a saving throw, damage, a condition, a fall. Its state is saved
//! (`WorldState::traps`); *Trap sprung* / *Trap disarmed* triggers follow.

use gfx::prelude::*;

use super::{Game, Source};
use crate::entity::EntityKind;
use crate::events::GameEvent;
use crate::map::ZoneKind;
use crate::srd::traps::{TrapSpec, TrapState};
use crate::srd::Mode;

/// Tiles within which the hero notices a trap.
const NOTICE_TILES: i32 = 2;
/// Game hours before another try at disarming one that didn't give.
const DISARM_RETRY_HOURS: f64 = 10.0 / 60.0;

/// A trap on the map: where, its spec, its tiles (x, y, w, h).
type TrapHere = (TrapAt, TrapSpec, (i32, i32, i32, i32));

/// Where a trap is: a zone or a prop, by uid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrapAt {
    Zone(u32),
    Prop(u32),
}

impl Game {
    fn trap_key(&self, uid: u32) -> String {
        format!("{}/{uid}", self.loaded.map.name)
    }

    fn trap_state(&self, uid: u32) -> TrapState {
        self.world.traps.get(&self.trap_key(uid)).cloned().unwrap_or_default()
    }

    fn set_trap_state(&mut self, uid: u32, st: TrapState) {
        let key = self.trap_key(uid);
        self.world.traps.insert(key, st);
    }

    /// The traps on this map: where, the spec, its tiles (a zone's rectangle, a prop's tile).
    fn traps_here(&self) -> Vec<TrapHere> {
        let mut out = Vec::new();
        for z in &self.loaded.map.zones {
            if let ZoneKind::Trap { trap } = &z.kind {
                out.push((TrapAt::Zone(z.uid), trap.clone(), (z.x as i32, z.y as i32, z.w as i32, z.h as i32)));
            }
        }
        for e in self.loaded.entities.iter().filter(|e| e.active && e.kind == EntityKind::Prop) {
            if let Some(t) = self.project.db.prop(&e.def).and_then(|d| d.trap.clone()) {
                let p = self.tile_of(e.body.pos);
                out.push((TrapAt::Prop(e.uid), t, (p.0, p.1, 1, 1)));
            }
        }
        out
    }

    fn uid_of(at: TrapAt) -> u32 {
        match at {
            TrapAt::Zone(u) | TrapAt::Prop(u) => u,
        }
    }

    /// Tiles between the hero and a rectangle (0 = inside).
    fn tiles_to(&self, r: (i32, i32, i32, i32)) -> i32 {
        let (px, py) = self.player_tile();
        let dx = if px < r.0 { r.0 - px } else if px >= r.0 + r.2 { px - (r.0 + r.2 - 1) } else { 0 };
        let dy = if py < r.1 { r.1 - py } else if py >= r.1 + r.3 { py - (r.1 + r.3 - 1) } else { 0 };
        dx.max(dy)
    }

    /// Once a second: armed traps within 2 tiles are noticed by the hero's passive Perception or
    /// Investigation (dim light counts).
    pub(super) fn notice_traps(&mut self) {
        let now = self.world.total_hours();
        for (at, spec, r) in self.traps_here() {
            let uid = Self::uid_of(at);
            let st = self.trap_state(uid);
            if st.found || !st.armed(spec.reset, now) || spec.detect_dc <= 0 || self.tiles_to(r) > NOTICE_TILES {
                continue;
            }
            let passive = self.world.passive(spec.detect.skill()).unwrap_or(0);
            if passive >= spec.detect_dc as i64 {
                self.found_trap(uid, st);
            }
        }
    }

    /// A Search whose Perception roll came to `roll`: armed traps within 2 tiles whose DC it
    /// reaches are found; one found by Investigation (a pressure plate, a needle) takes an
    /// Investigation roll, made once when such a trap is in reach.
    pub(super) fn search_traps(&mut self, roll: i64) -> bool {
        let now = self.world.total_hours();
        let mut study: Option<i64> = None;
        let mut any = false;
        for (at, spec, r) in self.traps_here() {
            let uid = Self::uid_of(at);
            let st = self.trap_state(uid);
            if st.found || !st.armed(spec.reset, now) || spec.detect_dc <= 0 || self.tiles_to(r) > NOTICE_TILES {
                continue;
            }
            let total = match spec.detect {
                crate::srd::traps::Detect::Perception => roll,
                crate::srd::traps::Detect::Investigation => *study.get_or_insert_with(|| self.world.roll_check("investigation").map_or(0, |r| r.total)),
            };
            if total >= spec.detect_dc as i64 {
                self.found_trap(uid, st);
                any = true;
            }
        }
        any
    }

    /// Find Traps: the armed traps within `tiles` of the hero are found. Returns how many.
    pub(super) fn reveal_traps(&mut self, tiles: i32) -> usize {
        let now = self.world.total_hours();
        let mut n = 0;
        for (at, spec, r) in self.traps_here() {
            let uid = Self::uid_of(at);
            let mut st = self.trap_state(uid);
            if st.found || !st.armed(spec.reset, now) || self.tiles_to(r) > tiles {
                continue;
            }
            st.found = true;
            self.set_trap_state(uid, st);
            n += 1;
        }
        n
    }

    fn found_trap(&mut self, uid: u32, mut st: TrapState) {
        st.found = true;
        self.set_trap_state(uid, st);
        self.ui.toast(crate::lang::tr_ui("You spot a trap.").into_owned());
    }

    /// The hero walked into zone `uid`: an armed trap there springs.
    pub(super) fn enter_trap_zone(&mut self, uid: u32) {
        let Some(spec) = self.loaded.map.zones.iter().find(|z| z.uid == uid).and_then(|z| if let ZoneKind::Trap { trap } = &z.kind { Some(trap.clone()) } else { None }) else { return };
        if self.trap_state(uid).armed(spec.reset, self.world.total_hours()) {
            self.spring_trap(TrapAt::Zone(uid), &spec);
        }
    }

    /// E: disarming a found, armed trap the hero stands in or beside (zones) or faces (prop `i`).
    /// True = E was used for it.
    pub(super) fn try_disarm(&mut self, prop: Option<usize>) -> bool {
        let now = self.world.total_hours();
        let target = match prop {
            Some(i) => {
                let e = &self.loaded.entities[i];
                let Some(spec) = self.project.db.prop(&e.def).and_then(|d| d.trap.clone()) else { return false };
                (TrapAt::Prop(e.uid), spec)
            }
            None => {
                let Some((at, spec, _)) = self.traps_here().into_iter().find(|(at, spec, r)| matches!(at, TrapAt::Zone(_)) && self.tiles_to(*r) <= 1 && {
                    let st = self.trap_state(Self::uid_of(*at));
                    st.found && st.armed(spec.reset, now)
                }) else {
                    return false;
                };
                (at, spec)
            }
        };
        let (at, spec) = target;
        let uid = Self::uid_of(at);
        let st = self.trap_state(uid);
        if !st.found || !st.armed(spec.reset, now) {
            return false;
        }
        if spec.disarm_dc <= 0 {
            self.ui.toast(crate::lang::tr_ui("This trap can't be disarmed. Go around it.").into_owned());
            return true;
        }
        let tools = |d: &crate::defs::ItemDef| d.srd == "thieves-tools";
        if self.project.db.items.iter().any(tools) && !self.world.inventory.slots.iter().flatten().any(|s| self.project.db.item(&s.item).is_some_and(tools)) {
            self.ui.toast(crate::lang::tr_ui("You need thieves' tools to disarm it.").into_owned());
            return true;
        }
        if st.retry_at.is_some_and(|t| now < t) {
            self.ui.toast(crate::lang::tr_ui("The mechanism won't give. Try again later.").into_owned());
            return true;
        }
        let Some(rec) = self.world.check("dex:thieves-tools", spec.disarm_dc, Mode::Normal, false) else { return true };
        let mut st = st;
        if rec.success == Some(true) {
            st.disarmed = true;
            st.retry_at = None;
            self.set_trap_state(uid, st);
            self.ui.toast(crate::lang::tr_ui("You disarm the trap.").into_owned());
            self.fire(GameEvent::TrapDisarmed, Self::source(at));
        } else if rec.total < (spec.disarm_dc - 5) as i64 {
            self.set_trap_state(uid, st);
            self.spring_trap(at, &spec);
        } else {
            st.retry_at = Some(now + DISARM_RETRY_HOURS);
            self.set_trap_state(uid, st);
            self.ui.toast(crate::lang::tr_ui("The mechanism won't give. Try again later.").into_owned());
        }
        true
    }

    /// Using prop `i` (E): an armed trap on it springs (not found) or is disarmed (found). True =
    /// E was used up by the trap (a disarming attempt).
    pub(super) fn prop_trap_on_use(&mut self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        let Some(spec) = self.project.db.prop(&e.def).and_then(|d| d.trap.clone()) else { return false };
        let uid = e.uid;
        let st = self.trap_state(uid);
        if !st.armed(spec.reset, self.world.total_hours()) {
            return false;
        }
        if st.found {
            return self.try_disarm(Some(i));
        }
        self.spring_trap(TrapAt::Prop(uid), &spec);
        false
    }

    fn source(at: TrapAt) -> Source {
        match at {
            TrapAt::Zone(u) => Source::Zone(u),
            TrapAt::Prop(u) => Source::Entity(u),
        }
    }

    /// The trap springs on the hero: its text, an attack roll or a save, damage, the second
    /// save (poison), the condition, a fall; then its *Trap sprung* triggers.
    pub(super) fn spring_trap(&mut self, at: TrapAt, spec: &TrapSpec) {
        let uid = Self::uid_of(at);
        let mut st = self.trap_state(uid);
        st.sprung_at = Some(self.world.total_hours());
        st.found = true;
        self.set_trap_state(uid, st);
        let text = if spec.text.is_empty() { crate::lang::tr_ui("A trap!").into_owned() } else { crate::lang::tr(&spec.text).to_string() };
        self.ui.toast(text);
        let hit_sfx = self.project.config.sfx.hit.clone();
        self.audio.play(&hit_sfx);
        let from = self.player.pos;
        // The first part: an attack roll, a save, or simply hits.
        let mut hit = true;
        let mut dmg = 0;
        if spec.attack > 0 {
            let roll = self.world.roll_dice(&format!("1d20{:+}", spec.attack)).map_or(0, |r| r.total);
            hit = roll >= self.armor_class() as i64;
        }
        if hit && !spec.damage.is_empty() {
            dmg = self.world.roll_dice(&spec.damage).map_or(0, |r| r.total.max(0) as i32);
        }
        if let Some(save) = spec.save {
            let saved = self.world.check(save.ability.id(), save.dc, Mode::Normal, true).is_some_and(|r| r.success == Some(true));
            dmg = TrapSpec::after_save(dmg, saved, save.half);
            hit = !saved;
        }
        if dmg > 0 {
            self.hurt_player_by(dmg, from, spec.damage_type);
        } else if spec.attack > 0 && !hit {
            self.miss_number((self.player.pos, self.ts()));
        }
        // The second part after a hit (the poison of darts and needles).
        if hit && self.game_over.is_none() {
            if let Some(save) = spec.then_save {
                // (A poison's save: Antitoxin and a dwarf's resilience give advantage.)
                let mode = if spec.then_type == Some(crate::srd::DamageType::Poison) { self.poison_save_mode(super::turns::Who::Player) } else { Mode::Normal };
                let saved = self.world.check(save.ability.id(), save.dc, mode, true).is_some_and(|r| r.success == Some(true));
                let rolled = if spec.then_damage.is_empty() { 0 } else { self.world.roll_dice(&spec.then_damage).map_or(0, |r| r.total.max(0) as i32) };
                let n = TrapSpec::after_save(rolled, saved, save.half);
                if n > 0 {
                    self.hurt_player_by(n, from, spec.then_type);
                }
                hit = !saved;
            }
            if hit {
                if let Some(c) = &spec.condition {
                    self.apply_player_effect(c);
                }
            }
        }
        if spec.fall_ft > 0 && self.game_over.is_none() {
            self.fall(spec.fall_ft, "", "", None);
        }
        self.fire(GameEvent::TrapSprung, Self::source(at));
    }

    /// Disarms the trap of the zone or prop named `name` on this map (`disarm_trap`, a lever).
    pub(super) fn disarm_named_trap(&mut self, name: &str) {
        let at = self.traps_here().into_iter().map(|(at, _, _)| at).find(|at| self.trap_name(*at) == name);
        match at {
            Some(at) => {
                let uid = Self::uid_of(at);
                let mut st = self.trap_state(uid);
                if st.disarmed {
                    return;
                }
                st.disarmed = true;
                self.set_trap_state(uid, st);
                self.fire(GameEvent::TrapDisarmed, Self::source(at));
            }
            None => eprintln!("[events] disarm_trap: no trap '{name}' on this map"),
        }
    }

    /// A trap's name: its zone's or prop's ("" = unnamed).
    fn trap_name(&self, at: TrapAt) -> String {
        match at {
            TrapAt::Zone(u) => self.loaded.map.zones.iter().find(|z| z.uid == u).map_or(String::new(), |z| z.name.clone()),
            TrapAt::Prop(u) => self.loaded.entities.iter().find(|e| e.uid == u).map_or(String::new(), |e| e.name.clone()),
        }
    }

    /// The traps on this map for scripts: (name, state word).
    pub(super) fn trap_list(&self) -> Vec<(String, String)> {
        let now = self.world.total_hours();
        self.traps_here()
            .into_iter()
            .map(|(at, spec, _)| (self.trap_name(at), self.trap_state(Self::uid_of(at)).word(spec.reset, now).to_string()))
            .collect()
    }

    /// Found, armed traps drawn in the world: a red outline around a zone, a mark over a prop.
    pub(super) fn draw_traps(&self) {
        let ts = self.ts();
        let now = self.world.total_hours();
        for (at, spec, r) in self.traps_here() {
            let st = self.trap_state(Self::uid_of(at));
            if !st.found || !st.armed(spec.reset, now) {
                continue;
            }
            let red = Color::new(0.9, 0.2, 0.15, 0.85);
            match at {
                TrapAt::Zone(_) => {
                    let (x, y, w, h) = (r.0 as f32 * ts, r.1 as f32 * ts, r.2 as f32 * ts, r.3 as f32 * ts);
                    draw_rectangle(x, y, w, h, Color::new(0.9, 0.2, 0.15, 0.15));
                    draw_rectangle_lines(x, y, w, h, 2.0, red);
                }
                TrapAt::Prop(_) => {
                    let (cx, cy) = ((r.0 as f32 + 0.5) * ts, r.1 as f32 * ts - ts * 0.4);
                    // (Over the prop, standing: its feet are its tile's bottom.)
                    self.upright(vec2(cx, (r.1 + 1) as f32 * ts), || {
                        draw_triangle(vec2(cx, cy - 7.0), vec2(cx - 7.0, cy + 5.0), vec2(cx + 7.0, cy + 5.0), red);
                        draw_line(cx, cy - 3.0, cx, cy + 1.0, 1.5, WHITE);
                    });
                }
            }
        }
    }

    /// `traps=` for status: `name:armed|found|disarmed|sprung@x,y` (the uid when unnamed).
    pub(super) fn traps_status(&self) -> Vec<String> {
        let now = self.world.total_hours();
        self.traps_here()
            .into_iter()
            .map(|(at, spec, r)| {
                let uid = Self::uid_of(at);
                let name = Some(self.trap_name(at)).filter(|n| !n.is_empty()).unwrap_or_else(|| uid.to_string());
                format!("{name}:{}@{},{}", self.trap_state(uid).word(spec.reset, now), r.0, r.1)
            })
            .collect()
    }
}
