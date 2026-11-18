//! UI skins: [`SkinDef`] (`defs/skins.ron`) restyles the game's windows, menus, dialog box, HUD
//! and toasts without code; `ProjectConfig::skin` picks the one the game uses (empty = the
//! built-in look). Every part is optional: a part left empty keeps the built-in look.
//! Pictures are frames of sprite sheets drawn as nine-slices ([`NineSlice`]), so they travel
//! like any other art. `runtime/skin.rs` draws with the chosen skin; high contrast (Options ›
//! Accessibility) always uses the built-in look.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A colour with opacity, written `"#rrggbb"` or `"#rrggbbaa"` in files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkinColor(pub [u8; 4]);

impl SkinColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        SkinColor([r, g, b, 255])
    }

    /// `#rrggbb` or `#rrggbbaa` (the `#` may be left out).
    pub fn parse(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        if !h.is_ascii() || !(h.len() == 6 || h.len() == 8) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(SkinColor([byte(0)?, byte(2)?, byte(4)?, if h.len() == 8 { byte(6)? } else { 255 }]))
    }

    pub fn hex(&self) -> String {
        let [r, g, b, a] = self.0;
        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }

    pub fn rgb3(&self) -> [u8; 3] {
        [self.0[0], self.0[1], self.0[2]]
    }
}

impl Serialize for SkinColor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for SkinColor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        SkinColor::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("not a colour: {s:?} (write \"#rrggbb\" or \"#rrggbbaa\")")))
    }
}

/// A picture that stretches to any size: a frame of a sprite sheet cut into nine parts. The
/// corners keep their size, the edges and the middle stretch (or repeat) between them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NineSlice {
    /// Sprite sheet id.
    pub sheet: String,
    /// Frame of the sheet.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub frame: u32,
    /// Corner size in the picture's pixels.
    #[serde(default = "four")]
    pub corner: u32,
    /// Screen pixels per picture pixel: whole numbers keep pixel art crisp.
    #[serde(default = "two")]
    pub scale: u32,
    /// Repeat the edges and the middle instead of stretching them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tile: bool,
}

impl NineSlice {
    pub fn new(sheet: &str) -> Self {
        NineSlice { sheet: sheet.to_string(), frame: 0, corner: 4, scale: 2, tile: false }
    }
}

fn four() -> u32 {
    4
}
fn two() -> u32 {
    2
}
fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// The colours of texts in windows and menus. Empty = built-in.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextColors {
    /// Most text (built-in: white).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<SkinColor>,
    /// Secondary text: hints, descriptions, choices not selected (built-in: light grey).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<SkinColor>,
    /// The selected row, the open tab (built-in: gold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<SkinColor>,
    /// Entries that can't be chosen (built-in: grey).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<SkinColor>,
    /// Window titles and the speaker's name (built-in: gold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<SkinColor>,
    /// Good news: a better stat, enough money (built-in: green).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positive: Option<SkinColor>,
    /// Bad news: a worse stat, not enough money (built-in: red).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative: Option<SkinColor>,
}

/// The look of a HUD bar (hit points, mana, experience). Empty = built-in.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BarStyle {
    /// The filled part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<SkinColor>,
    /// The empty part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<SkinColor>,
    /// A picture drawn around the bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<NineSlice>,
}

impl BarStyle {
    pub fn is_empty(&self) -> bool {
        *self == BarStyle::default()
    }
}

/// A UI skin (`defs/skins.ron`). Its default is the empty skin (= the built-in look).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SkinDef {
    pub id: String,
    /// Name shown in the editor.
    #[serde(default)]
    pub name: String,
    /// The frame of every window and menu panel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<NineSlice>,
    /// A panel's fill when it has no frame picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<SkinColor>,
    /// A panel's border when it has no frame picture (also tab and slot outlines).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<SkinColor>,
    /// The dialog and choice box (empty = `frame`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialog: Option<NineSlice>,
    /// A box for the speaker's name on the dialog box's top edge (empty = the name inside the
    /// box, as built in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_box: Option<NineSlice>,
    /// The HUD panel (empty = `frame`, or the built-in gold frame).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hud: Option<NineSlice>,
    /// Buttons, tabs and the close button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<NineSlice>,
    /// A button under the pointer, the open tab (empty = `button`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_hover: Option<NineSlice>,
    /// A button's fill without a picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_fill: Option<SkinColor>,
    /// The fill of a button under the pointer or an open tab, without a picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_hover_fill: Option<SkinColor>,
    /// The selection marker of menus and lists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<NineSlice>,
    /// The selection marker's fill without a picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_fill: Option<SkinColor>,
    /// Toasts (the short messages at the top).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toast: Option<NineSlice>,
    /// A toast's fill without a picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toast_fill: Option<SkinColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toast_text: Option<SkinColor>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub text: TextColors,
    #[serde(default, skip_serializing_if = "BarStyle::is_empty")]
    pub hp: BarStyle,
    #[serde(default, skip_serializing_if = "BarStyle::is_empty")]
    pub mana: BarStyle,
    #[serde(default, skip_serializing_if = "BarStyle::is_empty")]
    pub xp: BarStyle,
    /// Colour names for dialog texts (`{c:ember}`): new names, or other shades of the built-in
    /// ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<(String, SkinColor)>,
}

fn is_default(t: &TextColors) -> bool {
    *t == TextColors::default()
}

impl SkinDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> Self {
        SkinDef { id: id.to_string(), name: id.to_string(), ..Default::default() }
    }

    /// Every picture of the skin with the part's name ("frame", "mana bar", …).
    pub fn pictures(&self) -> Vec<(&'static str, &NineSlice)> {
        let parts = [
            ("frame", &self.frame),
            ("dialog", &self.dialog),
            ("name box", &self.name_box),
            ("HUD", &self.hud),
            ("button", &self.button),
            ("button under the pointer", &self.button_hover),
            ("cursor", &self.cursor),
            ("toast", &self.toast),
            ("health bar", &self.hp.frame),
            ("mana bar", &self.mana.frame),
            ("experience bar", &self.xp.frame),
        ];
        parts.into_iter().filter_map(|(n, p)| p.as_ref().map(|p| (n, p))).collect()
    }

    pub fn pictures_mut(&mut self) -> Vec<&mut NineSlice> {
        [
            &mut self.frame,
            &mut self.dialog,
            &mut self.name_box,
            &mut self.hud,
            &mut self.button,
            &mut self.button_hover,
            &mut self.cursor,
            &mut self.toast,
            &mut self.hp.frame,
            &mut self.mana.frame,
            &mut self.xp.frame,
        ]
        .into_iter()
        .filter_map(|p| p.as_mut())
        .collect()
    }

    /// A dialog colour name of this skin (`{c:name}`), any case.
    pub fn color(&self, name: &str) -> Option<[u8; 3]> {
        self.colors.iter().find(|(n, _)| n.eq_ignore_ascii_case(name.trim())).map(|(_, c)| c.rgb3())
    }
}

/// Where the nine parts of a nine-slice go: (source rect, destination rect) in drawing order,
/// as `[x, y, w, h]`. `src` is the frame in the picture, `dest` the place on screen; corners are
/// `corner` picture pixels drawn `scale` times as big, made smaller when the place is too small
/// for two corners. Parts with no size are left out.
pub fn nine_parts(src: [f32; 4], dest: [f32; 4], corner: f32, scale: f32) -> Vec<([f32; 4], [f32; 4])> {
    let [sx, sy, sw, sh] = src;
    let [dx, dy, dw, dh] = dest;
    let c = corner.min(sw / 2.0).min(sh / 2.0).max(0.0);
    // The corner on screen, never more than half the place.
    let dc = (c * scale.max(1.0)).min(dw / 2.0).min(dh / 2.0).floor();
    let cols = [(sx, c, dx, dc), (sx + c, sw - 2.0 * c, dx + dc, dw - 2.0 * dc), (sx + sw - c, c, dx + dw - dc, dc)];
    let rows = [(sy, c, dy, dc), (sy + c, sh - 2.0 * c, dy + dc, dh - 2.0 * dc), (sy + sh - c, c, dy + dh - dc, dc)];
    let mut out = Vec::with_capacity(9);
    for (y0, yl, ty, th) in rows {
        for (x0, xl, tx, tw) in cols {
            if xl > 0.0 && yl > 0.0 && tw > 0.0 && th > 0.0 {
                out.push(([x0, y0, xl, yl], [tx, ty, tw, th]));
            }
        }
    }
    out
}

/// A stretched part cut into repeats: `len` screen pixels filled with copies of a `piece`
/// picture pixels long drawn `scale` times as big; the last copy is cut short. Returns
/// (length in picture pixels, screen offset, screen length) per copy.
pub fn repeats(piece: f32, len: f32, scale: f32) -> Vec<(f32, f32, f32)> {
    let step = piece * scale.max(1.0);
    if step <= 0.0 || len <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut at = 0.0;
    while at < len - 0.001 {
        let l = step.min(len - at);
        out.push((l / scale.max(1.0), at, l));
        at += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip() {
        assert_eq!(SkinColor::parse("#ff8000").unwrap().0, [255, 128, 0, 255]);
        assert_eq!(SkinColor::parse("10203040").unwrap().0, [16, 32, 48, 64]);
        assert!(SkinColor::parse("#fff").is_none());
        assert_eq!(SkinColor([1, 2, 3, 255]).hex(), "#010203");
        assert_eq!(SkinColor([1, 2, 3, 4]).hex(), "#01020304");
        let s = SkinDef { id: "wood".into(), background: Some(SkinColor([10, 20, 30, 200])), colors: vec![("ember".into(), SkinColor::rgb(250, 120, 40))], ..Default::default() };
        let text = ron::to_string(&s).unwrap();
        assert!(text.contains("\"#0a141ec8\""), "{text}");
        assert_eq!(ron::from_str::<SkinDef>(&text).unwrap(), s);
        assert_eq!(s.color("Ember"), Some([250, 120, 40]));
        assert!(ron::from_str::<SkinDef>(r#"(id: "x", border: "red")"#).is_err());
    }

    #[test]
    fn a_skin_without_parts_is_just_an_id() {
        let s: SkinDef = ron::from_str(r#"(id: "plain")"#).unwrap();
        assert_eq!(s, SkinDef { id: "plain".into(), ..Default::default() });
        let n: NineSlice = ron::from_str(r#"(sheet: "ui")"#).unwrap();
        assert_eq!(n, NineSlice::new("ui"));
    }

    #[test]
    fn nine_parts_keep_corners_whole() {
        // A 12 px frame with 4 px corners at 2x into a 100 x 50 place.
        let p = nine_parts([0.0, 0.0, 12.0, 12.0], [10.0, 20.0, 100.0, 50.0], 4.0, 2.0);
        assert_eq!(p.len(), 9);
        assert_eq!(p[0], ([0.0, 0.0, 4.0, 4.0], [10.0, 20.0, 8.0, 8.0]));
        assert_eq!(p[4], ([4.0, 4.0, 4.0, 4.0], [18.0, 28.0, 84.0, 34.0]));
        assert_eq!(p[8], ([8.0, 8.0, 4.0, 4.0], [102.0, 62.0, 8.0, 8.0]));
        // Too small for two whole corners: they shrink, the middle vanishes.
        let p = nine_parts([0.0, 0.0, 12.0, 12.0], [0.0, 0.0, 10.0, 40.0], 4.0, 2.0);
        assert!(p.iter().all(|(_, d)| d[2] > 0.0 && d[3] > 0.0));
        assert_eq!(p[0].1, [0.0, 0.0, 5.0, 5.0]);
        assert_eq!(p.len(), 6, "no middle column");
    }

    #[test]
    fn repeats_fill_the_length() {
        let r = repeats(4.0, 20.0, 2.0);
        assert_eq!(r, vec![(4.0, 0.0, 8.0), (4.0, 8.0, 8.0), (2.0, 16.0, 4.0)]);
        assert!(repeats(0.0, 10.0, 2.0).is_empty());
    }
}
