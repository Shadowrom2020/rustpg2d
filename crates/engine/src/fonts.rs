//! Project fonts: [`FontDef`] (`defs/fonts.ron`: a TTF / OTF file under `fonts/` with its
//! pixel size and licence) and [`FontRoles`] (`ProjectConfig::fonts`: which font the dialogs,
//! windows, HUD and title screen use, and the fallbacks tried per character). Without fonts the
//! game draws with macroquad's built-in ProggyClean, which has Latin-1 only ([`builtin_covers`]).
//! `runtime/text.rs` draws with them; the editor's problem check asks [`Coverage`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// A font file of the project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontDef {
    pub id: String,
    /// Name shown in the editor.
    #[serde(default)]
    pub name: String,
    /// TTF / OTF file, relative to the project root (`fonts/…`).
    pub file: String,
    /// The pixel size a pixel font is drawn for (its glyphs sit on that grid): drawn sizes snap
    /// to whole multiples of it, so it stays crisp. 0 = a smooth font, drawn at any size.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pixel: u32,
    /// Size against the built-in font: 1 = text asked for at 20 px is drawn at 20 px (before
    /// snapping); fonts that look small at the same size take more.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f32,
    /// Moves the text down (+) or up (-) by this part of the drawn size, for fonts whose baseline
    /// sits differently from the built-in font's.
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub offset_y: f32,
    /// The font's licence text (a file under `fonts/`), exported with the font.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    /// Who made it ("monogram by datagoblin, CC0").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub credit: String,
}

impl Default for FontDef {
    fn default() -> Self {
        FontDef { id: String::new(), name: String::new(), file: String::new(), pixel: 0, scale: 1.0, offset_y: 0.0, license: String::new(), credit: String::new() }
    }
}

impl FontDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> Self {
        FontDef { id: id.to_string(), name: id.to_string(), ..Default::default() }
    }

    /// The pixel size `size` (asked for as with the built-in font) is drawn at: scaled, and for
    /// a pixel font snapped to a whole multiple of its pixel size (at least once).
    pub fn draw_size(&self, size: f32) -> f32 {
        let s = size * if self.scale > 0.0 { self.scale } else { 1.0 };
        if self.pixel > 0 {
            let p = self.pixel as f32;
            (s / p).round().max(1.0) * p
        } else {
            s.round().max(1.0)
        }
    }
}

fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_zero(v: &u32) -> bool {
    *v == 0
}
fn is_zero_f(v: &f32) -> bool {
    *v == 0.0
}

/// What a piece of text is: picks the font it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Menus, windows, tooltips: everything without a role of its own.
    Ui,
    /// Dialog and choice boxes, speech bubbles, captions.
    Dialog,
    /// The HUD: bars, the action bar, toasts, numbers over heads.
    Hud,
    /// The game's name on the title screen.
    Title,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Ui, Role::Dialog, Role::Hud, Role::Title];

    pub fn name(self) -> &'static str {
        match self {
            Role::Ui => "ui",
            Role::Dialog => "dialog",
            Role::Hud => "hud",
            Role::Title => "title",
        }
    }
}

/// Which fonts the game uses (`ProjectConfig::fonts`): font ids; empty = the built-in font for
/// `ui`, and `ui`'s font for the others.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FontRoles {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ui: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dialog: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hud: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Tried per character, in order, when a role's font has no glyph for it (e.g. a CJK font);
    /// the built-in font comes last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback: Vec<String>,
}

impl FontRoles {
    pub fn is_empty(&self) -> bool {
        *self == FontRoles::default()
    }

    /// The font id a role is set to (empty: none).
    pub fn get(&self, role: Role) -> &str {
        match role {
            Role::Ui => &self.ui,
            Role::Dialog => &self.dialog,
            Role::Hud => &self.hud,
            Role::Title => &self.title,
        }
    }

    pub fn get_mut(&mut self, role: Role) -> &mut String {
        match role {
            Role::Ui => &mut self.ui,
            Role::Dialog => &mut self.dialog,
            Role::Hud => &mut self.hud,
            Role::Title => &mut self.title,
        }
    }

    /// The font a role draws with first: its own, else `ui`'s (empty = the built-in font).
    pub fn resolve(&self, role: Role) -> &str {
        let own = self.get(role);
        if own.is_empty() {
            &self.ui
        } else {
            own
        }
    }

    /// The fonts a role tries in order (ids, without the built-in font at the end).
    pub fn chain(&self, role: Role) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for id in std::iter::once(self.resolve(role)).chain(self.fallback.iter().map(String::as_str)) {
            if !id.is_empty() && !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }

    /// Every font id used.
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        [&self.ui, &self.dialog, &self.hud, &self.title].into_iter().chain(self.fallback.iter()).filter(|s| !s.is_empty())
    }
}

/// Whether macroquad's built-in font (ProggyClean) has a glyph for `c`: Latin-1 and the euro sign.
pub fn builtin_covers(c: char) -> bool {
    matches!(c as u32, 0x20..=0x7e | 0xa0..=0xff | 0x20ac)
}

/// Characters that need no glyph (line breaks, tabs, zero-width marks).
pub fn invisible(c: char) -> bool {
    c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{feff}')
}

/// The characters a font file has glyphs for (from its character maps).
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    chars: BTreeSet<u32>,
}

impl Coverage {
    /// Reads a TTF / OTF file's character maps; `None` if it is no font.
    pub fn of(bytes: &[u8]) -> Option<Coverage> {
        let face = ttf_parser::Face::parse(bytes, 0).ok()?;
        let mut chars = BTreeSet::new();
        for t in face.tables().cmap?.subtables {
            if t.is_unicode() {
                t.codepoints(|c| {
                    if t.glyph_index(c).is_some_and(|g| g.0 != 0) {
                        chars.insert(c);
                    }
                });
            }
        }
        Some(Coverage { chars })
    }

    /// Coverage of exactly these characters (tests, previews).
    pub fn from_chars(chars: impl IntoIterator<Item = char>) -> Coverage {
        Coverage { chars: chars.into_iter().map(|c| c as u32).collect() }
    }

    pub fn has(&self, c: char) -> bool {
        self.chars.contains(&(c as u32))
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_snap_to_the_pixel_grid() {
        let f = FontDef { pixel: 12, ..FontDef::fresh("m") };
        assert_eq!(f.draw_size(20.0), 24.0);
        assert_eq!(f.draw_size(16.0), 12.0);
        assert_eq!(f.draw_size(4.0), 12.0, "never below one pixel size");
        assert_eq!(f.draw_size(31.0), 36.0);
        let big = FontDef { scale: 1.5, ..f.clone() };
        assert_eq!(big.draw_size(16.0), 24.0);
        let smooth = FontDef { pixel: 0, ..f };
        assert_eq!(smooth.draw_size(20.4), 20.0);
    }

    #[test]
    fn roles_fall_back_to_ui_and_the_fallbacks() {
        let r = FontRoles { ui: "mono".into(), title: "big".into(), fallback: vec!["cjk".into(), "mono".into()], ..Default::default() };
        assert_eq!(r.chain(Role::Dialog), vec!["mono", "cjk"]);
        assert_eq!(r.chain(Role::Title), vec!["big", "cjk", "mono"]);
        assert!(FontRoles::default().chain(Role::Hud).is_empty());
        assert!(FontRoles::default().is_empty());
    }

    #[test]
    fn the_builtin_font_has_latin_1() {
        assert!("Grüße, Señor! 5 €".chars().all(builtin_covers));
        assert!(!builtin_covers('ж'));
        assert!(!builtin_covers('日'));
    }

    #[test]
    fn defaults_match_the_file_format() {
        let f: FontDef = ron::from_str("(id: \"a\", file: \"fonts/a.ttf\")").unwrap();
        assert_eq!(f, FontDef { id: "a".into(), file: "fonts/a.ttf".into(), ..Default::default() });
        let r: FontRoles = ron::from_str("()").unwrap();
        assert!(r.is_empty());
    }
}
