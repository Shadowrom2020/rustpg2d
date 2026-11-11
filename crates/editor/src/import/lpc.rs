//! LPC character sheets (the Universal LPC Spritesheet layout: 64×64 frames, one animation
//! per row, 13 frames wide and 21+ rows high; or a plain 4-row walk cycle, 9 frames wide) become
//! character figures in the engine's layout: 4 columns, rows Down, Left, Right, Up (`Facing`)
//! per animation. Walk: LPC walk frames 0 (standing), 2, 4 and 6; idle: the standing frame;
//! full sheets also give the attack (the slash rows: wind-up, swing, follow-through, recover) and
//! hurt (the falling row's second frame, facing the viewer, for every facing). A walk-only sheet
//! has 8 rows (walk and idle): the game shows the standing frame for the rest.

use anyhow::{bail, Result};
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::render::character::LAYOUT_FRAMES;

/// LPC frame size.
pub const FRAME: u32 = 64;
/// Which LPC walk frames become the engine's 4 (frame 0 stands: the idle pose).
const PICK: [u32; LAYOUT_FRAMES as usize] = [0, 2, 4, 6];
/// Which LPC slash frames become the attack's wind-up, swing, follow-through and recover.
const SLASH: [u32; 4] = [1, 3, 4, 5];
/// Engine facing (Down, Left, Right, Up) ← LPC row offset within an animation (up 0, left 1,
/// down 2, right 3).
const FROM: [u32; 4] = [2, 1, 3, 0];

/// The LPC sheet's kind: (full sheet, note).
fn kind(img: &Rgba) -> Result<(bool, &'static str)> {
    let (cols, rows) = (img.w / FRAME, img.h / FRAME);
    if cols < 9 || !img.w.is_multiple_of(FRAME) || !img.h.is_multiple_of(FRAME) {
        bail!("not an LPC sheet: expected 64×64 frames and at least 9 columns (got {}×{} px)", img.w, img.h);
    }
    match rows {
        4 => Ok((false, "a walk cycle only: no attack and hurt rows (the game shows the standing frame)")),
        r if r >= 21 => Ok((true, "spellcast, thrust and shoot were left out")),
        _ => bail!("not an LPC sheet: expected the 4 rows of a walk cycle or the full sheet (21 rows), got {rows} rows"),
    }
}

fn copy(out: &mut Rgba, img: &Rgba, (sx, sy): (u32, u32), (dx, dy): (u32, u32)) {
    for y in 0..FRAME {
        for x in 0..FRAME {
            out.set((dx + x) as i32, (dy + y) as i32, img.get((sx + x) as i32, (sy + y) as i32));
        }
    }
}

/// Converts an LPC sheet into a character figure (64×64 frames, 4 columns, 8 or 16 rows).
/// Returns the image and a note for the report.
pub fn convert(img: &Rgba) -> Result<(Rgba, &'static str)> {
    let (full, note) = kind(img)?;
    let walk = if full { 8 } else { 0 };
    let rows = if full { 16 } else { 8 };
    let mut out = Rgba::new(FRAME * LAYOUT_FRAMES, FRAME * rows);
    for (row, lpc_row) in FROM.iter().enumerate() {
        let row = row as u32;
        for (col, &f) in PICK.iter().enumerate() {
            copy(&mut out, img, (f * FRAME, (walk + lpc_row) * FRAME), (col as u32 * FRAME, row * FRAME));
        }
        // Idle: standing (both breathing frames).
        for col in 0..2 {
            copy(&mut out, img, (0, (walk + lpc_row) * FRAME), (col * FRAME, (4 + row) * FRAME));
        }
        if full {
            for (col, &f) in SLASH.iter().enumerate() {
                copy(&mut out, img, (f * FRAME, (12 + lpc_row) * FRAME), (col as u32 * FRAME, (8 + row) * FRAME));
            }
            copy(&mut out, img, (FRAME, 20 * FRAME), (0, (12 + row) * FRAME));
        }
    }
    Ok((out, note))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture colors each frame (row r, column c) with (r * 60 + 20, c * 25, 100).
    fn color(r: u32, c: u32) -> [u8; 4] {
        [(r * 60 + 20) as u8, (c * 25) as u8, 100, 255]
    }

    #[test]
    fn walk_cycle_rows_are_reordered() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lpc_walk.png");
        let img = Rgba::decode(&std::fs::read(path).unwrap()).unwrap();
        let (out, note) = convert(&img).unwrap();
        assert_eq!((out.w, out.h), (256, 512));
        assert!(note.contains("walk cycle only"));
        // Engine rows Down, Left, Right, Up ← LPC rows 2, 1, 3, 0; columns ← LPC frames 0, 2, 4, 6.
        for (row, lpc) in [2, 1, 3, 0].into_iter().enumerate() {
            for (col, f) in [0, 2, 4, 6].into_iter().enumerate() {
                assert_eq!(out.get(col as i32 * 64 + 10, row as i32 * 64 + 10), color(lpc, f), "row {row} col {col}");
            }
        }
    }

    #[test]
    fn full_sheet_gives_walk_idle_attack_and_hurt() {
        let mut img = Rgba::new(13 * 64, 21 * 64);
        img.set(2 * 64 + 5, 10 * 64 + 5, [9, 9, 9, 255]); // walk down (row 10), frame 2
        img.set(5, 9 * 64 + 5, [7, 7, 7, 255]); // walk left (row 9), standing frame 0
        img.set(3 * 64 + 5, 15 * 64 + 5, [5, 5, 5, 255]); // slash right (row 15), frame 3
        img.set(64 + 5, 20 * 64 + 5, [3, 3, 3, 255]); // hurt, frame 1
        let (out, note) = convert(&img).unwrap();
        assert_eq!((out.w, out.h), (256, 1024));
        assert_eq!(out.get(64 + 5, 5), [9, 9, 9, 255], "Down row, second frame");
        assert_eq!(out.get(64 + 5, 5 * 64 + 5), [7, 7, 7, 255], "idle Left, breathing frame 1 = standing");
        assert_eq!(out.get(64 + 5, 10 * 64 + 5), [5, 5, 5, 255], "attack Right, swing = slash frame 3");
        assert_eq!(out.get(5, 15 * 64 + 5), [3, 3, 3, 255], "hurt Up");
        assert!(note.contains("left out"));
        assert!(convert(&Rgba::new(64, 64)).is_err());
        assert!(convert(&Rgba::new(9 * 64, 7 * 64)).is_err());
    }
}
