//! Aseprite sprite sheets: the PNG plus the JSON of *File › Export Sprite Sheet* (`frames` as
//! an array or a hash, trimmed or not, `meta.frameTags`). Frames are repacked into a plain grid
//! (frame size = the sprites' size), tags become named frame ranges (`SheetDef::tags`) with
//! the speed from the frames' durations.

use anyhow::{bail, Result};
use rustpg2d_engine::defs::SheetTag;
use rustpg2d_engine::imageio::Rgba;

use super::tiled::{j_arr, j_map, j_num, j_str, json};

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Where the (possibly trimmed) frame is in the exported image.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Where it goes inside the untrimmed sprite.
    pub off_x: u32,
    pub off_y: u32,
    pub duration: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    /// `meta.image` (relative to the JSON file).
    pub image: String,
    pub frame_w: u32,
    pub frame_h: u32,
    pub frames: Vec<Frame>,
    /// (name, from, to, direction).
    pub tags: Vec<(String, u32, u32, String)>,
}

/// The number at the end of a frame name ("boom 12.aseprite" → 12), for ordering hash frames.
fn frame_number(name: &str) -> Option<u32> {
    let stem = name.rsplit_once('.').map_or(name, |p| p.0);
    let digits: String = stem.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().ok()
}

pub fn parse(text: &str) -> Result<Parsed> {
    let m = json(text)?;
    let meta = m.get("meta").and_then(j_map).unwrap_or_default();
    // Frames: an array, or a hash keyed by name (ordered by the number in the name).
    let mut entries: Vec<(Option<u32>, String, rhai::Map)> = match m.get("frames") {
        Some(v) if v.is_array() => j_arr(&m, "frames").iter().filter_map(j_map).enumerate().map(|(i, f)| (Some(i as u32), String::new(), f)).collect(),
        Some(v) => j_map(v).unwrap_or_default().into_iter().filter_map(|(k, f)| j_map(&f).map(|f| (frame_number(&k), k.to_string(), f))).collect(),
        None => bail!("not an Aseprite sprite sheet: no \"frames\""),
    };
    entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut frames = Vec::new();
    let (mut fw, mut fh) = (0, 0);
    for (_, _, f) in &entries {
        let r = f.get("frame").and_then(j_map).unwrap_or_default();
        let sss = f.get("spriteSourceSize").and_then(j_map).unwrap_or_default();
        let src = f.get("sourceSize").and_then(j_map).unwrap_or_default();
        if f.get("rotated").and_then(|v| v.as_bool().ok()).unwrap_or(false) {
            bail!("rotated frames aren't supported: export without \"Rotate\"");
        }
        let (w, h) = (j_num(&r, "w") as u32, j_num(&r, "h") as u32);
        fw = fw.max(if src.is_empty() { w } else { j_num(&src, "w") as u32 });
        fh = fh.max(if src.is_empty() { h } else { j_num(&src, "h") as u32 });
        frames.push(Frame { x: j_num(&r, "x") as u32, y: j_num(&r, "y") as u32, w, h, off_x: j_num(&sss, "x") as u32, off_y: j_num(&sss, "y") as u32, duration: j_num(f, "duration").max(1.0) as u32 });
    }
    if frames.is_empty() || fw == 0 || fh == 0 {
        bail!("the sprite sheet has no frames");
    }
    let tags = j_arr(&meta, "frameTags").iter().filter_map(j_map).map(|t| (j_str(&t, "name"), j_num(&t, "from") as u32, j_num(&t, "to") as u32, j_str(&t, "direction"))).collect();
    Ok(Parsed { image: j_str(&meta, "image"), frame_w: fw, frame_h: fh, frames, tags })
}

/// Frames per row of the repacked sheet.
const COLUMNS: u32 = 8;

/// Repacks the frames into a grid (row by row, `COLUMNS` wide) and turns the tags into
/// `SheetTag`s. Returns the image, the tags and notes for the report.
pub fn build(p: &Parsed, img: &Rgba) -> (Rgba, Vec<SheetTag>, Vec<String>) {
    let n = p.frames.len() as u32;
    let cols = n.min(COLUMNS);
    let mut out = Rgba::new(cols * p.frame_w, n.div_ceil(cols) * p.frame_h);
    for (i, f) in p.frames.iter().enumerate() {
        let (cx, cy) = ((i as u32 % cols) * p.frame_w + f.off_x, (i as u32 / cols) * p.frame_h + f.off_y);
        for y in 0..f.h {
            for x in 0..f.w {
                out.set((cx + x) as i32, (cy + y) as i32, img.get((f.x + x) as i32, (f.y + y) as i32));
            }
        }
    }
    let mut notes = Vec::new();
    let mut tags = Vec::new();
    for (name, from, to, dir) in &p.tags {
        let (first, last) = ((*from).min(n - 1), (*to).min(n - 1).max((*from).min(n - 1)));
        let range = &p.frames[first as usize..=last as usize];
        let ms = range.iter().map(|f| f.duration).sum::<u32>() as f32 / range.len() as f32;
        if !dir.is_empty() && dir != "forward" {
            notes.push(format!("tag {name} plays {dir} in Aseprite, forward here"));
        }
        tags.push(SheetTag { name: name.clone(), first, last, fps: (1000.0 / ms.max(1.0) * 10.0).round() / 10.0 });
    }
    (out, tags, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_frames_and_tags() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let p = parse(&std::fs::read_to_string(dir.join("boom.json")).unwrap()).unwrap();
        assert_eq!((p.image.as_str(), p.frame_w, p.frame_h, p.frames.len()), ("boom.png", 16, 16, 4));
        let img = Rgba::decode(&std::fs::read(dir.join("boom.png")).unwrap()).unwrap();
        let (sheet, tags, notes) = build(&p, &img);
        assert_eq!((sheet.w, sheet.h), (64, 16));
        assert_eq!(sheet.get(3 * 16 + 2, 2), [180, 200, 50, 255]);
        assert_eq!(tags, vec![SheetTag { name: "explode".into(), first: 0, last: 2, fps: 10.0 }, SheetTag { name: "smoke".into(), first: 3, last: 3, fps: 5.0 }]);
        assert!(notes.is_empty());
    }

    #[test]
    fn hash_frames_trimmed_and_ordered_by_number() {
        let json = r#"{ "frames": {
            "a 10.ase": { "frame": {"x": 0, "y": 0, "w": 2, "h": 2}, "spriteSourceSize": {"x": 1, "y": 1, "w": 2, "h": 2}, "sourceSize": {"w": 4, "h": 4}, "duration": 50 },
            "a 2.ase": { "frame": {"x": 2, "y": 0, "w": 4, "h": 4}, "spriteSourceSize": {"x": 0, "y": 0, "w": 4, "h": 4}, "sourceSize": {"w": 4, "h": 4}, "duration": 50 }
          }, "meta": { "image": "a.png", "frameTags": [ { "name": "all", "from": 0, "to": 1, "direction": "pingpong" } ] } }"#;
        let p = parse(json).unwrap();
        assert_eq!(p.frames.iter().map(|f| f.x).collect::<Vec<_>>(), vec![2, 0], "frame 2 before frame 10");
        let mut img = Rgba::new(6, 4);
        img.set(0, 0, [1, 2, 3, 255]);
        let (sheet, tags, notes) = build(&p, &img);
        assert_eq!((sheet.w, sheet.h), (8, 4));
        assert_eq!(sheet.get(4 + 1, 1), [1, 2, 3, 255], "trimmed frame placed at its offset in the second cell");
        assert_eq!(tags[0].fps, 20.0);
        assert_eq!(notes.len(), 1);
    }
}
