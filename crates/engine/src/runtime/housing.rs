//! Homes and furniture: maps the player owns (`WorldState::homes`, `set_home`), furniture items
//! (`ItemDef::furniture`) placed there as props (`WorldState::furniture`, uids from
//! [`FURNITURE_UID_BASE`]) and picked up again. E on placed furniture asks first: use it (a
//! bed, a chair, a chest) or pick it up.

use macroquad::prelude::*;

use super::occupancy::Occupancy;
use super::{Game, RtEntity};
use crate::entity::{EntityInstance, EntityKind, Facing};
use crate::events::runner::{ActionCtx, ActionHost};
use crate::events::Action;
use crate::map::ZoneKind;
use crate::state::{DROPPED_UID_BASE, FURNITURE_UID_BASE};

/// The variable the furniture question answers into.
const FURNITURE_ANSWER: &str = "_furniture_answer";

/// The furniture question waiting for its answer.
#[derive(Clone, Copy, Debug)]
pub(super) struct FurnitureAsk {
    uid: u32,
    /// Asked "Use / Pick up / Leave it" (else "Pick up / Leave it").
    usable: bool,
}

fn ui(text: &str) -> String {
    crate::lang::tr_ui(text).into_owned()
}

impl Game {
    /// Whether `uid` is a piece of furniture the player placed.
    pub(super) fn is_placed_furniture(uid: u32) -> bool {
        (FURNITURE_UID_BASE..DROPPED_UID_BASE).contains(&uid)
    }

    /// The furniture placed on `map` as placed props: entering the map adds them to its entities,
    /// so they work like props placed in the editor (a chest's contents, states, triggers).
    pub(super) fn furniture_instances(&self, map: &str) -> Vec<EntityInstance> {
        self.world.furniture.get(map).into_iter().flatten().map(|f| EntityInstance::new(f.uid, EntityKind::Prop, &f.prop, f.x, f.y)).collect()
    }

    /// Using furniture item `item`: placed on the free tile in front in a home of the player's.
    /// Returns false if the item isn't furniture.
    pub(super) fn place_furniture(&mut self, item: &str) -> bool {
        let Some(prop) = self.project.db.item(item).map(|d| d.furniture.clone()).filter(|p| !p.is_empty()) else { return false };
        let map = self.loaded.map.name.clone();
        if !self.world.homes.contains(&map) {
            self.ui.toast("Furniture goes in your home.".into());
            return true;
        }
        if self.project.db.prop(&prop).is_none() {
            eprintln!("[game] furniture item '{item}' places the unknown prop '{prop}'");
            self.ui.toast("Nothing happens.".into());
            return true;
        }
        let ts = self.ts();
        let (dx, dy) = self.player.facing.dir();
        let tile = self.tile_of(self.player.pos + vec2(dx, dy) * ts * 0.9);
        let mut pos = self.tile_spot(tile);
        // Furniture wider (or deeper) than a tile reaches away from the player, not over them.
        let r = self.footprint(&self.new_rt_entity(EntityKind::Prop, &prop, 0, "", pos, Facing::Down, true, 0));
        pos += match self.player.facing {
            Facing::Right => vec2((r.w - ts).max(0.0) / 2.0, 0.0),
            Facing::Left => vec2(-(r.w - ts).max(0.0) / 2.0, 0.0),
            Facing::Down => vec2(0.0, (r.h - ts).max(0.0)),
            Facing::Up => Vec2::ZERO,
        };
        let rt = self.new_rt_entity(EntityKind::Prop, &prop, 0, "", pos, Facing::Down, true, 0);
        if !self.room_for(&rt) {
            self.ui.toast("There's no room there.".into());
            return true;
        }
        if self.world.take_item(item, 1) == 0 {
            return true;
        }
        let uid = self.world.add_furniture(&map, &prop, item, pos.x, pos.y);
        let inst = EntityInstance::new(uid, EntityKind::Prop, &prop, pos.x, pos.y);
        let rt = self.build_placed(&map, &inst);
        self.loaded.map.entities.push(inst);
        let name = self.project.db.prop(&prop).map_or(prop.clone(), |d| d.name.clone());
        self.loaded.entities.push(rt);
        let i = self.loaded.entities.len() - 1;
        self.occupy(i);
        self.audio.play(&self.project.config.sfx.pickup);
        self.ui.toast(crate::lang::tr_ui(&format!("Placed the {name}.")).into_owned());
        true
    }

    /// The ground a piece of furniture takes: its hitbox, or for a flat prop (a rug) its picture.
    fn footprint(&self, e: &RtEntity) -> Rect {
        if !e.flat {
            return e.body.rect();
        }
        let size = self.project.db.prop(&e.def).map_or(vec2(self.ts(), self.ts()), |d| self.assets.sprite_size(&d.sprite, self.ts()));
        // (2 px in: rugs side by side may touch.)
        Rect::new(e.body.pos.x - size.x / 2.0 + 2.0, e.body.pos.y - size.y + 2.0, size.x - 4.0, size.y - 4.0)
    }

    /// Whether furniture `rt` fits where it stands: on the map, on walkable tiles away from
    /// doors and arrival points, clear of solid decorations (a flat piece at its feet). A flat piece (a rug) may lie under
    /// the player and other things, but not on another flat piece; anything else needs the
    /// player and other entities out of the way, flat ones excepted (a table goes on a rug).
    fn room_for(&self, rt: &RtEntity) -> bool {
        let ts = self.ts();
        let r = self.footprint(rt);
        let mut tiles: Vec<_> = Occupancy::covered(r, ts).collect();
        tiles.push(self.tile_of(rt.body.pos));
        let doorway = |x: i32, y: i32| {
            self.loaded.map.zones.iter().any(|z| {
                matches!(z.kind, ZoneKind::Portal { .. } | ZoneKind::Spawn | ZoneKind::Transition { .. })
                    && x >= z.x as i32
                    && y >= z.y as i32
                    && x < (z.x + z.w) as i32
                    && y < (z.y + z.h) as i32
            })
        };
        // (A rug's picture may reach under a table decoration; where it lies has to be clear.)
        if tiles.iter().any(|&(x, y)| self.tile_solid(x, y) || doorway(x, y)) || self.decor_blocks(if rt.flat { rt.body.rect() } else { r }) {
            return false;
        }
        let others = self.loaded.entities.iter().filter(|e| e.active && e.flat == rt.flat);
        if rt.flat {
            return !others.into_iter().any(|e| self.footprint(e).overlaps(&r));
        }
        let player = Rect::new(self.player.pos.x - self.player.hitbox.x / 2.0, self.player.pos.y - self.player.hitbox.y, self.player.hitbox.x, self.player.hitbox.y);
        !r.overlaps(&player) && !others.into_iter().any(|e| e.body.rect().overlaps(&r))
    }

    /// E on placed furniture `i`: asks whether to use it or pick it up. Returns false when it
    /// isn't placed furniture, or when the answer was *Use* (the ordinary interaction follows).
    pub(super) fn ask_furniture(&mut self, i: usize) -> bool {
        let uid = self.loaded.entities[i].uid;
        if !Self::is_placed_furniture(uid) || self.furniture_use.take() == Some(uid) {
            return false;
        }
        if self.runner.busy() || self.ui.dialog_open() {
            return true;
        }
        let Some(d) = self.project.db.prop(&self.loaded.entities[i].def) else { return false };
        let usable = d.use_spot.is_some() || d.container.is_some() || !d.triggers.is_empty();
        let ask = if usable {
            Action::Choice { speaker: String::new(), text: d.name.clone(), options: vec![ui("Use"), ui("Pick up"), ui("Leave it")], var: FURNITURE_ANSWER.into() }
        } else {
            Action::Choice { speaker: String::new(), text: ui(&format!("Pick up the {}?", d.name)), options: vec![ui("Pick up"), ui("Leave it")], var: FURNITURE_ANSWER.into() }
        };
        self.world.set_var(FURNITURE_ANSWER, -1);
        self.runner.push(&[ask], &ActionCtx { owner: Some(uid), ..Default::default() });
        self.furniture_ask = Some(FurnitureAsk { uid, usable });
        true
    }

    /// Acts on the furniture question once it is answered.
    pub(super) fn furniture_answered(&mut self) {
        let Some(ask) = self.furniture_ask else { return };
        if self.runner.busy() || self.ui.dialog_open() {
            return;
        }
        self.furniture_ask = None;
        let answer = self.world.var(FURNITURE_ANSWER);
        self.world.vars.remove(FURNITURE_ANSWER);
        let Some(i) = self.loaded.entities.iter().position(|e| e.active && e.uid == ask.uid) else { return };
        match (ask.usable, answer) {
            (true, 0) => {
                self.furniture_use = Some(ask.uid);
                self.interact_entity(i);
                self.furniture_use = None;
            }
            (true, 1) | (false, 0) => self.pick_up_furniture(i),
            _ => {}
        }
    }

    /// Picks placed furniture `i` up again: the item it was made from goes into the inventory.
    fn pick_up_furniture(&mut self, i: usize) {
        let map = self.loaded.map.name.clone();
        let uid = self.loaded.entities[i].uid;
        if self.world.containers.get(&map).and_then(|m| m.get(&uid)).is_some_and(|inv| !inv.is_empty()) {
            self.ui.toast("Empty it first.".into());
            return;
        }
        if self.seat.as_ref().is_some_and(|s| s.uid == uid) || self.furniture_taken(uid) {
            self.ui.toast("Someone is using it.".into());
            return;
        }
        let Some(item) = self.world.furniture.get(&map).and_then(|l| l.iter().find(|f| f.uid == uid)).map(|f| f.item.clone()) else { return };
        if self.world.give_item(&item, 1) > 0 {
            self.ui.toast("Inventory full".into());
            return;
        }
        self.world.remove_furniture(&map, uid);
        self.loaded.map.entities.retain(|e| e.uid != uid);
        self.loaded.entities[i].active = false;
        self.item_given(&item, 1, 0);
    }

    /// For `status()`: the player's homes.
    pub(super) fn homes_status(&self) -> Vec<String> {
        self.world.homes.iter().cloned().collect()
    }

    /// For `status()`: placed furniture as map:prop@x,y (tiles).
    pub(super) fn furniture_status(&self) -> Vec<String> {
        let ts = self.ts();
        let mut out = Vec::new();
        for (map, list) in &self.world.furniture {
            for f in list {
                let (x, y) = crate::grid::feet_tile(vec2(f.x, f.y), ts);
                out.push(format!("{map}:{}@{x},{y}", f.prop));
            }
        }
        out
    }
}
