//! Map tools on the main thread: reading maps as text / images and editing them through the
//! `MapDoc` (undoable strokes and object edits), screenshots of the canvas.

use rustpg2d_engine::autotile;
use rustpg2d_engine::entity::{EntityInstance, EntityKind, Facing};
use rustpg2d_engine::events::{Trigger, TriggerOwner};
use rustpg2d_engine::map::{CollisionOverride, Map, Tile, Zone, ZoneKind, MAX_MAP_SIZE};
use rustpg2d_engine::refactor::RefKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app::{Editor, ViewOpts};
use crate::doc::Objects;

use super::exec::{json, suggestions, text, Reply, ShotSpec};
use super::tools::*;
use super::{grid, patch, ShotRequest};

/// Cells one terrain / collision call may touch.
const MAX_CELLS: usize = 4000;

/// Who may carry which triggers.
pub(crate) fn owner_of(kind: EntityKind) -> TriggerOwner {
    match kind {
        EntityKind::Character => TriggerOwner::Character,
        EntityKind::Prop => TriggerOwner::Prop,
        EntityKind::Item => TriggerOwner::Pickup,
    }
}

pub(crate) fn parse_kind(s: &str) -> Result<EntityKind, String> {
    match s.to_lowercase().as_str() {
        "prop" | "props" => Ok(EntityKind::Prop),
        "character" | "characters" | "npc" => Ok(EntityKind::Character),
        "item" | "items" | "pickup" => Ok(EntityKind::Item),
        _ => Err(format!("kind \"{s}\" must be prop, character or item")),
    }
}

pub(crate) fn parse_facing(s: &str) -> Result<Facing, String> {
    match s.to_lowercase().as_str() {
        "down" => Ok(Facing::Down),
        "left" => Ok(Facing::Left),
        "right" => Ok(Facing::Right),
        "up" => Ok(Facing::Up),
        _ => Err(format!("facing \"{s}\" must be down, left, right or up")),
    }
}

pub(crate) fn parse_ref_kind(s: &str) -> Result<RefKind, String> {
    Ok(match s.to_lowercase().trim_end_matches('s') {
        "tileset" => RefKind::Tileset,
        "sheet" => RefKind::Sheet,
        "item" => RefKind::Item,
        "prop" => RefKind::Prop,
        "character" => RefKind::Character,
        "sound" => RefKind::Sound,
        "map" => RefKind::Map,
        "structure" => RefKind::Structure,
        "skill" => RefKind::Skill,
        "spell" => RefKind::Spell,
        "recipe" => RefKind::Recipe,
        _ => return Err(format!("kind \"{s}\" must be one of tileset, sheet, item, prop, character, sound, map, structure, skill, spell, recipe")),
    })
}

/// A zone's parsed kind, triggers, loot and the warnings found.
type ZoneParts = (ZoneKind, Vec<Trigger>, Option<Vec<rustpg2d_engine::defs::LootEntry>>, Vec<String>);

/// The map settings agents patch (`set_map_settings`): the non-tile, non-object parts of a map.
#[derive(Serialize, Deserialize)]
struct MapSettings {
    background: [u8; 3],
    music: String,
    lighting: rustpg2d_engine::map::MapLighting,
    fog: bool,
    weather: rustpg2d_engine::map::Weather,
    parallax: Vec<rustpg2d_engine::map::ParallaxLayer>,
    edges: rustpg2d_engine::map::EdgeLinks,
}

impl Editor {
    /// For reads: the named map (the open document's copy or loaded from disk), else the open map.
    pub(crate) fn mcp_read_map(&self, name: &Option<String>) -> Result<Map, String> {
        self.mcp_need_project()?;
        match name {
            Some(n) => {
                if !self.maps.contains(n) {
                    return Err(format!("\"{n}\" is not a map.{}", suggestions(n, self.maps.iter().map(String::as_str))));
                }
                self.mcp_load_map(n)
            }
            None => self.doc.as_ref().map(|d| d.map.clone()).ok_or_else(|| "no map is open in the editor: give `map`".to_string()),
        }
    }

    /// For writes: makes the named map the open one (saving the current one first), or keeps the
    /// open map. Afterwards `self.doc` is that map.
    pub(crate) fn mcp_target_map(&mut self, name: &Option<String>) -> Result<(), String> {
        self.mcp_writable()?;
        match name {
            Some(n) if self.doc.as_ref().is_none_or(|d| &d.map.name != n) => {
                if !self.maps.contains(n) {
                    return Err(format!("\"{n}\" is not a map.{}", suggestions(n, self.maps.iter().map(String::as_str))));
                }
                self.save_all();
                self.open_map(n);
                if self.doc.as_ref().is_none_or(|d| &d.map.name != n) {
                    return Err(format!("the editor could not open {n}: {}", self.status.0));
                }
                Ok(())
            }
            Some(_) => Ok(()),
            None if self.doc.is_some() => Ok(()),
            None => Err("no map is open in the editor: give `map`".into()),
        }
    }

    /// Legend text for a tile: its terrain, solid, animated, light.
    fn describe_tile(&self, tileset: &str, idx: u32) -> String {
        let mut parts = Vec::new();
        if let Some(def) = self.project.db.tileset(tileset) {
            if let Some(k) = autotile::terrain_of(&def.terrains, idx) {
                parts.push(format!("terrain \"{}\"", def.terrains[k].name));
            }
            if def.animations.iter().any(|a| a.tile == idx) {
                parts.push("animated".into());
            }
            if def.lights.iter().any(|l| l.tile == idx) {
                parts.push("light".into());
            }
        }
        if self.assets.tilesets.get(tileset).is_some_and(|t| t.is_solid(idx)) {
            parts.push("solid".into());
        }
        parts.join(" ")
    }

    fn tileset_slot_checked(&self, map: &Map, tileset: &str) -> Result<(u16, u32), String> {
        let tex = self.assets.tilesets.get(tileset).ok_or_else(|| format!("\"{tileset}\" is not a tileset of the project.{}", suggestions(tileset, self.project.db.tilesets.iter().map(|t| t.id.as_str()))))?;
        let slot = map.tilesets.iter().position(|t| t == tileset).map(|p| p as u16).unwrap_or(map.tilesets.len() as u16);
        Ok((slot, tex.count))
    }

    pub(crate) fn mcp_list_maps(&mut self) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let cfg_start = self.project.config.start_map.clone();
        let mut lines = Vec::new();
        for m in self.maps.clone() {
            match self.mcp_load_map(&m) {
                Ok(map) => {
                    let spawns: Vec<String> = map.zones.iter().filter(|z| z.kind == ZoneKind::Spawn).map(|z| z.name.clone()).collect();
                    let mut links: Vec<String> = map
                        .zones
                        .iter()
                        .filter_map(|z| match &z.kind {
                            ZoneKind::Transition { target_map, target_zone } => Some(format!("{target_map}/{target_zone}")),
                            ZoneKind::Portal { target_map, target_spawn } => Some(format!("{target_map}/{target_spawn} (portal)")),
                            _ => None,
                        })
                        .collect();
                    for (side, e) in [("N", &map.edges.north), ("S", &map.edges.south), ("E", &map.edges.east), ("W", &map.edges.west)] {
                        if let Some(e) = e {
                            links.push(format!("{side} edge -> {}", e.target_map));
                        }
                    }
                    lines.push(format!(
                        "{m}: {}x{}{}; tilesets [{}]; {} layers; {} entities, {} zones; spawns [{}]; leads to [{}]{}",
                        map.width,
                        map.height,
                        if m == cfg_start { " (start map)" } else { "" },
                        map.tilesets.join(", "),
                        map.layers.len(),
                        map.entities.len(),
                        map.zones.len(),
                        spawns.join(", "),
                        links.join(", "),
                        if self.doc.as_ref().is_some_and(|d| d.map.name == m) { " [open in the editor]" } else { "" }
                    ));
                }
                Err(e) => lines.push(format!("{m}: cannot load ({e})")),
            }
        }
        if lines.is_empty() {
            lines.push("(no maps yet: create_map)".into());
        }
        text(lines.join("\n"))
    }

    pub(crate) fn mcp_get_map(&mut self, p: GetMapParams) -> Result<Reply, String> {
        let map = self.mcp_read_map(&p.map)?;
        let r = grid::region(map.width, map.height, p.x, p.y, p.w, p.h)?;
        let mut out = format!(
            "map \"{}\" {}x{} tiles; layers: {}; tilesets: {}; music {}; lighting {:?}; weather {:?}; fog of war {}; map triggers {}\n",
            map.name,
            map.width,
            map.height,
            map.layers.iter().enumerate().map(|(i, l)| format!("{i}={}", l.name)).collect::<Vec<_>>().join(", "),
            map.tilesets.iter().enumerate().map(|(i, t)| format!("slot {i}={t}")).collect::<Vec<_>>().join(", "),
            if map.music.is_empty() { "-" } else { &map.music },
            map.lighting.mode,
            map.weather.kind,
            map.fog,
            map.triggers.len()
        );
        let layers: Vec<usize> = p.layers.unwrap_or_else(|| (0..map.layers.len()).collect());
        let describe = |ts: &str, idx: u32| self.describe_tile(ts, idx);
        for l in layers {
            out += &grid::render_layer(&map, l, &r, &describe);
        }
        if p.collision.unwrap_or(true) {
            let solid = self.assets.compute_solid(&map);
            let w = map.width as usize;
            out += &grid::render_collision(&map, &r, &|x, y| solid.get(y as usize * w + x as usize).copied().unwrap_or(false));
        }
        if p.objects.unwrap_or(true) {
            out += &grid::render_objects(&map, &r, self.ts());
        }
        text(out)
    }

    pub(crate) fn mcp_get_object(&mut self, p: ObjectParams) -> Result<Reply, String> {
        let map = self.mcp_read_map(&p.map)?;
        if let Some(e) = map.entities.iter().find(|e| e.uid == p.uid) {
            return text(format!("entity (update_entity patches this):\n{}", json(&patch::entity_json(e, self.ts()))));
        }
        if let Some(z) = map.zones.iter().find(|z| z.uid == p.uid) {
            return text(format!("zone (update_zone patches this):\n{}", json(z)));
        }
        Err(format!("no entity or zone with uid {} on {} (get_map lists them)", p.uid, map.name))
    }

    pub(crate) fn mcp_tileset_legend(&mut self, p: TilesetParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let def = self.project.db.tileset(&p.tileset).ok_or_else(|| format!("\"{}\" is not a tileset.{}", p.tileset, suggestions(&p.tileset, self.project.db.tilesets.iter().map(|t| t.id.as_str()))))?;
        let tex = self.assets.tilesets.get(&p.tileset);
        let mut out = format!("tileset \"{}\": image {}", def.id, def.image);
        match tex {
            Some(t) => out += &format!(" ({}x{} px), {} columns, {} tiles (index = row * {} + column; view_image shows them)\n", t.image.width, t.image.height, t.columns, t.count, t.columns),
            None => out += " (image not loaded)\n",
        }
        out += &format!("solid tiles: {}\n", if def.solid.is_empty() { "none".to_string() } else { def.solid.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ") });
        if !def.ladder.is_empty() {
            out += &format!("ladder tiles (the player climbs them, characters keep off): {}\n", def.ladder.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", "));
        }
        if !def.animations.is_empty() {
            out += &format!("animations: {}\n", def.animations.iter().map(|a| format!("{} -> [{}] @{} fps", a.tile, a.frames.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","), a.fps)).collect::<Vec<_>>().join("; "));
        }
        if !def.lights.is_empty() {
            out += &format!("light tiles: {}\n", def.lights.iter().map(|l| format!("{} (radius {}, color {:?})", l.tile, l.light.radius, l.light.color)).collect::<Vec<_>>().join("; "));
        }
        if def.terrains.is_empty() {
            out += "terrains: none (paint_terrain needs one; set_tiles / paint_grid work with indices)\n";
        } else {
            out += "terrains (paint_terrain name):\n";
            for t in &def.terrains {
                let filled = t.tiles.iter().filter(|s| s.is_some()).count();
                out += &format!(
                    "  \"{}\": inside tile {}, {} of 16 edge tiles{}{}\n",
                    t.name,
                    t.base.map_or("-".to_string(), |b| b.to_string()),
                    filled,
                    if t.blob.iter().any(|b| b.is_some()) { ", corner set (47 tiles)" } else { "" },
                    if t.joins.is_empty() { String::new() } else { format!(", joins {}", t.joins.join(", ")) }
                );
            }
        }
        text(out)
    }

    pub(crate) fn mcp_view_image(&mut self, p: ViewImageParams) -> Result<Reply, String> {
        self.mcp_need_project()?;
        let (path, cell, what) = match p.kind.to_lowercase().as_str() {
            "tileset" | "tilesets" => {
                let d = self.project.db.tileset(&p.id).ok_or_else(|| format!("\"{}\" is not a tileset.{}", p.id, suggestions(&p.id, self.project.db.tilesets.iter().map(|t| t.id.as_str()))))?;
                (d.image.clone(), (rustpg2d_engine::project::TILE_SIZE, rustpg2d_engine::project::TILE_SIZE), format!("tileset {} (32 px tiles; index = row * columns + column)", d.id))
            }
            "sheet" | "sheets" => {
                let d = self.project.db.sheet(&p.id).ok_or_else(|| format!("\"{}\" is not a sprite sheet.{}", p.id, suggestions(&p.id, self.project.db.sheets.iter().map(|t| t.id.as_str()))))?;
                (d.image.clone(), (d.frame_w, d.frame_h), format!("sprite sheet {} ({:?}, {}x{} px frames; frame = row * columns + column)", d.id, d.kind, d.frame_w, d.frame_h))
            }
            k => return Err(format!("kind \"{k}\" must be tileset or sheet")),
        };
        let bytes = std::fs::read(self.project.root.join(&path)).map_err(|e| format!("cannot read {path}: {e}"))?;
        let mut img = rustpg2d_engine::imageio::Rgba::decode(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let (cw, ch) = (cell.0.max(1), cell.1.max(1));
        let columns = img.w / cw;
        let mut note = format!("{what}: {}x{} px, {} columns x {} rows", img.w, img.h, columns, img.h / ch);
        if p.grid.unwrap_or(true) && img.w <= 2048 && img.h <= 2048 {
            // Scale up small images so the grid lines don't eat tile pixels.
            let scale = if img.w.max(img.h) <= 512 { 3 } else if img.w.max(img.h) <= 1024 { 2 } else { 1 };
            if scale > 1 {
                img = img.resized(img.w * scale, img.h * scale);
            }
            let (cw, ch) = (cw * scale, ch * scale);
            for y in 0..img.h {
                for x in 0..img.w {
                    if x % cw == 0 || y % ch == 0 {
                        let on = (x / 2 + y / 2) % 2 == 0;
                        img.set(x as i32, y as i32, if on { [255, 0, 255, 255] } else { [0, 0, 0, 255] });
                    }
                }
            }
            note += &format!("; magenta/black grid = cell borders (image shown {scale}x)");
        }
        let png = super::play::downscale(&img.encode().map_err(|e| e.to_string())?)?;
        Ok(Reply::TextImages(note, vec![png]))
    }

    pub(crate) fn mcp_screenshot_map(&mut self, p: ScreenshotParams) -> Result<ShotSpec, String> {
        self.mcp_target_map(&p.map)?;
        let (w, h) = self.doc.as_ref().map(|d| (d.map.width, d.map.height)).unwrap_or((1, 1));
        let (x, y) = (p.x.unwrap_or(0).min(w - 1), p.y.unwrap_or(0).min(h - 1));
        let rw = p.w.unwrap_or(w - x).clamp(1, w - x);
        let rh = p.h.unwrap_or(h - y).clamp(1, h - y);
        let name = self.doc.as_ref().map(|d| d.map.name.clone()).unwrap_or_default();
        Ok(ShotSpec { rect: Some((x, y, rw, rh)), zoom: p.zoom, grid: p.grid, collision: p.collision, zones: p.zones, text: format!("map {name}, tiles x{x}..{} y{y}..{}", x + rw - 1, y + rh - 1) })
    }

    /// Frames the region for a screenshot; returns what to restore afterwards.
    pub(crate) fn mcp_prepare_shot(&mut self, spec: &ShotSpec) -> Option<(macroquad::math::Vec2, f32, ViewOpts)> {
        let (x, y, w, h) = spec.rect?;
        let restore = (self.cam.center, self.cam.zoom, self.view);
        let ts = self.ts();
        self.cam.center = macroquad::math::vec2((x as f32 + w as f32 / 2.0) * ts, (y as f32 + h as f32 / 2.0) * ts);
        let r = self.canvas_rect;
        let (cw, ch) = if r.is_positive() && r.width() > 50.0 { (r.width(), r.height()) } else { (macroquad::window::screen_width() * 0.6, macroquad::window::screen_height() * 0.8) };
        self.cam.zoom = spec.zoom.unwrap_or_else(|| ((cw / (w as f32 * ts)).min(ch / (h as f32 * ts)) * 0.95).clamp(0.02, 16.0));
        if r.is_positive() {
            let sc = macroquad::math::vec2(macroquad::window::screen_width() / 2.0, macroquad::window::screen_height() / 2.0);
            let off = (macroquad::math::vec2(r.center().x, r.center().y) - sc) / self.cam.zoom;
            self.cam.center -= off;
        }
        if let Some(g) = spec.grid {
            self.view.grid = g;
        }
        if let Some(c) = spec.collision {
            self.view.collision = c;
        }
        if let Some(z) = spec.zones {
            self.view.zones = z;
        }
        Some(restore)
    }

    /// After the frame was drawn: takes the screenshots whose frames came, replies, restores.
    pub fn mcp_capture_shots(&mut self) {
        if self.mcp.shots.is_empty() {
            return;
        }
        let mut shots = std::mem::take(&mut self.mcp.shots);
        let mut keep = Vec::new();
        for mut s in shots.drain(..) {
            s.frames_left = s.frames_left.saturating_sub(1);
            if s.frames_left > 0 {
                keep.push(s);
                continue;
            }
            let result = self.mcp_take_shot(&s);
            if let Some((center, zoom, view)) = s.restore {
                self.cam.center = center;
                self.cam.zoom = zoom;
                self.view = view;
            }
            let ok = result.is_ok();
            let ShotRequest { reply, started, summary, .. } = s;
            let _ = reply.send(result);
            self.mcp.log_call(if summary.is_empty() { "screenshot_editor" } else { "screenshot_map" }, summary, ok, started);
        }
        self.mcp.shots = keep;
    }

    fn mcp_take_shot(&self, s: &ShotRequest) -> Result<Reply, String> {
        let img = macroquad::prelude::get_screen_data();
        let (w, h) = (img.width as u32, img.height as u32);
        let scale = macroquad::window::screen_dpi_scale();
        // Rows come bottom-up from the GPU.
        let mut out = rustpg2d_engine::imageio::Rgba::new(w, h);
        for y in 0..h {
            let src = (h - 1 - y) as usize * w as usize * 4;
            for x in 0..w as usize {
                let p = &img.bytes[src + x * 4..src + x * 4 + 4];
                out.set(x as i32, y as i32, [p[0], p[1], p[2], 255]);
            }
        }
        if s.spec.rect.is_some() && self.canvas_rect.is_positive() {
            let r = self.canvas_rect;
            let (x0, y0) = (((r.min.x * scale) as u32).min(w), ((r.min.y * scale) as u32).min(h));
            let (x1, y1) = (((r.max.x * scale) as u32).min(w), ((r.max.y * scale) as u32).min(h));
            if x1 > x0 && y1 > y0 {
                let mut c = rustpg2d_engine::imageio::Rgba::new(x1 - x0, y1 - y0);
                for y in y0..y1 {
                    for x in x0..x1 {
                        c.set((x - x0) as i32, (y - y0) as i32, out.get(x as i32, y as i32));
                    }
                }
                out = c;
            }
        }
        let png = super::play::downscale(&out.encode().map_err(|e| e.to_string())?)?;
        Ok(Reply::TextImages(format!("{} ({}x{} px)", s.spec.text, out.w, out.h), vec![png]))
    }

    // ------------------------------------------------------------ writes

    pub(crate) fn mcp_create_map(&mut self, p: CreateMapParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        if !crate::app::valid_name(&p.name) {
            return Err("map names use letters, digits, '_' and '-'".into());
        }
        if self.maps.contains(&p.name) {
            return Err(format!("a map named \"{}\" exists", p.name));
        }
        if p.width == 0 || p.height == 0 || p.width > MAX_MAP_SIZE || p.height > MAX_MAP_SIZE {
            return Err(format!("the size must be 1..{MAX_MAP_SIZE} tiles each way"));
        }
        let mut map = Map::new(&p.name, p.width, p.height);
        if let Some(f) = &p.fill {
            let (_, count) = self.tileset_slot_checked(&map, &f.tileset)?;
            if f.index >= count {
                return Err(format!("tile index {} is outside {} ({} tiles)", f.index, f.tileset, count));
            }
            let slot = map.tileset_slot(&f.tileset);
            let t = Tile::new(slot, f.index);
            for y in 0..map.height {
                for x in 0..map.width {
                    map.layers[0].set(x, y, t);
                }
            }
        }
        self.save_all();
        self.doc = None;
        self.load_doc(map);
        self.save_map();
        self.mcp_after_write(format!("Created map \"{}\" ({}x{}) and opened it; layers 0=Ground 1=Detail 2=Overhead.", p.name, p.width, p.height))
    }

    /// Paints `cells` (x, y, tile) on `layer` as one stroke.
    fn paint_cells(&mut self, layer: usize, cells: &[(i32, i32, Tile)]) -> Result<usize, String> {
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if layer >= doc.map.layers.len() {
            return Err(format!("layer {layer} doesn't exist: the map has layers 0..{}", doc.map.layers.len() - 1));
        }
        let mut n = 0;
        doc.begin_stroke(layer, false);
        for (x, y, t) in cells {
            if doc.map.in_bounds(*x, *y) {
                doc.paint(layer, *x, *y, *t, assets);
                n += 1;
            }
        }
        doc.end_stroke();
        Ok(n)
    }

    pub(crate) fn mcp_set_tiles(&mut self, p: SetTilesParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let map = self.doc.as_ref().unwrap().map.clone();
        let (slot, count) = self.tileset_slot_checked(&map, &p.tileset)?;
        let mut cells = Vec::new();
        for c in &p.cells {
            let t = match c[2] {
                i if i < 0 => Tile::EMPTY,
                i if (i as u32) < count => Tile::new(slot, i as u32),
                i => return Err(format!("tile index {i} is outside {} ({} tiles)", p.tileset, count)),
            };
            cells.push((c[0] as i32, c[1] as i32, t));
        }
        if let Some(d) = &mut self.doc {
            d.map.tileset_slot(&p.tileset);
        }
        let n = self.paint_cells(p.layer, &cells)?;
        self.mcp_after_write(format!("Set {n} of {} tiles on layer {} of \"{}\".", p.cells.len(), p.layer, map.name))
    }

    pub(crate) fn mcp_paint_grid(&mut self, p: PaintGridParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let map = self.doc.as_ref().unwrap().map.clone();
        let (slot, count) = self.tileset_slot_checked(&map, &p.tileset)?;
        let parsed = grid::parse_grid(&p.rows, &p.legend)?;
        let mut cells = Vec::new();
        for (dx, dy, t) in parsed {
            let tile = match t {
                None => Tile::EMPTY,
                Some(i) if i < count => Tile::new(slot, i),
                Some(i) => return Err(format!("tile index {i} is outside {} ({} tiles)", p.tileset, count)),
            };
            cells.push((p.x + dx as i32, p.y + dy as i32, tile));
        }
        if let Some(d) = &mut self.doc {
            d.map.tileset_slot(&p.tileset);
        }
        let n = self.paint_cells(p.layer, &cells)?;
        self.mcp_after_write(format!("Painted {n} tiles on layer {} of \"{}\" from a {}-row grid at {},{}.", p.layer, map.name, p.rows.len(), p.x, p.y))
    }

    pub(crate) fn mcp_fill_rect(&mut self, p: FillRectParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let map = self.doc.as_ref().unwrap().map.clone();
        let (slot, count) = self.tileset_slot_checked(&map, &p.tileset)?;
        let tile = match p.index {
            i if i < 0 => Tile::EMPTY,
            i if (i as u32) < count => Tile::new(slot, i as u32),
            i => return Err(format!("tile index {i} is outside {} ({} tiles)", p.tileset, count)),
        };
        if (p.w * p.h) as usize > MAX_CELLS * 4 {
            return Err(format!("at most {} tiles per call", MAX_CELLS * 4));
        }
        let cells: Vec<(i32, i32, Tile)> = (p.y..p.y + p.h).flat_map(|y| (p.x..p.x + p.w).map(move |x| (x as i32, y as i32, tile))).collect();
        if let Some(d) = &mut self.doc {
            d.map.tileset_slot(&p.tileset);
        }
        let n = self.paint_cells(p.layer, &cells)?;
        self.mcp_after_write(format!("Filled {n} tiles ({}x{} at {},{}) on layer {} of \"{}\".", p.w, p.h, p.x, p.y, p.layer, map.name))
    }

    fn cells_of(cells: &Option<Vec<[i32; 2]>>, rect: &Option<[u32; 4]>) -> Result<Vec<(i32, i32)>, String> {
        let mut out: Vec<(i32, i32)> = cells.iter().flatten().map(|c| (c[0], c[1])).collect();
        if let Some([x, y, w, h]) = rect {
            for yy in *y..y + h {
                for xx in *x..x + w {
                    out.push((xx as i32, yy as i32));
                }
            }
        }
        if out.is_empty() {
            return Err("give `cells` and / or `rect`".into());
        }
        if out.len() > MAX_CELLS {
            return Err(format!("at most {MAX_CELLS} cells per call"));
        }
        Ok(out)
    }

    pub(crate) fn mcp_paint_terrain(&mut self, p: PaintTerrainParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let found = self.project.db.tilesets.iter().find_map(|d| d.terrains.iter().position(|t| t.name.eq_ignore_ascii_case(&p.terrain)).map(|k| (d.id.clone(), k, d.terrains.clone())));
        let Some((id, k, terrains)) = found else {
            let names: Vec<String> = self.project.db.tilesets.iter().flat_map(|d| d.terrains.iter().map(|t| t.name.clone())).collect();
            return Err(format!("no terrain named \"{}\".{}", p.terrain, suggestions(&p.terrain, names.iter().map(String::as_str))));
        };
        let cells = Self::cells_of(&p.cells, &p.rect)?;
        let layer = p.layer.unwrap_or(0);
        let erase = p.erase.unwrap_or(false);
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if layer >= doc.map.layers.len() {
            return Err(format!("layer {layer} doesn't exist: the map has layers 0..{}", doc.map.layers.len() - 1));
        }
        let slot = doc.map.tileset_slot(&id);
        let mut n = 0;
        doc.begin_stroke(layer, false);
        for cell in cells {
            if !doc.map.in_bounds(cell.0, cell.1) {
                continue;
            }
            for (x, y, new) in autotile::paint(&doc.map, layer, slot, &terrains, k, cell, erase) {
                doc.paint(layer, x as i32, y as i32, new, assets);
            }
            n += 1;
        }
        doc.end_stroke();
        let name = doc.map.name.clone();
        self.mcp_after_write(format!("{} terrain \"{}\" ({id}) on {n} cells of layer {layer} of \"{name}\".", if erase { "Erased" } else { "Painted" }, p.terrain))
    }

    pub(crate) fn mcp_set_collision(&mut self, p: SetCollisionParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let value = match p.value.to_lowercase().as_str() {
            "auto" => CollisionOverride::Auto,
            "solid" => CollisionOverride::Solid,
            "walkable" => CollisionOverride::Walkable,
            v => return Err(format!("value \"{v}\" must be auto, solid or walkable")),
        };
        let cells = Self::cells_of(&p.cells, &p.rect)?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        doc.begin_stroke(0, true);
        let mut n = 0;
        for (x, y) in cells {
            if doc.map.in_bounds(x, y) {
                doc.paint_collision(x, y, value as u8);
                n += 1;
            }
        }
        doc.end_stroke();
        let name = doc.map.name.clone();
        self.mcp_after_write(format!("Collision set to {} on {n} cells of \"{name}\".", p.value.to_lowercase()))
    }

    pub(crate) fn mcp_clear_region(&mut self, p: RectParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if p.w == 0 || p.h == 0 || p.x >= doc.map.width || p.y >= doc.map.height {
            return Err("the rect is empty or outside the map".into());
        }
        let before = doc.map.entities.len();
        doc.clear_region((p.x, p.y, p.w.min(doc.map.width - p.x), p.h.min(doc.map.height - p.y)), assets);
        let removed = before - doc.map.entities.len();
        let name = doc.map.name.clone();
        self.mcp_after_write(format!("Cleared {}x{} tiles at {},{} on every layer of \"{name}\" ({removed} entities removed).", p.w, p.h, p.x, p.y))
    }

    pub(crate) fn mcp_resize_map(&mut self, p: ResizeParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        if p.width == 0 || p.height == 0 || p.width > MAX_MAP_SIZE || p.height > MAX_MAP_SIZE {
            return Err(format!("the size must be 1..{MAX_MAP_SIZE} tiles each way"));
        }
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let before = doc.map.clone();
        let old = (before.width, before.height);
        doc.map.resize(p.width, p.height);
        doc.record_map(before, assets);
        let name = doc.map.name.clone();
        self.mcp_after_write(format!("Resized \"{name}\" from {}x{} to {}x{} (top-left kept).", old.0, old.1, p.width, p.height))
    }

    /// Checks the ids a trigger list refers to and the functions it calls.
    pub(crate) fn mcp_check_triggers(&self, owner: TriggerOwner, ts: &[Trigger]) -> Result<Vec<String>, String> {
        for t in ts {
            if !owner.allows(&t.on) {
                let allowed: Vec<&str> = owner.templates().iter().map(|o| o.label()).collect();
                return Err(format!("a {:?} can't have an \"{}\" trigger; allowed: {}", owner, t.on.label(), allowed.join(", ")));
            }
        }
        let mut warnings = Vec::new();
        for (kind, id) in rustpg2d_engine::refactor::refs_of_triggers(ts) {
            if !self.mcp_id_exists(kind, &id) {
                return Err(format!("the triggers refer to {} \"{id}\", which doesn't exist.{}", rustpg2d_engine::library::kind_label(kind), suggestions(&id, self.mcp_ids_of(kind).iter().map(String::as_str))));
            }
        }
        for f in functions_in(ts) {
            if !self.refs.functions.contains(&f) {
                warnings.push(format!("script function \"{f}\" is not defined in scripts/ yet"));
            }
        }
        Ok(warnings)
    }

    pub(crate) fn mcp_id_exists(&self, kind: RefKind, id: &str) -> bool {
        if id.is_empty() {
            return true;
        }
        match kind {
            RefKind::Map => self.maps.contains(&id.to_string()),
            _ => rustpg2d_engine::library::exists(&self.project.db, &(kind, id.to_string())),
        }
    }

    pub(crate) fn mcp_ids_of(&self, kind: RefKind) -> Vec<String> {
        let db = &self.project.db;
        match kind {
            RefKind::Tileset => db.tilesets.iter().map(|d| d.id.clone()).collect(),
            RefKind::Sheet => db.sheets.iter().map(|d| d.id.clone()).collect(),
            RefKind::Item => db.items.iter().map(|d| d.id.clone()).collect(),
            RefKind::Prop => db.props.iter().map(|d| d.id.clone()).collect(),
            RefKind::Character => db.characters.iter().map(|d| d.id.clone()).collect(),
            RefKind::Sound => db.sounds.iter().map(|d| d.id.clone()).collect(),
            RefKind::Map => self.maps.clone(),
            RefKind::Structure => db.structures.iter().map(|d| d.id.clone()).collect(),
            RefKind::Skill => db.skills.iter().map(|d| d.id.clone()).collect(),
            RefKind::Spell => db.spells.iter().map(|d| d.id.clone()).collect(),
            RefKind::Recipe => db.recipes.iter().map(|d| d.id.clone()).collect(),
        }
    }

    pub(crate) fn mcp_place_entity(&mut self, p: PlaceEntityParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let kind = parse_kind(&p.kind)?;
        let ids = self.project.db.ids(kind);
        if !ids.contains(&p.def) {
            return Err(format!("\"{}\" is not a {} definition.{}", p.def, p.kind.to_lowercase(), suggestions(&p.def, ids.iter().map(String::as_str))));
        }
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if !doc.map.in_bounds(p.x, p.y) {
            return Err(format!("{},{} is outside the map ({}x{})", p.x, p.y, doc.map.width, doc.map.height));
        }
        let before = Objects::of(&doc.map);
        let uid = doc.map.alloc_uid();
        let (fx, fy) = patch::feet_of(p.x, p.y, ts);
        let mut e = EntityInstance::new(uid, kind, &p.def, fx, fy);
        if let Some(n) = &p.name {
            e.name = n.clone();
        }
        if let Some(f) = &p.facing {
            e.facing = parse_facing(f)?;
        }
        if let Some(s) = &p.state {
            e.state = s.clone();
        }
        if let Some(c) = p.count {
            e.count = c;
        }
        if let Some(patch) = &p.patch {
            e = patch::patch_entity(&e, patch, ts)?;
        }
        self.project.db.upgrade_instance(&mut e);
        let warnings = self.mcp_check_triggers(owner_of(kind), &e.triggers)?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        doc.map.entities.push(e.clone());
        doc.record_objects(before, "mcp-entity");
        let name = doc.map.name.clone();
        let mut msg = format!("Placed {} \"{}\" on \"{name}\" at {},{} with uid {uid}.\n{}", p.kind.to_lowercase(), p.def, p.x, p.y, json(&patch::entity_json(&e, ts)));
        for w in warnings {
            msg += &format!("\nWarning: {w}");
        }
        self.mcp_after_write(msg)
    }

    pub(crate) fn mcp_update_entity(&mut self, p: UpdateParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let ts = self.ts();
        let doc = self.doc.as_ref().unwrap();
        let Some(old) = doc.map.entities.iter().find(|e| e.uid == p.uid).cloned() else { return Err(format!("no entity with uid {} on \"{}\"", p.uid, doc.map.name)) };
        let mut new = patch::patch_entity(&old, &p.patch, ts)?;
        let ids = self.project.db.ids(new.kind);
        if !ids.contains(&new.def) {
            return Err(format!("\"{}\" is not a {:?} definition.{}", new.def, new.kind, suggestions(&new.def, ids.iter().map(String::as_str))));
        }
        if !doc.map.in_bounds((new.x / ts).floor() as i32, (new.y / ts).floor() as i32) {
            return Err("the new position is outside the map".into());
        }
        self.project.db.upgrade_instance(&mut new);
        let warnings = self.mcp_check_triggers(owner_of(new.kind), &new.triggers)?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let before = Objects::of(&doc.map);
        if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == p.uid) {
            *e = new.clone();
        }
        doc.record_objects(before, "mcp-entity-update");
        let name = doc.map.name.clone();
        let mut msg = format!("Updated entity {} on \"{name}\".\n{}", p.uid, json(&patch::entity_json(&new, ts)));
        for w in warnings {
            msg += &format!("\nWarning: {w}");
        }
        self.mcp_after_write(msg)
    }

    pub(crate) fn mcp_remove_entity(&mut self, p: ObjectParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(e) = doc.map.entities.iter().find(|e| e.uid == p.uid).cloned() else { return Err(format!("no entity with uid {} on \"{}\"", p.uid, doc.map.name)) };
        let before = Objects::of(&doc.map);
        doc.map.entities.retain(|x| x.uid != p.uid);
        doc.record_objects(before, "mcp-entity-remove");
        let name = doc.map.name.clone();
        self.mcp_after_write(format!("Removed {:?} \"{}\" (uid {}) from \"{name}\".", e.kind, e.def, p.uid))
    }

    /// A zone's kind, triggers and loot from JSON, checked against the project.
    fn zone_parts(&self, kind: &Value, triggers: &Option<Value>, loot: &Option<Value>) -> Result<ZoneParts, String> {
        let kind: ZoneKind = serde_json::from_value(kind.clone()).map_err(|e| format!("kind: {e} (e.g. \"Spawn\", \"Trigger\", {{\"Portal\":{{\"target_map\":\"m\",\"target_spawn\":\"s\"}}}})"))?;
        match &kind {
            ZoneKind::Portal { target_map, target_spawn } => {
                if !self.maps.contains(target_map) {
                    return Err(format!("portal target_map \"{target_map}\" is not a map.{}", suggestions(target_map, self.maps.iter().map(String::as_str))));
                }
                let spawns = self.refs.spawns.get(target_map).cloned().unwrap_or_default();
                if !spawns.contains(target_spawn) {
                    return Err(format!("portal target_spawn \"{target_spawn}\" is not a spawn zone of {target_map}.{}", suggestions(target_spawn, spawns.iter().map(String::as_str))));
                }
            }
            ZoneKind::Transition { target_map, .. } => {
                if !self.maps.contains(target_map) {
                    return Err(format!("transition target_map \"{target_map}\" is not a map (link_maps makes matching pairs).{}", suggestions(target_map, self.maps.iter().map(String::as_str))));
                }
            }
            ZoneKind::Spawner { character, .. } => {
                if self.project.db.character(character).is_none() {
                    return Err(format!("spawner character \"{character}\" doesn't exist.{}", suggestions(character, self.project.db.characters.iter().map(|c| c.id.as_str()))));
                }
            }
            ZoneKind::FishingSpot { rod } => {
                if !rod.is_empty() && self.project.db.item(rod).is_none() {
                    return Err(format!("fishing rod item \"{rod}\" doesn't exist"));
                }
            }
            ZoneKind::Spawn | ZoneKind::Trigger => {}
        }
        let triggers: Vec<Trigger> = match triggers {
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("triggers: {e} (see events_reference)"))?,
            None => vec![],
        };
        let warnings = self.mcp_check_triggers(TriggerOwner::Zone, &triggers)?;
        let loot = match loot {
            Some(v) => Some(serde_json::from_value::<Vec<rustpg2d_engine::defs::LootEntry>>(v.clone()).map_err(|e| format!("loot: {e} (array of {{item, chance, min, max}})"))?),
            None => None,
        };
        if let Some(l) = &loot {
            for e in l {
                if self.project.db.item(&e.item).is_none() {
                    return Err(format!("loot item \"{}\" doesn't exist", e.item));
                }
            }
        }
        Ok((kind, triggers, loot, warnings))
    }

    pub(crate) fn mcp_add_zone(&mut self, p: AddZoneParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let (kind, triggers, loot, warnings) = self.zone_parts(&p.kind, &p.triggers, &p.loot)?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if p.w == 0 || p.h == 0 || p.x + p.w > doc.map.width || p.y + p.h > doc.map.height {
            return Err(format!("the zone {},{} {}x{} is empty or outside the map ({}x{})", p.x, p.y, p.w, p.h, doc.map.width, doc.map.height));
        }
        let name = crate::maps::unique_zone_name(&doc.map, &p.name);
        let before = Objects::of(&doc.map);
        let uid = doc.map.alloc_uid();
        let z = Zone { uid, name: name.clone(), x: p.x, y: p.y, w: p.w, h: p.h, kind, triggers, loot };
        doc.map.zones.push(z.clone());
        doc.record_objects(before, "mcp-zone");
        let map_name = doc.map.name.clone();
        let mut msg = format!("Added zone \"{name}\" ({}) to \"{map_name}\" with uid {uid}.{}\n{}", grid::zone_kind(&z.kind), if name != p.name { format!(" (\"{}\" was taken.)", p.name) } else { String::new() }, json(&z));
        for w in warnings {
            msg += &format!("\nWarning: {w}");
        }
        self.mcp_after_write(msg)
    }

    pub(crate) fn mcp_update_zone(&mut self, p: UpdateParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let doc = self.doc.as_ref().unwrap();
        let Some(old) = doc.map.zones.iter().find(|z| z.uid == p.uid).cloned() else { return Err(format!("no zone with uid {} on \"{}\"", p.uid, doc.map.name)) };
        let new = patch::patch_zone(&old, &p.patch)?;
        let (_, _, _, warnings) = self.zone_parts(&serde_json::to_value(&new.kind).unwrap_or(Value::Null), &Some(serde_json::to_value(&new.triggers).unwrap_or(Value::Null)), &new.loot.as_ref().map(|l| serde_json::to_value(l).unwrap_or(Value::Null)))?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if new.w == 0 || new.h == 0 || new.x + new.w > doc.map.width || new.y + new.h > doc.map.height {
            return Err("the zone's rect is empty or outside the map".into());
        }
        if new.name != old.name && doc.map.zones.iter().any(|z| z.name == new.name) {
            return Err(format!("a zone named \"{}\" exists", new.name));
        }
        let before = Objects::of(&doc.map);
        if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == p.uid) {
            *z = new.clone();
        }
        doc.record_objects(before, "mcp-zone-update");
        let map_name = doc.map.name.clone();
        let mut msg = format!("Updated zone {} on \"{map_name}\".\n{}", p.uid, json(&new));
        for w in warnings {
            msg += &format!("\nWarning: {w}");
        }
        self.mcp_after_write(msg)
    }

    pub(crate) fn mcp_remove_zone(&mut self, p: ObjectParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(z) = doc.map.zones.iter().find(|z| z.uid == p.uid).cloned() else { return Err(format!("no zone with uid {} on \"{}\"", p.uid, doc.map.name)) };
        let before = Objects::of(&doc.map);
        doc.map.zones.retain(|x| x.uid != p.uid);
        doc.record_objects(before, "mcp-zone-remove");
        let name = doc.map.name.clone();
        self.mcp_after_write(format!("Removed zone \"{}\" (uid {}) from \"{name}\".", z.name, p.uid))
    }

    pub(crate) fn mcp_link_maps(&mut self, p: LinkMapsParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let this = self.doc.as_ref().unwrap().map.name.clone();
        if p.to_map == this {
            return Err("link_maps connects two different maps".into());
        }
        if !self.maps.contains(&p.to_map) {
            return Err(format!("\"{}\" is not a map.{}", p.to_map, suggestions(&p.to_map, self.maps.iter().map(String::as_str))));
        }
        let mut other = self.project.load_map(&p.to_map).map_err(|e| format!("{e:#}"))?;
        let [ox, oy, ow, oh] = p.to_rect;
        if ow == 0 || oh == 0 || ox + ow > other.width || oy + oh > other.height {
            return Err(format!("to_rect is empty or outside {} ({}x{})", p.to_map, other.width, other.height));
        }
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let [x, y, w, h] = p.rect;
        if w == 0 || h == 0 || x + w > doc.map.width || y + h > doc.map.height {
            return Err(format!("rect is empty or outside {this} ({}x{})", doc.map.width, doc.map.height));
        }
        let before = Objects::of(&doc.map);
        let mut here = crate::maps::add_transition(&mut doc.map, (x, y, w, h), &p.to_map, "");
        let mut there = crate::maps::add_transition(&mut other, (ox, oy, ow, oh), &this, &here);
        if let Some(n) = &p.name {
            let n = n.trim();
            if !n.is_empty() {
                if doc.map.zones.iter().any(|z| z.name == n && z.name != here) || other.zones.iter().any(|z| z.name == n && z.name != there) {
                    return Err(format!("a zone named \"{n}\" exists on one of the maps"));
                }
                for z in doc.map.zones.iter_mut().filter(|z| z.name == here) {
                    z.name = n.to_string();
                }
                for z in other.zones.iter_mut().filter(|z| z.name == there) {
                    z.name = n.to_string();
                }
                here = n.to_string();
                there = n.to_string();
            }
        }
        crate::maps::set_target(&mut doc.map, &here, &p.to_map, &there);
        crate::maps::set_target(&mut other, &there, &this, &here);
        doc.record_objects(before, "mcp-link");
        self.project.save_map(&other).map_err(|e| format!("saving {}: {e:#}", p.to_map))?;
        self.mcp_after_write(format!("Linked \"{this}\" (zone \"{here}\" at {x},{y} {w}x{h}) with \"{}\" (zone \"{there}\" at {ox},{oy} {ow}x{oh}). The change to {} was written to disk directly (not undoable there).", p.to_map, p.to_map))
    }

    /// Parses an owner value and applies `f` to its trigger list; returns a description.
    fn with_triggers(&mut self, owner: &Value, triggers: Vec<Trigger>, append: bool) -> Result<String, String> {
        let obj = owner.as_object().ok_or("owner must be an object: {\"map\":true}, {\"entity\":UID}, {\"zone\":UID} or {\"def\":{\"kind\":..,\"id\":..}}")?;
        if obj.contains_key("map") {
            let warnings = self.mcp_check_triggers(TriggerOwner::Map, &triggers)?;
            let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
            let before = Objects::of(&doc.map);
            if append {
                doc.map.triggers.extend(triggers);
            } else {
                doc.map.triggers = triggers;
            }
            let n = doc.map.triggers.len();
            doc.record_objects(before, "mcp-triggers");
            return Ok(format!("map \"{}\" now has {n} triggers{}", doc.map.name, warn_text(&warnings)));
        }
        if let Some(uid) = obj.get("entity").and_then(|v| v.as_u64()) {
            let doc = self.doc.as_ref().ok_or("no map is open")?;
            let e = doc.map.entities.iter().find(|e| e.uid == uid as u32).cloned().ok_or_else(|| format!("no entity with uid {uid}"))?;
            let mut ts = if append { e.triggers.clone() } else { vec![] };
            ts.extend(triggers);
            let warnings = self.mcp_check_triggers(owner_of(e.kind), &ts)?;
            let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
            let before = Objects::of(&doc.map);
            let n = ts.len();
            if let Some(x) = doc.map.entities.iter_mut().find(|x| x.uid == uid as u32) {
                x.triggers = ts;
            }
            doc.record_objects(before, "mcp-triggers");
            return Ok(format!("entity {uid} ({} \"{}\") now has {n} triggers{}", e.def, e.name, warn_text(&warnings)));
        }
        if let Some(uid) = obj.get("zone").and_then(|v| v.as_u64()) {
            let doc = self.doc.as_ref().ok_or("no map is open")?;
            let z = doc.map.zones.iter().find(|z| z.uid == uid as u32).cloned().ok_or_else(|| format!("no zone with uid {uid}"))?;
            let mut ts = if append { z.triggers.clone() } else { vec![] };
            ts.extend(triggers);
            let warnings = self.mcp_check_triggers(TriggerOwner::Zone, &ts)?;
            let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
            let before = Objects::of(&doc.map);
            let n = ts.len();
            if let Some(x) = doc.map.zones.iter_mut().find(|x| x.uid == uid as u32) {
                x.triggers = ts;
            }
            doc.record_objects(before, "mcp-triggers");
            return Ok(format!("zone {uid} \"{}\" now has {n} triggers{}", z.name, warn_text(&warnings)));
        }
        if let Some(d) = obj.get("def") {
            let kind = parse_kind(d.get("kind").and_then(|v| v.as_str()).unwrap_or(""))?;
            let id = d.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let owner = match kind {
                EntityKind::Item => TriggerOwner::ItemUse,
                k => owner_of(k),
            };
            let existing: Vec<Trigger> = self.project.db.def_triggers(kind, &id).to_vec();
            if !self.project.db.ids(kind).contains(&id) {
                return Err(format!("\"{id}\" is not a {kind:?} definition"));
            }
            let mut ts = if append { existing } else { vec![] };
            ts.extend(triggers);
            let warnings = self.mcp_check_triggers(owner, &ts)?;
            let n = ts.len();
            let db = &mut self.project.db;
            match kind {
                EntityKind::Item => db.items.iter_mut().find(|x| x.id == id).map(|x| x.on_use = ts),
                EntityKind::Prop => db.props.iter_mut().find(|x| x.id == id).map(|x| x.triggers = ts),
                EntityKind::Character => db.characters.iter_mut().find(|x| x.id == id).map(|x| x.triggers = ts),
            };
            self.defs_dirty = true;
            return Ok(format!("{kind:?} definition \"{id}\" now has {n} triggers{}", warn_text(&warnings)));
        }
        Err("owner must be {\"map\":true}, {\"entity\":UID}, {\"zone\":UID} or {\"def\":{\"kind\":..,\"id\":..}}".into())
    }

    pub(crate) fn mcp_set_triggers(&mut self, p: TriggersParams) -> Result<Reply, String> {
        if p.owner.get("def").is_some() {
            self.mcp_writable()?;
        } else {
            self.mcp_target_map(&p.map)?;
        }
        let triggers: Vec<Trigger> = serde_json::from_value(Value::Array(p.triggers)).map_err(|e| format!("triggers: {e} (see events_reference)"))?;
        let msg = self.with_triggers(&p.owner, triggers, false)?;
        self.mcp_after_write(format!("Triggers replaced: {msg}."))
    }

    pub(crate) fn mcp_add_trigger(&mut self, p: AddTriggerParams) -> Result<Reply, String> {
        if p.owner.get("def").is_some() {
            self.mcp_writable()?;
        } else {
            self.mcp_target_map(&p.map)?;
        }
        let trigger: Trigger = serde_json::from_value(p.trigger).map_err(|e| format!("trigger: {e} (see events_reference)"))?;
        let msg = self.with_triggers(&p.owner, vec![trigger], true)?;
        self.mcp_after_write(format!("Trigger added: {msg}."))
    }

    pub(crate) fn mcp_set_map_settings(&mut self, p: MapSettingsParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let doc = self.doc.as_ref().unwrap();
        let m = &doc.map;
        let cur = MapSettings { background: m.background, music: m.music.clone(), lighting: m.lighting, fog: m.fog, weather: m.weather, parallax: m.parallax.clone(), edges: m.edges.clone() };
        let new: MapSettings = patch::apply(&cur, &p.patch)?;
        if !new.music.is_empty() && self.project.db.sound(&new.music).is_none() {
            return Err(format!("music \"{}\" is not a sound.{}", new.music, suggestions(&new.music, self.project.db.sounds.iter().map(|s| s.id.as_str()))));
        }
        for (side, e) in [("north", &new.edges.north), ("south", &new.edges.south), ("east", &new.edges.east), ("west", &new.edges.west)] {
            if let Some(e) = e {
                if !self.maps.contains(&e.target_map) {
                    return Err(format!("edges.{side}.target_map \"{}\" is not a map", e.target_map));
                }
            }
        }
        for l in &new.parallax {
            if self.project.db.sheet(&l.sheet).is_none() {
                return Err(format!("parallax sheet \"{}\" is not a sprite sheet", l.sheet));
            }
        }
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let before = Objects::of(&doc.map);
        doc.map.background = new.background;
        doc.map.music = new.music;
        doc.map.lighting = new.lighting;
        doc.map.fog = new.fog;
        doc.map.weather = new.weather;
        doc.map.parallax = new.parallax;
        doc.map.edges = new.edges;
        doc.record_objects(before, "mcp-settings");
        let name = doc.map.name.clone();
        let changed: Vec<String> = p.patch.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        self.mcp_after_write(format!("Map settings of \"{name}\" changed: {}.", changed.join(", ")))
    }

    pub(crate) fn mcp_place_structure(&mut self, p: PlaceStructureParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let def = self.project.db.structure(&p.structure).cloned().ok_or_else(|| format!("\"{}\" is not a structure.{}", p.structure, suggestions(&p.structure, self.project.db.structures.iter().map(|s| s.id.as_str()))))?;
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let partly = p.x < 0 || p.y < 0 || p.x as u32 + def.w > doc.map.width || p.y as u32 + def.h > doc.map.height;
        let uids = doc.place_structure(&def, p.x, p.y, assets);
        let name = doc.map.name.clone();
        self.mcp_after_write(format!(
            "Placed structure \"{}\" ({}x{}) on \"{name}\" at {},{}; {} entities added (uids {:?}).{}",
            def.id,
            def.w,
            def.h,
            p.x,
            p.y,
            uids.len(),
            uids,
            if partly { " Warning: it reaches outside the map; the part outside was dropped." } else { "" }
        ))
    }

    pub(crate) fn mcp_capture_structure(&mut self, p: CaptureStructureParams) -> Result<Reply, String> {
        self.mcp_target_map(&p.map)?;
        let id = p.id.trim().to_string();
        if id.is_empty() {
            return Err("the structure needs an id".into());
        }
        if self.project.db.structure(&id).is_some() {
            return Err(format!("a structure \"{id}\" exists already (delete_def it first, or pick another id)"));
        }
        let Some(doc) = &self.doc else { return Err("no map is open".into()) };
        if p.w == 0 || p.h == 0 || p.x + p.w > doc.map.width || p.y + p.h > doc.map.height {
            return Err(format!("the region {},{} {}x{} is not inside the map ({}x{})", p.x, p.y, p.w, p.h, doc.map.width, doc.map.height));
        }
        let name = p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&id).to_string();
        let mut d = rustpg2d_engine::defs::StructureDef::capture(&doc.map, &id, &name, (p.x, p.y, p.w, p.h), p.ground.unwrap_or(true));
        d.category = p.category.clone().unwrap_or_default().trim().to_string();
        let msg = format!("New structure \"{id}\" ({}x{} tiles, {} entities) from \"{}\".", d.w, d.h, d.entities.len(), doc.map.name);
        self.project.db.structures.push(d);
        self.defs_dirty = true;
        self.mcp_after_write(msg)
    }

    pub(crate) fn mcp_rename(&mut self, p: RenameParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let kind = parse_ref_kind(&p.kind)?;
        if !self.mcp_id_exists(kind, &p.old) || p.old.is_empty() {
            return Err(format!("{} \"{}\" doesn't exist.{}", rustpg2d_engine::library::kind_label(kind), p.old, suggestions(&p.old, self.mcp_ids_of(kind).iter().map(String::as_str))));
        }
        // The status text uses the editor's icon font for its arrow.
        let msg = self.try_rename_id(kind, &p.old, &p.new)?.replace('\u{E13A}', "->");
        self.mcp_after_write(msg)
    }
}

fn warn_text(w: &[String]) -> String {
    if w.is_empty() {
        String::new()
    } else {
        format!("; warnings: {}", w.join("; "))
    }
}

/// Script function names a trigger list calls (RunScript, Cutscene, Aim, Fly, Script conditions).
pub(crate) fn functions_in(ts: &[Trigger]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    if k == "function" {
                        if let Some(s) = v.as_str() {
                            if !s.is_empty() {
                                out.push(s.to_string());
                            }
                        }
                    } else {
                        walk(v, out);
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    if let Ok(v) = serde_json::to_value(ts) {
        walk(&v, &mut out);
    }
    out.sort();
    out.dedup();
    out
}
