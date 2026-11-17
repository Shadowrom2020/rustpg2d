//! Drawing with the project's UI skin ([`crate::skin::SkinDef`]): every window frame, button,
//! selection marker, toast, HUD frame and bar, and the colours of window texts go through here.
//! Each function takes the built-in look of the place it draws (its colours as they always
//! were); a skin replaces only the parts it sets, so without a skin everything looks exactly as
//! built in. High contrast (Options › Accessibility) always uses the built-in look.
//!
//! The skin is installed once per game start / hot reload ([`install`]); its pictures are
//! resolved to textures then, so drawing needs no asset lookups. The editor installs a skin the
//! same way for its preview.

use std::cell::RefCell;
use std::rc::Rc;

use macroquad::prelude::*;

use crate::skin::{NineSlice, SkinColor, SkinDef};

/// A nine-slice picture, ready to draw.
#[derive(Clone)]
struct Pic {
    tex: Texture2D,
    src: Rect,
    corner: f32,
    scale: f32,
    tile: bool,
}

#[derive(Default)]
struct Active {
    def: SkinDef,
    frame: Option<Pic>,
    dialog: Option<Pic>,
    name_box: Option<Pic>,
    hud: Option<Pic>,
    button: Option<Pic>,
    button_hover: Option<Pic>,
    cursor: Option<Pic>,
    toast: Option<Pic>,
    hp: Option<Pic>,
    mana: Option<Pic>,
    xp: Option<Pic>,
    /// Pictures that could not be found (sheet:frame), for `status`.
    missing: Vec<String>,
}

thread_local! {
    static ACTIVE: RefCell<Rc<Active>> = RefCell::new(Rc::new(Active::default()));
    static BUILTIN: Rc<Active> = Rc::new(Active::default());
}

/// The skin to draw with now: the built-in look while high contrast is on.
fn cur() -> Rc<Active> {
    if super::ui::contrast() {
        return BUILTIN.with(|b| b.clone());
    }
    ACTIVE.with(|a| a.borrow().clone())
}

/// Installs the skin the game draws with (`None` = the built-in look). `pic` finds a
/// picture's texture and its frame's rectangle in it. Also hands the skin's colour names to
/// the dialog codes.
pub fn install(def: Option<&SkinDef>, pic: &dyn Fn(&NineSlice) -> Option<(Texture2D, Rect)>) {
    let def = def.cloned().unwrap_or_default();
    let mut missing = Vec::new();
    let mut load = |n: &Option<NineSlice>| {
        let n = n.as_ref()?;
        match pic(n) {
            Some((tex, src)) => Some(Pic { tex, src, corner: n.corner as f32, scale: n.scale.max(1) as f32, tile: n.tile }),
            None => {
                missing.push(format!("{}:{}", n.sheet, n.frame));
                None
            }
        }
    };
    let a = Active {
        frame: load(&def.frame),
        dialog: load(&def.dialog),
        name_box: load(&def.name_box),
        hud: load(&def.hud),
        button: load(&def.button),
        button_hover: load(&def.button_hover),
        cursor: load(&def.cursor),
        toast: load(&def.toast),
        hp: load(&def.hp.frame),
        mana: load(&def.mana.frame),
        xp: load(&def.xp.frame),
        missing,
        def,
    };
    for m in &a.missing {
        eprintln!("[skin] {}: picture {m} not found", a.def.id);
    }
    crate::dialog_codes::set_skin_colors(a.def.colors.iter().map(|(n, c)| (n.clone(), c.rgb3())).collect());
    ACTIVE.with(|s| *s.borrow_mut() = Rc::new(a));
}

/// For `Game::status()`: `default`, the skin's id, `:contrast` while high contrast overrides
/// it, `!sheet:frame` per picture that was not found.
pub fn status() -> String {
    ACTIVE.with(|a| {
        let a = a.borrow();
        let mut s = if a.def.id.is_empty() { "default".to_string() } else { a.def.id.clone() };
        if super::ui::contrast() {
            s.push_str(":contrast");
        }
        for m in &a.missing {
            s.push_str(&format!("!{m}"));
        }
        s
    })
}

fn col(c: SkinColor) -> Color {
    Color::from_rgba(c.0[0], c.0[1], c.0[2], c.0[3])
}

/// A skin colour with the built-in colour's opacity on top of its own (fading toasts, dimmed
/// rows), or the built-in colour.
fn over(c: Option<SkinColor>, builtin: Color) -> Color {
    match c {
        Some(c) => {
            let mut c = col(c);
            c.a *= builtin.a;
            c
        }
        None => builtin,
    }
}

/// What a text in a window is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    Normal,
    Dim,
    Highlight,
    Disabled,
    Title,
    Positive,
    Negative,
}

/// The colour of a window text: the skin's for its kind, else `builtin` (the place's own).
pub fn text(kind: Text, builtin: Color) -> Color {
    let a = cur();
    let t = &a.def.text;
    let c = match kind {
        Text::Normal => t.normal,
        Text::Dim => t.dim,
        Text::Highlight => t.highlight,
        Text::Disabled => t.disabled,
        Text::Title => t.title,
        Text::Positive => t.positive,
        Text::Negative => t.negative,
    };
    over(c, builtin)
}

/// Shorthands for the usual built-in colours.
pub fn normal() -> Color {
    text(Text::Normal, WHITE)
}
pub fn dim() -> Color {
    text(Text::Dim, LIGHTGRAY)
}
pub fn highlight() -> Color {
    text(Text::Highlight, GOLD)
}
pub fn disabled() -> Color {
    text(Text::Disabled, GRAY)
}
pub fn title() -> Color {
    text(Text::Title, GOLD)
}

/// The border of frames without a picture: the skin's, else `builtin`.
pub fn border(builtin: Color) -> Color {
    over(cur().def.border, builtin)
}

/// The usual beige border with opacity `a`.
pub fn line(a: f32) -> Color {
    border(Color::new(0.85, 0.8, 0.6, a))
}

/// A window or menu panel.
pub fn panel(x: f32, y: f32, w: f32, h: f32) {
    let a = cur();
    if super::ui::contrast() {
        // High contrast: opaque black, a bright border.
        draw_rectangle(x, y, w, h, BLACK);
        draw_rectangle_lines(x, y, w, h, 3.0, Color::new(1.0, 0.85, 0.2, 1.0));
        return;
    }
    if let Some(p) = &a.frame {
        draw_pic(p, Rect::new(x, y, w, h), WHITE);
        return;
    }
    draw_rectangle(x, y, w, h, over(a.def.background, Color::new(0.06, 0.06, 0.12, 0.92)));
    draw_rectangle_lines(x, y, w, h, 3.0, over(a.def.border, Color::new(0.85, 0.8, 0.6, 1.0)));
}

/// Whether panels have a frame picture (places that draw extra lines on a panel skip them).
pub fn framed() -> bool {
    cur().frame.is_some()
}

/// The dialog and choice box: the skin's dialog picture, else its frame, else a panel.
pub fn dialog_box(x: f32, y: f32, w: f32, h: f32) {
    let a = cur();
    match &a.dialog {
        Some(p) => draw_pic(p, Rect::new(x, y, w, h), WHITE),
        None => panel(x, y, w, h),
    }
}

/// Whether the speaker's name goes into a box of its own on the dialog box's edge.
pub fn has_name_box() -> bool {
    cur().name_box.is_some()
}

/// Draws the name box (when the skin has one).
pub fn name_box(r: Rect) {
    if let Some(p) = &cur().name_box {
        draw_pic(p, r, WHITE);
    }
}

/// The name box's padding around the name: the picture's corner on screen.
pub fn name_box_pad() -> f32 {
    cur().name_box.as_ref().map_or(10.0, |p| (p.corner * p.scale).max(6.0))
}

/// The HUD panel: the skin's HUD picture, else its frame picture, else the built-in dark
/// panel with a gold border, an inner line and corner studs (in the skin's border colour).
pub fn hud_frame(frame: Rect) {
    let a = cur();
    if let Some(p) = a.hud.as_ref().or(a.frame.as_ref()) {
        draw_pic(p, frame, WHITE);
        return;
    }
    let contrast = super::ui::contrast();
    let gold = if contrast { Color::new(1.0, 0.85, 0.2, 1.0) } else { over(a.def.border, Color::new(0.85, 0.72, 0.38, 1.0)) };
    let dim_gold = match a.def.border {
        Some(b) => {
            let c = col(b);
            Color::new(c.r * 0.65, c.g * 0.65, c.b * 0.65, 1.0)
        }
        None => Color::new(0.55, 0.45, 0.22, 1.0),
    };
    let back = if contrast { BLACK } else { over(a.def.background, Color::new(0.05, 0.05, 0.1, 0.78)) };
    draw_rectangle(frame.x, frame.y, frame.w, frame.h, back);
    draw_rectangle_lines(frame.x, frame.y, frame.w, frame.h, 2.5, gold);
    draw_rectangle_lines(frame.x + 4.0, frame.y + 4.0, frame.w - 8.0, frame.h - 8.0, 1.0, dim_gold);
    let stud = match a.def.border {
        Some(b) => {
            let c = col(b);
            Color::new((c.r + 0.3).min(1.0), (c.g + 0.3).min(1.0), (c.b + 0.3).min(1.0), 1.0)
        }
        None => Color::new(0.95, 0.88, 0.6, 1.0),
    };
    for (cx, cy) in [(frame.x, frame.y), (frame.right(), frame.y), (frame.x, frame.bottom()), (frame.right(), frame.bottom())] {
        draw_poly(cx, cy, 4, 5.0, 45.0, gold);
        draw_poly(cx, cy, 4, 2.5, 45.0, stud);
    }
}

/// The dim gold of the HUD's inner lines (portrait plate).
pub fn hud_dim_line() -> Color {
    match cur().def.border {
        Some(b) => {
            let c = col(b);
            Color::new(c.r * 0.65, c.g * 0.65, c.b * 0.65, 1.0)
        }
        None => Color::new(0.55, 0.45, 0.22, 1.0),
    }
}

/// A button, tab or close button: draws the skin's picture (`hot` = under the pointer / open)
/// and returns true, or returns false and the place draws its own (with [`button_fill`]).
pub fn button_picture(r: Rect, hot: bool) -> bool {
    let a = cur();
    let p = if hot { a.button_hover.as_ref().or(a.button.as_ref()) } else { a.button.as_ref() };
    match p {
        Some(p) => {
            draw_pic(p, r, WHITE);
            true
        }
        None => false,
    }
}

/// A button's fill without a picture: the skin's (hot or not), else `builtin`.
pub fn button_fill(hot: bool, builtin: Color) -> Color {
    let a = cur();
    over(if hot { a.def.button_hover_fill } else { a.def.button_fill }, builtin)
}

/// The selection marker of a row or slot: the skin's picture (returns true; the place then
/// skips its own outline), else `builtin` filled (or the skin's cursor fill).
pub fn cursor(r: Rect, builtin: Color) -> bool {
    let a = cur();
    if let Some(p) = &a.cursor {
        draw_pic(p, r, WHITE);
        return true;
    }
    draw_rectangle(r.x, r.y, r.w, r.h, over(a.def.cursor_fill, builtin));
    false
}

/// Draws the skin's cursor picture over `r` and returns true; false without one (the place
/// draws its own selection outline).
pub fn cursor_picture(r: Rect) -> bool {
    match &cur().cursor {
        Some(p) => {
            draw_pic(p, r, WHITE);
            true
        }
        None => false,
    }
}

/// Whether the skin has a cursor picture (places without a marker of their own draw it then).
pub fn has_cursor() -> bool {
    cur().cursor.is_some()
}

/// A toast's box at opacity `alpha` (built in: a dark box, high contrast: black with a border).
pub fn toast_box(r: Rect, alpha: f32) {
    let a = cur();
    if super::ui::contrast() {
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, alpha));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(1.0, 0.85, 0.2, alpha));
        return;
    }
    if let Some(p) = &a.toast {
        draw_pic(p, r, Color::new(1.0, 1.0, 1.0, alpha));
        return;
    }
    draw_rectangle(r.x, r.y, r.w, r.h, over(a.def.toast_fill, Color::new(0.05, 0.05, 0.1, 0.8 * alpha)));
}

/// A toast's text colour at opacity `alpha`.
pub fn toast_text(alpha: f32) -> Color {
    over(cur().def.toast_text, Color::new(1., 1., 1., alpha))
}

/// The HUD's bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Mana,
    Xp,
}

/// A bar `w` × `h` at `x, y` filled to `frac` (0 to 1), as the HUD draws mana and experience:
/// a dark back, the fill with a lighter top edge, a thin outline (or the skin's picture around
/// it).
pub fn bar(kind: Bar, x: f32, y: f32, w: f32, h: f32, frac: f32) {
    let a = cur();
    let (style, pic) = match kind {
        Bar::Mana => (&a.def.mana, &a.mana),
        Bar::Xp => (&a.def.xp, &a.xp),
    };
    // Built in: (fill, top edge, its height, outline).
    let (fill, top, top_h, outline) = match kind {
        Bar::Mana => (Color::new(0.3, 0.45, 1.0, 1.0), Color::new(0.55, 0.7, 1.0, 1.0), 3.0, Color::new(0.6, 0.7, 1.0, 0.8)),
        Bar::Xp => (Color::new(0.85, 0.68, 0.2, 1.0), Color::new(1.0, 0.88, 0.5, 1.0), 2.0, Color::new(0.85, 0.72, 0.38, 0.8)),
    };
    let (fill, top) = match style.fill {
        Some(c) => {
            let f = col(c);
            (f, Color::new(f.r + (1.0 - f.r) * 0.35, f.g + (1.0 - f.g) * 0.35, f.b + (1.0 - f.b) * 0.35, f.a))
        }
        None => (fill, top),
    };
    draw_rectangle(x - 1.0, y - 1.0, w + 2.0, h + 2.0, over(style.back, Color::new(0.0, 0.0, 0.0, 0.7)));
    let f = w * frac.clamp(0.0, 1.0);
    draw_rectangle(x, y, f, h, fill);
    draw_rectangle(x, y, f, top_h, top);
    match pic {
        Some(p) => {
            // The picture's border sits around the bar.
            let pad = (p.corner * p.scale).min(6.0);
            draw_pic(p, Rect::new(x - pad, y - pad, w + 2.0 * pad, h + 2.0 * pad), WHITE);
        }
        None => draw_rectangle_lines(x - 1.0, y - 1.0, w + 2.0, h + 2.0, 1.0, outline),
    }
}

/// The colours of full and empty hearts: the skin's health bar colours, else `full` / `empty`.
pub fn hearts(full: Color, empty: Color) -> (Color, Color) {
    let a = cur();
    (over(a.def.hp.fill, full), over(a.def.hp.back, empty))
}

/// The skin's health picture behind the row of hearts `r` (nothing without one).
pub fn hearts_plate(r: Rect) {
    if let Some(p) = &cur().hp {
        // (Thin: the mana bar sits close under the hearts.)
        let pad = (p.corner * p.scale / 2.0).min(3.0);
        draw_pic(p, Rect::new(r.x - pad, r.y - pad, r.w + 2.0 * pad, r.h + 2.0 * pad), WHITE);
    }
}

/// The colour a dialog text goes back to after `{/c}` (the skin's normal text colour).
pub fn dialog_text() -> Color {
    normal()
}

fn draw_pic(p: &Pic, dest: Rect, tint: Color) {
    crate::render::draw_nine_slice(&p.tex, p.src, dest, p.corner, p.scale, p.tile, tint);
}

/// Draws a sample of the skin into a `w` × `h` area at the origin (the editor's preview, drawn
/// into a render target): a menu with a selected row and a button, the HUD with hearts and
/// bars, a toast, and a dialog box with the speaker's name and a choice.
pub fn draw_preview(w: f32, h: f32) {
    use super::text::{draw_plain, measure_plain, role, Role};
    clear_background(Color::new(0.16, 0.2, 0.14, 1.0));
    // A hint of a map behind the windows: a checker of grass shades.
    for y in 0..(h as i32 / 32 + 1) {
        for x in 0..(w as i32 / 32 + 1) {
            if (x + y) % 2 == 0 {
                draw_rectangle(x as f32 * 32.0, y as f32 * 32.0, 32.0, 32.0, Color::new(0.19, 0.24, 0.16, 1.0));
            }
        }
    }
    // The menu.
    {
        let _f = role(Role::Ui);
        let p = Rect::new(12.0, 12.0, 220.0, h - 24.0);
        panel(p.x, p.y, p.w, p.h);
        let t = "Paused";
        let tw = measure_plain(t, 28.0).width;
        draw_plain(t, p.x + (p.w - tw) / 2.0, p.y + 38.0, 28.0, title());
        let rows = [("Resume", Text::Highlight), ("Options", Text::Normal), ("Load game", Text::Disabled)];
        for (k, (label, kind)) in rows.iter().enumerate() {
            let r = Rect::new(p.x + 14.0, p.y + 56.0 + k as f32 * 34.0, p.w - 28.0, 30.0);
            if k == 0 && !cursor(r, Color::new(1.0, 1.0, 1.0, 0.12)) {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, highlight());
            }
            let builtin = match kind {
                Text::Highlight => GOLD,
                Text::Disabled => GRAY,
                _ => WHITE,
            };
            draw_plain(label, r.x + 12.0, r.y + 22.0, 22.0, text(*kind, builtin));
        }
        let y = p.y + 172.0;
        draw_plain("Better", p.x + 26.0, y, 18.0, text(Text::Positive, Color::new(0.6, 0.95, 0.6, 1.0)));
        draw_plain("Worse", p.x + 100.0, y, 18.0, text(Text::Negative, Color::new(1.0, 0.55, 0.5, 1.0)));
        draw_plain("A dim hint line", p.x + 26.0, y + 22.0, 16.0, dim());
        let b = Rect::new(p.x + 26.0, p.bottom() - 46.0, 78.0, 32.0);
        let b2 = Rect::new(p.x + 116.0, p.bottom() - 46.0, 78.0, 32.0);
        for (r, hot, label) in [(b, false, "Save"), (b2, true, "Load")] {
            if !button_picture(r, hot) {
                draw_rectangle(r.x, r.y, r.w, r.h, button_fill(hot, Color::new(0.3, 0.25, 0.1, if hot { 1.0 } else { 0.8 })));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, highlight());
            }
            let tw = measure_plain(label, 20.0).width;
            draw_plain(label, r.x + (r.w - tw) / 2.0, r.y + 22.0, 20.0, normal());
        }
    }
    // The HUD.
    {
        let _f = role(Role::Hud);
        let f = Rect::new(248.0, 12.0, 200.0, 98.0);
        hud_frame(f);
        draw_plain("Alex", f.x + 14.0, f.y + 26.0, 22.0, text(Text::Title, Color::new(1.0, 0.86, 0.45, 1.0)));
        let (full, empty) = hearts(Color::new(0.9, 0.15, 0.2, 1.0), Color::new(0.2, 0.2, 0.25, 0.9));
        let (hx, hy) = (f.x + 14.0, f.y + 34.0);
        // (As the HUD: name, hearts, then the bars below.)
        hearts_plate(Rect::new(hx, hy, 4.0 * 21.0 - 1.0, 20.0));
        for i in 0..4 {
            let x = hx + i as f32 * 21.0;
            super::combat::draw_heart(x, hy, 20.0, Color::new(0.0, 0.0, 0.0, 0.6));
            super::combat::draw_heart(x + 1.0, hy + 1.0, 18.0, if i < 3 { full } else { empty });
        }
        bar(Bar::Mana, f.x + 14.0, f.y + 62.0, 120.0, 9.0, 0.7);
        bar(Bar::Xp, f.x + 14.0, f.y + 80.0, 120.0, 6.0, 0.4);
    }
    // A toast.
    {
        let _f = role(Role::Hud);
        let t = "Quest updated";
        let tw = measure_plain(t, 20.0).width + 24.0;
        let r = Rect::new(w - 12.0 - tw, 12.0, tw, 34.0);
        toast_box(r, 1.0);
        draw_plain(t, r.x + 12.0, r.y + 24.0, 20.0, toast_text(1.0));
    }
    // The dialog box with a name and a choice.
    {
        let _f = role(Role::Dialog);
        let d = Rect::new(248.0, h - 126.0, w - 260.0, 114.0);
        dialog_box(d.x, d.y, d.w, d.h);
        let mut ty = d.y + 30.0;
        if has_name_box() {
            let pad = name_box_pad();
            let nh = (24.0 + 2.0 * pad).round();
            let nw = (measure_plain("Greta", 24.0).width + 2.0 * pad).round();
            let nr = Rect::new(d.x + 16.0, d.y - (nh / 2.0).round(), nw, nh);
            name_box(nr);
            draw_plain("Greta", nr.x + pad, nr.y + pad + 24.0 * 0.8, 24.0, title());
            ty = d.y + (nh / 2.0).round() + 24.0;
        } else {
            draw_plain("Greta", d.x + 20.0, ty, 24.0, title());
            ty += 26.0;
        }
        draw_plain("Welcome to the Copper Kettle!", d.x + 20.0, ty, 20.0, dialog_text());
        ty += 28.0;
        let r = Rect::new(d.x + 20.0, ty - 19.0, d.w - 40.0, 26.0);
        if has_cursor() {
            cursor(r, Color::new(0.0, 0.0, 0.0, 0.0));
        }
        draw_plain("> A room for the night", d.x + 36.0, ty, 20.0, highlight());
    }
}
