//! Light sources of the running game and the HUD clock.

use macroquad::prelude::*;

use super::{Game, RtEntity};
use crate::defs::{EquipSlot, LightSpec};
use crate::entity::EntityKind;
use crate::map::LightMode;
use crate::render::lighting::Light;
use super::text::{draw_text, measure_text};

impl Game {
    /// Light of a placed entity: its own, else its definition's (props: of the current state).
    fn entity_light(&self, e: &RtEntity) -> Option<LightSpec> {
        if let Some(l) = &e.light {
            return Some(l.clone());
        }
        let db = &self.project.db;
        match e.kind {
            EntityKind::Prop => db.prop(&e.def).and_then(|d| d.state_light(e.state)).cloned(),
            EntityKind::Character => db.character(&e.def).and_then(|d| d.light.clone()),
            EntityKind::Item => db.item(&e.def).and_then(|d| d.light.clone()),
        }
    }

    /// Light carried by the player: every equipped item that gives light.
    pub(super) fn player_light(&self) -> Option<LightSpec> {
        EquipSlot::ALL
            .iter()
            .filter_map(|s| self.world.equipment.item(*s))
            .filter_map(|id| self.project.db.item(id)?.light.clone())
            .chain(self.spell_light())
            .max_by(|a, b| a.radius.total_cmp(&b.radius))
    }

    /// All light sources on the current map this frame.
    pub(super) fn lights(&self) -> Vec<Light> {
        let ts = self.ts();
        let mut out = self.loaded.tile_lights.clone();
        if !self.on_title() {
            if let Some(l) = self.player_light() {
                out.push(Light::new(self.player.pos - vec2(0.0, ts * 0.6), &l));
            }
        }
        for e in self.loaded.entities.iter().filter(|e| e.active) {
            let Some(l) = self.entity_light(e) else { continue };
            // Roughly the middle of the sprite (the flame of a tall lamp is near its top).
            let lift = match e.kind {
                EntityKind::Prop => {
                    let sprite = self.project.db.prop(&e.def).map(|d| d.state_look(e.state).0.clone());
                    sprite.map_or(ts * 0.5, |s| self.assets.sprite_size(&s, ts).y * 0.65)
                }
                EntityKind::Character => ts * 0.6,
                EntityKind::Item => ts * 0.35,
            };
            out.push(Light::new(e.body.pos - vec2(0.0, lift), &l));
        }
        out.extend(self.fx_lights());
        out
    }

    /// Sets the time of day (automated play-throughs, debugging).
    pub fn set_hour(&mut self, hour: f32) {
        self.world.hour = hour.rem_euclid(24.0);
    }

    /// "Hh:mm" of the in-game clock.
    pub fn clock_text(&self) -> String {
        let h = self.world.hour.rem_euclid(24.0);
        format!("{:02}:{:02}", h as u32, ((h.fract()) * 60.0) as u32)
    }

    /// Small clock (top right) on maps with day and night.
    pub(super) fn draw_clock(&self) {
        if self.loaded.map.lighting.mode != LightMode::Outdoor {
            return;
        }
        let night = crate::render::lighting::is_night(self.world.hour);
        let text = format!("{} {}", if night { "Night" } else { "Day" }, self.clock_text());
        let size = 20.0;
        let w = measure_text(&text, None, size as u16, 1.0).width;
        let (x, y) = (screen_width() - w - 18.0, 30.0);
        draw_rectangle(x - 8.0, y - 20.0, w + 16.0, 28.0, Color::new(0.04, 0.04, 0.08, 0.6));
        draw_text(&text, x, y, size, if night { Color::new(0.7, 0.8, 1.0, 1.0) } else { Color::new(1.0, 0.92, 0.6, 1.0) });
    }
}
