//! How the window starts.

use crate::texture::FilterMode;

/// The window's icon in three sizes, RGBA.
#[derive(Clone)]
pub struct Icon {
    pub small: [u8; 16 * 16 * 4],
    pub medium: [u8; 32 * 32 * 4],
    pub big: [u8; 64 * 64 * 4],
}

#[derive(Clone)]
pub struct Conf {
    pub window_title: String,
    /// The window's size in points.
    pub window_width: i32,
    pub window_height: i32,
    pub fullscreen: bool,
    pub window_resizable: bool,
    pub icon: Option<Icon>,
    /// The filter new textures get.
    pub default_filter_mode: FilterMode,
    /// Wait for the display's refresh (no tearing, no more frames than it shows).
    pub vsync: bool,
}

impl Default for Conf {
    fn default() -> Conf {
        Conf { window_title: String::new(), window_width: 800, window_height: 600, fullscreen: false, window_resizable: true, icon: None, default_filter_mode: FilterMode::Linear, vsync: true }
    }
}
