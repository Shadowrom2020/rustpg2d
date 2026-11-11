//! Hands of character figures and what they hold. A figure sheet marks per frame where each hand
//! is and how it is turned ([`Grip`]: position in the frame's image pixels, the grip axis — the
//! line a handle runs through the closed fist —, its size, in front of or behind the body). What
//! is held has marks too ([`HeldSprite`]: its hold point and orientation on its own picture):
//! drawing puts the hold point on the hand and turns the picture so its orientation mark lies on
//! the hand's axis. Two-handed things run through both fists. Items use their own held picture or
//! their inventory icon; characters also carry props in both hands (`PropDef.carry`).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::defs::{Database, SpriteRef};
use crate::entity::Facing;
use crate::render::character::Anim;
use crate::state::Equipment;

/// Which hand holds an item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hand {
    /// The weapon hand (the default for weapons and tools).
    #[default]
    Right,
    /// The shield hand (left-hand items; bows).
    Left,
    /// Both hands: between them, along the line through both fists (two-handed weapons, carried
    /// props; the right hand's axis when the fists are close together).
    Both,
}

/// Where a hand is in a frame and how it holds things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    /// Image pixels in the frame (0, 0 = its top-left corner).
    pub x: f32,
    pub y: f32,
    /// Degrees: the grip axis, the direction a handle runs through the fist toward what it holds
    /// (0 = right, 90 = down, -90 = up). Held things turn so their orientation mark lies on it.
    pub angle: f32,
    /// Size of what it holds (1 = normal; bigger hands hold bigger things).
    pub scale: f32,
    /// Drawn over the figure (else behind it).
    pub front: bool,
}

impl Grip {
    pub fn new(x: f32, y: f32, angle: f32) -> Grip {
        Grip { x, y, angle, scale: 1.0, front: true }
    }

    /// The grip mirrored in a frame `width` pixels wide (Left rows -> Right rows).
    pub fn mirrored(&self, width: f32) -> Grip {
        Grip { x: width - 1.0 - self.x, angle: 180.0 - self.angle, ..*self }
    }

    fn fmt(&self, tag: char) -> String {
        let num = |v: f32| {
            let s = format!("{:.2}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        format!("{tag} {} {} {} {} {}", num(self.x), num(self.y), num(self.angle), num(self.scale), if self.front { 'f' } else { 'b' })
    }

    fn parse(s: &str) -> Option<(char, Grip)> {
        let mut it = s.split_whitespace();
        let tag = it.next()?.chars().next()?;
        let mut n = || it.next().and_then(|v| v.parse::<f32>().ok());
        let (x, y, angle) = (n()?, n()?, n()?);
        let scale = n().unwrap_or(1.0);
        let front = s.split_whitespace().nth(5).is_none_or(|v| v != "b");
        Some((tag, Grip { x, y, angle, scale, front }))
    }
}

/// The hands of one frame. Stored as a line like `"12: R 41 58 -40 1 f | L 22 57 220 1 b"`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameHands {
    pub frame: u32,
    pub right: Option<Grip>,
    pub left: Option<Grip>,
}

impl FrameHands {
    pub fn to_line(&self) -> String {
        let hands: Vec<String> = [self.right.map(|g| g.fmt('R')), self.left.map(|g| g.fmt('L'))].into_iter().flatten().collect();
        format!("{}: {}", self.frame, hands.join(" | "))
    }

    pub fn from_line(s: &str) -> Option<FrameHands> {
        let (frame, rest) = s.split_once(':')?;
        let mut out = FrameHands { frame: frame.trim().parse().ok()?, ..Default::default() };
        for part in rest.split('|').map(str::trim).filter(|p| !p.is_empty()) {
            match Grip::parse(part)? {
                ('R', g) => out.right = Some(g),
                ('L', g) => out.left = Some(g),
                _ => return None,
            }
        }
        Some(out)
    }

    pub fn get(&self, hand: Hand) -> Option<Grip> {
        match hand {
            Hand::Left => self.left,
            _ => self.right,
        }
    }

    pub fn set(&mut self, hand: Hand, g: Option<Grip>) {
        match hand {
            Hand::Left => self.left = g,
            _ => self.right = g,
        }
    }
}

impl Serialize for FrameHands {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_line())
    }
}

impl<'de> Deserialize<'de> for FrameHands {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        FrameHands::from_line(&s).ok_or_else(|| serde::de::Error::custom(format!("not a frame's hands: {s:?} (e.g. \"12: R 41 58 -40 1 f | L 22 57 220 1 b\")")))
    }
}

/// The hands of `frame` in `hands` (sorted or not), else those of `fallback`.
pub fn grips_of(hands: &[FrameHands], frame: u32, fallback: u32) -> (Option<Grip>, Option<Grip>) {
    let find = |f: u32| hands.iter().find(|h| h.frame == f);
    match find(frame).or_else(|| find(fallback)) {
        Some(h) => (h.right, h.left),
        None => (None, None),
    }
}

/// How something looks in a hand: its picture and marks on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeldSprite {
    /// Its picture ("" sheet = the item's icon, the prop's sprite).
    #[serde(default, skip_serializing_if = "SpriteRef::is_empty")]
    pub sprite: SpriteRef,
    /// The hold point: where the hand holds it, a fraction of the picture (0, 0 = top-left).
    #[serde(default = "icon_grip")]
    pub grip: [f32; 2],
    /// The orientation mark: degrees, the direction in its picture that lies on the hand's grip
    /// axis (icons of weapons point up-right: -45, the blade's direction).
    #[serde(default = "icon_points")]
    pub points: f32,
    /// Size in the hand (1 = its world size; the hand's size multiplies it).
    #[serde(default = "held_scale")]
    pub scale: f32,
    /// The hand it is held in (None: by its kind: left-hand items and bows the left, two-handed
    /// weapons both, others the right).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand: Option<Hand>,
}

fn icon_grip() -> [f32; 2] {
    [0.22, 0.78]
}
fn icon_points() -> f32 {
    -45.0
}
fn held_scale() -> f32 {
    0.5
}

impl Default for HeldSprite {
    fn default() -> Self {
        HeldSprite { sprite: SpriteRef::default(), grip: icon_grip(), points: icon_points(), scale: held_scale(), hand: None }
    }
}

impl HeldSprite {
    /// A carried prop's marks when it has none: held at the middle of its lower half, its
    /// horizontal running along the hands, at 0.6 of its size (a crate or barrel in the arms).
    pub fn carried() -> HeldSprite {
        HeldSprite { grip: [0.5, 0.6], points: 0.0, scale: 0.6, hand: Some(Hand::Both), ..Default::default() }
    }
}

/// Something to draw in a figure's hands this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldLayer {
    /// Item id, or the prop id of a carried prop.
    pub id: String,
    pub sprite: SpriteRef,
    /// The grip point in the frame's image pixels.
    pub at: (f32, f32),
    /// Where on the sprite the hand holds it (fraction).
    pub grip: [f32; 2],
    /// Rotation in degrees (clockwise on screen).
    pub rotation: f32,
    /// The sprite's orientation mark (after `rotation` it lies on the grip axis; `grip` and
    /// `points` are those of the mirrored picture when `flip`).
    pub points: f32,
    pub scale: f32,
    pub front: bool,
    pub hand: Hand,
    /// Drawn mirrored top to bottom (the hand's axis points left, see [`lay`]).
    pub flip: bool,
}

/// The items a figure holds this frame: `tool` in the right hand instead of the weapon (while it
/// is used), a carried prop in both hands instead of anything else.
pub fn held(db: &Database, body: &str, eq: &Equipment, carrying: &str, tool: Option<&str>, facing: Facing, anim: Anim) -> Vec<HeldLayer> {
    let Some(sheet) = db.sheet(body).filter(|s| !s.hands.is_empty()) else { return Vec::new() };
    let (frame, fallback) = anim.frames(facing);
    held_with(db, grips_of(&sheet.hands, frame, fallback), eq, carrying, tool, facing)
}

/// Fists closer than this (image pixels) hold a two-handed thing along the right hand's axis.
const APART: f32 = 6.0;

/// The grip of both hands: between the fists, its axis the line through them (pointing the way
/// the right hand's axis does), else the one hand there is.
pub fn both_hands(right: Option<Grip>, left: Option<Grip>) -> Option<Grip> {
    match (right, left) {
        (Some(r), Some(l)) => {
            let (dx, dy) = (r.x - l.x, r.y - l.y);
            let angle = if dx.hypot(dy) >= APART {
                let a = dy.atan2(dx).to_degrees();
                // Of the line's two directions, the one closer to the right hand's axis.
                let off = (a - r.angle).to_radians().cos();
                if off >= 0.0 { a } else { a + 180.0 }
            } else {
                r.angle
            };
            Some(Grip { x: (r.x + l.x) / 2.0, y: (r.y + l.y) / 2.0, angle, scale: (r.scale + l.scale) / 2.0, front: r.front || l.front })
        }
        (r, l) => r.or(l),
    }
}

/// How a picture with hold point `grip` and orientation mark `points` lies on a hand with grip
/// axis `axis`: (hold point, rotation, mark, flipped) as drawn. An axis pointing left mirrors the
/// picture across its mark, so what is tilted up in the picture stays up facing either way
/// (`upright`: carried props instead never mirror nor turn upside down).
pub fn lay(axis: f32, grip: [f32; 2], points: f32, upright: bool) -> ([f32; 2], f32, f32, bool) {
    if upright {
        let mut rotation = turn(axis, points);
        if rotation.abs() > 90.0 {
            rotation = turn(axis + 180.0, points);
        }
        return (grip, rotation, points, false);
    }
    if axis.to_radians().cos() < -0.01 {
        ([grip[0], 1.0 - grip[1]], turn(axis, -points), -points, true)
    } else {
        (grip, turn(axis, points), points, false)
    }
}

/// The rotation that lays orientation mark `points` on axis `angle`.
pub fn turn(angle: f32, points: f32) -> f32 {
    let r = (angle - points).rem_euclid(360.0);
    if r > 180.0 { r - 360.0 } else { r }
}

/// [`held`] for given grips (the editor's previews of unsaved grips).
pub fn held_with(db: &Database, (right, left): (Option<Grip>, Option<Grip>), eq: &Equipment, carrying: &str, tool: Option<&str>, facing: Facing) -> Vec<HeldLayer> {
    let grip_of = |hand: Hand| match hand {
        Hand::Right => right,
        Hand::Left => left,
        Hand::Both => both_hands(right, left),
    };
    let mut out = Vec::new();
    if !carrying.is_empty() {
        if let (Some(p), Some(g)) = (db.prop(carrying), both_hands(right, left)) {
            let m = p.carry.clone().unwrap_or_else(HeldSprite::carried);
            let sprite = if m.sprite.is_empty() { p.sprite.clone() } else { m.sprite.clone() };
            let (grip, rotation, points, flip) = lay(g.angle, m.grip, m.points, true);
            out.push(HeldLayer { id: p.id.clone(), sprite, at: (g.x, g.y), grip, rotation, points, scale: g.scale * m.scale, front: facing != Facing::Up, hand: Hand::Both, flip });
        }
        return out;
    }
    let weapon = tool.or(eq.item(crate::defs::EquipSlot::Weapon));
    for (id, offhand) in [(weapon, false), (eq.item(crate::defs::EquipSlot::Offhand), true)] {
        let Some(d) = id.and_then(|id| db.item(id)) else { continue };
        let h = d.held.clone().unwrap_or_default();
        let hand = h.hand.unwrap_or(if offhand {
            Hand::Left
        } else {
            match &d.weapon {
                Some(w) if w.ranged.is_some() => Hand::Left,
                Some(w) if w.two_handed => Hand::Both,
                _ => Hand::Right,
            }
        });
        let Some(g) = grip_of(hand) else { continue };
        let sprite = if h.sprite.is_empty() { d.icon.clone() } else { h.sprite.clone() };
        let (grip, rotation, points, flip) = lay(g.angle, h.grip, h.points, false);
        out.push(HeldLayer { id: d.id.clone(), sprite, at: (g.x, g.y), grip, rotation, points, scale: g.scale * h.scale, front: g.front, hand, flip });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{ItemDef, SheetDef, SheetKind, Weapon};
    use crate::inventory::ItemStack;

    #[test]
    fn frame_hands_round_trip_as_one_line() {
        let h = FrameHands { frame: 12, right: Some(Grip { x: 41.0, y: 58.5, angle: -40.0, scale: 1.0, front: true }), left: Some(Grip { x: 22.0, y: 57.0, angle: 220.0, scale: 1.25, front: false }) };
        assert_eq!(h.to_line(), "12: R 41 58.5 -40 1 f | L 22 57 220 1.25 b");
        assert_eq!(FrameHands::from_line(&h.to_line()), Some(h.clone()));
        let ron = ron::to_string(&vec![h.clone()]).unwrap();
        assert_eq!(ron::from_str::<Vec<FrameHands>>(&ron).unwrap(), vec![h]);
        assert_eq!(FrameHands::from_line("3: L 1 2 90").unwrap().left.unwrap().scale, 1.0, "scale and front are optional");
        assert!(ron::from_str::<Vec<FrameHands>>("[\"oops\"]").is_err());
        let m = Grip::new(10.0, 5.0, -45.0).mirrored(64.0);
        assert_eq!((m.x, m.angle), (53.0, 225.0));
    }

    fn db() -> Database {
        let mut db = Database::default();
        let hands = vec![
            FrameHands { frame: 0, right: Some(Grip::new(40.0, 60.0, 90.0)), left: Some(Grip { front: false, ..Grip::new(24.0, 60.0, 90.0) }) },
            FrameHands { frame: 37, right: Some(Grip::new(50.0, 30.0, -60.0)), left: None },
        ];
        db.sheets.push(SheetDef { kind: SheetKind::Body, hands, ..SheetDef::new("hero", "hero.png", 64, 96) });
        let item = |id: &str| ItemDef { id: id.into(), name: id.into(), icon: SpriteRef::new("items", 1), ..Default::default() };
        db.items.push(ItemDef { weapon: Some(Weapon::melee(2, 0.4)), ..item("sword") });
        let mut bow = ItemDef { weapon: Some(Weapon::melee(1, 0.5)), ..item("bow") };
        bow.weapon.as_mut().unwrap().ranged = Some(crate::defs::Ranged { projectile: SpriteRef::new("items", 2), speed: 100.0, range: 6.0, ammo: String::new() });
        db.items.push(bow);
        db.items.push(item("shield"));
        db.props.push(crate::defs::PropDef { id: "crate".into(), sprite: SpriteRef::new("props", 3), ..crate::defs::PropDef::fresh("crate") });
        db
    }

    fn item_of(id: &str) -> ItemDef {
        ItemDef { id: id.into(), name: id.into(), icon: SpriteRef::new("items", 1), ..Default::default() }
    }

    #[test]
    fn items_go_into_their_hands_with_the_standing_fallback() {
        let db = db();
        let eq = Equipment { weapon: Some(ItemStack::new("sword", 1)), offhand: Some(ItemStack::new("shield", 1)), ..Default::default() };
        // Idle facing down (frame 16) has no grips: the standing frame 0's.
        let l = held(&db, "hero", &eq, "", None, Facing::Down, Anim::Idle(0));
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].id.as_str(), l[0].at, l[0].rotation, l[0].hand), ("sword", (40.0, 60.0), 135.0, Hand::Right));
        assert_eq!((l[1].id.as_str(), l[1].hand, l[1].front), ("shield", Hand::Left, false));
        // Attack facing left, frame 1 = frame (8 + 1) * 4 + 1 = 37: its own grip, no left hand there.
        let l = held(&db, "hero", &eq, "", None, Facing::Left, Anim::Attack(1));
        assert_eq!((l.len(), l[0].at), (1, (50.0, 30.0)));
        // A bow goes into the left hand; a tool replaces the weapon; a carried prop replaces all.
        let bow = Equipment { weapon: Some(ItemStack::new("bow", 1)), ..Default::default() };
        assert_eq!(held(&db, "hero", &bow, "", None, Facing::Down, Anim::Walk(0))[0].hand, Hand::Left);
        assert_eq!(held(&db, "hero", &eq, "", Some("bow"), Facing::Down, Anim::Walk(0))[0].id, "bow");
        let c = held(&db, "hero", &eq, "crate", None, Facing::Down, Anim::Walk(0));
        assert_eq!((c.len(), c[0].id.as_str(), c[0].at, c[0].rotation), (1, "crate", (32.0, 60.0), 0.0));
        // A figure without hands holds nothing visible.
        let mut db2 = db.clone();
        db2.sheets[0].hands.clear();
        assert!(held(&db2, "hero", &eq, "", None, Facing::Down, Anim::Walk(0)).is_empty());
        // Two-handed: the axis through both fists, pointing the right hand's way; a spear marked
        // pointing up in its picture (-90) lies on it.
        let mut db3 = db.clone();
        db3.items.push(ItemDef { weapon: Some(Weapon { two_handed: true, ..Weapon::melee(2, 0.5) }), held: Some(HeldSprite { points: -90.0, ..Default::default() }), ..item_of("spear") });
        db3.sheets[0].hands.push(FrameHands { frame: 4, right: Some(Grip::new(30.0, 40.0, -80.0)), left: Some(Grip::new(30.0, 60.0, 100.0)) });
        let spear = Equipment { weapon: Some(ItemStack::new("spear", 1)), ..Default::default() };
        let l = held(&db3, "hero", &spear, "", None, Facing::Left, Anim::Walk(0));
        assert_eq!((l[0].hand, l[0].at, l[0].rotation), (Hand::Both, (30.0, 50.0), 0.0));
        // Fists close together: the right hand's axis. A carried prop never turns upside down.
        assert_eq!(both_hands(Some(Grip::new(30.0, 40.0, 180.0)), Some(Grip::new(32.0, 41.0, 0.0))).unwrap().angle, 180.0);
        db3.sheets[0].hands.push(FrameHands { frame: 8, right: Some(Grip::new(30.0, 60.0, 180.0)), left: None });
        let c = held(&db3, "hero", &eq, "crate", None, Facing::Right, Anim::Walk(0));
        assert_eq!(c[0].rotation, 0.0);
        assert_eq!((turn(-90.0, 90.0), turn(10.0, -45.0)), (180.0, 55.0));
        // An axis pointing left mirrors the picture across its mark: a rod marked 40 degrees below
        // its shaft points up-right facing right and up-left facing left.
        let (_, r, p, flip) = lay(0.0, [0.5, 0.9], -50.0, false);
        assert_eq!((r + -90.0, flip), (-40.0, false));
        let (g, r, p2, flip) = lay(180.0, [0.5, 0.9], -50.0, false);
        assert!(flip && (g[1] - 0.1).abs() < 1e-6 && p2 == -p);
        assert_eq!(r + 90.0, 220.0, "the shaft (pointing down in the mirrored picture) turns up-left");
    }
}
