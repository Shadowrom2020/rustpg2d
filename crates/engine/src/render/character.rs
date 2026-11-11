//! Layered characters: which sheets make up a character (body, clothing, armor, hair, held
//! items) in drawing order. Used by the game and by the editor's previews.
//!
//! Layer order (back to front): held items on the far side → body → clothing → body armor →
//! hair, beard → head armor → held items on the near side. Every layer uses the character
//! layout (4 frames per row, rows = facings: down, left, right, up); the body sheet sets the
//! frame size. A sheet may continue below the walk with more rows of the same kind
//! ([`ANIM_ROWS`]): idle (2 frames, breathing), attack (4: wind-up, swing, follow-through,
//! recover) and hurt (1). A layer whose sheet lacks a row shows its standing frame instead.

use crate::defs::{Appearance, CharacterDef, Database, EquipSlot};
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
    /// Standing still: breathing frame 0 / 1.
    Idle(u32),
    /// An attack: 0 wind-up, 1 swing, 2 follow-through, 3 recover.
    Attack(u32),
    /// Just hit.
    Hurt,
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
            Anim::Idle(k) => (4, k % 2),
            Anim::Attack(k) => (8, k.min(3)),
            Anim::Hurt => (12, 0),
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
            Anim::Walk(_) => "walk".into(),
            Anim::Idle(_) => "idle".into(),
            Anim::Attack(k) => format!("attack{}", k.min(3)),
            Anim::Hurt => "hurt".into(),
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

/// One sheet frame to draw (recolored for bodies / hair).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub sheet: String,
    /// The frame of the animation, and the one to show when the sheet has no such row.
    pub frame: u32,
    pub fallback: u32,
    pub skin: Option<[u8; 3]>,
    pub hair: Option<[u8; 3]>,
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

/// Is the item in `slot` on the far side of the body (drawn behind it) when facing `f`? The
/// weapon is in the right hand, the left-hand slot in the left one.
pub fn held_behind(slot: EquipSlot, f: Facing) -> bool {
    match f {
        Facing::Up => true,
        Facing::Down => false,
        Facing::Left => slot == EquipSlot::Weapon,
        Facing::Right => slot == EquipSlot::Offhand,
    }
}

/// Overlay sheet an equipped item adds in `slot` ("" = none): outfit / armor overlays (the
/// female cut on a female body; for a body with a `cut`, `<overlay>_<cut>` / `<overlay>_<cut>_f`
/// when that sheet exists), the held sprite of weapons and left-hand items.
fn overlay_of(db: &Database, eq: &Equipment, slot: EquipSlot, female: bool, cut: &str) -> Option<String> {
    let d = db.item(eq.item(slot)?)?;
    let s = match slot {
        EquipSlot::Weapon | EquipSlot::Offhand => d.held.clone(),
        _ => {
            let a = d.armor.as_ref().filter(|a| a.slot == slot)?;
            let cut_sheet = (!cut.is_empty() && !a.overlay.is_empty()).then(|| {
                let base = format!("{}_{cut}", a.overlay);
                [if female { Some(format!("{base}_f")) } else { None }, Some(base)].into_iter().flatten().find(|s| db.sheet(s).is_some())
            });
            cut_sheet.flatten().unwrap_or_else(|| a.overlay_for(female).to_string())
        }
    };
    (!s.is_empty()).then_some(s)
}

/// The layers of a character in drawing order. `anim`: what it does (a number = walk frame);
/// `hide_weapon` while a melee swing shows the weapon instead.
pub fn layers(db: &Database, a: &Appearance, eq: &Equipment, facing: Facing, anim: impl Into<Anim>, hide_weapon: bool) -> Vec<Layer> {
    let female = db.female_body(&a.body);
    let cut = db.sheet(&a.body).map_or("", |s| s.cut.as_str());
    let (over, fallback) = anim.into().frames(facing);
    // A complete figure has everything painted in: only the body, not recoloured.
    if db.complete_body(&a.body) {
        return vec![Layer { sheet: a.body.clone(), frame: over, fallback, skin: None, hair: None }];
    }
    let plain = |sheet: String| Layer { sheet, frame: over, fallback, skin: None, hair: None };
    let held = |behind: bool| {
        [EquipSlot::Weapon, EquipSlot::Offhand]
            .into_iter()
            .filter(move |s| held_behind(*s, facing) == behind && !(hide_weapon && *s == EquipSlot::Weapon))
            .filter_map(|s| overlay_of(db, eq, s, female, cut))
            .map(plain)
    };
    let mut out: Vec<Layer> = held(true).collect();
    out.push(Layer { sheet: a.body.clone(), frame: over, fallback, skin: Some(a.skin), hair: None });
    out.extend(overlay_of(db, eq, EquipSlot::Clothing, female, cut).map(plain));
    out.extend(overlay_of(db, eq, EquipSlot::Body, female, cut).map(plain));
    for s in [&a.hair, &a.beard].into_iter().filter(|s| !s.is_empty()) {
        out.push(Layer { sheet: s.clone(), frame: over, fallback, skin: None, hair: Some(a.hair_color) });
    }
    out.extend(overlay_of(db, eq, EquipSlot::Head, female, cut).map(plain));
    out.extend(held(false));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{Armor, ItemDef, SheetDef, SheetKind};
    use crate::inventory::ItemStack;

    #[test]
    fn animation_rows_fall_back_to_standing() {
        // Facing right (row 2): the walk frame 1, idle, attack and hurt rows below the walk.
        assert_eq!(Anim::Walk(1).frames(Facing::Right), (9, 9));
        assert_eq!(Anim::Idle(1).frames(Facing::Right), ((4 + 2) * 4 + 1, 8));
        assert_eq!(Anim::Attack(2).frames(Facing::Right), ((8 + 2) * 4 + 2, 8));
        assert_eq!(Anim::Hurt.frames(Facing::Down), (12 * 4, 0));
        // A 4-row sheet (16 frames) shows the standing frame; a 16-row sheet the animation.
        let l = Layer { sheet: "s".into(), frame: 41, fallback: 8, skin: None, hair: None };
        assert_eq!((l.frame_in(16), l.frame_in(ANIM_ROWS * LAYOUT_FRAMES)), (8, 41));
    }

    #[test]
    fn complete_figures_are_drawn_alone() {
        let mut db = Database::default();
        db.sheets.push(SheetDef { kind: SheetKind::Body, complete: true, ..SheetDef::new("hero", "hero.png", 64, 96) });
        let mut sword: ItemDef = ron::from_str(r#"(id: "sword", name: "Sword", icon: (sheet: "items", frame: 0), held: "held_sword")"#).unwrap();
        sword.weapon = Some(crate::defs::Weapon::melee(2, 0.4));
        db.items.push(sword);
        let eq = Equipment { weapon: Some(ItemStack::new("sword", 1)), ..Default::default() };
        let a = Appearance { body: "hero".into(), hair: "hair_short".into(), ..Default::default() };
        let l = layers(&db, &a, &eq, Facing::Down, 0, false);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].sheet.as_str(), l[0].skin, l[0].hair), ("hero", None, None));
    }

    #[test]
    fn hands_by_facing() {
        // Facing down both hands are in front, facing up both behind; sideways the far hand.
        assert!(!held_behind(EquipSlot::Weapon, Facing::Down) && !held_behind(EquipSlot::Offhand, Facing::Down));
        assert!(held_behind(EquipSlot::Weapon, Facing::Up) && held_behind(EquipSlot::Offhand, Facing::Up));
        assert!(held_behind(EquipSlot::Weapon, Facing::Left) && !held_behind(EquipSlot::Offhand, Facing::Left));
        assert!(!held_behind(EquipSlot::Weapon, Facing::Right) && held_behind(EquipSlot::Offhand, Facing::Right));
    }

    #[test]
    fn bodies_with_a_cut_wear_that_cut_where_one_exists() {
        let mut db = Database::default();
        db.sheets.push(SheetDef { kind: SheetKind::Body, cut: "dwarf".into(), ..SheetDef::new("body_dwarf", "d.png", 32, 48) });
        db.sheets.push(SheetDef { kind: SheetKind::Body, female: true, cut: "dwarf".into(), ..SheetDef::new("body_dwarf_f", "df.png", 32, 48) });
        for s in ["outfit_tunic", "outfit_tunic_f", "outfit_tunic_dwarf", "outfit_tunic_dwarf_f", "outfit_robe", "outfit_robe_f"] {
            db.sheets.push(SheetDef::new(s, "x.png", 32, 48));
        }
        let item = |id: &str, overlay: &str| {
            let mut d: ItemDef = ron::from_str(&format!(r#"(id: "{id}", name: "x", icon: (sheet: "items", frame: 0))"#)).unwrap();
            d.armor = Some(Armor { slot: EquipSlot::Clothing, defense: 0, speed: 0.0, overlay: overlay.into(), overlay_f: format!("{overlay}_f") });
            d
        };
        db.items.extend([item("tunic", "outfit_tunic"), item("robe", "outfit_robe")]);
        let worn = |body: &str, it: &str| {
            let eq = Equipment { clothing: Some(ItemStack::new(it, 1)), ..Default::default() };
            let a = Appearance { body: body.into(), ..Default::default() };
            layers(&db, &a, &eq, Facing::Down, 1, false).into_iter().map(|l| l.sheet).nth(1).unwrap()
        };
        assert_eq!(worn("body_dwarf", "tunic"), "outfit_tunic_dwarf");
        assert_eq!(worn("body_dwarf_f", "tunic"), "outfit_tunic_dwarf_f");
        // No dwarf cut of the robe: the standard (female) one.
        assert_eq!(worn("body_dwarf", "robe"), "outfit_robe");
        assert_eq!(worn("body_dwarf_f", "robe"), "outfit_robe_f");
    }

    #[test]
    fn female_bodies_wear_the_female_cut() {
        let mut db = Database::default();
        db.sheets.push(SheetDef { kind: SheetKind::Body, ..SheetDef::new("body_male", "m.png", 32, 48) });
        db.sheets.push(SheetDef { kind: SheetKind::Body, female: true, ..SheetDef::new("body_female", "f.png", 32, 48) });
        let mut tunic: ItemDef = ron::from_str(r#"(id: "tunic", name: "Tunic", icon: (sheet: "items", frame: 0))"#).unwrap();
        tunic.armor = Some(Armor { slot: EquipSlot::Clothing, defense: 0, speed: 0.0, overlay: "outfit_tunic".into(), overlay_f: "outfit_tunic_f".into() });
        db.items.push(tunic);
        let eq = Equipment { clothing: Some(ItemStack::new("tunic", 1)), ..Default::default() };
        let sheets = |body: &str| {
            let a = Appearance { body: body.into(), hair: "hair_long".into(), ..Default::default() };
            layers(&db, &a, &eq, Facing::Down, 1, false).into_iter().map(|l| l.sheet).collect::<Vec<_>>()
        };
        assert_eq!(sheets("body_male"), ["body_male", "outfit_tunic", "hair_long"]);
        assert_eq!(sheets("body_female"), ["body_female", "outfit_tunic_f", "hair_long"]);
        // Old projects: the pre-drawn sheet becomes the body.
        let mut old = Database::default();
        old.sheets.push(SheetDef::new("hero", "hero.png", 32, 48));
        old.characters.push(ron::from_str(r#"(id: "hero", name: "Hero", sheet: "hero")"#).unwrap());
        old.upgrade_characters();
        assert_eq!(old.characters[0].appearance.body, "hero");
        assert!(old.characters[0].sheet.is_empty());
        assert_eq!(old.sheets[0].kind, SheetKind::Body);
    }
}
