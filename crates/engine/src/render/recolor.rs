//! Colour features of character figures (`defs::ColorFeature`): the art's own shades of a part
//! (skin, hair, clothing...) are replaced by a chosen colour, each shade as much darker or lighter
//! than the chosen colour as it was than the feature's middle shade, so light and shadow stay.

use std::collections::HashMap;

/// Placeholder ramps a creator can paint a feature in when its colours are shared with another
/// part of the art (dark, mid-dark, base, light): magenta for skin, cyan for hair.
pub const SKIN_REF: [[u8; 3]; 4] = [[0x6e, 0x00, 0x6e], [0x9c, 0x00, 0x9c], [0xc8, 0x00, 0xc8], [0xf0, 0x00, 0xf0]];
pub const HAIR_REF: [[u8; 3]; 4] = [[0x00, 0x6e, 0x6e], [0x00, 0x9c, 0x9c], [0x00, 0xc8, 0xc8], [0x00, 0xf0, 0xf0]];

/// Dark, mid-dark, base and light shade of a color.
pub fn ramp(c: [u8; 3]) -> [[u8; 3]; 4] {
    let mul = |f: f32| c.map(|v| (v as f32 * f).round().clamp(0.0, 255.0) as u8);
    [mul(0.55), mul(0.78), c, mul(1.22)]
}

/// Perceived brightness 0..255.
pub fn luma(c: [u8; 3]) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

/// The shades of one feature recoloured to `target`: shade -> new colour.
pub fn feature_map(shades: &[[u8; 3]], target: [u8; 3]) -> Vec<([u8; 3], [u8; 3])> {
    if shades.is_empty() {
        return Vec::new();
    }
    let mut by_light: Vec<f32> = shades.iter().map(|s| luma(*s)).collect();
    by_light.sort_by(f32::total_cmp);
    let mid = by_light[by_light.len() / 2].max(1.0);
    shades.iter().map(|s| (*s, target.map(|v| (v as f32 * luma(*s) / mid).round().clamp(0.0, 255.0) as u8))).collect()
}

/// Recolours RGBA pixels in place: every pixel whose colour is one of a feature's shades takes
/// that feature's chosen colour (`features`: (shades, chosen colour)). Transparent pixels and
/// other colours stay.
pub fn recolor(rgba: &mut [u8], features: &[(&[[u8; 3]], [u8; 3])]) {
    let map: HashMap<[u8; 3], [u8; 3]> = features.iter().flat_map(|(shades, target)| feature_map(shades, *target)).collect();
    if map.is_empty() {
        return;
    }
    for px in rgba.as_chunks_mut::<4>().0 {
        if px[3] == 0 {
            continue;
        }
        if let Some(c) = map.get(&[px[0], px[1], px[2]]) {
            px[..3].copy_from_slice(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_features_shades_take_the_chosen_colour_with_their_light() {
        // A brown hair feature: three shades; the middle one becomes the chosen colour exactly.
        let shades = [[60, 40, 20], [100, 70, 40], [150, 110, 70]];
        let blond = [220, 190, 110];
        let mut px: Vec<u8> = shades.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect();
        px.extend([10, 20, 30, 255, 60, 40, 20, 0]); // another colour and a transparent pixel stay
        recolor(&mut px, &[(&shades, blond)]);
        let got: Vec<[u8; 3]> = px.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
        assert_eq!(got[1], blond);
        assert!(luma(got[0]) < luma(got[1]) && luma(got[1]) < luma(got[2]), "{got:?}");
        assert_eq!(got[3], [10, 20, 30]);
        assert_eq!(got[4], [60, 40, 20]);
        // No features: nothing changes.
        let mut same = vec![60, 40, 20, 255];
        recolor(&mut same, &[]);
        assert_eq!(same, vec![60, 40, 20, 255]);
    }
}
