//! Mouse controls: left click walks (or attacks an enemy), right click walks over and interacts.

use std::collections::VecDeque;

use macroquad::prelude::*;

use super::{input, Game, RtEntity, Source};
use crate::entity::{EntityKind, Facing};
use crate::events::GameEvent;
use crate::path::{find_path, TilePos};

/// Upper bound on A* expansions per plan (keeps clicks cheap on 1024x1024 maps).
const MAX_NODES: usize = 40_000;
/// How often paths to moving targets are recomputed.
const REPATH_TIME: f32 = 0.5;
/// After the path ends next to a target that is still out of reach, walk straight at it
/// for at most this long (seconds) before giving up.
const APPROACH_TIME: f32 = 1.0;
/// Give up after this many re-plans in a row without making progress.
const MAX_STUCK_REPLANS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Goal {
    Walk(TilePos),
    /// Walk to the tile, then interact with zones there.
    InteractTile(TilePos),
    Interact(u32),
    Attack(u32),
}

pub(super) struct Order {
    goal: Goal,
    path: VecDeque<TilePos>,
    repath: f32,
    stuck: f32,
    last_pos: Vec2,
    /// Time spent walking straight at the target after the path ended.
    approach: f32,
    /// Re-plans in a row caused by being stuck.
    replans: u8,
}

impl Order {
    fn new(goal: Goal, path: VecDeque<TilePos>, pos: Vec2) -> Self {
        Order { goal, path, repath: REPATH_TIME, stuck: 0.0, last_pos: pos, approach: 0.0, replans: 0 }
    }
}

/// Mouse bookkeeping (hover display only shows while the mouse is in use).
#[derive(Default)]
pub(super) struct MouseState {
    last: Vec2,
    idle: f32,
}

impl Game {
    /// Debug description of the current order.
    pub fn order_debug(&self) -> String {
        match &self.order {
            Some(o) => format!("{:?} path={} cd={:.2}", o.goal, o.path.len(), self.combat.cooldown),
            None => "none".into(),
        }
    }

    pub(super) fn player_tile(&self) -> TilePos {
        self.tile_of(self.player.pos)
    }

    fn entity_tile(&self, e: &RtEntity) -> TilePos {
        self.tile_of(e.body.pos)
    }

    pub(super) fn mouse_world(&self) -> Vec2 {
        self.mouse_world_at(input::mouse_pos())
    }

    /// The world point under screen point `p`.
    pub(super) fn mouse_world_at(&self, p: Vec2) -> Vec2 {
        self.camera().screen_to_world(p)
    }

    /// Sprite-sized rect of an entity (what the player sees and clicks).
    pub(super) fn entity_click_rect(&self, e: &RtEntity) -> Rect {
        let db = &self.project.db;
        let ts = self.ts();
        let size = match e.kind {
            EntityKind::Prop => db.prop(&e.def).map(|d| self.assets.sprite_size(d.state_look(e.state).0, ts)),
            EntityKind::Character => db.character(&e.def).and_then(|d| self.assets.sheets.get(&d.appearance.body)).map(|s| vec2(s.frame_w, s.frame_h)),
            EntityKind::Item => db.item(&e.def).map(|d| self.assets.sprite_size(&d.icon, ts)),
        }
        .unwrap_or(vec2(ts, ts));
        Rect::new(e.body.pos.x - size.x / 2.0, e.body.pos.y - size.y, size.x, size.y)
    }

    pub(super) fn entity_under(&self, p: Vec2) -> Option<usize> {
        self.loaded
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.active && self.entity_click_rect(e).contains(p))
            .max_by(|a, b| a.1.body.pos.y.total_cmp(&b.1.body.pos.y))
            // Furniture in use: its user (clicking a bed talks to the sleeper).
            .map(|(i, _)| self.user_of(i).unwrap_or(i))
    }

    pub(super) fn uid_index(&self, uid: u32) -> Option<usize> {
        self.loaded.entities.iter().position(|e| e.uid == uid && e.active)
    }

    /// Close enough to talk to / hit an entity (also from a diagonally adjacent tile).
    fn in_reach(&self, i: usize) -> bool {
        let ts = self.ts();
        let r = self.player.rect();
        let reach = Rect::new(r.x - ts * 0.45, r.y - ts * 0.45, r.w + ts * 0.9, r.h + ts * 0.9);
        let e = &self.loaded.entities[i];
        reach.overlaps(&e.body.rect()) || self.player.pos.distance(e.body.pos) <= ts * 1.5 || self.across_counter(i)
    }

    fn face_towards(&mut self, p: Vec2) {
        let d = p - self.player.pos;
        if let Some(f) = Facing::from_vec(d.x, d.y) {
            self.player.facing = f;
        }
    }

    /// Whether a straight shot between two points is free of solid tiles
    /// (the tiles of the shooter and the target themselves don't count).
    pub(super) fn line_clear(&self, a: Vec2, b: Vec2) -> bool {
        let ts = self.ts();
        let tile = |p: Vec2| ((p.x / ts).floor() as i32, (p.y / ts).floor() as i32);
        let (ta, tb) = (tile(a), tile(b));
        // 1 px steps so lines that only clip a tile corner are caught too.
        let steps = (b - a).length().ceil().max(1.0) as i32;
        (0..=steps).all(|k| {
            let t = tile(a.lerp(b, k as f32 / steps as f32));
            t == ta || t == tb || !self.tile_solid(t.0, t.1)
        })
    }

    /// Plans a path for a goal (tiles occupied by solid entities are avoided).
    fn plan(&self, goal: Goal) -> Option<VecDeque<TilePos>> {
        let ignore = match goal {
            Goal::Interact(u) | Goal::Attack(u) => Some(u),
            _ => None,
        };
        let occupied = self.solid_entity_tiles(|e| Some(e.uid) != ignore);
        let blocked = |x: i32, y: i32| self.tile_solid(x, y) || occupied.contains(&(x, y));
        let near = |a: TilePos, b: TilePos| (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1;
        let start = self.player_tile();
        let path = match goal {
            Goal::Walk(t) | Goal::InteractTile(t) => {
                if blocked(t.0, t.1) {
                    find_path(start, t, |p| near(p, t), blocked, MAX_NODES)
                } else {
                    find_path(start, t, |p| p == t, blocked, MAX_NODES)
                }
            }
            Goal::Interact(uid) | Goal::Attack(uid) => {
                let i = self.uid_index(uid)?;
                let t = self.entity_tile(&self.loaded.entities[i]);
                // Prefer standing straight next to the target (facing it); diagonal as a fallback.
                let beside = |p: TilePos| (p.0 - t.0).abs() + (p.1 - t.1).abs() == 1;
                let path = find_path(start, t, beside, blocked, MAX_NODES).or_else(|| find_path(start, t, |p| near(p, t) && p != t, blocked, MAX_NODES));
                // Characters behind a counter: stand in front of it (two tiles away, a solid
                // tile or prop in between, e.g. a market stall) and talk across — unless walking
                // round to them is shorter.
                let props = self.solid_entity_tiles(|p| p.kind == EntityKind::Prop);
                let counter = |p: TilePos| {
                    let (dx, dy) = (t.0 - p.0, t.1 - p.1);
                    let mid = (p.0 + dx / 2, p.1 + dy / 2);
                    ((dx.abs() == 2 && dy == 0) || (dy.abs() == 2 && dx == 0)) && (self.tile_solid(mid.0, mid.1) || props.contains(&mid))
                };
                let talk = matches!(goal, Goal::Interact(_)) && self.loaded.entities[i].kind == EntityKind::Character;
                let across = if talk { find_path(start, t, counter, blocked, MAX_NODES) } else { None };
                match (path, across) {
                    (Some(p), Some(c)) => Some(if c.len() <= p.len() { c } else { p }),
                    (p, c) => p.or(c),
                }
            }
        };
        path.map(VecDeque::from)
    }

    /// Mouse clicks in the world. Called every frame while gameplay accepts input.
    pub(super) fn handle_mouse(&mut self) {
        let m = input::mouse_pos();
        if m.distance(self.mouse.last) > 0.5 {
            self.mouse.idle = 0.0;
        } else {
            self.mouse.idle += get_frame_time();
        }
        self.mouse.last = m;
        let (left, right) = (input::mouse_pressed(MouseButton::Left), input::mouse_pressed(MouseButton::Right));
        if !left && !right {
            return;
        }
        self.mouse.idle = 0.0;
        let w = self.mouse_world();
        let ts = self.ts();
        let tile = ((w.x / ts).floor() as i32, (w.y / ts).floor() as i32);
        if !self.loaded.map.in_bounds(tile.0, tile.1) {
            return;
        }
        let goal = match self.entity_under(w) {
            Some(i) if self.loaded.entities[i].hostile && self.loaded.entities[i].max_hp > 0 => Goal::Attack(self.loaded.entities[i].uid),
            Some(i) if right => Goal::Interact(self.loaded.entities[i].uid),
            None if right => Goal::InteractTile(tile),
            _ => Goal::Walk(tile),
        };
        self.click_marker = Some((w, 0.0));
        // Already next to the target: no path needed.
        let ready = match goal {
            Goal::Interact(u) => self.uid_index(u).is_some_and(|i| self.in_reach(i)),
            Goal::Attack(u) => self.uid_index(u).is_some_and(|i| self.melee_reaches(i)),
            _ => false,
        };
        let path = if ready { Some(VecDeque::new()) } else { self.plan(goal) };
        match path {
            Some(path) => {
                self.order = Some(Order::new(goal, path, self.player.pos))
            }
            None => {
                // Ranged attacks don't need a path if the target is within range.
                if matches!(goal, Goal::Attack(_)) && self.current_weapon().is_some_and(|w| w.ranged.is_some()) {
                    self.order = Some(Order::new(goal, VecDeque::new(), self.player.pos));
                } else {
                    self.order = None;
                    self.ui.toast("Can't get there.".into());
                }
            }
        }
    }

    /// Advances the current order; returns the direction to walk this frame.
    pub(super) fn steer_order(&mut self, dt: f32) -> Vec2 {
        let Some(mut order) = self.order.take() else { return Vec2::ZERO };
        let dir = self.steer(&mut order, dt);
        if let Some(dir) = dir {
            if self.order.is_none() {
                self.order = Some(order);
            }
            dir
        } else {
            Vec2::ZERO
        }
    }

    /// Returns `None` when the order is finished or cancelled.
    fn steer(&mut self, order: &mut Order, dt: f32) -> Option<Vec2> {
        let ts = self.ts();
        match order.goal {
            Goal::Interact(uid) => {
                let i = self.uid_index(uid)?;
                if self.in_reach(i) {
                    let p = self.loaded.entities[i].body.pos;
                    self.face_towards(p);
                    // A counter that does nothing itself (a market stall): talk to whoever
                    // stands behind it.
                    let i = if self.inert_prop(i) { self.behind_counter(i).unwrap_or(i) } else { i };
                    self.interact_entity(i);
                    return None;
                }
            }
            Goal::Attack(uid) => {
                let i = self.uid_index(uid)?;
                if self.loaded.entities[i].max_hp <= 0 {
                    return None;
                }
                let e = &self.loaded.entities[i];
                let target = e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0);
                let me = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
                let ranged = self.current_weapon().and_then(|w| w.ranged);
                let can_hit = match &ranged {
                    Some(r) => me.distance(target) <= r.range * ts * 0.85 && self.line_clear(me, target),
                    None => self.melee_reaches(i),
                };
                if can_hit {
                    self.face_towards(target + vec2(0.0, e.body.hitbox.y / 2.0));
                    if self.combat.cooldown <= 0.0 {
                        self.attack_at(Some(target));
                    }
                    order.path.clear();
                    self.player.moving = false;
                    return Some(Vec2::ZERO);
                }
            }
            _ => {}
        }

        // Moving targets: refresh the path now and then.
        let entity_goal = matches!(order.goal, Goal::Interact(_) | Goal::Attack(_));
        order.repath -= dt;
        if entity_goal && (order.repath <= 0.0 || order.path.is_empty()) {
            order.repath = REPATH_TIME;
            order.path = self.plan(order.goal)?;
        }

        // Stuck (e.g. an NPC walked into the way): plan again, give up if impossible. "Hardly
        // moved" is a tenth of a normal step, whatever the frame rate.
        if self.player.pos.distance(order.last_pos) < self.player.speed * dt * 0.1 {
            order.stuck += dt;
        } else {
            order.stuck = 0.0;
            order.replans = 0;
        }
        order.last_pos = self.player.pos;
        if order.stuck > 0.4 {
            order.stuck = 0.0;
            order.replans += 1;
            if order.replans >= MAX_STUCK_REPLANS {
                self.ui.toast("Can't get there.".into());
                return None;
            }
            order.path = self.plan(order.goal)?;
        }

        // Follow the path.
        while let Some(&next) = order.path.front() {
            let wp = vec2((next.0 as f32 + 0.5) * ts, (next.1 as f32 + 0.5) * ts + self.player.hitbox.y / 2.0);
            let d = wp - self.player.pos;
            if d.length() < 1.5 {
                order.path.pop_front();
                continue;
            }
            return Some(d.normalize());
        }
        match order.goal {
            Goal::Walk(_) => None,
            Goal::InteractTile(_) => {
                for uid in self.zones_inside.clone() {
                    self.fire(GameEvent::Interact, Source::Zone(uid));
                }
                if let Some(i) = self.facing_entity() {
                    self.interact_entity(i);
                }
                None
            }
            // The path ended on a tile next to the target, but the target is still out of
            // reach (e.g. standing at the far edge of that tile, or the target is not centred
            // on its tile): walk straight at it for the last bit.
            Goal::Interact(uid) | Goal::Attack(uid) => {
                let i = self.uid_index(uid)?;
                order.approach += dt;
                if order.approach > APPROACH_TIME {
                    self.ui.toast("Can't reach that.".into());
                    return None;
                }
                let d = self.loaded.entities[i].body.pos - self.player.pos;
                Some(d.normalize_or_zero())
            }
        }
    }

    /// Hover highlight, planned path and click marker (world space).
    pub(super) fn draw_mouse_world(&self) {
        let (ts, u) = (self.ts(), self.u());
        if let Some(o) = &self.order {
            for p in &o.path {
                draw_rectangle((p.0 as f32 + 0.5) * ts - u, (p.1 as f32 + 0.5) * ts - u, 2.0 * u, 2.0 * u, Color::new(1.0, 1.0, 1.0, 0.5));
            }
        }
        if let Some((p, age)) = self.click_marker {
            let r = (6.0 * (1.0 - age / 0.6) + 1.0) * u;
            draw_circle_lines(p.x, p.y, r, u, Color::new(1.0, 1.0, 0.5, 1.0 - age / 0.6));
        }
        if self.mouse.idle > 3.0 || self.modal() {
            return;
        }
        let w = self.mouse_world();
        match self.entity_under(w) {
            Some(i) => {
                let e = &self.loaded.entities[i];
                let r = self.entity_click_rect(e);
                let c = if e.hostile && e.max_hp > 0 { Color::new(1.0, 0.3, 0.3, 0.9) } else { Color::new(1.0, 1.0, 0.4, 0.9) };
                draw_rectangle_lines(r.x, r.y, r.w, r.h, u, c);
            }
            None => {
                let (tx, ty) = ((w.x / ts).floor(), (w.y / ts).floor());
                if self.loaded.map.in_bounds(tx as i32, ty as i32) {
                    let c = if self.tile_solid(tx as i32, ty as i32) { Color::new(1.0, 0.3, 0.3, 0.4) } else { Color::new(1.0, 1.0, 1.0, 0.35) };
                    draw_rectangle_lines(tx * ts, ty * ts, ts, ts, u, c);
                }
            }
        }
    }
}
