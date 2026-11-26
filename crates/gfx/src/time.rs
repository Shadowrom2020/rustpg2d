//! The clock of frames.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::get_context;

pub(crate) struct Clock {
    #[cfg(not(target_arch = "wasm32"))]
    start: Instant,
    /// Seconds since the start, at the last frame's begin.
    now: f64,
    frame: f32,
    /// A fixed step instead of the real clock (tests that must repeat).
    fixed: Option<f32>,
}

impl Clock {
    pub fn new() -> Clock {
        Clock {
            #[cfg(not(target_arch = "wasm32"))]
            start: Instant::now(),
            now: 0.0,
            frame: 1.0 / 60.0,
            fixed: None,
        }
    }

    fn real(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.start.elapsed().as_secs_f64()
        }
        #[cfg(target_arch = "wasm32")]
        {
            crate::window::web_now()
        }
    }

    pub fn tick(&mut self) {
        match self.fixed {
            Some(step) => {
                self.frame = step;
                self.now += step as f64;
            }
            None => {
                let t = self.real();
                self.frame = (t - self.now) as f32;
                self.now = t;
            }
        }
    }

    /// `_Time` of shaders: seconds since the start.
    pub fn shader_time(&self) -> f32 {
        self.now as f32
    }
}

/// Seconds since the program started (the frame's time while a fixed step is set).
pub fn get_time() -> f64 {
    let c = &get_context().clock;
    if c.fixed.is_some() {
        c.now
    } else {
        c.real()
    }
}

/// Seconds the last frame took.
pub fn get_frame_time() -> f32 {
    get_context().clock.frame
}

/// Frames per second, from the last frame.
pub fn get_fps() -> i32 {
    (1.0 / get_context().clock.frame.max(1e-6)) as i32
}

/// Every frame takes `step` seconds whatever the real clock says (`None`: the real clock again).
pub fn set_fixed_step(step: Option<f32>) {
    get_context().clock.fixed = step;
}
