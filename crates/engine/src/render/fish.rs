//! Fish shadows that mark a fishing spot: now and then a dark fish shape glides a short way
//! through the spot's water and fades again. A pure function of the time (nothing to update or
//! save), so the game and the editor draw the same thing.

use gfx::prelude::*;

/// A stable 0..1 value for (seed, a, b).
fn hash(seed: u32, a: u32, b: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9e37_79b9) ^ a.wrapping_mul(0x85eb_ca6b) ^ b.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h % 10_000) as f32 / 10_000.0
}

/// Shadows in `area` (world pixels; the zone) at time `t` (seconds). `seed` keeps spots apart
/// (the zone uid), `ts` = tile size. Draw with the world camera, above the ground layers.
pub fn draw_shadows(area: Rect, seed: u32, t: f32, ts: f32) {
    let tiles = (area.w / ts) * (area.h / ts);
    let n = ((tiles / 10.0).round() as u32).clamp(1, 4);
    for k in 0..n {
        // Each fish shows for part of its own cycle (7-11 s), at a new place every cycle.
        let period = 7.0 + 4.0 * hash(seed, k, 999);
        let shifted = t + period * hash(seed, k, 998);
        let cycle = (shifted / period).floor() as u32;
        let u = shifted / period - cycle as f32;
        const SHOWN: f32 = 0.45;
        if u > SHOWN {
            continue;
        }
        let fade = (u / SHOWN * std::f32::consts::PI).sin();
        let inset = ts * 0.35;
        let (w, h) = ((area.w - 2.0 * inset).max(1.0), (area.h - 2.0 * inset).max(1.0));
        let start = vec2(area.x + inset + hash(seed, k, cycle * 3) * w, area.y + inset + hash(seed, k, cycle * 3 + 1) * h);
        let angle = hash(seed, k, cycle * 3 + 2) * std::f32::consts::TAU;
        let dir = Vec2::from_angle(angle);
        // Glides about a tile, staying inside the spot.
        let mut pos = start + dir * (u / SHOWN) * ts * 1.1;
        pos.x = pos.x.clamp(area.x + inset, area.right() - inset);
        pos.y = pos.y.clamp(area.y + inset, area.bottom() - inset);
        let wiggle = (t * 7.0 + k as f32).sin() * 8.0;
        let deg = angle.to_degrees();
        let c = Color::new(0.02, 0.07, 0.16, 0.38 * fade);
        let len = ts * (0.24 + 0.08 * hash(seed, k, 997));
        draw_ellipse(pos.x, pos.y, len, len * 0.36, deg, c);
        // The tail fin behind it.
        let back = Vec2::from_angle((deg + 180.0 + wiggle).to_radians());
        let tail = pos + back * len * 1.05;
        let side = vec2(-back.y, back.x) * len * 0.35;
        draw_triangle(pos + back * len * 0.75, tail + side, tail - side, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_spread() {
        assert_eq!(hash(3, 1, 2), hash(3, 1, 2));
        let vals: Vec<f32> = (0..200).map(|i| hash(7, i, 0)).collect();
        assert!(vals.iter().all(|v| (0.0..1.0).contains(v)));
        assert!(vals.iter().any(|v| *v < 0.2) && vals.iter().any(|v| *v > 0.8));
    }
}
