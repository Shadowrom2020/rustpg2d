//! Character figures: the frame of a character's figure sheet (a complete picture: clothes,
//! hair and what it carries painted in) and the colours of its colour features. Used by the game
//! and by the editor's previews.
//!
//! Figure sheets use the character layout: 4 frames per row, rows = facings (down, left, right,
//! up); walk, then more rows of the same kind ([`ANIM_ROWS`]): idle (2 frames, breathing), attack
//! (4: wind-up, swing, follow-through, recover) and hurt (1). A sheet that lacks a row shows its
//! standing frame instead.

use crate::defs::{Appearance, CharacterDef, Database};
use crate::entity::{EntityInstance, Facing};
use crate::state::Equipment;

/// Walk frames per row of the character layout.
pub const LAYOUT_FRAMES: u32 = 4;
/// Rows of a sheet with every animation: walk, idle, attack and hurt, four facings each.
pub const ANIM_ROWS: u32 = 16;

/// What a character is doing, for its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anim {
    /// Walk frame (0 = standing).
    Walk(u32),
    /// Walking for this many hundredths of a second (figures: 8 frames a second; models play
    /// their walk clip at that time).
    Walking(u32),
    /// Standing still: breathing frame 0 / 1.
    Idle(u32),
    /// An attack: 0 wind-up, 1 swing, 2 follow-through, 3 recover.
    Attack(u32),
    /// Just hit.
    Hurt,
    /// A model's extra animation (`ModelDef::clips`: "fish", "hammer", "sit"…); figures show
    /// their standing frame (or the walk column `.1` of activity strokes).
    Extra(&'static str, u32),
}

impl From<u32> for Anim {
    fn from(col: u32) -> Self {
        Anim::Walk(col)
    }
}

impl Anim {
    /// The first row of its block and the column within it.
    fn block(self) -> (u32, u32) {
        match self {
            Anim::Walk(c) => (0, c % LAYOUT_FRAMES),
            Anim::Walking(t) => (0, (t * 8 / 100) % LAYOUT_FRAMES),
            Anim::Idle(k) => (4, k % 2),
            Anim::Attack(k) => (8, k.min(3)),
            Anim::Hurt => (12, 0),
            Anim::Extra(_, c) => (0, c % LAYOUT_FRAMES),
        }
    }

    /// Frame of this animation for `facing`, and the frame a sheet without it shows.
    pub fn frames(self, facing: Facing) -> (u32, u32) {
        let (row, col) = self.block();
        let frame = (row + facing as u32) * LAYOUT_FRAMES + col;
        let fallback = facing as u32 * LAYOUT_FRAMES + if row == 0 { col } else { 0 };
        (frame, fallback)
    }

    /// For `status`: walk / idle / attackN / hurt.
    pub fn name(self) -> String {
        match self {
            Anim::Walk(_) | Anim::Walking(_) => "walk".into(),
            Anim::Idle(_) => "idle".into(),
            Anim::Attack(k) => format!("attack{}", k.min(3)),
            Anim::Hurt => "hurt".into(),
            Anim::Extra(n, _) => n.into(),
        }
    }
}

/// The look of a placed character: its own, else its definition's.
pub fn appearance_of<'a>(def: &'a CharacterDef, inst: Option<&'a EntityInstance>) -> &'a Appearance {
    inst.and_then(|e| e.appearance.as_ref()).unwrap_or(&def.appearance)
}

/// The equipment of a placed character: its own, else its definition's.
pub fn equipment_of<'a>(def: &'a CharacterDef, inst: Option<&'a EntityInstance>) -> &'a Equipment {
    inst.and_then(|e| e.equipment.as_ref()).unwrap_or(&def.equipment)
}

/// One sheet frame to draw, with the colours of the sheet's colour features.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub sheet: String,
    /// The frame of the animation, and the one to show when the sheet has no such row.
    pub frame: u32,
    pub fallback: u32,
    /// The chosen colour of each of the sheet's colour features, in feature order (`None` = as
    /// drawn; empty = nothing recoloured).
    pub colors: Vec<Option<[u8; 3]>>,
}

impl Layer {
    /// The frame to draw from a sheet of `count` frames.
    pub fn frame_in(&self, count: u32) -> u32 {
        if self.frame < count {
            self.frame
        } else {
            self.fallback
        }
    }
}

/// The colours an appearance gives its figure's colour features, in feature order (empty when
/// nothing is recoloured).
pub fn feature_colors(db: &Database, a: &Appearance) -> Vec<Option<[u8; 3]>> {
    let Some(sheet) = db.sheet(&a.body) else { return Vec::new() };
    let colors: Vec<Option<[u8; 3]>> = sheet.features.iter().map(|f| a.colors.get(&f.name).copied()).collect();
    if colors.iter().all(Option::is_none) {
        Vec::new()
    } else {
        colors
    }
}

/// A character's figure frame. `anim`: what it does (a number = walk frame).
pub fn figure(db: &Database, a: &Appearance, facing: Facing, anim: impl Into<Anim>) -> Layer {
    let (frame, fallback) = anim.into().frames(facing);
    Layer { sheet: a.body.clone(), frame, fallback, colors: feature_colors(db, a) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{ColorFeature, SheetDef, SheetKind};

    #[test]
    fn animation_rows_fall_back_to_standing() {
        // Facing right (row 2): the walk frame 1, idle, attack and hurt rows below the walk.
        assert_eq!(Anim::Walk(1).frames(Facing::Right), (9, 9));
        // Walking 0.4 s: 8 frames a second -> column 3 (0.375 s), then on.
        assert_eq!(Anim::Walking(40).frames(Facing::Right), (11, 11));
        assert_eq!(Anim::Idle(1).frames(Facing::Right), ((4 + 2) * 4 + 1, 8));
        assert_eq!(Anim::Attack(2).frames(Facing::Right), ((8 + 2) * 4 + 2, 8));
        assert_eq!(Anim::Hurt.frames(Facing::Down), (12 * 4, 0));
        // A 4-row sheet (16 frames) shows the standing frame; a 16-row sheet the animation.
        let l = Layer { sheet: "s".into(), frame: 41, fallback: 8, colors: Vec::new() };
        assert_eq!((l.frame_in(16), l.frame_in(ANIM_ROWS * LAYOUT_FRAMES)), (8, 41));
    }

    #[test]
    fn a_figure_carries_its_feature_colours_in_feature_order() {
        let mut db = Database::default();
        let feature = |name: &str| ColorFeature { name: name.into(), shades: vec![[1, 2, 3]], colors: Vec::new() };
        db.sheets.push(SheetDef { kind: SheetKind::Body, features: vec![feature("skin"), feature("hair")], ..SheetDef::new("hero", "hero.png", 64, 96) });
        let mut a = Appearance { body: "hero".into(), ..Default::default() };
        assert!(figure(&db, &a, Facing::Down, 0).colors.is_empty());
        a.colors.insert("hair".into(), [200, 180, 90]);
        a.colors.insert("eyes".into(), [0, 0, 255]); // not a feature of this figure: ignored
        let l = figure(&db, &a, Facing::Left, Anim::Idle(1));
        assert_eq!(l.sheet, "hero");
        assert_eq!(l.colors, vec![None, Some([200, 180, 90])]);
        assert_eq!(l.frame, (4 + 1) * 4 + 1);
    }

    #[test]
    fn the_removed_layer_kinds_load_as_sprites() {
        let s: SheetDef = ron::from_str(r#"(id: "hair_short", image: "h.png", frame_w: 64, frame_h: 96, kind: Hair)"#).unwrap();
        assert_eq!(s.kind, SheetKind::Sprite);
        let b: SheetDef = ron::from_str(r#"(id: "hero", image: "h.png", frame_w: 64, frame_h: 96, kind: Body, complete: true, cut: "")"#).unwrap();
        assert_eq!(b.kind, SheetKind::Body);
        let a: Appearance = ron::from_str(r#"(body: "body_male", skin: (1, 2, 3), hair: "hair_short", hair_color: (4, 5, 6), beard: "")"#).unwrap();
        assert_eq!(a, Appearance { body: "body_male".into(), ..Default::default() });
    }
}
