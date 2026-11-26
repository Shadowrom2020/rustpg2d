//! Vectors and matrices (glam, without SIMD alignment: `Vec4` and `Mat4` are plain `f32`s, as the
//! vertex layout needs) and rectangles.

pub use glam::*;

pub use crate::rect::{Rect, RectOffset};
