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

use gfx::prelude::{Color, TextDimensions};
use gfx::text::{Font, TextParams};

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
        match gfx::text::load_ttf_font_from_bytes(&bytes) {
            Ok(mut font) => {
                if def.pixel > 0 {
                    font.set_filter(gfx::texture::FilterMode::Nearest);
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
        return gfx::text::draw_text(text, x, y, font_size, color);
    }
    SET.with(|s| {
        let mut set = s.borrow_mut();
        let runs = runs(&mut set, text);
        let mut dims = TextDimensions { width: 0.0, height: 0.0, offset_y: 0.0 };
        for (part, face) in runs {
            let (p, dy) = params(&set, face, font_size, color);
            let d = gfx::text::draw_text_ex(part, (x + dims.width).round(), (y + dy).round(), p);
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
        return gfx::text::measure_text(text, None, font_size as u16, 1.0);
    }
    SET.with(|s| {
        let mut set = s.borrow_mut();
        let runs = runs(&mut set, text);
        let mut dims = TextDimensions { width: 0.0, height: 0.0, offset_y: 0.0 };
        for (part, face) in runs {
            let (p, _) = params(&set, face, font_size, Color::default());
            let d = gfx::text::measure_text(part, p.font, p.font_size, p.font_scale);
            dims.width += d.width;
            dims.height = dims.height.max(d.height);
            dims.offset_y = dims.offset_y.max(d.offset_y);
        }
        dims
    })
}

/// `text` (already in the player's language) cut to `width` pixels at `font_size`, with "..."
/// at the end when cut.
pub fn fit_plain(text: &str, font_size: f32, width: f32) -> String {
    if measure_plain(text, font_size).width <= width {
        return text.to_string();
    }
    let mut cut: Vec<char> = text.chars().collect();
    while !cut.is_empty() && measure_plain(&format!("{}...", cut.iter().collect::<String>()), font_size).width > width {
        cut.pop();
    }
    format!("{}...", cut.into_iter().collect::<String>().trim_end())
}

/// The font size at most `size` (down to `min`, whole pixels) at which `text` (in the player's
/// language) is no wider than `width`: labels that would overflow their box get smaller.
pub fn fit_size(text: &str, size: f32, width: f32, min: f32) -> f32 {
    let mut s = size.floor();
    while s > min && measure_plain(text, s).width > width {
        s -= 1.0;
    }
    s
}

/// A runtime text translated, then cut like [`fit_plain`] (draw it with [`draw_plain`]).
pub fn fit_ui(text: &str, font_size: f32, width: f32) -> String {
    fit_plain(&crate::lang::tr_ui(text), font_size, width)
}

/// Whether `c` is written without spaces between words (Chinese, Japanese): a line may break
/// before and after it. Korean, which puts spaces between words, wraps at its spaces.
pub fn breaks_anywhere(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FFF        // CJK radicals, Kangxi radicals
        | 0x3000..=0x303F      // CJK symbols and punctuation
        | 0x3040..=0x30FF      // Hiragana, Katakana
        | 0x3100..=0x312F      // Bopomofo
        | 0x31F0..=0x31FF      // Katakana extensions
        | 0x3400..=0x4DBF      // CJK extension A
        | 0x4E00..=0x9FFF      // CJK unified ideographs
        | 0xF900..=0xFAFF      // CJK compatibility ideographs
        | 0xFF00..=0xFFEF      // full-width forms, half-width katakana
        | 0x20000..=0x3FFFF) // CJK extensions B and later
}

/// Punctuation a line never starts with (Japanese *kinsoku*: closing brackets, stops, small kana,
/// the long-vowel mark) and Latin closing punctuation after CJK text.
fn no_break_before(c: char) -> bool {
    "、。，．・：；？！ー〜～…‥）〕］｝〉》」』】〙〗〟’”ゝゞヽヾ々ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶㇰㇱㇲㇳㇴㇵㇶㇷㇸㇹㇺㇻㇼㇽㇾㇿ,.;:!?)]}%".contains(c)
}

/// Punctuation a line never ends with (opening brackets).
fn no_break_after(c: char) -> bool {
    "（〔［｛〈《「『【〘〖〝‘“([{".contains(c)
}

/// How far to move down for `line` at `font_size`: `step` (the layout's line spacing), or more
/// when the line is taller than that (a fallback font's glyphs: Japanese in a pixel font's
/// layout), so lines never touch.
pub fn line_step(line: &str, font_size: f32, step: f32) -> f32 {
    if !has_fonts() {
        return step;
    }
    step.max(measure_plain(line, font_size).height.ceil() + 4.0)
}

/// Lines of `text` no wider than `max_w` pixels at `font_size` in the current role's font: breaks
/// at spaces and line breaks, and in Chinese and Japanese text between characters too (never
/// before closing or after opening punctuation). A word wider than a line gets a line of its own.
pub fn wrap(text: &str, font_size: f32, max_w: f32) -> Vec<String> {
    wrap_counted(text, font_size, max_w).into_iter().map(|(l, _)| l).collect()
}

/// [`wrap`], each line with how many characters of `text` it uses (with the spaces or line break
/// it ends on): what a text that types out character by character shows of each line.
pub fn wrap_counted(text: &str, font_size: f32, max_w: f32) -> Vec<(String, usize)> {
    wrap_with(text, max_w, |s| measure_plain(s, font_size).width)
}

/// The wrapping itself, with any `measure` (tests measure without a graphics context).
pub(crate) fn wrap_with(text: &str, max_w: f32, measure: impl Fn(&str) -> f32) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let paragraphs: Vec<&str> = text.split('\n').collect();
    for (k, para) in paragraphs.iter().enumerate() {
        // Pieces that stay together: (spaces before it, its text).
        let mut pieces: Vec<(usize, String)> = Vec::new();
        let mut spaces = 0;
        let mut prev: Option<char> = None;
        for c in para.chars() {
            if c.is_whitespace() {
                spaces += 1;
                prev = None;
                continue;
            }
            let joins = match (pieces.last(), prev) {
                (Some(_), Some(p)) if spaces == 0 => !((breaks_anywhere(c) || breaks_anywhere(p)) && !no_break_before(c) && !no_break_after(p)),
                _ => false,
            };
            match pieces.last_mut() {
                Some((_, t)) if joins => t.push(c),
                _ => pieces.push((std::mem::take(&mut spaces), c.to_string())),
            }
            prev = Some(c);
        }
        let mut line = String::new();
        let mut used = 0;
        for (sp, piece) in pieces {
            let sep = if line.is_empty() || sp == 0 { "" } else { " " };
            let candidate = format!("{line}{sep}{piece}");
            if !line.is_empty() && measure(&candidate) > max_w {
                out.push((std::mem::take(&mut line), used + sp));
                used = 0;
                line = piece.clone();
            } else {
                // (Spaces before a paragraph's first piece are used but not shown.)
                line = candidate;
                used += sp;
            }
            used += piece.chars().count();
        }
        used += spaces + usize::from(k + 1 < paragraphs.len());
        out.push((line, used));
    }
    out
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

    /// Wraps with every character 10 px wide.
    fn wrapped(text: &str, max_w: f32) -> Vec<(String, usize)> {
        wrap_with(text, max_w, |s| s.chars().count() as f32 * 10.0)
    }

    #[test]
    fn latin_text_wraps_at_spaces() {
        assert_eq!(wrapped("the quick brown fox", 100.0), vec![("the quick".to_string(), 10), ("brown fox".to_string(), 9)]);
        // A word wider than the line keeps a line of its own; a line break always breaks.
        assert_eq!(wrapped("a extraordinarily b\nc", 50.0), vec![("a".to_string(), 2), ("extraordinarily".to_string(), 16), ("b".to_string(), 2), ("c".to_string(), 1)]);
        assert_eq!(wrapped("", 50.0), vec![(String::new(), 0)]);
    }

    #[test]
    fn japanese_breaks_between_characters_but_not_before_closing_marks() {
        // 6 characters per line.
        let lines = wrapped("銅のやかん亭へようこそ、アレックス！", 60.0);
        let text: Vec<&str> = lines.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(text, vec!["銅のやかん亭", "へようこそ、", "アレックス！"]);
        // Every character is used once: a typing-out text stays in step.
        assert_eq!(lines.iter().map(|(_, n)| n).sum::<usize>(), "銅のやかん亭へようこそ、アレックス！".chars().count());
        // 、 never starts a line: it goes with the character before it.
        let lines = wrapped("あいうえお、かき", 50.0);
        assert_eq!(lines.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), vec!["あいうえ", "お、かき"]);
        // 「 never ends a line.
        let lines = wrapped("あいう「えお」", 40.0);
        assert_eq!(lines.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), vec!["あいう", "「えお」"]);
    }

    #[test]
    fn mixed_text_breaks_between_latin_and_cjk() {
        let lines = wrapped("Alexは村へ行く", 60.0);
        assert_eq!(lines.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), vec!["Alexは村", "へ行く"]);
        // Korean keeps its words whole and breaks at spaces.
        let lines = wrapped("안녕하세요 반갑습니다", 60.0);
        assert_eq!(lines.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), vec!["안녕하세요", "반갑습니다"]);
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
