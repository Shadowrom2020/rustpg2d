//! What a character does, for the pose its model is rendered in ([`Anim`]), and the look and
//! equipment of a placed character. Characters are 3D models (`render/model.rs`).

use crate::defs::{Appearance, CharacterDef};
use crate::entity::EntityInstance;
use crate::state::Equipment;

/// Steps of a walk cycle: `Anim::Walk(c)` is step `c` of this many (a quarter of the clip each).
pub const LAYOUT_FRAMES: u32 = 4;

/// What a character is doing, for its pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anim {
    /// Walk step (0 = standing).
    Walk(u32),
    /// Walking for this many hundredths of a second (the walk clip at that time).
    Walking(u32),
    /// Standing still (the idle clip; the number keeps neighbours apart).
    Idle(u32),
    /// An attack: 0 wind-up, 1 swing, 2 follow-through, 3 recover.
    Attack(u32),
    /// Just hit.
    Hurt,
    /// An extra animation (`ModelDef::clips`: "fish", "craft", "sit"…) and a step of it.
    Extra(&'static str, u32),
}

impl From<u32> for Anim {
    fn from(col: u32) -> Self {
        Anim::Walk(col)
    }
}

impl Anim {
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

#[cfg(test)]
mod tests {
    use crate::defs::{Appearance, SheetDef, SheetKind};

    #[test]
    fn character_sheets_of_earlier_versions_load_as_sprites() {
        let s: SheetDef = ron::from_str(r#"(id: "hair_short", image: "h.png", frame_w: 64, frame_h: 96, kind: Hair)"#).unwrap();
        assert_eq!(s.kind, SheetKind::Sprite);
        // A 0.17 figure with its colour features, flags and hand marks.
        let b: SheetDef = ron::from_str(r#"(id: "hero", image: "h.png", frame_w: 64, frame_h: 96, kind: Body, playable: true, female: false, features: [(name: "skin", shades: [(1, 2, 3)])], hands: ["0: R 1 2 3"], complete: true, cut: "")"#).unwrap();
        assert_eq!(b.kind, SheetKind::Sprite);
        let a: Appearance = ron::from_str(r#"(body: "body_male", skin: (1, 2, 3), hair: "hair_short", hair_color: (4, 5, 6), beard: "")"#).unwrap();
        assert_eq!(a, Appearance { body: "body_male".into(), ..Default::default() });
    }
}
