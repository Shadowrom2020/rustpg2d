//! A map as the game draws it, for the map view: the painted ground, flat things on it, then
//! everything standing in depth order (isometric, or from straight above). Drawn with the
//! engine's renderer onto whatever is the screen (`gfx::with_screen`).

use gfx::prelude::*;
use rustpg2d_engine::defs::{Appearance, Database};
use rustpg2d_engine::entity::{EntityInstance, EntityKind, Facing};
use rustpg2d_engine::map::Map;
use rustpg2d_engine::render::model::{Gear, PoseReq};
use rustpg2d_engine::render::{self, iso, Assets};

/// Where a view looks and how: the camera of one frame.
#[derive(Clone, Copy)]
pub struct Look {
    /// The world point in the middle.
    pub center: Vec2,
    /// Screen pixels per world pixel (of something upright).
    pub zoom: f32,
    /// The picture's size in pixels.
    pub screen: Vec2,
    pub topdown: bool,
}

impl Look {
    pub fn iso(&self) -> Option<iso::View> {
        (!self.topdown).then_some(iso::View { target: self.center, scale: self.zoom / iso::UPRIGHT, screen: self.screen })
    }

    /// The camera for what lies on the ground.
    pub fn ground(&self) -> Camera2D {
        match self.iso() {
            Some(v) => v.ground(),
            None => Camera2D { target: self.center, zoom: vec2(2.0 * self.zoom / self.screen.x, 2.0 * self.zoom / self.screen.y), ..Default::default() },
        }
    }

    /// The pixel of the picture showing world point `p` (on the ground).
    pub fn to_screen(self, p: Vec2) -> Vec2 {
        match self.iso() {
            Some(v) => v.to_screen(p),
            None => (p - self.center) * self.zoom + self.screen / 2.0,
        }
    }

    /// The world point (on the ground) under pixel `s`.
    pub fn to_world(self, s: Vec2) -> Vec2 {
        match self.iso() {
            Some(v) => v.to_world(s),
            None => (s - self.screen / 2.0) / self.zoom + self.center,
        }
    }

    /// The bounding box (world) of the ground the picture shows.
    pub fn visible(&self) -> Rect {
        let s = self.screen;
        let ps = [vec2(0.0, 0.0), vec2(s.x, 0.0), vec2(0.0, s.y), s].map(|p| self.to_world(p));
        let (mut lo, mut hi) = (ps[0], ps[0]);
        for p in ps {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
    }

    /// How far the camera's middle moves when the picture is dragged by `d` pixels.
    pub fn pan(&self, d: Vec2) -> Vec2 {
        match self.iso() {
            Some(v) => iso::unproject(d) / v.scale,
            None => d / self.zoom,
        }
    }

    /// The zoom at which a map of `w` × `h` world pixels fills a picture of `screen` pixels.
    pub fn fit(w: f32, h: f32, screen: Vec2, topdown: bool) -> f32 {
        if topdown {
            (screen.x / w).min(screen.y / h) * 0.92
        } else {
            (screen.x / (w + h)).min(screen.y / ((w + h) * 0.5)) * 0.92 * iso::UPRIGHT
        }
    }
}

/// What drawing a map needs besides the map.
pub struct Scene<'a> {
    pub db: &'a Database,
    pub assets: &'a Assets,
    pub look: Look,
}

impl Scene<'_> {
    pub fn ts(&self) -> f32 {
        self.assets.tile_size
    }

    /// Runs `f` with the camera of something standing at `feet` (isometric: upright at its
    /// projected feet), then puts the ground's camera back.
    pub fn standing<R>(&self, feet: Vec2, f: impl FnOnce() -> R) -> R {
        let Some(v) = self.look.iso() else { return f() };
        set_camera(&v.upright(feet));
        let r = f();
        set_camera(&v.ground());
        r
    }

    fn char_look(&self, def: &str, inst: Option<&EntityInstance>) -> Option<(Appearance, Gear)> {
        use rustpg2d_engine::render::character;
        let d = self.db.character(def)?;
        let a = self.assets.models.look_of(character::appearance_of(d, inst));
        let gear = Gear::of(character::equipment_of(d, inst), "", None);
        Some((a, gear))
    }

    /// Draws a character standing with the bottom-center at `feet`.
    pub fn draw_char(&self, def: &str, inst: Option<&EntityInstance>, facing: Facing, feet: Vec2, tint: Color) -> bool {
        let Some((a, gear)) = self.char_look(def, inst) else { return false };
        let req = PoseReq { look: &a, gear: &gear, anim: "idle", time: 0.0, once: true, facing, turn: 0, bust: false };
        let Some(cell) = self.assets.models.pose(self.db, &self.assets.sheets, &req) else { return false };
        // (The race's size times the definition's own.)
        let own = self.db.character(def).map_or(1.0, |d| d.appearance.scale());
        self.assets.models.draw(&cell, feet, self.db.body_size(&a.model) * own, tint);
        true
    }

    fn char_size(&self, def: &str, inst: Option<&EntityInstance>) -> Vec2 {
        let ts = self.ts();
        self.char_look(def, inst).and_then(|(a, _)| self.assets.models.frame_size(&a.model)).map_or(vec2(ts, ts * 1.5), |(w, h)| vec2(w, h))
    }

    /// Draws a definition as it would stand at `feet` when placed (the Place tool's ghost).
    pub fn draw_def(&self, kind: EntityKind, def: &str, feet: Vec2, tint: Color) -> bool {
        match kind {
            EntityKind::Prop => self.db.prop(def).is_some_and(|d| self.assets.draw_prop(self.db, d, 0, Facing::Down, 0, feet, false, tint, 0.0, !self.look.topdown)),
            EntityKind::Character => self.draw_char(def, None, Facing::Down, feet, tint),
            EntityKind::Item => self.db.item(def).is_some_and(|d| self.assets.draw_sprite(&d.icon, 0, feet, tint)),
        }
    }

    /// The frame an entity is drawn in (world pixels around its feet, upright).
    pub fn entity_rect(&self, e: &EntityInstance) -> Rect {
        let ts = self.ts();
        let size = match e.kind {
            EntityKind::Prop => match self.db.prop(&e.def) {
                Some(d) => return self.assets.prop_rect(d, d.state_index(&e.state).unwrap_or(0), e.facing, vec2(e.x, e.y), ts, !self.look.topdown),
                None => vec2(ts, ts),
            },
            EntityKind::Character => self.char_size(&e.def, Some(e)),
            EntityKind::Item => self.db.item(&e.def).map_or(vec2(ts, ts), |d| self.assets.sprite_size(&d.icon, ts)),
        };
        Rect::new(e.x - size.x / 2.0, e.y - size.y, size.x, size.y)
    }

    /// What blocks the way of an entity (props, characters), around its feet.
    pub fn entity_hitbox(&self, e: &EntityInstance) -> Option<Rect> {
        let hb = match e.kind {
            EntityKind::Prop => self.db.prop(&e.def).map(|d| d.hitbox_facing(e.facing))?,
            EntityKind::Character => self.db.character(&e.def).map(|d| d.hitbox)?,
            EntityKind::Item => return None,
        };
        Some(Rect::new(e.x - hb[0] / 2.0, e.y - hb[1], hb[0], hb[1]))
    }

    /// The entities of `map` whose picture covers pixel `at` or that stand on the tile under
    /// it, the one in front first.
    pub fn entities_at(&self, map: &Map, at: Vec2) -> Vec<u32> {
        let ts = self.ts();
        let p = self.look.to_world(at);
        let tile = ((p.x / ts).floor() as i32, (p.y / ts).floor() as i32);
        let feet = |e: &EntityInstance| ((e.x / ts).floor() as i32, ((e.y - 0.5) / ts).floor() as i32);
        let hit = |e: &EntityInstance| {
            let prop = (e.kind == EntityKind::Prop).then(|| self.db.prop(&e.def)).flatten();
            self.covers(self.entity_rect(e), vec2(e.x, e.y), prop.is_some_and(|d| d.flat), prop.map(|d| d.hitbox_facing(e.facing)), at)
        };
        let mut hits: Vec<&EntityInstance> = map.entities.iter().filter(|e| hit(e)).collect();
        hits.sort_by(|a, b| (b.x + b.y).total_cmp(&(a.x + a.y)));
        for e in map.entities.iter().filter(|e| feet(e) == tile) {
            if !hits.iter().any(|x| x.uid == e.uid) {
                hits.push(e);
            }
        }
        hits.iter().map(|e| e.uid).collect()
    }

    /// The frame a piece of scenery is drawn in (world pixels around its feet, upright), and
    /// whether it lies flat on the ground.
    pub fn decor_rect(&self, map: &Map, d: &rustpg2d_engine::decor::Decor) -> (Rect, bool) {
        let ts = self.ts();
        let feet = vec2(d.x as f32, d.y as f32);
        match map.decor_defs.get(d.def as usize).and_then(|id| self.db.prop(id)) {
            Some(p) => (self.assets.prop_rect(p, d.state_index(), d.facing(), feet, ts, !self.look.topdown), p.flat || d.layer == rustpg2d_engine::decor::DecorLayer::Flat),
            None => (Rect::new(feet.x - 8.0, feet.y - 16.0, 16.0, 16.0), false),
        }
    }

    /// A frame around feet (as [`Self::entity_rect`] gives it) in the picture's pixels.
    fn screen_rect(&self, r: Rect, feet: Vec2, flat: bool) -> Rect {
        match self.look.iso() {
            Some(v) if !flat => {
                let (u, f) = (v.upright_scale(), v.to_screen(feet));
                Rect::new(f.x + (r.x - feet.x) * u, f.y + (r.y - feet.y) * u, r.w * u, r.h * u)
            }
            _ => {
                let ps = [vec2(r.x, r.y), vec2(r.x + r.w, r.y), vec2(r.x, r.y + r.h), vec2(r.x + r.w, r.y + r.h)].map(|p| self.look.to_screen(p));
                let (mut lo, mut hi) = (ps[0], ps[0]);
                for p in ps {
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
                Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
            }
        }
    }

    /// Whether pixel `at` is on something standing at `feet` with picture frame `r`. Small
    /// things are hit by their picture. Big ones (a house, a wall: `hitbox` of more than a
    /// tile) are hit by their shape instead, the footprint raised to the picture's height, so
    /// that the empty corners of a big picture's frame don't catch clicks meant for what
    /// stands beside it.
    fn covers(&self, r: Rect, feet: Vec2, flat: bool, hitbox: Option<[f32; 2]>, at: Vec2) -> bool {
        let big = self.ts() * 1.25;
        match (self.look.iso(), hitbox) {
            (Some(v), Some([w, h])) if !flat && (w >= big || h >= big) => {
                let print = Rect::new(feet.x - w / 2.0, feet.y - h, w, h);
                let lift = r.h * v.upright_scale();
                (0..=12).any(|k| print.contains(self.look.to_world(at + vec2(0.0, lift * k as f32 / 12.0))))
            }
            _ => self.screen_rect(r, feet, flat).contains(at),
        }
    }

    /// The scenery of `map` whose picture covers pixel `at`, the one in front first.
    pub fn scenery_at(&self, map: &Map, at: Vec2) -> Vec<rustpg2d_engine::decor::Decor> {
        let p = self.look.to_world(at);
        // (Pictures reach a few tiles from their feet at most.)
        let reach = rustpg2d_engine::decor::REACH + 8.0 * self.ts();
        let mut hits: Vec<&rustpg2d_engine::decor::Decor> = map
            .decor
            .iter()
            .filter(|d| (d.x as f32 - p.x).abs() < reach && (d.y as f32 - p.y).abs() < reach)
            .filter(|d| {
                let (r, flat) = self.decor_rect(map, d);
                let hitbox = map.decor_defs.get(d.def as usize).and_then(|id| self.db.prop(id)).map(|q| if q.turns() { q.hitbox_facing(d.facing()) } else { q.hitbox });
                self.covers(r, vec2(d.x as f32, d.y as f32), flat, hitbox, at)
            })
            .collect();
        // What stands in front first; what lies on the ground last.
        hits.sort_by(|a, b| (a.layer == rustpg2d_engine::decor::DecorLayer::Flat).cmp(&(b.layer == rustpg2d_engine::decor::DecorLayer::Flat)).then((b.x + b.y).cmp(&(a.x + a.y))));
        hits.into_iter().copied().collect()
    }

    fn draw_decor(&self, map: &Map, i: usize, tint: Color) {
        let d = &map.decor[i];
        let drawn = map.decor_defs.get(d.def as usize).and_then(|id| self.db.prop(id)).is_some_and(|p| self.assets.draw_prop(self.db, p, d.state_index(), d.facing(), 0, vec2(d.x as f32, d.y as f32), d.flip, tint, get_time(), !self.look.topdown));
        if !drawn {
            draw_rectangle(d.x as f32 - 8.0, d.y as f32 - 16.0, 16.0, 16.0, Color::new(1.0, 0.0, 1.0, 0.6));
        }
    }

    /// Draws `map`'s decorations and entities as the game does (flat ones on the ground, the rest
    /// upright in depth order, blocks as boxes, models and buildings), tinted by `tint`. `faint`:
    /// what hangs over everyone is drawn see-through (while painting under it).
    pub fn draw_objects(&self, map: &Map, view: Rect, overview: bool, tint: Color, faint: bool) {
        let ts = self.ts();
        use rustpg2d_engine::decor::DecorLayer;
        let reach = rustpg2d_engine::decor::REACH;
        let near_view = |x: f32, y: f32| x > view.x - reach && x < view.x + view.w + reach && y > view.y - reach && y < view.y + view.h + reach;
        let decor_in = |layer: DecorLayer| {
            let mut v: Vec<usize> = map.decor.iter().enumerate().filter(|(_, d)| d.layer == layer && near_view(d.x as f32, d.y as f32)).map(|(i, _)| i).collect();
            v.sort_by_key(|&i| map.decor[i].y);
            v
        };
        if !overview {
            for i in decor_in(DecorLayer::Flat) {
                self.draw_decor(map, i, tint);
            }
        }
        // Flat props and flat decorations lie on the ground under everyone; the rest stand, in
        // depth order (isometric: `iso::order` with footprints, as the game draws them; from
        // above: by y), each drawn upright where its feet are.
        let db = self.db;
        let iso = self.look.iso();
        let near = if iso.is_some() { Rect::new(view.x - 6.0 * ts, view.y - 6.0 * ts, view.w + 8.0 * ts, view.h + 8.0 * ts) } else { Rect::new(view.x - 64.0, view.y - 64.0, view.w + 128.0, view.h + 128.0) };
        let ents: Vec<&EntityInstance> = map.entities.iter().filter(|e| near.contains(vec2(e.x, e.y))).collect();
        let flat = |e: &EntityInstance| e.kind == EntityKind::Prop && db.prop(&e.def).is_some_and(|d| d.flat);
        let big = ts * 1.25;
        let footprint = |feet: Vec2, hitbox: Option<[f32; 2]>| match hitbox {
            Some([w, h]) if w >= big || h >= big => iso::Footprint::at_feet(feet, w, h),
            _ => iso::Footprint::at_feet(feet, 0.0, 0.0),
        };
        enum Drawn<'a> {
            Ent(&'a EntityInstance),
            Decor(usize),
        }
        let mut flats: Vec<&EntityInstance> = ents.iter().copied().filter(|e| flat(e)).collect();
        flats.sort_by(|a, b| a.y.total_cmp(&b.y));
        let mut items: Vec<(Drawn, Vec2, iso::Footprint)> = Vec::new();
        if ents.len() < 5000 {
            for e in ents.iter().copied().filter(|e| !flat(e)) {
                let feet = vec2(e.x, e.y);
                let prop = (e.kind == EntityKind::Prop).then(|| db.prop(&e.def)).flatten();
                let hitbox = prop.map(|d| d.hitbox_facing(e.facing));
                let print = match (prop, hitbox) {
                    (Some(d), Some(hb)) if d.block > 0.0 => iso::Footprint::at_feet(feet, hb[0], hb[1]),
                    _ => footprint(feet, hitbox),
                };
                items.push((Drawn::Ent(e), feet, print));
            }
        } else {
            flats.clear();
        }
        if !overview {
            for i in decor_in(DecorLayer::Sorted) {
                let d = &map.decor[i];
                let prop = map.decor_defs.get(d.def as usize).and_then(|id| db.prop(id));
                let feet = vec2(d.x as f32, d.y as f32);
                if prop.is_some_and(|p| p.flat) {
                    self.draw_decor(map, i, tint);
                } else {
                    let hitbox = prop.map(|p| if p.turns() { p.hitbox_facing(d.facing()) } else { p.hitbox });
                    let print = match (prop, hitbox) {
                        (Some(p), Some(hb)) if p.block > 0.0 => iso::Footprint::at_feet(feet, hb[0], hb[1]),
                        _ => footprint(feet, hitbox),
                    };
                    items.push((Drawn::Decor(i), feet, print));
                }
            }
        }
        let draw_ent = |e: &EntityInstance| {
            let col = if e.active { tint } else { Color { a: tint.a * 0.4, ..tint } };
            let pos = vec2(e.x, e.y);
            let drawn = match e.kind {
                EntityKind::Prop => db.prop(&e.def).is_some_and(|d| self.assets.draw_prop(db, d, d.state_index(&e.state).unwrap_or(0), e.facing, e.frame, pos, false, col, 0.0, iso.is_some())),
                EntityKind::Character => self.draw_char(&e.def, Some(e), e.facing, pos, col),
                EntityKind::Item => db.item(&e.def).is_some_and(|d| self.assets.draw_sprite(&d.icon, 0, pos, col)),
            };
            if !drawn {
                draw_rectangle(e.x - ts / 2.0, e.y - ts, ts, ts, Color::new(1.0, 0.0, 1.0, 0.6));
            }
        };
        for e in flats {
            draw_ent(e);
        }
        let order: Vec<usize> = if iso.is_some() {
            iso::order(&items.iter().map(|it| it.2).collect::<Vec<_>>(), 0.5)
        } else {
            let mut o: Vec<usize> = (0..items.len()).collect();
            o.sort_by(|&a, &b| items[a].1.y.total_cmp(&items[b].1.y));
            o
        };
        // The ground shadows of props (isometric): soft patches under 3D ones, then the shadows
        // of those drawn as pictures, under everything standing.
        if iso.is_some() {
            for (what, feet, _) in &items {
                match what {
                    Drawn::Ent(e) if e.kind == EntityKind::Prop => {
                        if let Some(p) = db.prop(&e.def) {
                            self.assets.draw_prop_contact(p, p.state_index(&e.state).unwrap_or(0), e.facing, *feet, tint.a);
                        }
                    }
                    Drawn::Decor(i) => {
                        let d = &map.decor[*i];
                        if let Some(p) = map.decor_defs.get(d.def as usize).and_then(|id| db.prop(id)) {
                            self.assets.draw_prop_contact(p, d.state_index(), d.facing(), *feet, tint.a);
                        }
                    }
                    _ => {}
                }
            }
            for (what, feet, _) in &items {
                let (p, state, facing, flip) = match what {
                    Drawn::Ent(e) if e.kind == EntityKind::Prop => match db.prop(&e.def) {
                        Some(p) => (p, p.state_index(&e.state).unwrap_or(0), e.facing, false),
                        None => continue,
                    },
                    Drawn::Decor(i) => {
                        let d = &map.decor[*i];
                        match map.decor_defs.get(d.def as usize).and_then(|id| db.prop(id)) {
                            Some(p) => (p, d.state_index(), d.facing(), d.flip),
                            None => continue,
                        }
                    }
                    _ => continue,
                };
                self.standing(*feet, || self.assets.draw_prop_shadow(p, state, facing, *feet, flip, 0.0, tint.a));
            }
        }
        // Walls and counters drawn as blocks (isometric): boxes on their hitbox, at full height.
        let block = |p: &rustpg2d_engine::defs::PropDef, state: usize, facing, feet: Vec2| -> bool {
            let Some(v) = iso.filter(|_| p.block > 0.0) else { return false };
            set_default_camera();
            self.assets.draw_prop_block(&v, p, state, facing, feet, p.block, tint);
            set_camera(&v.ground());
            true
        };
        for k in order {
            let (what, feet, _) = &items[k];
            let blocked = match what {
                Drawn::Ent(e) => e.kind == EntityKind::Prop && db.prop(&e.def).is_some_and(|d| block(d, d.state_index(&e.state).unwrap_or(0), e.facing, *feet)),
                Drawn::Decor(i) => {
                    let d = &map.decor[*i];
                    map.decor_defs.get(d.def as usize).and_then(|id| db.prop(id)).is_some_and(|p| block(p, d.state_index(), if p.turns() { d.facing() } else { Facing::Down }, *feet))
                }
            };
            if blocked {
                continue;
            }
            self.standing(*feet, || match what {
                Drawn::Ent(e) => draw_ent(e),
                Drawn::Decor(i) => self.draw_decor(map, *i, tint),
            });
        }
        if !overview {
            let over = if faint { Color { a: tint.a * 0.6, ..tint } } else { tint };
            for i in decor_in(DecorLayer::Above) {
                self.standing(vec2(map.decor[i].x as f32, map.decor[i].y as f32), || self.draw_decor(map, i, over));
            }
        }
    }

    /// The lights of `map` for the lighting preview: decorations, placed props, characters and
    /// items.
    pub fn lights(&self, map: &Map) -> Vec<render::lighting::Light> {
        use render::lighting::Light;
        let ts = self.ts();
        let db = self.db;
        let mut out = Vec::new();
        for d in &map.decor {
            let Some(p) = map.decor_defs.get(d.def as usize).and_then(|id| db.prop(id)) else { continue };
            if let Some(l) = p.state_light(d.state_index()) {
                let lift = self.assets.sprite_size(p.state_look(d.state_index()).0, ts).y * 0.65;
                out.push(Light::new(vec2(d.x as f32, d.y as f32 - lift), l));
            }
        }
        for e in map.entities.iter().filter(|e| e.active) {
            let (spec, lift) = match e.kind {
                EntityKind::Prop => {
                    let Some(d) = db.prop(&e.def) else { continue };
                    let i = d.state_index(&e.state).unwrap_or(0);
                    let lift = self.assets.sprite_size(d.state_look(i).0, ts).y * 0.65;
                    (e.light.clone().or_else(|| d.state_light(i).cloned()), lift)
                }
                EntityKind::Character => (e.light.clone().or_else(|| db.character(&e.def).and_then(|d| d.light.clone())), ts * 0.6),
                EntityKind::Item => (e.light.clone().or_else(|| db.item(&e.def).and_then(|d| d.light.clone())), ts * 0.35),
            };
            if let Some(l) = spec {
                out.push(Light::new(vec2(e.x, e.y - lift), &l));
            }
        }
        out
    }
}

/// The colour a kind of zone is shown in.
pub fn zone_color(k: &rustpg2d_engine::map::ZoneKind) -> Color {
    use rustpg2d_engine::map::ZoneKind;
    match k {
        ZoneKind::Spawn => Color::new(0.3, 0.8, 1.0, 1.0),
        ZoneKind::Portal { .. } => Color::new(0.8, 0.4, 1.0, 1.0),
        ZoneKind::Transition { .. } => Color::new(1.0, 0.6, 0.1, 1.0),
        ZoneKind::Trigger => Color::new(1.0, 0.65, 0.2, 1.0),
        ZoneKind::Spawner { .. } => Color::new(1.0, 0.25, 0.3, 1.0),
        ZoneKind::Encounter { .. } => Color::new(0.85, 0.15, 0.55, 1.0),
        ZoneKind::FishingSpot { .. } => Color::new(0.2, 0.55, 1.0, 1.0),
        ZoneKind::Ambience { .. } => Color::new(0.45, 0.9, 0.55, 1.0),
        ZoneKind::Drop { .. } => Color::new(0.55, 0.35, 0.2, 1.0),
        ZoneKind::Trap { .. } => Color::new(0.9, 0.15, 0.15, 1.0),
    }
}
