//! Zoomed far out, the map view draws each 32 x 32 chunk of a tile layer from a small texture
//! rendered once ([`TEX_TILE`] pixels per tile) instead of 1,024 quads per chunk and frame. A
//! chunk's texture is known by a hash of its tiles and of the tilesets' textures, so every way a
//! map changes (painting, undo, terrains, pasting, agent writes, reloads, tileset edits) shows
//! without hooks into them. Animated tiles stay out of the texture and are drawn per tile on top.
//! Closer in, layers are drawn per tile as in the game (`render::draw_layer`).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use macroquad::prelude::*;
use rustpg2d_engine::map::{Map, CHUNK_SIZE};
use rustpg2d_engine::render::{self, TilesetTex};

/// Pixels per tile in a cached chunk (a chunk texture is 256 x 256, 256 KB).
const TEX_TILE: u32 = 8;
/// Views whose tiles appear this small on screen (pixels) or smaller draw from the cache.
pub const CACHE_AT_PX: f32 = 8.0;
/// Most chunk textures kept (80 MB of GPU memory).
const MAX_ENTRIES: usize = 320;

struct Entry {
    key: u64,
    rt: RenderTarget,
    /// Animated tiles of the chunk (tile positions in the chunk), drawn per tile.
    animated: Vec<(u32, u32)>,
    used: u64,
}

/// The cached chunk textures of the open map.
#[derive(Default)]
pub struct TileCache {
    map: String,
    entries: HashMap<(usize, u32, u32), Entry>,
    frame: u64,
    /// Chunk textures rendered so far (`status`).
    pub builds: u64,
}

/// A hash of the tilesets a map draws with: their ids and textures (a tileset edited in the
/// pixel editor gets a new texture).
pub fn sets_key(map: &Map, sets: &[Option<&TilesetTex>]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    map.tilesets.hash(&mut h);
    for s in sets {
        format!("{:?}", s.map(|s| s.texture.raw_miniquad_id())).hash(&mut h);
    }
    h.finish()
}

/// FNV-1a over a chunk's tiles, seeded with the tilesets' key.
fn chunk_key(tiles: &[rustpg2d_engine::map::Tile], sets: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ sets;
    for t in tiles {
        h ^= t.0 as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

impl TileCache {
    /// Starts a frame of map `name` (another map than before: the old textures are dropped).
    pub fn begin(&mut self, name: &str) {
        if self.map != name {
            self.entries.clear();
            self.map = name.to_string();
        }
        self.frame += 1;
    }

    /// Chunk textures kept now.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Draws the part of layer `li` inside `view` (world pixels) from chunk textures, rendering
    /// those that are missing or changed. A chunk that finds no room in the cache is drawn per
    /// tile.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_layer(&mut self, map: &Map, li: usize, sets: &[Option<&TilesetTex>], sets_key: u64, ts: f32, view: Rect, tint: Color) {
        let layer = &map.layers[li];
        let span = CHUNK_SIZE as f32 * ts;
        let (cw, ch) = (map.width.div_ceil(CHUNK_SIZE), map.height.div_ceil(CHUNK_SIZE));
        let cx0 = (view.x / span).floor().max(0.0) as u32;
        let cy0 = (view.y / span).floor().max(0.0) as u32;
        let cx1 = ((view.x + view.w) / span).ceil().clamp(0.0, cw as f32) as u32;
        let cy1 = ((view.y + view.h) / span).ceil().clamp(0.0, ch as f32) as u32;
        let time = get_time();
        for cy in cy0..cy1 {
            for cx in cx0..cx1 {
                let Some(tiles) = layer.chunk_tiles(cx, cy) else { continue };
                let key = chunk_key(tiles, sets_key);
                let id = (li, cx, cy);
                let fresh = self.entries.get(&id).is_some_and(|e| e.key == key);
                if !fresh && !self.make_room(&id) {
                    // Every texture is in use this frame: this chunk the slow way.
                    let r = Rect::new(cx as f32 * span, cy as f32 * span, span, span);
                    render::draw_layer(map, layer, sets, ts, r, tint);
                    continue;
                }
                if !fresh {
                    let old = self.entries.remove(&id).map(|e| e.rt);
                    let e = self.render_chunk(tiles, sets, ts, key, old);
                    self.entries.insert(id, e);
                    self.builds += 1;
                }
                let e = self.entries.get_mut(&id).expect("just made");
                e.used = self.frame;
                let at = vec2(cx as f32 * span, cy as f32 * span);
                // (Render targets come out bottom row first.)
                draw_texture_ex(&e.rt.texture, at.x, at.y, tint, DrawTextureParams { dest_size: Some(vec2(span, span)), flip_y: true, ..Default::default() });
                for &(x, y) in &e.animated {
                    let Some((slot, idx)) = tiles[(y * CHUNK_SIZE + x) as usize].parts() else { continue };
                    let Some(Some(t)) = sets.get(slot) else { continue };
                    let src = render::inset(t.src_rect(t.display_index(idx, time), ts));
                    draw_texture_ex(&t.texture, at.x + x as f32 * ts, at.y + y as f32 * ts, tint, DrawTextureParams { source: Some(src), dest_size: Some(vec2(ts, ts)), ..Default::default() });
                }
            }
        }
    }

    /// Room for chunk `id`: it has an entry already, or there is space, or the texture drawn
    /// longest ago (not this frame) goes.
    fn make_room(&mut self, id: &(usize, u32, u32)) -> bool {
        if self.entries.contains_key(id) || self.entries.len() < MAX_ENTRIES {
            return true;
        }
        let oldest = self.entries.iter().filter(|(_, e)| e.used < self.frame).min_by_key(|(_, e)| e.used).map(|(k, _)| *k);
        match oldest {
            Some(k) => {
                self.entries.remove(&k);
                true
            }
            None => false,
        }
    }

    /// Renders a chunk's tiles (all but the animated ones) into a texture, reusing `old`.
    fn render_chunk(&self, tiles: &[rustpg2d_engine::map::Tile], sets: &[Option<&TilesetTex>], ts: f32, key: u64, old: Option<RenderTarget>) -> Entry {
        let size = CHUNK_SIZE * TEX_TILE;
        let rt = old.unwrap_or_else(|| {
            let rt = render_target_ex(size, size, RenderTargetParams { sample_count: 1, depth: false });
            rt.texture.set_filter(FilterMode::Nearest);
            rt
        });
        let mut animated = Vec::new();
        push_camera_state();
        set_camera(&Camera2D { render_target: Some(rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, size as f32, size as f32)) });
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        let px = TEX_TILE as f32;
        for (i, t) in tiles.iter().enumerate() {
            let Some((slot, idx)) = t.parts() else { continue };
            let Some(Some(set)) = sets.get(slot) else { continue };
            let (x, y) = (i as u32 % CHUNK_SIZE, i as u32 / CHUNK_SIZE);
            if set.is_animated(idx) {
                animated.push((x, y));
                continue;
            }
            let src = render::inset(set.src_rect(idx, ts));
            draw_texture_ex(&set.texture, x as f32 * px, y as f32 * px, WHITE, DrawTextureParams { source: Some(src), dest_size: Some(vec2(px, px)), ..Default::default() });
        }
        pop_camera_state();
        Entry { key, rt, animated, used: self.frame }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::map::Tile;

    #[test]
    fn chunks_are_told_apart_by_their_tiles_and_tilesets() {
        let a = vec![Tile::new(0, 3); 1024];
        let mut b = a.clone();
        assert_eq!(chunk_key(&a, 1), chunk_key(&b, 1));
        b[517] = Tile::new(0, 4);
        assert_ne!(chunk_key(&a, 1), chunk_key(&b, 1));
        assert_ne!(chunk_key(&a, 1), chunk_key(&a, 2), "another tileset texture: render again");
    }
}
