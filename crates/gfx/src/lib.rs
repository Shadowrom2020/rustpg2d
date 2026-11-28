//! R(ust)PG2D's platform layer: the window, drawing (wgpu), text, input and time.
//!
//! The drawing API keeps the names and the behaviour of macroquad 0.4 (`draw_rectangle`,
//! `draw_texture_ex`, `set_camera`, `render_target`, `Material`…), which the engine was written
//! against; parts of it (shapes, text, cameras, colours, rectangles) are macroquad's own code,
//! MIT OR Apache-2.0, Copyright (c) 2020-2021 Fedor Logachev (see LICENSE-macroquad-MIT).
//!
//! What is different from macroquad:
//! - Everything is drawn into a picture (the *screen*, [`Screen`]) instead of the window. The
//!   window shows that picture; an application that embeds the engine shows it wherever it likes
//!   ([`with_screen`]). Several screens can exist in one process.
//! - Draw calls are kept until the frame ends (or something reads a texture back); changing the
//!   camera doesn't send them to the GPU.
//! - Shaders are WGSL (see [`material`]).

pub mod audio;
pub mod batch;
pub mod camera;
pub mod color;
pub mod conf;
pub mod file;
pub mod gpu;
pub mod input;
pub mod logging;
pub mod material;
pub mod math;
pub mod models;
pub mod prelude;
pub mod shapes;
pub mod text;
pub mod texture;
pub mod time;
#[cfg(target_arch = "wasm32")]
pub mod web;
pub mod window;

mod rect;

pub use quad_rand as rand;

use std::cell::Cell;
use std::sync::Arc;

pub use texture::Screen;

/// A failure of the platform layer.
#[derive(Debug)]
pub enum Error {
    /// No usable graphics adapter, or the device could not be made.
    Gpu(String),
    /// A font file fontdue can't read.
    Font(&'static str),
    /// A picture that can't be decoded.
    Image(String),
    /// A shader that doesn't compile.
    Shader(String),
    /// A file that can't be read.
    File(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Gpu(s) => write!(f, "graphics: {s}"),
            Error::Font(s) => write!(f, "font: {s}"),
            Error::Image(s) => write!(f, "picture: {s}"),
            Error::Shader(s) => write!(f, "shader: {s}"),
            Error::File(s) => write!(f, "file: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// Everything the drawing, input and time functions work on. One per thread that draws.
pub(crate) struct Context {
    pub gpu: gpu::Gpu,
    pub gl: batch::Batch,
    /// What "the screen" is right now.
    pub screen: Screen,
    pub input: input::Input,
    pub camera_stack: Vec<camera::CameraState>,
    pub fonts: text::FontsStorage,
    pub default_filter_mode: texture::FilterMode,
    pub clock: time::Clock,
    /// Set by the window; functions of [`window`] leave their wishes here.
    pub wishes: window::Wishes,
}

thread_local! {
    static CONTEXT: Cell<*mut Context> = const { Cell::new(std::ptr::null_mut()) };
}

/// The thread's context. Panics when nothing was set up ([`init`], [`init_headless`], the window).
pub(crate) fn get_context() -> &'static mut Context {
    let p = CONTEXT.with(|c| c.get());
    assert!(!p.is_null(), "gfx: no graphics context on this thread (the window isn't open yet)");
    // Safety: the pointer is a leaked Box owned by this thread; the functions of this crate take the
    // reference for the length of one call and never keep it.
    unsafe { &mut *p }
}

/// Is there a context on this thread?
pub fn ready() -> bool {
    CONTEXT.with(|c| !c.get().is_null())
}

/// Sets up drawing on this thread with a device made elsewhere (an application that embeds the
/// engine, e.g. the editor with eframe's device). The screen starts as `width` × `height` pixels.
pub fn init(device: wgpu::Device, queue: wgpu::Queue, width: u32, height: u32, conf: &conf::Conf) {
    let gpu = gpu::Gpu::new(Arc::new(device), Arc::new(queue));
    install(gpu, width, height, conf);
}

/// Sets up drawing on this thread without a window: pictures are drawn off screen (tests, tools).
#[cfg(not(target_arch = "wasm32"))]
pub fn init_headless(width: u32, height: u32) -> Result<(), Error> {
    if ready() {
        return Ok(());
    }
    let (device, queue) = gpu::headless_device()?;
    init(device, queue, width, height, &conf::Conf::default());
    Ok(())
}

pub(crate) fn install(gpu: gpu::Gpu, width: u32, height: u32, conf: &conf::Conf) {
    let screen = Screen::new_with(&gpu, width.max(1), height.max(1), 1.0);
    let mut gl = batch::Batch::new(&gpu);
    gl.screen_size = screen.logical_size();
    let fonts = text::FontsStorage::new(&gpu);
    let ctx = Context {
        gpu,
        gl,
        screen,
        input: input::Input::new(),
        camera_stack: Vec::new(),
        fonts,
        default_filter_mode: conf.default_filter_mode,
        clock: time::Clock::new(),
        wishes: window::Wishes::default(),
    };
    let old = CONTEXT.with(|c| c.replace(Box::into_raw(Box::new(ctx))));
    if !old.is_null() {
        // Safety: made by Box::into_raw above.
        drop(unsafe { Box::from_raw(old) });
    }
}

/// Starts a frame: the clock moves on, the screen is cleared to black, the camera is the screen's.
pub fn begin_frame() {
    let ctx = get_context();
    ctx.clock.tick();
    ctx.camera_stack.clear();
    ctx.gl.reset();
    ctx.gl.screen_size = ctx.screen.logical_size();
    ctx.gl.clear(color::BLACK);
}

/// Ends a frame: everything drawn is sent to the GPU, and what was "pressed this frame" is over.
pub fn end_frame() {
    let ctx = get_context();
    flush();
    ctx.input.end_frame();
}

/// Sends every pending draw call to the GPU now. Frames do it when they end; reading a texture
/// back does it first.
pub fn flush() {
    let ctx = get_context();
    ctx.gl.flush(&ctx.gpu, &ctx.screen, ctx.clock.shader_time());
}

/// Runs `f` with `screen` as the screen and `input` as the input: what `f` draws on "the screen"
/// lands in the picture, `screen_width()` is its width, and the key / mouse functions answer from
/// `input`. Draw calls are sent when `f` returns. (How an editor shows a game or a map view.)
pub fn with_screen<R>(screen: &mut Screen, input: &mut input::Input, f: impl FnOnce() -> R) -> R {
    flush();
    {
        let ctx = get_context();
        std::mem::swap(&mut ctx.screen, screen);
        std::mem::swap(&mut ctx.input, input);
        ctx.gl.reset();
        ctx.gl.screen_size = ctx.screen.logical_size();
    }
    let r = f();
    flush();
    let ctx = get_context();
    std::mem::swap(&mut ctx.screen, screen);
    std::mem::swap(&mut ctx.input, input);
    ctx.gl.reset();
    ctx.gl.screen_size = ctx.screen.logical_size();
    r
}

/// The device and queue drawing goes to (for code that shows a [`Screen`] itself).
pub fn device() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
    let ctx = get_context();
    (ctx.gpu.device.clone(), ctx.gpu.queue.clone())
}
