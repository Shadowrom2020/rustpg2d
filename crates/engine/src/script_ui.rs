//! Script UI: HUD elements scripts set once (`hud_text`, `hud_image`, `hud_bar`; kept in
//! `WorldState::hud`, so saved with the game) and simple windows scripts open (`window_open`:
//! text, pictures and buttons that run a function; not saved). `runtime/script_hud.rs` draws
//! them in the skin's look.

use serde::{Deserialize, Serialize};

/// Where an element sits on the screen; its offset counts from there (inwards from an edge).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Anchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [Anchor::TopLeft, Anchor::Top, Anchor::TopRight, Anchor::Left, Anchor::Center, Anchor::Right, Anchor::BottomLeft, Anchor::Bottom, Anchor::BottomRight];

    pub fn name(self) -> &'static str {
        match self {
            Anchor::TopLeft => "top_left",
            Anchor::Top => "top",
            Anchor::TopRight => "top_right",
            Anchor::Left => "left",
            Anchor::Center => "center",
            Anchor::Right => "right",
            Anchor::BottomLeft => "bottom_left",
            Anchor::Bottom => "bottom",
            Anchor::BottomRight => "bottom_right",
        }
    }

    /// "top_right", "top right", "TopRight", … (`None` for other words).
    pub fn parse(s: &str) -> Option<Anchor> {
        let k: String = s.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        Anchor::ALL.into_iter().find(|a| a.name().replace('_', "") == k || (k == "middle" && *a == Anchor::Center))
    }

    /// The top-left corner of an element `w` × `h` at offset `x`, `y` on a screen `sw` × `sh`:
    /// the offset counts inwards from the anchor's edges (a right anchor moves left with `x`,
    /// a bottom one up with `y`); centred anchors centre the element and add the offset.
    pub fn place(self, sw: f32, sh: f32, w: f32, h: f32, x: f32, y: f32) -> (f32, f32) {
        let px = match self {
            Anchor::TopLeft | Anchor::Left | Anchor::BottomLeft => x,
            Anchor::Top | Anchor::Center | Anchor::Bottom => (sw - w) / 2.0 + x,
            Anchor::TopRight | Anchor::Right | Anchor::BottomRight => sw - w - x,
        };
        let py = match self {
            Anchor::TopLeft | Anchor::Top | Anchor::TopRight => y,
            Anchor::Left | Anchor::Center | Anchor::Right => (sh - h) / 2.0 + y,
            Anchor::BottomLeft | Anchor::Bottom | Anchor::BottomRight => sh - h - y,
        };
        (px.round(), py.round())
    }
}

/// Where a bar's value (or maximum) comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Number(f32),
    /// A variable.
    Var(String),
    /// A character's HP on the current map (`player` = the player).
    Hp(String),
    MaxHp(String),
}

impl Source {
    /// "hp:Grumbold", "max_hp:player", "coins" (a variable), "100".
    pub fn parse(s: &str) -> Source {
        let s = s.trim();
        if let Ok(n) = s.parse::<f32>() {
            return Source::Number(n);
        }
        if let Some(n) = s.strip_prefix("max_hp:") {
            return Source::MaxHp(n.trim().to_string());
        }
        if let Some(n) = s.strip_prefix("hp:") {
            return Source::Hp(n.trim().to_string());
        }
        Source::Var(s.to_string())
    }
}

/// What a HUD element shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum HudKind {
    /// A line; placeholders (`{coins}`, `{item:coin}`, `{player}`) are filled in every frame.
    Text { text: String },
    /// A frame of a sprite sheet.
    Image {
        sheet: String,
        #[serde(default)]
        frame: u32,
    },
    /// A bar `w` pixels wide: `value` of `max` ([`Source`] strings), an optional label over it.
    Bar {
        value: String,
        max: String,
        w: f32,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        label: String,
    },
    /// A small map around the player (like the map screen's): a square of the element's `size`
    /// in pixels (0 = 160) showing `tiles` tiles across (0 = 24), the player in the middle.
    Minimap {
        #[serde(default)]
        tiles: f32,
    },
}

/// A HUD element set by a script (`WorldState::hud`, by id).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HudElement {
    pub kind: HudKind,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Text size (texts, labels) or scale (pictures); 0 = the usual.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub size: f32,
    /// A colour name or `#rrggbb` (texts, bars' fill); empty = the skin's.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl HudElement {
    pub fn new(kind: HudKind, x: f32, y: f32) -> Self {
        HudElement { kind, anchor: Anchor::TopLeft, x, y, size: 0.0, color: String::new() }
    }

    /// For `status`: "text", "image", "bar".
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            HudKind::Text { .. } => "text",
            HudKind::Image { .. } => "image",
            HudKind::Bar { .. } => "bar",
            HudKind::Minimap { .. } => "minimap",
        }
    }
}

/// A row of a script window.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowRow {
    /// A line of text (translated, placeholders filled in).
    Text(String),
    /// A frame of a sprite sheet.
    Image { sheet: String, frame: u32 },
    /// A button that runs a script function.
    Button { label: String, call: String },
}

/// A window a script opened.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptWindow {
    pub id: String,
    pub title: String,
    pub rows: Vec<WindowRow>,
}

/// What a script asked of the window (`WorldState::window_request`, not saved).
#[derive(Clone, Debug, PartialEq)]
pub enum WindowRequest {
    Open(ScriptWindow),
    Close,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_place_elements_inwards() {
        let (sw, sh) = (1280.0, 720.0);
        assert_eq!(Anchor::TopLeft.place(sw, sh, 100.0, 20.0, 10.0, 5.0), (10.0, 5.0));
        assert_eq!(Anchor::TopRight.place(sw, sh, 100.0, 20.0, 10.0, 5.0), (1170.0, 5.0));
        assert_eq!(Anchor::Top.place(sw, sh, 100.0, 20.0, 0.0, 40.0), (590.0, 40.0));
        assert_eq!(Anchor::BottomRight.place(sw, sh, 100.0, 20.0, 10.0, 5.0), (1170.0, 695.0));
        assert_eq!(Anchor::Center.place(sw, sh, 100.0, 20.0, 0.0, 0.0), (590.0, 350.0));
        assert_eq!(Anchor::parse("top right"), Some(Anchor::TopRight));
        assert_eq!(Anchor::parse("BottomLeft"), Some(Anchor::BottomLeft));
        assert_eq!(Anchor::parse("middle"), Some(Anchor::Center));
        assert_eq!(Anchor::parse("nowhere"), None);
    }

    #[test]
    fn bar_sources() {
        assert_eq!(Source::parse("100"), Source::Number(100.0));
        assert_eq!(Source::parse("hp:Grumbold"), Source::Hp("Grumbold".into()));
        assert_eq!(Source::parse("max_hp:player"), Source::MaxHp("player".into()));
        assert_eq!(Source::parse("coins"), Source::Var("coins".into()));
    }

    #[test]
    fn elements_round_trip() {
        let mut e = HudElement::new(HudKind::Bar { value: "hp:Grumbold".into(), max: "max_hp:Grumbold".into(), w: 320.0, label: "Grumbold".into() }, 0.0, 40.0);
        e.anchor = Anchor::Top;
        e.color = "red".into();
        let text = ron::to_string(&e).unwrap();
        assert_eq!(ron::from_str::<HudElement>(&text).unwrap(), e);
        let t: HudElement = ron::from_str(r#"(kind: Text(text: "{coins}"))"#).unwrap();
        assert_eq!(t, HudElement::new(HudKind::Text { text: "{coins}".into() }, 0.0, 0.0));
    }
}
