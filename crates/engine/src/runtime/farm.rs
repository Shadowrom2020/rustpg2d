//! Farming: props marked as farm plots (`PropDef::plot`) take a seed (an item with a `crop`).
//! E on an empty plot plants the first seed the player carries (or use a seed on it); the crop
//! grows through its stages over in-game hours (`EntityState::planted`, so also while the
//! player is elsewhere), faster while it rains on the plot's map; E on a ripe crop harvests it.

use macroquad::prelude::*;

use super::Game;
use crate::defs::Crop;
use crate::entity::EntityKind;
use crate::events::runner::ActionHost;
use crate::state::Planted;

/// Rain makes crops grow this much faster (extra hours per hour of rain).
const RAIN_BONUS: f64 = 0.5;

impl Game {
    fn is_plot(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        e.kind == EntityKind::Prop && e.active && self.project.db.prop(&e.def).is_some_and(|d| d.plot)
    }

    /// The crop growing on entity `i` and how long it has grown (hours, rain included).
    fn crop_of(&self, i: usize) -> Option<(Crop, f64, String)> {
        let p = self.loaded.entities[i].planted.as_ref()?;
        let d = self.project.db.item(&p.seed)?;
        let crop = d.crop.clone()?;
        Some((crop, self.world.total_hours() - p.at + p.bonus, d.name.clone()))
    }

    /// E on a farm plot: harvest, tell how long it still takes, or plant. Returns true if `i`
    /// is a plot (the interaction is used up by it).
    pub(super) fn try_farm(&mut self, i: usize) -> bool {
        if !self.is_plot(i) {
            return false;
        }
        if let Some((crop, grown, name)) = self.crop_of(i) {
            if crop.is_ripe(grown) {
                self.harvest(i, &crop);
            } else {
                let left = (crop.grow_hours() as f64 - grown).max(0.0).ceil();
                self.ui.toast(format!("{name}: ripe in about {left} hours."));
            }
            return true;
        }
        if self.loaded.entities[i].planted.is_some() {
            // An unknown seed (its item was removed): clear the plot.
            self.loaded.entities[i].planted = None;
            self.persist(i);
        }
        let seed = self.world.inventory.item_ids().into_iter().find(|id| self.project.db.item(id).is_some_and(|d| d.crop.as_ref().is_some_and(|c| !c.stages.is_empty())));
        match seed {
            Some(seed) => {
                self.plant(i, &seed);
            }
            None => self.ui.toast("You have no seeds to plant here.".into()),
        }
        true
    }

    /// Using a seed item: planted on the empty plot in front of the player. Returns true if so.
    pub(super) fn plant_on_facing(&mut self, item: &str) -> bool {
        if !self.project.db.item(item).is_some_and(|d| d.crop.is_some()) {
            return false;
        }
        let Some(i) = self.facing_entity().filter(|&i| self.is_plot(i)) else {
            self.ui.toast("Stand in front of a farm plot to plant it.".into());
            return true;
        };
        if self.loaded.entities[i].planted.is_some() {
            self.ui.toast("Something already grows there.".into());
            return true;
        }
        self.plant(i, item)
    }

    fn plant(&mut self, i: usize, seed: &str) -> bool {
        if self.world.take_item(seed, 1) == 0 {
            return false;
        }
        let now = self.world.total_hours();
        self.loaded.entities[i].planted = Some(Planted { seed: seed.to_string(), at: now, bonus: 0.0 });
        self.persist(i);
        let name = self.project.db.item(seed).map_or(seed.to_string(), |d| d.name.clone());
        self.ui.toast(format!("Planted {name}."));
        true
    }

    fn harvest(&mut self, i: usize, crop: &Crop) {
        let drops = crate::defs::roll_loot(&crop.harvest, || rand::gen_range(0.0, 1.0), |a, b| rand::gen_range(a, b + 1));
        self.loaded.entities[i].planted = None;
        self.persist(i);
        self.audio.play(&self.project.config.sfx.pickup);
        if drops.is_empty() {
            self.ui.toast("The harvest failed.".into());
        }
        for (item, n) in drops {
            let left = self.world.give_item(&item, n);
            if left < n {
                self.item_given(&item, n - left, 0);
            }
            if left > 0 {
                let p = self.player.pos;
                self.place_drop(&item, left, p);
                self.ui.toast("Inventory full".into());
            }
        }
    }

    /// `hours` of in-game time passed on this map: rain waters the growing crops.
    pub(super) fn water_crops(&mut self, hours: f32) {
        if self.weather().kind != crate::map::WeatherKind::Rain || hours <= 0.0 {
            return;
        }
        for i in 0..self.loaded.entities.len() {
            if self.crop_of(i).is_some_and(|(c, grown, _)| !c.is_ripe(grown)) {
                if let Some(p) = &mut self.loaded.entities[i].planted {
                    p.bonus += hours as f64 * RAIN_BONUS;
                }
            }
        }
        // (Saved with the plot's other state when it changes; on a long jump save it now.)
        if hours >= 1.0 {
            for i in 0..self.loaded.entities.len() {
                if self.loaded.entities[i].planted.is_some() {
                    self.persist(i);
                }
            }
        }
    }

    /// The crop on a plot (drawn over the plot).
    pub(super) fn draw_crop(&self, i: usize) {
        let Some((crop, grown, _)) = self.crop_of(i) else { return };
        let Some(k) = crop.stage_at(grown) else { return };
        self.assets.draw_sprite(&crop.stages[k].sprite, 0, self.loaded.entities[i].body.pos, WHITE);
    }

    /// `status()`: planted plots (`seed:stage/stages@x,y`, the last stage = ripe).
    pub(super) fn crops_status(&self) -> Vec<String> {
        let ts = self.ts();
        (0..self.loaded.entities.len())
            .filter_map(|i| {
                let (crop, grown, _) = self.crop_of(i)?;
                let e = &self.loaded.entities[i];
                let seed = &e.planted.as_ref()?.seed;
                let stage = if crop.is_ripe(grown) { "ripe".to_string() } else { format!("{}/{}", crop.stage_at(grown)? + 1, crop.stages.len()) };
                Some(format!("{seed}:{stage}@{},{}", (e.body.pos.x / ts).floor(), ((e.body.pos.y - 1.0) / ts).floor()))
            })
            .collect()
    }
}
