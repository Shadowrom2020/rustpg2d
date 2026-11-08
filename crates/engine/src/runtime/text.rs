//! The runtime's text drawing, in the current language: `draw_text` / `measure_text` (used
//! instead of macroquad's by the runtime's windows, menus and HUD) look the whole string up in
//! the language's runtime texts first (`lang::tr_ui`), so a text and its measured width always
//! agree. Content (dialog lines, names) is translated earlier (`lang::tr`) and drawn the same
//! way; dialog lines use macroquad's functions directly.

use macroquad::prelude::{Color, TextDimensions};

/// Draws a runtime text in the current language.
pub fn draw_text(text: impl AsRef<str>, x: f32, y: f32, font_size: f32, color: Color) -> TextDimensions {
    macroquad::text::draw_text(crate::lang::tr_ui(text.as_ref()), x, y, font_size, color)
}

/// Measures a runtime text as [`draw_text`] draws it.
pub fn measure_text(text: &str, font: Option<&macroquad::text::Font>, font_size: u16, font_scale: f32) -> TextDimensions {
    macroquad::text::measure_text(crate::lang::tr_ui(text), font, font_size, font_scale)
}

