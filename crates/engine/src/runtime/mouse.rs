//! Mouse controls: left click walks (or attacks an enemy), right click walks over and interacts.

use std::collections::VecDeque;

use macroquad::prelude::*;

use super::{input, Game, RtEntity, Source};
use crate::entity::{EntityKind, Facing};
use crate::events::GameEvent;
use crate::path::{find_path, find_path_ex, Search, TilePos};

/// Upper bound on A* expansions per plan (keeps clicks cheap on 1024x1024 maps).
const MAX_NODES: usize = 40_000;
/// How often paths to moving targets are recomputed.
const REPATH_TIME: f32 = 0.5;
/// After the path ends next to a target that is still out of reach, walk straight at it
/// for at most this long (seconds) before giving up.
const APPROACH_TIME: f32 = 1.0;
/// Tiles walked straight at a gap before a jump planned by a click (a little more than the 10 ft run-up).
const RUN_UP_TILES: i32 = 3;
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
    /// Talking to someone keeping a stall from its front, across it: being beside them on the
    /// way there doesn't start the talk.
    front: bool,
}

impl Order {
    fn new(goal: Goal, path: VecDeque<TilePos>, pos: Vec2) -> Self {
        Order { goal, path, repath: REPATH_TIME, stuck: 0.0, last_pos: pos, approach: 0.0, replans: 0, front: false }
    }
}

/// Mouse bookkeeping (hover display only shows while the mouse is in use).
#[derive(Default)]
pub(super) struct MouseState {
    last: Vec2,
    idle: f32,
    /// What the last right-click was on: worked again while the button stays down (hold to repeat).
    hold: Option<u32>,
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
            EntityKind::Prop => match db.prop(&e.def) {
                Some(d) => return self.assets.prop_rect(d, e.state, e.body.facing, e.body.pos, ts, true),
                None => None,
            },
            EntityKind::Character => db.character(&e.def).and_then(|d| self.assets.sheets.get(&d.appearance.body)).map(|s| s.world_size()),
            EntityKind::Item => db.item(&e.def).map(|d| self.assets.sprite_size(&d.icon, ts)),
        }
        .unwrap_or(vec2(ts, ts));
        Rect::new(e.body.pos.x - size.x / 2.0, e.body.pos.y - size.y, size.x, size.y)
    }

    /// Where an entity's sprite-sized rect is on the screen (it stands upright at its feet).
    pub(super) fn entity_screen_rect(&self, e: &RtEntity) -> Rect {
        let v = self.view();
        let (u, r, feet) = (v.upright_scale(), self.entity_click_rect(e), e.body.pos);
        let s = v.to_screen(feet);
        Rect::new(s.x + (r.x - feet.x) * u, s.y + (r.y - feet.y) * u, r.w * u, r.h * u)
    }

    /// The entity whose sprite is at the screen point over world point `p` (a point the mouse
    /// gave: the ground under the cursor), the nearest one when several overlap.
    pub(super) fn entity_under(&self, p: Vec2) -> Option<usize> {
        let s = self.view().to_screen(p);
        self.loaded
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.active && self.entity_screen_rect(e).contains(s))
            .max_by(|a, b| crate::render::iso::depth(a.1.body.pos).total_cmp(&crate::render::iso::depth(b.1.body.pos)))
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

    /// Whether a path for talking to character `i` ends in front of the stall they keep, across
    /// it (the planner takes that spot whenever it can be reached).
    fn ends_in_front(&self, i: usize, path: &VecDeque<TilePos>) -> bool {
        let end = path.back().copied().unwrap_or_else(|| self.player_tile());
        let t = self.entity_tile(&self.loaded.entities[i]);
        let (dx, dy) = (t.0 - end.0, t.1 - end.1);
        let mid = (end.0 + dx / 2, end.1 + dy / 2);
        self.keeps_counter(i) && ((dx.abs() == 2 && dy == 0) || (dy.abs() == 2 && dx == 0)) && (self.tile_solid(mid.0, mid.1) || self.prop_blocks(mid.0, mid.1))
    }

    fn face_towards(&mut self, p: Vec2) {
        let d = p - self.player.pos;
        if let Some(f) = Facing::from_vec(d.x, d.y) {
            self.player.facing = f;
        }
    }

    /// Whether a straight shot (or a look) between two points is free of what stops shots:
    /// walls, tall props; low obstacles don't count ([`Game::blocks_shot`]). The tiles of the
    /// shooter and the target themselves don't count.
    pub(super) fn line_clear(&self, a: Vec2, b: Vec2) -> bool {
        let ts = self.ts();
        let (ta, tb) = (((a.x / ts).floor() as i32, (a.y / ts).floor() as i32), ((b.x / ts).floor() as i32, (b.y / ts).floor() as i32));
        // Every tile the line crosses between its ends (a grid walk, not pixel steps).
        crate::grid::line_tiles(a, b, ts).all(|t| t == ta || t == tb || !self.blocks_shot(t.0, t.1))
    }

    /// Plans a path for a goal (tiles occupied by solid entities are avoided).
    fn plan(&self, goal: Goal) -> Option<VecDeque<TilePos>> {
        let ignore = match goal {
            Goal::Interact(u) | Goal::Attack(u) => Some(u),
            _ => None,
        };
        // Round solid props and people, except the tiles of the target itself.
        use super::occupancy::{Occupancy, OCC_CHAR, OCC_PROP};
        let occ = &self.loaded.occupancy;
        let targets: Vec<TilePos> = ignore.and_then(|u| self.uid_index(u)).map(|i| Occupancy::covered(self.loaded.entities[i].body.rect(), self.ts()).collect()).unwrap_or_default();
        // (Round pits and ledges too, unless clicked into.)
        let blocked = |x: i32, y: i32| self.tile_solid(x, y) || (occ.has(x, y, OCC_PROP | OCC_CHAR | super::occupancy::OCC_DROP) && !targets.contains(&(x, y)));
        let near = |a: TilePos, b: TilePos| (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1;
        let start = self.player_tile();
        // No way round: over a pit or a ledge's edge with a running jump, if the hero can clear it.
        let over_gap = |t: TilePos, is_goal: &dyn Fn(TilePos) -> bool| self.plan_jump(start, t, is_goal, &blocked);
        let path = match goal {
            Goal::Walk(t) | Goal::InteractTile(t) => {
                let goal_ok = |p: TilePos| if blocked(t.0, t.1) { near(p, t) } else { p == t };
                find_path(start, t, goal_ok, blocked, MAX_NODES).or_else(|| over_gap(t, &goal_ok))
            }
            Goal::Interact(uid) | Goal::Attack(uid) => {
                let i = self.uid_index(uid)?;
                let t = self.entity_tile(&self.loaded.entities[i]);
                // Prefer standing straight next to the target (facing it); diagonal as a fallback.
                let beside = |p: TilePos| (p.0 - t.0).abs() + (p.1 - t.1).abs() == 1;
                let (path, tried_all) = match find_path_ex(start, t, beside, blocked, MAX_NODES) {
                    Search::Found(p) => (Some(p), true),
                    // Beside it is taken: diagonal then. Out of budget: another search won't help.
                    Search::NoRoute => (find_path(start, t, |p| near(p, t) && p != t, blocked, MAX_NODES), true),
                    Search::OutOfBudget => (None, false),
                };
                // Characters behind a counter: stand in front of it (two tiles away, a solid
                // tile or prop in between, e.g. a market stall) and talk across — unless walking
                // round to them is shorter. Someone keeping a stall is always talked to across
                // it, from the front (not from beside them in the back).
                let counter = |p: TilePos| {
                    let (dx, dy) = (t.0 - p.0, t.1 - p.1);
                    let mid = (p.0 + dx / 2, p.1 + dy / 2);
                    ((dx.abs() == 2 && dy == 0) || (dy.abs() == 2 && dx == 0)) && (self.tile_solid(mid.0, mid.1) || self.prop_blocks(mid.0, mid.1))
                };
                let talk = tried_all && matches!(goal, Goal::Interact(_)) && self.loaded.entities[i].kind == EntityKind::Character;
                let across = if talk { find_path(start, t, counter, blocked, MAX_NODES) } else { None };
                let keeper = talk && self.keeps_counter(i);
                match (path, across) {
                    (_, Some(c)) if keeper => Some(c),
                    (Some(p), Some(c)) => Some(if c.len() <= p.len() { c } else { p }),
                    (None, None) if tried_all => over_gap(t, &|p: TilePos| near(p, t) && p != t),
                    (p, c) => p.or(c),
                }
            }
        };
        path.map(VecDeque::from)
    }

    /// A path over a pit or a ledge's edge (a *Drop* zone) when walking can't get there: to three
    /// tiles straight before the gap (the run-up: [`RUN_UP_TILES`]), on to its edge, the tile past
    /// it, then on to the goal. Only gaps the hero clears with a running jump (STR feet, three
    /// times with *Jump*); stepping into the gap from the edge makes the jump (`movement.rs`).
    fn plan_jump(&self, start: TilePos, target: TilePos, is_goal: &dyn Fn(TilePos) -> bool, blocked: &dyn Fn(i32, i32) -> bool) -> Option<Vec<TilePos>> {
        use super::occupancy::OCC_DROP;
        let occ = &self.loaded.occupancy;
        let is_drop = |t: TilePos| occ.has(t.0, t.1, OCC_DROP);
        let times = crate::srd::conditions::buffs(&self.world.effects).find_map(|b| if let crate::srd::conditions::Buff::Jump { times } = b { Some(*times) } else { None }).unwrap_or(1.0).max(1.0);
        let reach_ft = (super::movement::long_jump_ft(self.world.abilities.score(crate::srd::Ability::Str), true) as f32 * times) as u32;
        // Where the hero can walk (breadth first, as far as clicks look): tile -> steps.
        let mut dist: std::collections::HashMap<TilePos, u32> = std::collections::HashMap::from([(start, 0)]);
        let mut queue = VecDeque::from([start]);
        while let Some(t) = queue.pop_front() {
            if dist.len() > MAX_NODES {
                break;
            }
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (t.0 + d.0, t.1 + d.1);
                if !blocked(n.0, n.1) && !dist.contains_key(&n) {
                    dist.insert(n, dist[&t] + 1);
                    queue.push_back(n);
                }
            }
        }
        // The jumps from there: the edge, its run-up, the landing past the gap.
        let mut jumps: Vec<(u32, TilePos, TilePos, TilePos)> = Vec::new();
        for (&edge, &steps) in &dist {
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let first = (edge.0 + d.0, edge.1 + d.1);
                if !is_drop(first) || self.tile_solid(first.0, first.1) {
                    continue;
                }
                let (gap, land) = super::movement::gap_along(first, d, 8, is_drop);
                if gap as u32 * 5 > reach_ft || blocked(land.0, land.1) || !self.loaded.map.in_bounds(land.0, land.1) {
                    continue;
                }
                let run = (edge.0 - d.0 * RUN_UP_TILES, edge.1 - d.1 * RUN_UP_TILES);
                if (1..=RUN_UP_TILES).any(|k| !dist.contains_key(&(edge.0 - d.0 * k, edge.1 - d.1 * k))) {
                    continue;
                }
                let guess = steps + gap as u32 + ((land.0 - target.0).abs().max((land.1 - target.1).abs())) as u32;
                jumps.push((guess, run, edge, land));
            }
        }
        jumps.sort();
        for (_, run, edge, land) in jumps.into_iter().take(6) {
            let Some(after) = find_path(land, target, is_goal, blocked, MAX_NODES) else { continue };
            let mut path = find_path(start, run, |p| p == run, blocked, MAX_NODES)?;
            let d = ((edge.0 - run.0).signum(), (edge.1 - run.1).signum());
            path.extend((1..=RUN_UP_TILES).map(|k| (run.0 + d.0 * k, run.1 + d.1 * k)));
            path.push(land);
            path.extend(after);
            return Some(path);
        }
        None
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
            // A counter (a market stall) with someone keeping it: talk to them, across it.
            Some(i) if right => Goal::Interact(self.loaded.entities[self.counter_keeper(i, 2).unwrap_or(i)].uid),
            None if right => Goal::InteractTile(tile),
            _ => Goal::Walk(tile),
        };
        self.click_marker = Some((w, 0.0));
        // Already next to the target: no path needed. Someone keeping a stall: only from its
        // front, when that can be reached (else from wherever the hero can talk to them).
        let front = match goal {
            Goal::Interact(u) => self.uid_index(u).filter(|&i| self.keeps_counter(i) && !self.across_counter(i)).and_then(|i| self.plan(goal).filter(|p| self.ends_in_front(i, p)).map(|p| (i, p))),
            _ => None,
        };
        let ready = match goal {
            Goal::Interact(u) => front.is_none() && self.uid_index(u).is_some_and(|i| self.in_reach(i)),
            Goal::Attack(u) => self.uid_index(u).is_some_and(|i| self.starts_fight_with(i)),
            _ => false,
        };
        if right {
            self.mouse.hold = match goal {
                Goal::Interact(u) => Some(u),
                _ => None,
            };
        }
        let to_front = front.is_some();
        let path = if ready { Some(VecDeque::new()) } else { front.map(|(_, p)| p).or_else(|| self.plan(goal)) };
        match path {
            Some(path) => {
                self.order = Some(Order { front: to_front, ..Order::new(goal, path, self.player.pos) })
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

    /// Options › Accessibility › Hold to repeat: the right mouse button held on a tree or a rock
    /// keeps working it once the player stands there (like holding E).
    pub(super) fn mouse_hold_repeat(&mut self, dt: f32) {
        if !input::mouse_down(MouseButton::Right) {
            self.mouse.hold = None;
            return;
        }
        if !self.settings.hold_repeat || self.order.is_some() || self.ui.dialog_open() {
            return;
        }
        let Some(i) = self.mouse.hold.and_then(|u| self.uid_index(u)) else { return };
        if self.gatherable(i).is_none() || !self.in_reach(i) {
            return;
        }
        self.combat.hold_e -= dt;
        if self.combat.hold_e <= 0.0 {
            self.combat.hold_e = super::HOLD_REPEAT;
            let p = self.loaded.entities[i].body.pos;
            self.face_towards(p);
            self.interact_entity(i);
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
                if self.in_reach(i) && (!order.front || self.across_counter(i)) {
                    let p = self.loaded.entities[i].body.pos;
                    self.face_towards(p);
                    // A counter that does nothing itself (a market stall): talk to whoever
                    // stands behind it.
                    let i = if self.inert_prop(i) { self.behind_counter(i).unwrap_or(i) } else { i };
                    self.interact_entity(i);
                    self.combat.hold_e = super::HOLD_REPEAT;
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
                let can_hit = self.starts_fight_with(i);
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
            // (A ring on the ground.)
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
                self.upright(e.body.pos, || draw_rectangle_lines(r.x, r.y, r.w, r.h, u, c));
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
