//! Everything a drawing module needs, in one import (the names macroquad's prelude had).

pub use crate::batch::{DrawMode, Vertex};
pub use crate::camera::*;
pub use crate::color::{colors::*, Color};
pub use crate::conf::Conf;
pub use crate::input::*;
pub use crate::material::*;
pub use crate::math::*;
pub use crate::models::{draw_mesh, Mesh};
pub use crate::rand;
pub use crate::shapes::*;
pub use crate::text::*;
pub use crate::texture::*;
pub use crate::time::*;
pub use crate::window::{clear_background, dpi_scale, next_frame, request_new_screen_size, screen_height, screen_width, set_fullscreen};
pub use glam;
