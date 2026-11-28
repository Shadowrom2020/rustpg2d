//! Weather in the game: the map's weather (or the save game's override from *Set weather* /
//! `set_weather`), its particles over the lit scene, the darker ambient light of rain and the
//! built-in rain sound.

use gfx::prelude::*;

use super::Game;
use crate::map::{Weather, WeatherKind};
use crate::render;

impl Game {
    /// The weather on the current map now.
    pub(super) fn weather(&self) -> Weather {
        self.world.weather.get(&self.loaded.map.name).copied().unwrap_or(self.loaded.map.weather)
    }

    /// Particles and the rain sound (runs while the world runs).
    pub(super) fn update_weather(&mut self, dt: f32) {
        let w = self.weather();
        let view = render::camera_view(&self.camera());
        let scale = self.ts() / 32.0;
        self.weather_fx.update(dt, w, view, scale);
        let rain = if w.kind == WeatherKind::Rain && !self.on_title() { 0.1 + 0.2 * w.intensity } else { 0.0 };
        self.audio.set_rain(rain);
    }

    /// The ambient light with the weather's tint (rain darkens the scene).
    pub(super) fn weather_ambient(&self, ambient: Option<Color>) -> Option<Color> {
        render::weather::tint_ambient(ambient, self.weather())
    }

    /// The particle layer, after the light pass (camera set to the world). `ambient`: the light
    /// the scene got (dims the particles at night).
    pub(super) fn draw_weather(&mut self, view: Rect, ambient: Option<Color>) {
        let w = self.weather();
        if !w.active() || self.on_title() {
            return;
        }
        let brightness = ambient.map_or(1.0, |c| (c.r + c.g + c.b) / 3.0);
        let scale = self.ts() / 32.0;
        self.weather_fx.draw(w, view, scale, brightness);
    }

    /// `status()`: `weather=rain:0.60(map)` (or `(set)` for an override), `none` when clear.
    pub(super) fn weather_status(&self) -> String {
        let w = self.weather();
        if !w.active() {
            return "none".into();
        }
        let from = if self.world.weather.contains_key(&self.loaded.map.name) { "set" } else { "map" };
        format!("{}:{:.2}({from}) particles={} rain_sound={:.2}", w.kind.id(), w.intensity, self.weather_fx.count(), self.audio.rain_volume())
    }
}
