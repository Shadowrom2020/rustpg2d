//! The fight's screen: the turn order bar (top), the action bar with movement and the action /
//! bonus / reaction marks (bottom), the reachable tiles, the path, the cursor, the hit chance
//! and cover of the target under it, the fight log; and the player's input on their turns.

use macroquad::prelude::*;

use super::{Aim, Side, TurnAct, Who};
use crate::grid::TilePos;
use crate::runtime::input::{self, Act};
use crate::runtime::skin;
use crate::runtime::text::{self, draw_plain, draw_text, measure_plain, Role};
use crate::runtime::Game;

const BAR_FONT: f32 = 18.0;
const LOG_LINES: usize = 6;

impl Game {
    /// The player's input on a turn they play: the cursor (arrows, mouse), the action bar (1-8,
    /// Tab / Shift+Tab and Attack, clicks), Confirm on the cursor, End turn.
    pub(super) fn turn_input(&mut self) {
        self.refresh_reach();
        // The choice window (Cast, Class, Attack: strike or throw) takes the input while open.
        if self.fight_menu_input() {
            return;
        }
        let ts = self.ts();
        // The action bar: keys and clicks.
        for (k, act) in Act::QUICK.iter().enumerate() {
            if input::act_pressed(*act) {
                self.pick_act(k);
                return;
            }
        }
        if input::act_pressed(Act::EndTurn) {
            self.pick_act(TurnAct::BAR.len() - 1);
            return;
        }
        let n = TurnAct::BAR.len();
        if input::act_pressed(Act::NextTab) {
            let Some(c) = self.fight.as_mut() else { return };
            c.selected = (c.selected + 1) % n;
        }
        if input::act_pressed(Act::PrevTab) {
            let Some(c) = self.fight.as_mut() else { return };
            c.selected = (c.selected + n - 1) % n;
        }
        if input::act_pressed(Act::Attack) {
            let Some(k) = self.fight.as_ref().map(|c| c.selected) else { return };
            self.pick_act(k);
            return;
        }
        let mouse = input::mouse_pos();
        if input::mouse_pressed(MouseButton::Left) {
            if let Some(k) = self.bar_layout().iter().position(|r| r.contains(mouse)) {
                self.pick_act(k);
                return;
            }
        }
        // A right-click on items lying around picks them up (walking next to them first); else it
        // cancels the aimed action.
        if input::mouse_pressed(MouseButton::Right) {
            let world = self.mouse_world_at(mouse);
            let t = ((world.x / ts).floor() as i32, (world.y / ts).floor() as i32);
            if !self.piles_at(t).is_empty() {
                self.pick_up_or_walk(t);
                return;
            }
        }
        if input::mouse_pressed(MouseButton::Right) || input::act_pressed(Act::Cancel) {
            if let Some(c) = &mut self.fight {
                c.aim = Aim::Move;
            }
        }
        // The cursor.
        let Some(c) = self.fight.as_mut() else { return };
        let mut moved = false;
        // (Along the screen: up is the tile straight above on the diamond grid.)
        for (act, d) in [(Act::Left, (-1, 1)), (Act::Right, (1, -1)), (Act::Up, (-1, -1)), (Act::Down, (1, 1))] {
            if input::act_pressed(act) {
                c.cursor = (c.cursor.0 + d.0, c.cursor.1 + d.1);
                moved = true;
            }
        }
        let world = self.mouse_world_at(mouse);
        // (A figure under the cursor: its tile, not the ground behind its body.)
        let over = self.entity_under(world).map(|i| crate::grid::feet_tile(self.loaded.entities[i].body.pos, ts));
        let Some(c) = self.fight.as_mut() else { return };
        if !moved && (mouse != c.mouse_last || input::mouse_pressed(MouseButton::Left)) && mouse != Vec2::ZERO {
            c.cursor = over.unwrap_or(((world.x / ts).floor() as i32, (world.y / ts).floor() as i32));
        }
        c.mouse_last = mouse;
        if input::act_pressed(Act::Confirm) || input::mouse_pressed(MouseButton::Left) {
            self.confirm_cursor();
        }
    }

    /// Items on tile `t` for the current fighter: taken at once from its tile or next to it, else
    /// it walks next to them and takes them there.
    pub(super) fn pick_up_or_walk(&mut self, t: TilePos) {
        let Some(me) = self.fight.as_ref().map(|c| c.current().who) else { return };
        let here = self.who_tile(me);
        if here.0.abs_diff(t.0) <= 1 && here.1.abs_diff(t.1) <= 1 {
            self.pick_up_at(me, t);
            return;
        }
        let Some(reach) = self.fight.as_ref().map(|c| c.reach.clone()) else { return };
        let goal = reach.iter().filter(|(p, _)| p.0.abs_diff(t.0) <= 1 && p.1.abs_diff(t.1) <= 1).min_by_key(|(_, (cost, _))| *cost).map(|(p, _)| *p);
        match goal {
            Some(g) => {
                let path = Self::path_to(&reach, here, g);
                if let Some(c) = &mut self.fight {
                    c.then_pick_up = Some(t);
                }
                self.walk(path);
            }
            None => self.ui.toast(crate::lang::tr_ui("Too far").into_owned()),
        }
    }

    /// Bar entry `k` was chosen: an instant action happens, a targeted one aims the cursor.
    pub(super) fn pick_act(&mut self, k: usize) {
        let Some(&act) = TurnAct::BAR.get(k) else { return };
        if let Some(c) = &mut self.fight {
            c.selected = k;
        }
        if let Err(why) = self.take_act(act) {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
        }
    }

    /// The cursor is back to walking (after an action aimed with it).
    fn aim_move(&mut self) {
        if let Some(c) = &mut self.fight {
            c.aim = Aim::Move;
        }
    }

    /// Confirm on the cursor: the aimed act on whoever stands there; without an aim, walk there
    /// (a foe: attack it, walking into reach first).
    pub(super) fn confirm_cursor(&mut self) {
        let Some(c) = &self.fight else { return };
        let (cursor, aim, me) = (c.cursor, c.aim.clone(), c.current().clone());
        // A class action: on whoever the cursor is on, itself included (or itself for those that
        // need no target).
        if let Aim::Feature(id) = &aim {
            let on = self.who_at(cursor).or_else(|| self.class_act_default_target(id)).unwrap_or(me.who);
            match self.use_class_act(id, on) {
                Ok(()) => self.aim_move(),
                Err(why) => self.ui.toast(crate::lang::tr_ui(why).into_owned()),
            }
            return;
        }
        // A spell: at the creature or tile under the cursor.
        if let Aim::Cast(id, slot) = &aim {
            match self.confirm_spell(id, *slot) {
                Ok(()) => self.aim_move(),
                Err(why) => self.ui.toast(crate::lang::tr_ui(why).into_owned()),
            }
            return;
        }
        let target = self.who_at(cursor).filter(|w| *w != me.who);
        if aim != Aim::Move {
            match target {
                Some(t) => match self.act_on(&aim, t) {
                    Ok(()) => self.aim_move(),
                    Err(why) => self.ui.toast(crate::lang::tr_ui(why).into_owned()),
                },
                None => self.ui.toast(crate::lang::tr_ui("No one there").into_owned()),
            }
            return;
        }
        let foe = target.filter(|t| self.fight.as_ref().and_then(|c| c.fighter(*t)).is_some_and(|f| f.side != me.side));
        if let Some(t) = foe {
            if self.act_ready(TurnAct::Attack).is_err() {
                self.ui.toast(crate::lang::tr_ui("Action used").into_owned());
                return;
            }
            // (A click strikes: walking up rather than throwing the weapon; Attack › Throw throws.)
            let reaches = self.attack_specs(me.who).iter().filter(|s| !s.thrown).any(|s| self.can_attack(me.who, t, s).is_ok());
            if reaches {
                if let Err(why) = self.act_on(&Aim::Attack, t) {
                    self.ui.toast(crate::lang::tr_ui(why).into_owned());
                }
                return;
            }
            // Walk next to it first, then attack.
            let Some(reach) = self.fight.as_ref().map(|c| c.reach.clone()) else { return };
            let here = self.who_tile(me.who);
            // (Spaces next to each other: big movers and targets too.)
            let (mn, ta, tn) = (self.space_n(me.who), self.space_anchor(t), self.space_n(t));
            let goal = reach.iter().filter(|(p, _)| crate::srd::combat::space_dist(crate::srd::combat::space_anchor(**p, mn), mn, ta, tn) <= 1).min_by_key(|(_, (cost, _))| *cost).map(|(p, _)| *p);
            match goal {
                Some(g) => {
                    let path = Self::path_to(&reach, here, g);
                    if let Some(c) = &mut self.fight {
                        c.then_attack = Some(t);
                    }
                    self.walk(path);
                }
                None => self.ui.toast(crate::lang::tr_ui("Out of reach").into_owned()),
            }
            return;
        }
        if target.is_some() {
            return;
        }
        // Items there: picked up (from next to them, else walking up first).
        if !self.piles_at(cursor).is_empty() {
            self.pick_up_or_walk(cursor);
            return;
        }
        let Some(reach) = self.fight.as_ref().map(|c| c.reach.clone()) else { return };
        if reach.contains_key(&cursor) {
            let here = self.who_tile(me.who);
            let path = Self::path_to(&reach, here, cursor);
            self.walk(path);
        } else {
            self.ui.toast(crate::lang::tr_ui("Too far").into_owned());
        }
    }

    /// Recomputes the current fighter's reachable tiles when its turn or position changed.
    pub(super) fn refresh_reach(&mut self) {
        let Some(c) = &self.fight else { return };
        let who = c.current().who;
        // (Where the others stand counts too: a foe that died or moved no longer blocks a tile.)
        let others = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            self.fighter_tiles(who).iter().map(|(_, s, t)| (*s == Side::Party, *t)).collect::<Vec<_>>().hash(&mut h);
            h.finish()
        };
        let key = (c.current, c.round, c.turn.moved, c.turn.speed, self.who_tile(who), others);
        if c.reach_key == Some(key) {
            return;
        }
        let reach = self.reachable(who);
        let Some(c) = self.fight.as_mut() else { return };
        c.reach = reach;
        c.reach_key = Some(key);
    }

    // ---------------------------------------------------------------------------- drawing

    /// The action bar's buttons (screen rectangles).
    pub(in crate::runtime) fn bar_layout(&self) -> Vec<Rect> {
        let _font = text::role(Role::Hud);
        let gap = 4.0;
        let widths: Vec<f32> = TurnAct::BAR.iter().enumerate().map(|(k, a)| measure_plain(&self.bar_text(k, *a), BAR_FONT).width + 16.0).collect();
        // More rows when a row would reach under the HUD buttons in the corner (a narrow screen,
        // a language with long words).
        let room = (2.0 * self.hud_buttons_left() - screen_width() - 16.0).max(200.0);
        let n = widths.len();
        let row_width = |rows: usize| {
            let per = n.div_ceil(rows);
            widths.chunks(per).map(|ws| ws.iter().sum::<f32>() + gap * (ws.len() as f32 - 1.0)).fold(0.0, f32::max)
        };
        let rows = (1..=3).find(|r| row_width(*r) <= room).unwrap_or(3);
        let per = n.div_ceil(rows);
        let h = 28.0;
        let mut out = Vec::new();
        for r in 0..rows {
            let ws = &widths[r * per..((r + 1) * per).min(widths.len())];
            let row_w: f32 = ws.iter().sum::<f32>() + gap * (ws.len() as f32 - 1.0);
            let mut x = (screen_width() - row_w) / 2.0;
            let y = screen_height() - 12.0 - (rows - r) as f32 * (h + gap);
            for w in ws {
                out.push(Rect::new(x, y, *w, h));
                x += w + gap;
            }
        }
        out
    }

    fn bar_text(&self, k: usize, a: TurnAct) -> String {
        // (Held by a grapple or a restraint with an escape DC: Grapple is Escape.)
        let escape = a == TurnAct::Grapple && self.fight.as_ref().is_some_and(|c| self.escape_dc(c.current().who).is_some());
        let label = crate::lang::tr_ui(if escape { "Escape" } else { a.label() });
        match k {
            0..=7 => format!("{} {label}", k + 1),
            _ if a == TurnAct::EndTurn => format!("{label} [{}]", crate::runtime::glyphs::key_text("end_turn").unwrap_or_default()),
            _ => label.into_owned(),
        }
    }

    /// Overlays on the map (in the world camera): reachable tiles, the path, foes in reach, the
    /// cursor and whose turn it is.
    pub(in crate::runtime) fn draw_fight_world(&self) {
        let Some(c) = &self.fight else { return };
        let ts = self.ts();
        let me = c.current();
        let tile_rect = |t: TilePos| Rect::new(t.0 as f32 * ts, t.1 as f32 * ts, ts, ts);
        // Everyone's ring: party blue, foes red, the current one gold.
        for f in c.order.iter().filter(|f| !f.out) {
            // (A big creature's ring round its whole square.)
            let (a, n) = (self.space_anchor(f.who), self.space_n(f.who));
            let r = Rect::new(a.0 as f32 * ts, a.1 as f32 * ts, ts * n as f32, ts * n as f32);
            let col = if f.who == me.who {
                Color::new(1.0, 0.85, 0.3, 0.9)
            } else if f.side == Side::Party {
                Color::new(0.35, 0.6, 1.0, 0.6)
            } else {
                Color::new(1.0, 0.3, 0.25, 0.6)
            };
            draw_rectangle_lines(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0, 2.0, col);
            // (A foe the hero can't see: a question mark over its space.)
            if let Who::Npc(uid) = f.who {
                if self.unseen_foe(uid) {
                    // (Standing over its space.)
                    self.upright(vec2(r.x + r.w / 2.0, r.bottom()), || draw_text("?", r.x + r.w / 2.0 - 5.0, r.y - 4.0, 26.0, Color::new(1.0, 0.85, 0.4, 0.95)));
                }
            }
        }
        if !self.players_turn() {
            return;
        }
        // Reachable tiles; those whose way leaves a foe's reach (an opportunity attack) in red.
        let here = self.who_tile(me.who);
        let threats = self.threatening_foes(me.who);
        let mn = self.space_n(me.who);
        let provokes = |t: TilePos| !c.turn.disengaged && threats.iter().any(|(f, fnn, r)| crate::srd::combat::space_dist(crate::srd::combat::space_anchor(t, mn), mn, *f, *fnn) > *r);
        for t in c.reach.keys() {
            let r = tile_rect(*t);
            let col = if *t != here && provokes(*t) { Color::new(1.0, 0.35, 0.25, 0.2) } else { Color::new(0.3, 0.6, 1.0, 0.16) };
            draw_rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, col);
        }
        if c.aim == Aim::Move && c.reach.contains_key(&c.cursor) && self.who_at(c.cursor).is_none() {
            // (A big mover: the square it would take there.)
            if mn > 1 {
                let a = crate::srd::combat::space_anchor(c.cursor, mn);
                draw_rectangle_lines(a.0 as f32 * ts + 2.0, a.1 as f32 * ts + 2.0, ts * mn as f32 - 4.0, ts * mn as f32 - 4.0, 2.0, Color::new(1.0, 1.0, 1.0, 0.7));
            }
            let path = Self::path_to(&c.reach, here, c.cursor);
            let col = if path.iter().any(|t| provokes(*t)) { Color::new(1.0, 0.45, 0.35, 0.85) } else { Color::new(1.0, 1.0, 1.0, 0.7) };
            let mut prev = vec2((here.0 as f32 + 0.5) * ts, (here.1 as f32 + 0.5) * ts);
            for t in path {
                let p = vec2((t.0 as f32 + 0.5) * ts, (t.1 as f32 + 0.5) * ts);
                draw_line(prev.x, prev.y, p.x, p.y, 3.0, col);
                prev = p;
            }
        }
        // A spell's area under the cursor.
        for t in self.aimed_area() {
            let r = tile_rect(t);
            draw_rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, Color::new(1.0, 0.45, 0.15, 0.28));
        }
        let r = tile_rect(c.cursor);
        let col = match c.aim {
            Aim::Move => Color::new(1.0, 1.0, 1.0, 0.95),
            _ => Color::new(1.0, 0.55, 0.2, 0.95),
        };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0, col);
    }

    /// The tiles of the foes next to `who` that would take an opportunity attack when it leaves
    /// their reach (standing, able to act, their reaction not spent, a melee attack).
    pub(super) fn threatening_foes(&self, who: Who) -> Vec<(TilePos, i32, i32)> {
        let Some(c) = &self.fight else { return Vec::new() };
        let Some(me) = c.fighter(who) else { return Vec::new() };
        // (Each with its space and its melee reach: a glaive's two tiles.)
        c.order
            .iter()
            .filter(|f| f.side != me.side && !f.out && !f.down && !f.reacted)
            .filter(|f| crate::srd::conditions::can_act(self.who_conditions(f.who)))
            .filter_map(|f| Some((f.who, self.melee_spec(f.who)?.reach.max(1))))
            .filter(|(f, r)| self.who_dist(*f, who) <= *r)
            .map(|(f, r)| (self.space_anchor(f), self.space_n(f), r))
            .collect()
    }

    /// The bars, the log and the target's hit chance (screen space, after the world).
    pub(in crate::runtime) fn draw_fight_hud(&self, cam: &Camera2D) {
        let Some(c) = &self.fight else { return };
        let _font = text::role(Role::Hud);
        // Turn order (top middle).
        let alive: Vec<&super::Fighter> = c.order.iter().filter(|f| !f.out).collect();
        let w = 96.0;
        let total = alive.len() as f32 * (w + 4.0) - 4.0;
        let mut x = ((screen_width() - total) / 2.0).max(8.0);
        let y = 8.0;
        skin::panel(x - 6.0, y - 4.0, total + 12.0, 46.0);
        draw_text(format!("{} {}", crate::lang::tr_ui("Round"), c.round), x - 4.0, y + 56.0, 16.0, skin::dim());
        for f in &alive {
            let cur = f.who == c.current().who;
            let r = Rect::new(x, y, w, 38.0);
            draw_rectangle(r.x, r.y, r.w, r.h, skin::button_fill(cur, Color::new(0.15, 0.15, 0.2, 0.9)));
            if cur {
                skin::cursor(r, Color::new(1.0, 0.86, 0.45, 1.0));
            }
            let mut name = self.who_name(f.who);
            while name.chars().count() > 3 && measure_plain(&name, 15.0).width > w - 8.0 {
                name.pop();
            }
            let color = if f.side == Side::Party { skin::normal() } else { Color::new(1.0, 0.6, 0.55, 1.0) };
            draw_plain(&name, r.x + 4.0, r.y + 15.0, 15.0, color);
            let (hp, max) = self.who_hp(f.who);
            skin::color_bar(r.x + 4.0, r.y + 22.0, w - 8.0, 6.0, if max > 0 { hp.max(0) as f32 / max as f32 } else { 0.0 }, Some(if f.side == Side::Party { Color::new(0.3, 0.8, 0.35, 1.0) } else { Color::new(0.85, 0.25, 0.2, 1.0) }));
            let mark = if f.down { format!("{} {}/{}", crate::lang::tr_ui("down"), f.saves.successes, f.saves.failures) } else { format!("{}", f.init) };
            draw_plain(&mark, r.x + 4.0, r.y + 36.0, 12.0, skin::dim());
            x += w + 4.0;
        }
        // The fight log (bottom left, over the map).
        let lines: Vec<&String> = c.log.iter().rev().take(LOG_LINES).collect();
        let ly = screen_height() - 90.0 - LOG_LINES as f32 * 18.0;
        if !lines.is_empty() {
            let lw = lines.iter().map(|l| measure_plain(l, 16.0).width).fold(0.0, f32::max).min(screen_width() * 0.6) + 12.0;
            draw_rectangle(6.0, ly - 16.0, lw, lines.len() as f32 * 18.0 + 8.0, Color::new(0.0, 0.0, 0.0, 0.45));
        }
        for (k, l) in lines.iter().rev().enumerate() {
            draw_plain(l, 12.0, ly + k as f32 * 18.0, 16.0, Color::new(1.0, 1.0, 1.0, 0.92));
        }
        if !self.players_turn() {
            let who = self.who_name(c.current().who);
            let t = format!("{who}...");
            let tw = measure_plain(&t, 20.0).width;
            draw_plain(&t, (screen_width() - tw) / 2.0, screen_height() - 24.0, 20.0, skin::highlight());
            // (The Reaction window asks on others' turns.)
            self.draw_fight_menu();
            return;
        }
        // The action bar with the turn's economy above it.
        let rects = self.bar_layout();
        for (k, (act, r)) in TurnAct::BAR.iter().zip(&rects).enumerate() {
            let ok = self.act_ready(*act).is_ok();
            let picked = k == c.selected || matches!((&c.aim, act), (Aim::Attack, TurnAct::Attack) | (Aim::Cast(..), TurnAct::Cast) | (Aim::Feature(_), TurnAct::Class) | (Aim::Help, TurnAct::Help) | (Aim::Grapple, TurnAct::Grapple) | (Aim::Shove, TurnAct::Shove) | (Aim::OffHand, TurnAct::OffHand));
            if !skin::button_picture(*r, picked) {
                draw_rectangle(r.x, r.y, r.w, r.h, skin::button_fill(picked, Color::new(0.12, 0.12, 0.18, 0.92)));
            }
            if picked {
                skin::cursor(*r, Color::new(1.0, 0.86, 0.45, 1.0));
            }
            draw_plain(&self.bar_text(k, *act), r.x + 8.0, r.y + 19.0, BAR_FONT, if ok || picked { skin::normal() } else { skin::disabled() });
        }
        let top = rects.first().map_or(screen_height() - 40.0, |r| r.y) - 8.0;
        let t = &c.turn;
        let pip = |used: bool| if used { "-" } else { "o" };
        let me = c.current();
        let reacted = me.reacted;
        let econ = format!(
            "{}: {} {}/{} ft   {} {}   {} {}   {} {}",
            self.who_name(me.who),
            crate::lang::tr_ui("Move"),
            t.speed.saturating_sub(t.moved),
            t.speed,
            crate::lang::tr_ui("Action"),
            pip(t.action),
            crate::lang::tr_ui("Bonus"),
            pip(t.bonus),
            crate::lang::tr_ui("Reaction"),
            pip(reacted)
        );
        let ew = measure_plain(&econ, 18.0).width;
        draw_plain(&econ, (screen_width() - ew) / 2.0, top, 18.0, skin::highlight());
        if let Aim::Cast(s, slot) = &c.aim {
            let name = self.spell_aim_line(s, *slot);
            let nw = measure_plain(&name, 18.0).width;
            draw_plain(&name, (screen_width() - nw) / 2.0, top - 20.0, 18.0, skin::normal());
        }
        // The class action chosen: its name, cost, uses left, and why not when it can't be used.
        if let Aim::Feature(id) = &c.aim {
            if let Some(a) = self.class_acts(me.who).into_iter().find(|a| a.id == *id) {
                let mut line = format!("{} ({})", crate::lang::tr(&a.name), crate::lang::tr_ui(a.cost.label()));
                if let Some((left, max)) = (!a.resource.is_empty()).then(|| self.uses(me.who, &a.resource)).flatten() {
                    line += &format!("  {left}/{max}");
                }
                let ready = self.class_act_ready(me.who, &a);
                if let Err(why) = ready {
                    line += &format!("  - {}", crate::lang::tr_ui(why));
                }
                let nw = measure_plain(&line, 18.0).width;
                draw_plain(&line, (screen_width() - nw) / 2.0, top - 20.0, 18.0, if ready.is_ok() { skin::normal() } else { skin::disabled() });
            }
        }
        self.draw_fight_menu();
        // The target under the cursor: hit chance and cover.
        if let Some(t) = self.who_at(c.cursor).filter(|t| *t != me.who) {
            let foe = c.fighter(t).is_some_and(|f| f.side != me.side);
            let mut lines = vec![self.who_name(t)];
            let (hp, max) = self.who_hp(t);
            lines.push(format!("HP {hp}/{max}  AC {}", self.who_ac(t)));
            if foe && matches!(c.aim, Aim::Move | Aim::Attack | Aim::OffHand) {
                let throw = c.aim == Aim::Attack && c.attack_mode == Some(true);
                let specs: Vec<_> = self.attack_specs(me.who).into_iter().filter(|s| s.thrown == throw || (!throw && !s.thrown)).collect();
                let spec = if c.aim == Aim::OffHand { self.offhand_weapon(me.who).map(|w| self.hero_spec(&w, true)) } else { specs.iter().find(|s| self.can_attack(me.who, t, s).is_ok()).or_else(|| specs.first()).cloned() };
                if let Some(s) = spec {
                    // (Without an aimed action a click on a foe out of melee reach walks there first.)
                    match self.can_attack(me.who, t, &s) {
                        Err(why) if c.aim != Aim::Move || s.ranged => lines.push(crate::lang::tr_ui(why).into_owned()),
                        _ => lines.push(format!("{} {:.0}%", crate::lang::tr_ui("Hit"), self.hit_chance(me.who, t, &s) * 100.0)),
                    }
                }
                let cover = self.cover(me.who, t);
                if cover != crate::srd::combat::Cover::None {
                    lines.push(format!("{} {}", crate::lang::tr_ui("Cover:"), crate::lang::tr_ui(cover.id())));
                }
            }
            let ts = self.ts();
            let p = cam.world_to_screen(vec2((c.cursor.0 as f32 + 1.0) * ts + 4.0, c.cursor.1 as f32 * ts));
            let w = lines.iter().map(|l| measure_plain(l, 16.0).width).fold(0.0, f32::max) + 36.0;
            let h = lines.len() as f32 * 18.0 + 12.0;
            let x = p.x.min(screen_width() - w - 4.0);
            skin::panel(x, p.y, w, h);
            for (k, l) in lines.iter().enumerate() {
                draw_plain(l, x + 10.0, p.y + 20.0 + k as f32 * 18.0, 16.0, if k == 0 { skin::highlight() } else { skin::normal() });
            }
        }
    }
}
