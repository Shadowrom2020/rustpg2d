//! The isometric view: the square world (tiles of 32 px, unchanged in maps, collision and rules)
//! shown as 2:1 diamonds, seen from the south-east and 30° above.
//!
//! - A world point `p` lands at `project(p)` = (x − y, (x + y) / 2) screen pixels at zoom 1, so a
//!   tile is a 64 × 32 diamond. The ground camera ([`View::ground`]) is a `Camera2D` turned 45°
//!   and squashed to half height that does exactly this, so everything drawn flat in world
//!   coordinates (the painted ground, flat decorations, the fight grid, areas, the light map) and
//!   mouse picking (its inverse) need nothing else.
//! - Things that stand (characters, props, trees, overlays above heads) are drawn unturned
//!   through [`View::upright`]: a plain camera that puts their feet where the ground camera shows
//!   them, at [`UPRIGHT`] times their world size.
//! - Depth: what stands nearer the viewer (larger `x + y`) is drawn later; footprints larger than
//!   a point (buildings, counters, beds) are ordered by their boxes ([`order`]).

use macroquad::prelude::*;

/// Screen pixels (at zoom 1) of one world pixel of something upright: a tile's edge is √2 long on
/// screen, and figures, props and pictures keep their size against it.
pub const UPRIGHT: f32 = std::f32::consts::SQRT_2;

/// Screen offset (zoom 1) of world point `p`: (x − y, (x + y) / 2).
pub fn project(p: Vec2) -> Vec2 {
    vec2(p.x - p.y, (p.x + p.y) * 0.5)
}

/// The world point that [`project`] puts at screen offset `s`.
pub fn unproject(s: Vec2) -> Vec2 {
    vec2(s.x * 0.5 + s.y, -s.x * 0.5 + s.y)
}

/// A direction on the screen (keys, the stick: x right, y down) as a direction in the world,
/// normalized (zero stays zero).
pub fn screen_dir_to_world(d: Vec2) -> Vec2 {
    unproject(d).normalize_or_zero()
}

/// A world direction as the screen shows it (normalized).
pub fn world_dir_to_screen(d: Vec2) -> Vec2 {
    project(d).normalize_or_zero()
}

/// A ground offset `d` (world pixels) as an offset in the upright drawing of something standing
/// there: what the screen shows, at the upright scale (a fishing line reaching out over the water).
pub fn lay(d: Vec2) -> Vec2 {
    project(d) / UPRIGHT
}

/// How near the viewer a world point is: drawn later the larger it is.
pub fn depth(p: Vec2) -> f32 {
    p.x + p.y
}

/// The view of one frame: where the camera looks (a world point) and its zoom (screen pixels per
/// projected pixel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub target: Vec2,
    pub scale: f32,
    /// The window's size in pixels.
    pub screen: Vec2,
}

impl View {
    pub fn new(target: Vec2, scale: f32) -> Self {
        Self { target, scale, screen: vec2(screen_width(), screen_height()) }
    }

    /// The camera for everything lying on the ground.
    pub fn ground(&self) -> Camera2D {
        let k = 2.0 * self.scale * UPRIGHT;
        Camera2D { target: self.target, rotation: 45.0, zoom: vec2(k / self.screen.x, k * 0.5 / self.screen.y), ..Default::default() }
    }

    /// The screen position of world point `p` (what the ground camera shows there).
    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        project(p - self.target) * self.scale + self.screen / 2.0
    }

    /// The world point (on the ground) under screen position `s`.
    pub fn to_world(&self, s: Vec2) -> Vec2 {
        unproject((s - self.screen / 2.0) / self.scale) + self.target
    }

    /// Screen pixels per world pixel of something upright.
    pub fn upright_scale(&self) -> f32 {
        self.scale * UPRIGHT
    }

    /// The camera for something standing at `feet`: unturned, at [`View::upright_scale`], with
    /// `feet` where the ground camera shows it. Drawing code keeps its world coordinates around
    /// the feet (a head `h` pixels above them is `feet − (0, h)`).
    pub fn upright(&self, feet: Vec2) -> Camera2D {
        let u = self.upright_scale();
        let target = feet - project(feet - self.target) * (self.scale / u);
        Camera2D { target, zoom: vec2(2.0 * u / self.screen.x, 2.0 * u / self.screen.y), ..Default::default() }
    }

    /// The screen position `lift` world pixels above `feet` (an overlay over a head).
    pub fn above(&self, feet: Vec2, lift: f32) -> Vec2 {
        self.to_screen(feet) - vec2(0.0, lift * self.upright_scale())
    }

    /// The bounding box (world) of the ground the screen shows.
    pub fn visible(&self) -> Rect {
        let s = self.screen;
        let ps = [self.to_world(vec2(0.0, 0.0)), self.to_world(vec2(s.x, 0.0)), self.to_world(vec2(0.0, s.y)), self.to_world(s)];
        let (mut lo, mut hi) = (ps[0], ps[0]);
        for p in ps {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
    }
}

/// How much higher on the screen (at zoom 1, in upright pixels) a point `h` world pixels above the
/// ground is: the figure renderer's camera looks from 30° above, so heights shrink by its cosine.
pub const HEIGHT: f32 = 0.866_025_4;

/// The height a block in front of the hero is cut down to (world pixels): a stub that shows
/// where the wall runs.
pub const STUB: f32 = 6.0;

/// Draws a block: footprint `r` (world pixels) raised `height` world pixels, its visible faces
/// (south, east, top) through `v` in window pixels (the caller sets the default camera), each
/// showing `tex` (a texture and the source rect of the picture), shaded like the pre-rendered
/// props: light from the north-west, the east face darkest.
pub fn draw_block(v: &View, r: Rect, height: f32, tex: Option<(&Texture2D, Rect)>, tint: Color) {
    let lift = vec2(0.0, height * HEIGHT * v.upright_scale());
    let g = |x: f32, y: f32| v.to_screen(vec2(x, y));
    let (nw, ne, se, sw) = (g(r.x, r.y), g(r.right(), r.y), g(r.right(), r.bottom()), g(r.x, r.bottom()));
    let (tw, th) = tex.map_or((1.0, 1.0), |(t, _)| (t.width().max(1.0), t.height().max(1.0)));
    let src = tex.map_or(Rect::new(0.0, 0.0, 1.0, 1.0), |(_, s)| s);
    let shade = |k: f32| Color::new(tint.r * k, tint.g * k, tint.b * k, tint.a);
    let quad = |pts: [Vec2; 4], k: f32, src: Rect| {
        let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let c = shade(k);
        let vertices = pts
            .iter()
            .zip(uv)
            .map(|(p, (u, w))| macroquad::models::Vertex::new(p.x, p.y, 0.0, (src.x + u * src.w) / tw, (src.y + w * src.h) / th, c))
            .collect();
        draw_mesh(&Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: tex.map(|(t, _)| t.clone()) });
    };
    if height > 0.0 {
        quad([sw - lift, se - lift, se, sw], 0.92, src);
        quad([se - lift, ne - lift, ne, se], 0.68, src);
        // The top: the picture's top band (a wall picture's beam or coping) stretched over it.
        quad([nw - lift, ne - lift, se - lift, sw - lift], 0.62, Rect::new(src.x, src.y, src.w, (src.h / 8.0).max(1.0)));
    } else {
        quad([nw, ne, se, sw], 1.0, src);
    }
}

/// Whether a block on footprint `r` stands between the viewer and `hero` (world pixels): not wholly
/// west or north of the hero, near it on the screen (the room's south and east walls around it).
pub fn hides(r: Rect, hero: Vec2) -> bool {
    let near = 6.0 * 32.0;
    if r.right() <= hero.x || r.bottom() <= hero.y {
        return false;
    }
    let c = r.center();
    let off = project(c - hero);
    off.x.abs() < near && off.y < near * 0.75
}

/// A soft round shadow on the ground (through the ground camera, so it lies as an ellipse):
/// darkest `alpha` in the middle `c`, gone at `radius` world pixels.
pub fn blob_shadow(c: Vec2, radius: f32, alpha: f32) {
    const N: u16 = 20;
    let v = |p: Vec2, a: f32| macroquad::models::Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, Color::new(0.0, 0.0, 0.0, a));
    let mut vertices = vec![v(c, alpha)];
    let mut indices = Vec::with_capacity(N as usize * 9);
    for k in 0..N {
        let t = k as f32 / N as f32 * std::f32::consts::TAU;
        let d = vec2(t.cos(), t.sin());
        vertices.push(v(c + d * radius * 0.55, alpha * 0.7));
        vertices.push(v(c + d * radius, 0.0));
    }
    for k in 0..N {
        let (i0, o0, i1, o1) = (1 + 2 * k, 2 + 2 * k, 1 + 2 * ((k + 1) % N), 2 + 2 * ((k + 1) % N));
        indices.extend_from_slice(&[0, i0, i1, i0, o0, o1, i0, o1, i1]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

/// A soft dark patch on the ground under something standing on rectangle `r` (world pixels,
/// through the ground camera): darkest inside, fading out a little past its edges, so a 3D prop
/// sits on the ground on every side, not only where its cast shadow falls.
pub fn contact_shadow(r: Rect, alpha: f32) {
    let m = (r.w.min(r.h) * 0.3).clamp(2.0, 10.0);
    let inner = Rect::new(r.x + m * 0.5, r.y + m * 0.5, (r.w - m).max(1.0), (r.h - m).max(1.0));
    let outer = Rect::new(r.x - m, r.y - m, r.w + 2.0 * m, r.h + 2.0 * m);
    let v = |x: f32, y: f32, a: f32| macroquad::models::Vertex::new(x, y, 0.0, 0.0, 0.0, Color::new(0.0, 0.0, 0.0, a));
    let corners = |q: Rect, a: f32| [v(q.x, q.y, a), v(q.right(), q.y, a), v(q.right(), q.bottom(), a), v(q.x, q.bottom(), a)];
    let mut vertices = corners(inner, alpha).to_vec();
    vertices.extend(corners(outer, 0.0));
    let mut indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
    for k in 0..4u16 {
        let (i0, i1, o0, o1) = (k, (k + 1) % 4, 4 + k, 4 + (k + 1) % 4);
        indices.extend_from_slice(&[i0, o0, o1, i0, o1, i1]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

/// The map's border fading into `color` (the background past the edge) over `band` world pixels,
/// for a map `w` × `h` pixels; drawn on the ground (through the ground camera).
pub fn border_fade(w: f32, h: f32, band: f32, color: Color) {
    let band = band.min(w / 2.0).min(h / 2.0);
    if band <= 0.0 {
        return;
    }
    let clear = Color { a: 0.0, ..color };
    // Each side: the edge (full colour) and the inner line (clear), corners shared.
    let outer = [vec2(0.0, 0.0), vec2(w, 0.0), vec2(w, h), vec2(0.0, h)];
    let inner = [vec2(band, band), vec2(w - band, band), vec2(w - band, h - band), vec2(band, h - band)];
    let v = |p: Vec2, c: Color| macroquad::models::Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, c);
    let mut vertices = Vec::with_capacity(8);
    for k in 0..4 {
        vertices.push(v(outer[k], color));
        vertices.push(v(inner[k], clear));
    }
    let mut indices = Vec::with_capacity(24);
    for k in 0..4u16 {
        let (o0, i0, o1, i1) = (2 * k, 2 * k + 1, 2 * ((k + 1) % 4), 2 * ((k + 1) % 4) + 1);
        indices.extend_from_slice(&[o0, o1, i1, o0, i1, i0]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

/// Something to draw in depth order: its footprint on the ground (world pixels; a point is a
/// zero-sized box) and the depth of its feet ([`depth`]) for ties.
#[derive(Clone, Copy, Debug)]
pub struct Footprint {
    pub rect: Rect,
    pub depth: f32,
}

impl Footprint {
    /// A footprint `w` × `h` whose front edge's middle is `feet` (the anchor of props).
    pub fn at_feet(feet: Vec2, w: f32, h: f32) -> Self {
        Self { rect: Rect::new(feet.x - w / 2.0, feet.y - h, w, h), depth: depth(feet) }
    }

    /// The horizontal screen span (projected x) of the footprint.
    fn span(&self) -> (f32, f32) {
        let r = &self.rect;
        (r.x - r.bottom(), r.right() - r.y)
    }
}

/// Whether `a` must be drawn before `b` (`Some(true)`), after it (`Some(false)`), or either way.
/// Boxes side by side on the screen never overlap, so only those sharing screen columns are
/// ordered: the one wholly west or wholly north of the other is farther away.
fn before(a: &Footprint, b: &Footprint) -> Option<bool> {
    let (a0, a1) = a.span();
    let (b0, b1) = b.span();
    if a1 <= b0 || b1 <= a0 {
        return None;
    }
    let (ra, rb) = (&a.rect, &b.rect);
    if ra.right() <= rb.x || ra.bottom() <= rb.y {
        Some(true)
    } else if rb.right() <= ra.x || rb.bottom() <= ra.y {
        Some(false)
    } else {
        // Overlapping footprints (a character on a rug, in a doorway): by their feet.
        Some(a.depth <= b.depth)
    }
}

/// The drawing order of `items` (indices): farther first. Only pairs with a footprint wider than
/// `point` (pixels) are compared box to box; points among themselves go by depth.
pub fn order(items: &[Footprint], point: f32) -> Vec<usize> {
    let n = items.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| items[a].depth.total_cmp(&items[b].depth));
    let big: Vec<usize> = idx.iter().copied().filter(|&i| items[i].rect.w > point || items[i].rect.h > point).collect();
    if big.is_empty() {
        return idx;
    }
    // Edges a -> b (a before b) for pairs with a big footprint whose order differs from depth.
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut need = vec![0usize; n];
    let is_big = {
        let mut v = vec![false; n];
        for &i in &big {
            v[i] = true;
        }
        v
    };
    for &b in &big {
        for o in 0..n {
            if o == b || (is_big[o] && o < b) {
                continue;
            }
            if let Some(first) = before(&items[b], &items[o]) {
                let (x, y) = if first { (b, o) } else { (o, b) };
                after[x].push(y);
                need[y] += 1;
            }
        }
    }
    // Points among themselves keep their depth order: chain them.
    let points: Vec<usize> = idx.iter().copied().filter(|&i| !is_big[i]).collect();
    for w in points.windows(2) {
        after[w[0]].push(w[1]);
        need[w[1]] += 1;
    }
    // Kahn's order, nearest-depth first among the ready ones; what a cycle leaves goes by depth.
    let mut rank = vec![0usize; n];
    for (r, &i) in idx.iter().enumerate() {
        rank[i] = r;
    }
    let mut ready: std::collections::BTreeSet<(usize, usize)> = (0..n).filter(|&i| need[i] == 0).map(|i| (rank[i], i)).collect();
    let mut out = Vec::with_capacity(n);
    let mut done = vec![false; n];
    while out.len() < n {
        let next = match ready.pop_first() {
            Some((_, i)) => i,
            None => {
                // A cycle: take the farthest one left.
                let i = *idx.iter().find(|&&i| !done[i]).expect("items left");
                need[i] = 0;
                i
            }
        };
        if done[next] {
            continue;
        }
        done[next] = true;
        out.push(next);
        for &y in &after[next] {
            if !done[y] {
                need[y] = need[y].saturating_sub(1);
                if need[y] == 0 {
                    ready.insert((rank[y], y));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View { target: vec2(320.0, 240.0), scale: 2.0, screen: vec2(1280.0, 720.0) }
    }

    #[test]
    fn only_blocks_in_front_of_the_hero_are_cut() {
        let hero = vec2(160.0, 160.0);
        let tile = |x: f32, y: f32| Rect::new(x * 32.0, y * 32.0, 32.0, 32.0);
        // The wall south of the hero and the one east of them hide them; north and west don't.
        assert!(hides(tile(5.0, 6.0), hero));
        assert!(hides(tile(6.0, 5.0), hero));
        assert!(!hides(tile(5.0, 3.0), hero));
        assert!(!hides(tile(3.0, 5.0), hero));
        // Far away in front: left standing.
        assert!(!hides(tile(5.0, 20.0), hero));
    }

    #[test]
    fn a_tile_is_a_64_by_32_diamond() {
        let (a, b, c, d) = (project(vec2(0.0, 0.0)), project(vec2(32.0, 0.0)), project(vec2(32.0, 32.0)), project(vec2(0.0, 32.0)));
        assert_eq!(b - d, vec2(64.0, 0.0));
        assert_eq!(c - a, vec2(0.0, 32.0));
    }

    #[test]
    fn projection_round_trips() {
        for p in [vec2(0.0, 0.0), vec2(17.0, -3.5), vec2(1000.0, 64.0)] {
            assert!(unproject(project(p)).distance(p) < 1e-4);
            let v = view();
            assert!(v.to_world(v.to_screen(p)).distance(p) < 1e-3);
        }
        assert_eq!(view().to_screen(vec2(320.0, 240.0)), vec2(640.0, 360.0));
    }

    #[test]
    fn screen_up_walks_north_west() {
        let up = screen_dir_to_world(vec2(0.0, -1.0));
        assert!(up.x < 0.0 && up.y < 0.0 && (up.x - up.y).abs() < 1e-5);
        let right = screen_dir_to_world(vec2(1.0, 0.0));
        assert!(right.x > 0.0 && right.y < 0.0);
        assert!(world_dir_to_screen(vec2(1.0, 1.0)).distance(vec2(0.0, 1.0)) < 1e-5);
    }

    #[test]
    fn the_upright_camera_puts_the_feet_where_the_ground_shows_them() {
        let v = view();
        let feet = vec2(400.0, 300.0);
        let cam = v.upright(feet);
        // A plain camera: screen = (p - target) * u + centre.
        let u = cam.zoom.x * v.screen.x / 2.0;
        let s = (feet - cam.target) * u + v.screen / 2.0;
        assert!(s.distance(v.to_screen(feet)) < 1e-3);
        assert!((u - v.upright_scale()).abs() < 1e-4);
        // A head 40 px up is straight above on the screen.
        let head = (feet - vec2(0.0, 40.0) - cam.target) * u + v.screen / 2.0;
        assert!((head.x - s.x).abs() < 1e-3 && (s.y - head.y - 40.0 * u).abs() < 1e-3);
    }

    fn point(x: f32, y: f32) -> Footprint {
        Footprint { rect: Rect::new(x, y, 0.0, 0.0), depth: depth(vec2(x, y)) }
    }

    fn pos(order: &[usize], i: usize) -> usize {
        order.iter().position(|&k| k == i).unwrap()
    }

    #[test]
    fn a_character_beside_a_building_is_drawn_after_it() {
        // A house 224 x 160 with its front edge's middle at (500, 500); someone east of it, near
        // its back (north-east) corner: their feet have a smaller x + y than the house's front,
        // yet they stand in front of the east wall.
        let house = Footprint::at_feet(vec2(500.0, 500.0), 224.0, 160.0);
        let east = point(630.0, 360.0);
        assert!(east.depth < house.depth);
        let o = order(&[house, east], 8.0);
        assert!(pos(&o, 0) < pos(&o, 1));
        // Someone behind it (north) is drawn first.
        let north = point(500.0, 320.0);
        let o = order(&[house, north], 8.0);
        assert!(pos(&o, 1) < pos(&o, 0));
        // And someone south-west in front of it, after.
        let front = point(420.0, 530.0);
        let o = order(&[house, front], 8.0);
        assert!(pos(&o, 0) < pos(&o, 1));
    }

    #[test]
    fn points_keep_their_depth_order() {
        let items = [point(10.0, 10.0), point(0.0, 0.0), point(5.0, 30.0)];
        assert_eq!(order(&items, 8.0), vec![1, 0, 2]);
    }

    #[test]
    fn far_apart_on_screen_needs_no_order() {
        let a = Footprint::at_feet(vec2(0.0, 1000.0), 64.0, 64.0);
        let b = Footprint::at_feet(vec2(1000.0, 0.0), 64.0, 64.0);
        assert!(before(&a, &b).is_none());
    }
}
