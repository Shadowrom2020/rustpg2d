//! Dialog message codes: markup inside dialog and choice texts, in braces like the
//! placeholders of `WorldState::expand_text` (which leaves them alone), so translators see one
//! syntax:
//!
//! - `{c:gold}…{/c}` colours text: a named colour ([`COLORS`]) or `#rrggbb`; `{/c}` goes back;
//! - `{i:item_id}` shows an item's icon inside the line;
//! - `{w:0.5}` pauses the typing for that many seconds;
//! - `{s:2}` sets the typing speed (1 = normal, 2 = twice as fast);
//! - `{name:Someone}` puts another name into the name box;
//! - `{at:top}` / `{at:middle}` / `{at:bottom}` places the box.

/// Where the dialog box sits on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxAt {
    #[default]
    Bottom,
    Middle,
    Top,
}

impl BoxAt {
    pub const ALL: [BoxAt; 3] = [BoxAt::Bottom, BoxAt::Middle, BoxAt::Top];

    pub fn name(self) -> &'static str {
        match self {
            BoxAt::Bottom => "bottom",
            BoxAt::Middle => "middle",
            BoxAt::Top => "top",
        }
    }

    pub fn parse(s: &str) -> Option<BoxAt> {
        Self::ALL.into_iter().find(|b| b.name() == s.trim())
    }
}

/// One code.
#[derive(Clone, Debug, PartialEq)]
pub enum Code {
    /// A text colour; `None` = back to the usual colour.
    Color(Option<[u8; 3]>),
    /// An item's icon (its id).
    Icon(String),
    /// A pause in seconds.
    Wait(f32),
    /// The typing speed from here on (1 = normal).
    Speed(f32),
    /// The name shown in the name box.
    Name(String),
    /// The box's place.
    At(BoxAt),
}

/// The named colours of `{c:name}`.
pub const COLORS: &[(&str, [u8; 3])] = &[
    ("white", [255, 255, 255]),
    ("gray", [170, 170, 170]),
    ("red", [235, 80, 70]),
    ("green", [110, 210, 100]),
    ("blue", [100, 160, 255]),
    ("cyan", [90, 220, 230]),
    ("yellow", [250, 230, 90]),
    ("gold", [255, 203, 0]),
    ("orange", [250, 150, 50]),
    ("purple", [190, 120, 250]),
    ("pink", [250, 140, 200]),
    ("brown", [190, 130, 80]),
];

/// The prefixes of other `{prefix:…}` placeholders (not dialog codes, never "unknown").
const OTHER_PREFIXES: &[&str] = &["key", "uid"];

thread_local! {
    /// The colour names of the game's UI skin ([`set_skin_colors`]).
    static SKIN_COLORS: std::cell::RefCell<Vec<(String, [u8; 3])>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The colour names the UI skin adds or changes (`SkinDef::colors`): they win over [`COLORS`].
/// Set by the game when it installs its skin, and by the editor for its checks.
pub fn set_skin_colors(colors: Vec<(String, [u8; 3])>) {
    SKIN_COLORS.with(|c| *c.borrow_mut() = colors);
}

/// The colour of a name (the skin's names first) or `#rrggbb`.
pub fn color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim();
    if let Some(c) = SKIN_COLORS.with(|c| c.borrow().iter().find(|(n, _)| n.eq_ignore_ascii_case(s)).map(|(_, c)| *c)) {
        return Some(c);
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        return Some([byte(0)?, byte(2)?, byte(4)?]);
    }
    COLORS.iter().find(|(n, _)| n.eq_ignore_ascii_case(s)).map(|(_, c)| *c)
}

/// The code written between braces (`inner`, without them); `None` = not a valid dialog code.
pub fn code(inner: &str) -> Option<Code> {
    if inner == "/c" {
        return Some(Code::Color(None));
    }
    let (name, value) = inner.split_once(':')?;
    let num = || value.trim().parse::<f32>().ok().filter(|v| v.is_finite());
    match name {
        "c" => color(value).map(|c| Code::Color(Some(c))),
        "i" if !value.trim().is_empty() => Some(Code::Icon(value.trim().to_string())),
        "w" => num().filter(|v| *v >= 0.0 && *v <= 60.0).map(Code::Wait),
        "s" => num().filter(|v| *v > 0.0 && *v <= 100.0).map(Code::Speed),
        "name" => Some(Code::Name(value.to_string())),
        "at" => BoxAt::parse(value).map(Code::At),
        _ => None,
    }
}

/// A piece of a text: plain text or a code.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece<'t> {
    Text(&'t str),
    Code(Code, &'t str),
}

/// Splits a text into plain pieces and codes (with their source, braces included). Braces that
/// aren't a code stay text.
pub fn split(text: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut plain_from = 0;
    let mut at = 0;
    while let Some(i) = text[at..].find('{').map(|i| at + i) {
        let Some(j) = text[i + 1..].find('}').map(|j| i + 1 + j) else { break };
        let inner = &text[i + 1..j];
        match code(inner).filter(|_| !inner.contains('{')) {
            Some(c) => {
                if plain_from < i {
                    out.push(Piece::Text(&text[plain_from..i]));
                }
                out.push(Piece::Code(c, &text[i..=j]));
                plain_from = j + 1;
                at = j + 1;
            }
            None => at = i + 1,
        }
    }
    if plain_from < text.len() {
        out.push(Piece::Text(&text[plain_from..]));
    }
    out
}

/// The text without its codes (what the dialog log and `status` show).
pub fn plain(text: &str) -> String {
    split(text).into_iter().filter_map(|p| if let Piece::Text(t) = p { Some(t) } else { None }).collect()
}

/// The codes of a text as written (`{c:gold}`), sorted: a translation must keep the same ones.
pub fn written_codes(text: &str) -> Vec<&str> {
    let mut out: Vec<&str> = split(text).into_iter().filter_map(|p| if let Piece::Code(_, s) = p { Some(s) } else { None }).collect();
    out.sort_unstable();
    out
}

/// What looks like a code but isn't a valid one: `{x:…}` with an unknown name, or a known
/// name with a wrong value (`{c:sky}`, `{w:soon}`). Placeholders (`{player}`, `{a|b}`, a
/// variable, `{key:…}`) are not codes.
pub fn unknown_codes(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = text[at..].find('{').map(|i| at + i) {
        let Some(j) = text[i + 1..].find('}').map(|j| i + 1 + j) else { break };
        let inner = &text[i + 1..j];
        if inner.contains('{') {
            at = i + 1;
            continue;
        }
        let name = inner.split_once(':').map(|(n, _)| n).unwrap_or(inner);
        let shaped = (inner.contains(':') || inner.starts_with('/')) && !inner.contains(['|', ' ']) && !name.is_empty();
        if shaped && code(inner).is_none() && !OTHER_PREFIXES.contains(&name) {
            out.push(&text[i..=j]);
        }
        at = j + 1;
    }
    out
}

/// The item ids of a text's `{i:…}` codes.
pub fn icons(text: &str) -> Vec<String> {
    split(text).into_iter().filter_map(|p| if let Piece::Code(Code::Icon(id), _) = p { Some(id) } else { None }).collect()
}

/// The box's place set by an `{at:…}` code (the last one counts), and the text without it.
pub fn box_at(text: &str) -> BoxAt {
    split(text).into_iter().filter_map(|p| if let Piece::Code(Code::At(a), _) = p { Some(a) } else { None }).next_back().unwrap_or_default()
}

/// Sets (or, for `Bottom`, removes) the text's `{at:…}` code, at its start.
pub fn set_box_at(text: &str, at: BoxAt) -> String {
    let rest: String = split(text).into_iter().map(|p| match p {
        Piece::Text(t) => t,
        Piece::Code(Code::At(_), _) => "",
        Piece::Code(_, s) => s,
    }).collect();
    match at {
        BoxAt::Bottom => rest,
        at => format!("{{at:{}}}{rest}", at.name()),
    }
}

/// Splits a leading `{at:…}` code off a text: it isn't part of what translators translate
/// (`lang::content_texts` and the game leave it out of the lookup).
pub fn split_box_at(text: &str) -> (&str, &str) {
    if let Some(Piece::Code(Code::At(_), s)) = split(text).first() {
        return text.split_at(s.len());
    }
    ("", text)
}

/// The name an `{name:…}` code puts into the name box (the last one counts).
pub fn name_override(text: &str) -> Option<String> {
    split(text).into_iter().filter_map(|p| if let Piece::Code(Code::Name(n), _) = p { Some(n) } else { None }).next_back()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_parsed() {
        let t = "Take the {c:gold}{i:room_key} key{/c}.{w:0.5} {s:2}Quick!";
        let p = split(t);
        assert_eq!(p[0], Piece::Text("Take the "));
        assert_eq!(p[1], Piece::Code(Code::Color(Some([255, 203, 0])), "{c:gold}"));
        assert_eq!(p[2], Piece::Code(Code::Icon("room_key".into()), "{i:room_key}"));
        assert_eq!(p[3], Piece::Text(" key"));
        assert_eq!(p[4], Piece::Code(Code::Color(None), "{/c}"));
        assert_eq!(p[6], Piece::Code(Code::Wait(0.5), "{w:0.5}"));
        assert_eq!(p[8], Piece::Code(Code::Speed(2.0), "{s:2}"));
        assert_eq!(plain(t), "Take the  key. Quick!");
        assert_eq!(icons(t), vec!["room_key".to_string()]);
        assert_eq!(code("c:#20a0ff"), Some(Code::Color(Some([0x20, 0xa0, 0xff]))));
    }

    #[test]
    fn other_braces_stay_text() {
        // Placeholders are expanded before, but unexpanded ones and odd braces must survive.
        for t in ["{player}", "{sir|madam}", "a { b", "{key:confirm}", "{c:sky}", "{w:soon}", "{}"] {
            assert_eq!(plain(t), t);
        }
        assert_eq!(unknown_codes("{c:sky} {w:soon} {x:1} {/b} {key:confirm} {player} {sir|madam} {c:red}{/c}"), vec!["{c:sky}", "{w:soon}", "{x:1}", "{/b}"]);
    }

    #[test]
    fn name_and_place() {
        let t = "{at:top}{name:A voice}Who goes there?";
        assert_eq!(box_at(t), BoxAt::Top);
        assert_eq!(name_override(t).as_deref(), Some("A voice"));
        assert_eq!(plain(t), "Who goes there?");
        assert_eq!(set_box_at(t, BoxAt::Middle), "{at:middle}{name:A voice}Who goes there?");
        assert_eq!(set_box_at(t, BoxAt::Bottom), "{name:A voice}Who goes there?");
        assert_eq!(box_at("Hi"), BoxAt::Bottom);
        assert_eq!(split_box_at("{at:top}Hi {c:red}x{/c}"), ("{at:top}", "Hi {c:red}x{/c}"));
        assert_eq!(split_box_at("Hi{at:top}"), ("", "Hi{at:top}"));
    }

    #[test]
    fn translations_compare_codes() {
        assert_eq!(written_codes("{c:red}Fire{/c}{w:1}"), written_codes("{w:1}{c:red}Feuer{/c}"));
        assert_ne!(written_codes("{c:red}Fire{/c}"), written_codes("Feuer"));
    }
}
