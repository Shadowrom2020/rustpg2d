//! Gathering and regrowing props. A prop with `PropDef::gather` (an ore vein, a tree) is hit
//! with E or a melee attack while the player has its tool kind (equipped, held or carried):
//! after enough hits (fewer with a *Gathering* skill) its loot table rolls and it switches to
//! its "used up" state. A prop with `PropDef::regrow` turns from one state into another once
//! enough in-game hours have passed since it changed (`EntityState::changed_at`), also while
//! the player was elsewhere: checked on entering a map and every second.

use macroquad::prelude::*;

use super::Game;
use crate::events::runner::ActionHost;
use crate::entity::EntityKind;
use crate::skills::SkillEffect;

/// Hits are forgotten after this many seconds without one.
const FORGET: f32 = 4.0;

/// A chip flying off a hit prop.
#[derive(Clone, Debug)]
pub(super) struct Chip {
    pos: Vec2,
    vel: Vec2,
    t: f32,
    color: Color,
}

impl Game {
    /// Whether prop `i` can be gathered from now (it gathers and isn't used up).
    fn gatherable(&self, i: usize) -> Option<crate::defs::Gather> {
        let e = &self.loaded.entities[i];
        if e.kind != EntityKind::Prop || !e.active {
            return None;
        }
        let d = self.project.db.prop(&e.def)?;
        let g = d.gather.clone()?;
        let used_up = !g.then_state.is_empty() && d.state_index(&g.then_state) == Some(e.state);
        (!used_up).then_some(g)
    }

    /// Percent fewer hits for a tool kind from the player's *Gathering* skills.
    fn gather_bonus(&self, tool: &str) -> f32 {
        self.skill_sum(|e, l| match e {
            SkillEffect::Gather { tool: t, percent } if t.is_empty() || t.eq_ignore_ascii_case(tool) => percent * l as f32,
            _ => 0.0,
        })
        .clamp(0.0, 80.0)
    }

    /// Hits prop `i` needs with the player's skill.
    fn hits_needed(&self, g: &crate::defs::Gather) -> u32 {
        ((g.hits.max(1) as f32) * (1.0 - self.gather_bonus(&g.tool) / 100.0)).ceil().max(1.0) as u32
    }

    /// E or a swing at prop `i`: a gathering hit if it gathers. Returns true if it does (the
    /// interaction is used up by it, also when the tool is missing).
    pub(super) fn gather_hit(&mut self, i: usize) -> bool {
        let Some(g) = self.gatherable(i) else { return false };
        if !g.tool.is_empty() && !crate::crafting::has_tool(&self.world, &self.project.db.tool_kinds(), &g.tool) {
            let what = self.project.db.prop(&self.loaded.entities[i].def).map_or(String::new(), |d| d.name.clone());
            self.ui.toast(format!("You need a {} for the {}.", g.tool, what.to_lowercase()));
            return true;
        }
        let need = self.hits_needed(&g);
        let e = &mut self.loaded.entities[i];
        let hits = e.gather.map_or(0, |(h, _)| h) + 1;
        e.gather = Some((hits, 0.0));
        let pos = e.body.pos;
        self.audio.play(&self.project.config.sfx.hit);
        self.spawn_chips(pos, i);
        if hits < need {
            return true;
        }
        // Done: the loot, then the used-up state.
        self.loaded.entities[i].gather = None;
        let drops = crate::defs::roll_loot(&g.loot, || rand::gen_range(0.0, 1.0), |a, b| rand::gen_range(a, b + 1));
        if drops.is_empty() {
            self.ui.toast("Nothing useful this time.".into());
        }
        for (item, n) in drops {
            let left = self.world.give_item(&item, n);
            if left < n {
                self.item_given(&item, n - left, 0);
            }
            if left > 0 {
                // No room: at the player's feet.
                let p = self.player.pos;
                self.place_drop(&item, left, p);
                self.ui.toast("Inventory full".into());
            }
        }
        if !g.then_state.is_empty() {
            self.set_prop_state(i, &g.then_state);
        }
        true
    }

    /// A few chips in the colors of the prop's sprite middle, flying up and falling.
    fn spawn_chips(&mut self, pos: Vec2, i: usize) {
        let u = self.u();
        let color = self.project.db.prop(&self.loaded.entities[i].def).map_or(GRAY, |d| if d.id.contains("tree") || d.id.contains("log") { Color::new(0.55, 0.38, 0.2, 1.0) } else { Color::new(0.6, 0.58, 0.55, 1.0) });
        for k in 0..6 {
            let a = -std::f32::consts::FRAC_PI_2 + (k as f32 - 2.5) * 0.35 + rand::gen_range(-0.15, 0.15);
            let speed = rand::gen_range(40.0, 80.0) * u / 2.0;
            self.chips.push(Chip { pos: pos - vec2(0.0, 10.0 * u), vel: Vec2::from_angle(a) * speed, t: 0.0, color });
        }
    }

    /// Chips fly and fade; hits are forgotten after a pause; regrowing props are checked.
    pub(super) fn update_gather(&mut self, dt: f32) {
        let g = 220.0 * self.u() / 2.0;
        for c in &mut self.chips {
            c.t += dt;
            c.vel.y += g * dt;
            c.pos += c.vel * dt;
        }
        self.chips.retain(|c| c.t < 0.5);
        for e in &mut self.loaded.entities {
            if let Some((_, since)) = &mut e.gather {
                *since += dt;
                if *since > FORGET {
                    e.gather = None;
                }
            }
        }
        self.regrow_timer -= dt;
        if self.regrow_timer <= 0.0 {
            self.regrow_timer = 1.0;
            self.update_regrow();
        }
    }

    /// Props whose regrow time has passed switch state (entering a map, every second).
    pub(super) fn update_regrow(&mut self) {
        let now = self.world.total_hours();
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if e.kind != EntityKind::Prop {
                continue;
            }
            let Some(r) = self.project.db.prop(&e.def).and_then(|d| d.regrow.clone().filter(|r| d.state_index(&r.from) == Some(e.state))) else { continue };
            match e.changed_at {
                // In that state since before its time was recorded: count from now.
                None => {
                    self.loaded.entities[i].changed_at = Some(now);
                    self.persist(i);
                }
                Some(t) if now - t >= r.hours as f64 => {
                    self.set_prop_state(i, &r.to);
                }
                _ => {}
            }
        }
    }

    /// Progress bars over props being worked on, and the chips (world space).
    pub(super) fn draw_gather(&self) {
        let u = self.u();
        for (i, e) in self.loaded.entities.iter().enumerate() {
            let Some((hits, _)) = e.gather else { continue };
            let Some(g) = self.gatherable(i) else { continue };
            let need = self.hits_needed(&g).max(1);
            let h = self.project.db.prop(&e.def).map_or(32.0, |d| self.assets.sprite_size(d.state_look(e.state).0, self.ts()).y);
            let (w, x, y) = (24.0 * u / 2.0, e.body.pos.x - 12.0 * u / 2.0, e.body.pos.y - h - 6.0 * u / 2.0);
            draw_rectangle(x - 1.0, y - 1.0, w + 2.0, 5.0 * u / 2.0 + 2.0, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_rectangle(x, y, w * hits as f32 / need as f32, 5.0 * u / 2.0, Color::new(1.0, 0.8, 0.3, 1.0));
        }
        for c in &self.chips {
            let a = 1.0 - c.t / 0.5;
            draw_rectangle(c.pos.x, c.pos.y, 2.0 * u / 2.0, 2.0 * u / 2.0, Color::new(c.color.r, c.color.g, c.color.b, a));
        }
    }

    /// `status()`: props being worked on (`def:hits/needed`).
    pub(super) fn gather_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let (hits, _) = e.gather?;
                let g = self.gatherable(i)?;
                Some(format!("{}:{hits}/{}", e.def, self.hits_needed(&g)))
            })
            .collect()
    }

    /// `status()`: props not in their first state (`def:state@x,y`).
    pub(super) fn prop_states_status(&self) -> Vec<String> {
        let ts = self.ts();
        self.loaded
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Prop && e.active && e.state > 0)
            .filter_map(|e| {
                let d = self.project.db.prop(&e.def)?;
                Some(format!("{}:{}@{},{}", e.def, d.state_name(e.state), (e.body.pos.x / ts).floor(), ((e.body.pos.y - 1.0) / ts).floor()))
            })
            .collect()
    }

    /// Advances the clock by `hours` (counting days; automated play-throughs `settime +H`):
    /// regrowing props catch up at once.
    pub fn advance_time(&mut self, hours: f32) {
        self.world.advance_hours(hours);
        self.water_crops(hours);
        self.update_regrow();
    }
}
