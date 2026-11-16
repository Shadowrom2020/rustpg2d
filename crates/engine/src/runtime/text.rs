//! The runtime's text drawing, in the current language and the project's fonts.
//!
//! `draw_text` / `measure_text` (used instead of macroquad's by the runtime's windows, menus
//! and HUD) look the whole string up in the language's runtime texts first (`lang::tr_ui`), so a
//! text and its measured width always agree. Content (dialog lines, names) is translated earlier
//! (`lang::tr`) and drawn with [`draw_plain`] / [`measure_plain`], which skip that lookup.
//!
//! Fonts: [`install`] sets the project's fonts (`ProjectConfig::fonts`, `defs/fonts.ron`). The
//! [`role`] of what is drawn picks the font (dialog boxes, the HUD, the title screen, else the
//! windows' font). A string is split into runs by the first font of the role's chain with a glyph
//! for each character (the role's font, the fallbacks, then the built-in font), so a Japanese
//! word in a German line takes the CJK fallback for its characters only. Without project fonts
//! everything goes straight to macroquad's built-in font, as before.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use macroquad::prelude::{Color, TextDimensions};
use macroquad::text::{Font, TextParams};

pub use crate::fonts::Role;
use crate::fonts::{builtin_covers, invisible, Coverage, FontDef, FontRoles};

/// A loaded project font.
struct Face {
    /// (`None` only in tests, which have no macroquad context to load fonts.)
    font: Option<Font>,
    def: FontDef,
    cover: Coverage,
}

/// The fonts the game draws with.
#[derive(Default)]
struct FontSet {
    faces: Vec<Face>,
    /// Per role (`Role::ALL` order): the faces tried in order; the built-in font comes after.
    chains: [Vec<usize>; 4],
    /// Characters no font had (for `status`; drawn with the built-in font's empty box).
    missing: BTreeSet<char>,
}

thread_local! {
    // Never dropped: at exit the thread's locals go after macroquad has closed the graphics
    // context, and freeing a font's texture then crashes the game (the system frees it anyway).
    // [`install`] replacing the set while the game runs still frees the old one.
    static SET: std::mem::ManuallyDrop<RefCell<FontSet>> = std::mem::ManuallyDrop::new(RefCell::new(FontSet::default()));
    static ROLE: Cell<Role> = const { Cell::new(Role::Ui) };
}

fn role_index(r: Role) -> usize {
    Role::ALL.iter().position(|x| *x == r).unwrap_or(0)
}

/// Sets the fonts the game draws with: `fonts` = the definitions with their files' bytes (those
/// that fail to load are left out and logged), `roles` = which font each role uses. Empty =
/// the built-in font everywhere.
pub fn install(fonts: Vec<(FontDef, Vec<u8>)>, roles: &FontRoles) {
    let mut set = FontSet::default();
    for (def, bytes) in fonts {
        let Some(cover) = Coverage::of(&bytes) else {
            eprintln!("[game] font '{}' ({}) is no TTF / OTF font; the built-in font is used instead", def.id, def.file);
            continue;
        };
        match macroquad::text::load_ttf_font_from_bytes(&bytes) {
            Ok(mut font) => {
                if def.pixel > 0 {
                    font.set_filter(macroquad::miniquad::FilterMode::Nearest);
                }
                set.faces.push(Face { font: Some(font), def, cover });
            }
            Err(e) => eprintln!("[game] font '{}' ({}) can't be loaded: {e:?}", def.id, def.file),
        }
    }
    for (k, r) in Role::ALL.iter().enumerate() {
        set.chains[k] = roles.chain(*r).into_iter().filter_map(|id| set.faces.iter().position(|f| f.def.id == id)).collect();
    }
    SET.with(|s| *s.borrow_mut() = set);
}

/// Loads and installs the fonts `project` uses (the files through `sys`, so packed and web
/// games find them too).
pub fn install_project(project: &crate::project::Project) {
    let roles = &project.config.fonts;
    let mut fonts = Vec::new();
    for id in roles.ids().collect::<BTreeSet<_>>() {
        let Some(def) = project.db.font(id) else {
            eprintln!("[game] the project uses the font '{id}', which isn't defined; the built-in font is used instead");
            continue;
        };
        match crate::sys::read(&project.root.join(&def.file)) {
            Ok(bytes) => fonts.push((def.clone(), bytes)),
            Err(e) => eprintln!("[game] font '{id}': can't read {}: {e}", def.file),
        }
    }
    install(fonts, roles);
}

/// While the returned guard lives, texts are drawn in `r`'s font (the previous role comes back
/// when it is dropped): `let _font = text::role(Role::Dialog);` at the top of a drawing function.
#[must_use = "the role holds only while the guard lives"]
pub fn role(r: Role) -> RoleGuard {
    RoleGuard(ROLE.with(|c| c.replace(r)))
}

pub struct RoleGuard(Role);

impl Drop for RoleGuard {
    fn drop(&mut self) {
        ROLE.with(|c| c.set(self.0));
    }
}

/// For `status()`: the role fonts as role:id (`builtin` without one) and how many characters no
/// font had.
pub fn status() -> (Vec<String>, usize) {
    SET.with(|s| {
        let s = s.borrow();
        let names = Role::ALL.iter().enumerate().map(|(k, r)| format!("{}:{}", r.name(), s.chains[k].first().map_or("builtin", |&f| s.faces[f].def.id.as_str()))).collect();
        (names, s.missing.len())
    })
}

/// A run of `text` and the face it is drawn with (`None` = the built-in font).
type Run<'a> = (&'a str, Option<usize>);

/// Splits `text` by font: each character goes to the first face of the role's chain that has
/// it; spaces and marks stay with the run they are in.
fn runs<'a>(set: &mut FontSet, text: &'a str) -> Vec<Run<'a>> {
    let chain = &set.chains[role_index(ROLE.with(Cell::get))];
    let mut out: Vec<Run<'a>> = Vec::new();
    let mut start = 0;
    let mut cur: Option<Option<usize>> = None;
    for (i, c) in text.char_indices() {
        let neutral = c == ' ' || invisible(c);
        let face = if neutral {
            cur.unwrap_or_else(|| chain.first().copied())
        } else {
            let f = chain.iter().copied().find(|&f| set.faces[f].cover.has(c));
            if f.is_none() && !builtin_covers(c) {
                set.missing.insert(c);
            }
            f
        };
        match cur {
            Some(prev) if prev != face => {
                out.push((&text[start..i], prev));
                start = i;
            }
            _ => {}
        }
        cur = Some(face);
    }
    if let Some(face) = cur {
        out.push((&text[start..], face));
    }
    out
}

/// The macroquad parameters a run is drawn with: its font, size and baseline shift.
fn params<'a>(set: &'a FontSet, face: Option<usize>, size: f32, color: Color) -> (TextParams<'a>, f32) {
    match face {
        Some(f) => {
            let face = &set.faces[f];
            let px = face.def.draw_size(size);
            (TextParams { font: face.font.as_ref(), font_size: px as u16, font_scale: 1.0, color, ..Default::default() }, face.def.offset_y * px)
        }
        None => (TextParams { font_size: size as u16, font_scale: 1.0, color, ..Default::default() }, 0.0),
    }
}

/// Whether there are project fonts to pick from (else everything is the built-in font).
fn has_fonts() -> bool {
    SET.with(|s| !s.borrow().faces.is_empty())
}

/// Draws a runtime text in the current language.
pub fn draw_text(text: impl AsRef<str>, x: f32, y: f32, font_size: f32, color: Color) -> TextDimensions {
    draw_plain(&crate::lang::tr_ui(text.as_ref()), x, y, font_size, color)
}

/// Measures a runtime text as [`draw_text`] draws it (`font` is ignored: the role picks it).
pub fn measure_text(text: &str, _font: Option<&Font>, font_size: u16, font_scale: f32) -> TextDimensions {
    measure_plain(&crate::lang::tr_ui(text), font_size as f32 * font_scale)
}

/// Draws text that is already in the player's language (content, labels), at whole pixels.
pub fn draw_plain(text: &str, x: f32, y: f32, font_size: f32, color: Color) -> TextDimensions {
    // Whole pixels: at fractional positions the font's one-pixel details (umlaut dots) fade.
    let (x, y) = (x.round(), y.round());
    if !has_fonts() {
        note_missing(text);
        return macroquad::text::draw_text(text, x, y, font_size, color);
    }
    SET.with(|s| {
        let mut set = s.borrow_mut();
        let runs = runs(&mut set, text);
        let mut dims = TextDimensions { width: 0.0, height: 0.0, offset_y: 0.0 };
        for (part, face) in runs {
            let (p, dy) = params(&set, face, font_size, color);
            let d = macroquad::text::draw_text_ex(part, (x + dims.width).round(), (y + dy).round(), p);
            dims.width += d.width;
            dims.height = dims.height.max(d.height);
            dims.offset_y = dims.offset_y.max(d.offset_y);
        }
        dims
    })
}

/// Measures text as [`draw_plain`] draws it at `font_size`.
pub fn measure_plain(text: &str, font_size: f32) -> TextDimensions {
    if !has_fonts() {
        return macroquad::text::measure_text(text, None, font_size as u16, 1.0);
    }
    SET.with(|s| {
        let mut set = s.borrow_mut();
        let runs = runs(&mut set, text);
        let mut dims = TextDimensions { width: 0.0, height: 0.0, offset_y: 0.0 };
        for (part, face) in runs {
            let (p, _) = params(&set, face, font_size, Color::default());
            let d = macroquad::text::measure_text(part, p.font, p.font_size, p.font_scale);
            dims.width += d.width;
            dims.height = dims.height.max(d.height);
            dims.offset_y = dims.offset_y.max(d.offset_y);
        }
        dims
    })
}

/// Without project fonts: remembers characters the built-in font lacks (for `status`).
fn note_missing(text: &str) {
    if text.chars().all(|c| builtin_covers(c) || invisible(c)) {
        return;
    }
    SET.with(|s| s.borrow_mut().missing.extend(text.chars().filter(|&c| !builtin_covers(c) && !invisible(c))));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faces with the given characters, the first as every role's font, the rest as fallbacks.
    fn set_with(covers: &[&str]) -> FontSet {
        let faces = covers.iter().enumerate().map(|(k, c)| Face { font: None, def: FontDef::fresh(&format!("f{k}")), cover: Coverage::from_chars(c.chars()) }).collect();
        let chain: Vec<usize> = (0..covers.len()).collect();
        FontSet { faces, chains: [chain.clone(), chain.clone(), chain.clone(), chain], missing: BTreeSet::new() }
    }

    #[test]
    fn without_fonts_everything_is_one_builtin_run() {
        let mut set = set_with(&[]);
        assert_eq!(runs(&mut set, "Grüße, Mira"), vec![("Grüße, Mira", None)]);
        assert_eq!(runs(&mut set, "ж"), vec![("ж", None)]);
        assert!(set.missing.contains(&'ж'));
        assert!(!set.missing.contains(&'ü'));
    }

    #[test]
    fn text_is_split_by_the_first_font_with_the_glyph() {
        let mut set = set_with(&["Hilo ж", "日本語"]);
        // Spaces stay with the run they are in; a character no font has goes to the built-in font.
        assert_eq!(runs(&mut set, "Hi 日本 ж!"), vec![("Hi ", Some(0)), ("日本 ", Some(1)), ("ж", Some(0)), ("!", None)]);
        assert!(set.missing.is_empty(), "the built-in font has '!'");
        assert_eq!(runs(&mut set, "語€"), vec![("語", Some(1)), ("€", None)]);
        runs(&mut set, "ω");
        assert_eq!(set.missing.iter().collect::<Vec<_>>(), vec![&'ω']);
    }
}
