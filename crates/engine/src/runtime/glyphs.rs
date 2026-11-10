//! Gamepad button pictures, and hint lines that name the player's keys or show their pad's
//! buttons. A hint is a template with action tokens, `"{key:cancel}: close"`: translated as a
//! whole (`lang::tr_ui`), then every token is drawn as the action's first key ("Esc") or, while
//! the player uses a gamepad ([`input::device`]), as a picture of its button in the pad's style
//! (Xbox, also for the Steam Deck; PlayStation; Nintendo), detected from the pad's name or chosen
//! in the Options. The pictures are drawn from shapes and letters (the game font is Latin-1 only).
//! Content texts use the same tokens as plain text (`WorldState::expand_text`, [`key_text`]).

use std::cell::Cell;

use macroquad::prelude::*;

use super::input::{self, Act, Device, Pad};

/// How gamepad buttons are drawn and named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PadStyle {
    /// From the connected pad's name.
    #[default]
    Auto,
    Xbox,
    PlayStation,
    Nintendo,
}

impl PadStyle {
    pub const ALL: [PadStyle; 4] = [PadStyle::Auto, PadStyle::Xbox, PadStyle::PlayStation, PadStyle::Nintendo];

    /// The name in `settings.ron`.
    pub fn id(self) -> &'static str {
        match self {
            PadStyle::Auto => "auto",
            PadStyle::Xbox => "xbox",
            PadStyle::PlayStation => "playstation",
            PadStyle::Nintendo => "nintendo",
        }
    }

    pub fn from_id(id: &str) -> PadStyle {
        PadStyle::ALL.into_iter().find(|s| s.id() == id).unwrap_or_default()
    }

    /// How the Options page shows it.
    pub fn label(self) -> &'static str {
        match self {
            PadStyle::Auto => "Auto",
            PadStyle::Xbox => "Xbox",
            PadStyle::PlayStation => "PlayStation",
            PadStyle::Nintendo => "Nintendo",
        }
    }
}

thread_local! {
    static CHOSEN: Cell<PadStyle> = const { Cell::new(PadStyle::Auto) };
}

/// The player's choice (Options › Button style).
pub fn set_style(s: PadStyle) {
    CHOSEN.with(|c| c.set(s));
}

/// The style buttons are drawn in now: the player's choice, else the connected pad's.
pub fn style() -> PadStyle {
    match CHOSEN.with(|c| c.get()) {
        PadStyle::Auto => detect(&input::pad_name()),
        s => s,
    }
}

/// The style of a pad by its name; Xbox for everything else (also the Steam Deck).
fn detect(name: &str) -> PadStyle {
    let n = name.to_lowercase();
    if ["playstation", "dualsense", "dualshock", "sony", "ps3", "ps4", "ps5"].iter().any(|k| n.contains(k)) {
        PadStyle::PlayStation
    } else if ["nintendo", "switch", "pro controller", "joy-con", "joycon"].iter().any(|k| n.contains(k)) {
        PadStyle::Nintendo
    } else {
        PadStyle::Xbox
    }
}

/// The button's name in a style ("A", "Cross", "ZL").
pub fn pad_label_in(style: PadStyle, p: Pad) -> &'static str {
    use Pad::*;
    match style {
        PadStyle::PlayStation => match p {
            South => "Cross",
            East => "Circle",
            West => "Square",
            North => "Triangle",
            Lb => "L1",
            Rb => "R1",
            Lt => "L2",
            Rt => "R2",
            Select => "Share",
            Start => "Options",
            LStick => "L3",
            RStick => "R3",
            _ => dpad_label(p),
        },
        // Nintendo pads have A on the right and B at the bottom (positions are what counts).
        PadStyle::Nintendo => match p {
            South => "B",
            East => "A",
            West => "Y",
            North => "X",
            Lb => "L",
            Rb => "R",
            Lt => "ZL",
            Rt => "ZR",
            Select => "-",
            Start => "+",
            LStick => "LS",
            RStick => "RS",
            _ => dpad_label(p),
        },
        _ => match p {
            South => "A",
            East => "B",
            West => "X",
            North => "Y",
            Lb => "LB",
            Rb => "RB",
            Lt => "LT",
            Rt => "RT",
            Select => "View",
            Start => "Menu",
            LStick => "LS",
            RStick => "RS",
            _ => dpad_label(p),
        },
    }
}

fn dpad_label(p: Pad) -> &'static str {
    match p {
        Pad::Up => "D-pad up",
        Pad::Down => "D-pad down",
        Pad::Left => "D-pad left",
        Pad::Right => "D-pad right",
        _ => "?",
    }
}

/// The button's name in the current style.
pub fn pad_label(p: Pad) -> &'static str {
    pad_label_in(style(), p)
}

/// The action an `{key:ID}` token names (`interact` is *confirm*).
fn token_act(id: &str) -> Option<Act> {
    match id.trim() {
        "interact" | "talk" => Some(Act::Confirm),
        id => Act::from_id(id),
    }
}

/// What an action is pressed with now, as text: its first key ("E"), or after gamepad use its
/// button's name ("A", "Cross"). None for an unknown action.
pub fn key_text(act_id: &str) -> Option<String> {
    let a = token_act(act_id)?;
    // The names in the current language ("Leertaste", "Kreuz").
    Some(match (input::device(), input::pads_of(a).first()) {
        (Device::Pad, Some(p)) => crate::lang::tr_ui(pad_label(*p)).to_string(),
        _ => input::keys_of(a).first().map_or_else(|| "-".to_string(), |k| crate::lang::tr_ui(&input::key_label(*k)).to_string()),
    })
}

/// A text with its `{key:ACTION}` tokens as plain key / button names (item descriptions, which
/// don't go through `WorldState::expand_text`).
pub fn expand_keys(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("{key:") {
        let Some(j) = rest[i..].find('}') else { break };
        out.push_str(&rest[..i]);
        match key_text(&rest[i + 5..i + j]) {
            Some(k) => out.push_str(&k),
            None => out.push_str(&rest[i..i + j + 1]),
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out
}

/// A piece of a hint line.
#[derive(Debug, PartialEq)]
enum Seg {
    Text(String),
    /// A button picture.
    Button(Pad),
}

/// A hint template, translated, with its tokens resolved for the current device.
fn segments(template: &str) -> Vec<Seg> {
    let text = crate::lang::tr_ui(template).to_string();
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("{key:").into_iter().chain(rest.find("{pad:")).min() {
        let Some(j) = rest[i..].find('}') else { break };
        out.push(Seg::Text(rest[..i].to_string()));
        let id = &rest[i + 5..i + j];
        // `{pad:x}`: always that button's picture (hints shown only while a pad is used).
        if rest[i..].starts_with("{pad:") {
            match Pad::from_name(id) {
                Some(p) => out.push(Seg::Button(p)),
                None => out.push(Seg::Text(rest[i..i + j + 1].to_string())),
            }
            rest = &rest[i + j + 1..];
            continue;
        }
        match token_act(id) {
            Some(a) if input::device() == Device::Pad && !input::pads_of(a).is_empty() => out.push(Seg::Button(input::pads_of(a)[0])),
            Some(_) => out.push(Seg::Text(key_text(id).unwrap_or_default())),
            None => out.push(Seg::Text(rest[i..i + j + 1].to_string())),
        }
        rest = &rest[i + j + 1..];
    }
    out.push(Seg::Text(rest.to_string()));
    out.retain(|s| !matches!(s, Seg::Text(t) if t.is_empty()));
    out
}

/// The hint as plain text (keys and button names), e.g. for logs and tests.
pub fn hint_text(template: &str) -> String {
    segments(template)
        .into_iter()
        .map(|s| match s {
            Seg::Text(t) => t,
            Seg::Button(p) => format!("[{}]", pad_label(p)),
        })
        .collect()
}

/// Width of a hint line at `size`.
pub fn measure_hint(template: &str, size: f32) -> f32 {
    segments(template)
        .iter()
        .map(|s| match s {
            Seg::Text(t) => macroquad::text::measure_text(t, None, size as u16, 1.0).width,
            Seg::Button(p) => pad_width(*p, size) + 2.0,
        })
        .sum()
}

/// Draws a hint line with its text baseline at `y`; returns its width.
pub fn draw_hint(template: &str, x: f32, y: f32, size: f32, color: Color) -> f32 {
    let mut cx = x;
    for s in segments(template) {
        match s {
            Seg::Text(t) => cx += macroquad::text::draw_text(&t, cx, y, size, color).width,
            Seg::Button(p) => cx += draw_pad(p, cx + 1.0, y - size * 0.3, size) + 2.0,
        }
    }
    cx - x
}

/// The largest font size from `max` down to 12 at which the hint fits into `width`.
pub fn hint_size(template: &str, width: f32, max: u16) -> f32 {
    (12..=max).rev().find(|s| measure_hint(template, *s as f32) <= width).unwrap_or(12) as f32
}

// ------------------------------------------------------------------ the pictures

/// Width of button `p` drawn at text size `size`.
pub fn pad_width(p: Pad, size: f32) -> f32 {
    let h = size * 0.95;
    match p {
        Pad::South | Pad::East | Pad::West | Pad::North | Pad::LStick | Pad::RStick | Pad::Up | Pad::Down | Pad::Left | Pad::Right => h,
        Pad::Select | Pad::Start if style() == PadStyle::Nintendo => h,
        _ => {
            let label = pad_label(p);
            (macroquad::text::measure_text(label, None, (h * 0.62) as u16, 1.0).width + h * 0.55).max(h * 1.3)
        }
    }
}

/// Draws button `p` (vertically centered on `cy`, as tall as text of `size`); returns its width.
pub fn draw_pad(p: Pad, x: f32, cy: f32, size: f32) -> f32 {
    let h = size * 0.95;
    let r = h / 2.0;
    let w = pad_width(p, size);
    let st = style();
    let dark = Color::new(0.08, 0.08, 0.1, 1.0);
    let centered = |text: &str, cx: f32, fs: f32, c: Color| {
        let d = macroquad::text::measure_text(text, None, fs as u16, 1.0);
        macroquad::text::draw_text(text, (cx - d.width / 2.0).round(), (cy + d.offset_y / 2.0 - 1.0).round(), fs, c);
    };
    match p {
        Pad::South | Pad::East | Pad::West | Pad::North => {
            let cx = x + r;
            match st {
                PadStyle::PlayStation => {
                    draw_circle(cx, cy, r, Color::new(0.16, 0.16, 0.2, 1.0));
                    draw_circle_lines(cx, cy, r, 1.0, Color::new(0.45, 0.45, 0.5, 1.0));
                    let s = r * 0.48;
                    let th = (r * 0.16).max(1.5);
                    match p {
                        Pad::South => {
                            let c = Color::new(0.55, 0.75, 1.0, 1.0);
                            draw_line(cx - s, cy - s, cx + s, cy + s, th, c);
                            draw_line(cx - s, cy + s, cx + s, cy - s, th, c);
                        }
                        Pad::East => draw_circle_lines(cx, cy, s, th, Color::new(1.0, 0.45, 0.45, 1.0)),
                        Pad::West => draw_rectangle_lines(cx - s, cy - s, 2.0 * s, 2.0 * s, th * 1.6, Color::new(1.0, 0.55, 0.85, 1.0)),
                        _ => draw_triangle_lines(vec2(cx, cy - s * 1.05), vec2(cx - s, cy + s * 0.75), vec2(cx + s, cy + s * 0.75), th, Color::new(0.4, 0.9, 0.7, 1.0)),
                    }
                }
                PadStyle::Nintendo => {
                    draw_circle(cx, cy, r, Color::new(0.2, 0.2, 0.24, 1.0));
                    draw_circle_lines(cx, cy, r, 1.0, Color::new(0.5, 0.5, 0.55, 1.0));
                    centered(pad_label_in(st, p), cx, h * 0.8, WHITE);
                }
                _ => {
                    let fill = match p {
                        Pad::South => Color::new(0.36, 0.72, 0.25, 1.0),
                        Pad::East => Color::new(0.85, 0.25, 0.22, 1.0),
                        Pad::West => Color::new(0.2, 0.45, 0.9, 1.0),
                        _ => Color::new(0.95, 0.75, 0.15, 1.0),
                    };
                    draw_circle(cx, cy, r, fill);
                    draw_circle_lines(cx, cy, r, 1.0, dark);
                    centered(pad_label_in(st, p), cx, h * 0.8, if p == Pad::North { dark } else { WHITE });
                }
            }
        }
        Pad::LStick | Pad::RStick => {
            let cx = x + r;
            draw_circle(cx, cy, r, Color::new(0.2, 0.2, 0.24, 1.0));
            draw_circle_lines(cx, cy, r * 0.75, 1.0, Color::new(0.55, 0.55, 0.6, 1.0));
            centered(if p == Pad::LStick { "L" } else { "R" }, cx, h * 0.72, WHITE);
        }
        Pad::Up | Pad::Down | Pad::Left | Pad::Right => {
            // A plus with the pressed arm lit.
            let (cx, a) = (x + r, h / 3.0);
            let base = Color::new(0.3, 0.3, 0.35, 1.0);
            draw_rectangle(cx - a / 2.0, cy - r, a, h, base);
            draw_rectangle(cx - r, cy - a / 2.0, h, a, base);
            let lit = WHITE;
            match p {
                Pad::Up => draw_rectangle(cx - a / 2.0, cy - r, a, a, lit),
                Pad::Down => draw_rectangle(cx - a / 2.0, cy + r - a, a, a, lit),
                Pad::Left => draw_rectangle(cx - r, cy - a / 2.0, a, a, lit),
                _ => draw_rectangle(cx + r - a, cy - a / 2.0, a, a, lit),
            }
        }
        _ => {
            // Bumpers, triggers, Select / Start: a rounded label.
            let label = pad_label_in(st, p);
            let fill = if matches!(p, Pad::Lt | Pad::Rt) { Color::new(0.26, 0.26, 0.3, 1.0) } else { Color::new(0.2, 0.2, 0.24, 1.0) };
            let rr = if matches!(p, Pad::Select | Pad::Start) { r } else { r * 0.45 };
            draw_rectangle(x + rr, cy - r, w - 2.0 * rr, h, fill);
            draw_rectangle(x, cy - r + rr, w, h - 2.0 * rr, fill);
            for (ex, ey) in [(x + rr, cy - r + rr), (x + w - rr, cy - r + rr), (x + rr, cy + r - rr), (x + w - rr, cy + r - rr)] {
                draw_circle(ex, ey, rr, fill);
            }
            centered(label, x + w / 2.0, h * 0.62, WHITE);
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_from_pad_names() {
        assert_eq!(detect("Sony Interactive Entertainment DualSense Wireless Controller"), PadStyle::PlayStation);
        assert_eq!(detect("Nintendo Switch Pro Controller"), PadStyle::Nintendo);
        assert_eq!(detect("Steam Deck"), PadStyle::Xbox);
        assert_eq!(detect("Xbox Wireless Controller"), PadStyle::Xbox);
        for s in PadStyle::ALL {
            assert_eq!(PadStyle::from_id(s.id()), s);
        }
        assert_eq!(PadStyle::from_id("nonsense"), PadStyle::Auto);
    }

    #[test]
    fn button_names_follow_positions() {
        assert_eq!(pad_label_in(PadStyle::Xbox, Pad::South), "A");
        assert_eq!(pad_label_in(PadStyle::Nintendo, Pad::South), "B");
        assert_eq!(pad_label_in(PadStyle::Nintendo, Pad::East), "A");
        assert_eq!(pad_label_in(PadStyle::PlayStation, Pad::West), "Square");
    }

    #[test]
    fn tokens_name_the_keys() {
        // Keyboard (the default device): the first key of each action.
        assert_eq!(hint_text("{key:confirm}: use · {key:cancel}: close"), "E: use · Esc: close");
        assert_eq!(hint_text("{key:interact}"), "E");
        assert_eq!(hint_text("{key:nonsense} stays"), "{key:nonsense} stays");
        assert_eq!(key_text("journal").as_deref(), Some("J"));
        assert_eq!(key_text("nonsense"), None);
    }
}
