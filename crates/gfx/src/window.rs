//! The window: its size, the frame loop, and what a program may ask of it.
//!
//! [`run`] opens the window and drives a future, one step per frame: the program is written as a
//! loop that ends each turn with `next_frame().await`, as macroquad programs are.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};

use crate::color::Color;
use crate::get_context;

/// What the program asked of the window since the last frame.
#[derive(Default)]
pub(crate) struct Wishes {
    pub fullscreen: Option<bool>,
    pub show_mouse: Option<bool>,
    pub grab: Option<bool>,
    pub size: Option<(f32, f32)>,
    pub prevent_quit: bool,
    pub quit_requested: bool,
}

/// The screen's width in points.
pub fn screen_width() -> f32 {
    get_context().screen.logical_size().0
}

/// The screen's height in points.
pub fn screen_height() -> f32 {
    get_context().screen.logical_size().1
}

/// Pixels per point of the screen.
pub fn dpi_scale() -> f32 {
    get_context().screen.dpi_scale()
}

/// Clears what is drawn into now (the screen, or the camera's render target).
pub fn clear_background(color: Color) {
    get_context().gl.clear(color);
}

pub fn set_fullscreen(fullscreen: bool) {
    get_context().wishes.fullscreen = Some(fullscreen);
}

/// Asks for a window of this size in points.
pub fn request_new_screen_size(width: f32, height: f32) {
    get_context().wishes.size = Some((width, height));
}

/// Draw calls and render passes of the last frame.
pub fn frame_stats() -> crate::batch::Stats {
    get_context().gl.stats
}

/// Ends this turn of the program's loop: the frame is shown, and the loop goes on with the next.
pub fn next_frame() -> NextFrame {
    NextFrame { done: false }
}

pub struct NextFrame {
    done: bool,
}

impl Future for NextFrame {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _: &mut TaskContext<'_>) -> Poll<()> {
        if self.done {
            Poll::Ready(())
        } else {
            self.done = true;
            Poll::Pending
        }
    }
}

fn noop_waker() -> Waker {
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn noop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    // Safety: the vtable's functions do nothing with the pointer.
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

/// One step of a program's future. `true` when it has finished.
pub fn poll_once<F: Future<Output = ()> + ?Sized>(f: Pin<&mut F>) -> bool {
    let waker = noop_waker();
    f.poll(&mut TaskContext::from_waker(&waker)).is_ready()
}

/// Runs a program's future without a window: frames are drawn off screen, as fast as they come,
/// until it finishes (tools that render pictures, tests).
pub fn run_headless(width: u32, height: u32, main: impl Future<Output = ()>) -> Result<(), crate::Error> {
    crate::init_headless(width, height)?;
    let mut main = std::pin::pin!(main);
    loop {
        crate::begin_frame();
        let done = poll_once(main.as_mut());
        crate::end_frame();
        if done {
            return Ok(());
        }
    }
}

/// Waits for a future of wgpu (they are ready at once on desktops).
pub(crate) fn block_on<T>(f: impl Future<Output = T>) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        pollster::block_on(f)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut f = std::pin::pin!(f);
        let waker = noop_waker();
        match f.as_mut().poll(&mut TaskContext::from_waker(&waker)) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("gfx: a GPU answer wasn't ready"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::run;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use winit::application::ApplicationHandler;
    use winit::dpi::{LogicalSize, PhysicalSize};
    use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::keyboard::{Key, KeyCode as Physical, NamedKey, PhysicalKey};
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

    use crate::conf::Conf;
    use crate::input::{KeyCode, MouseButton, TouchPhase};
    use crate::{get_context, gpu, Error};

    struct Shown {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        /// The format the surface is drawn in: `config.format` without sRGB encoding, so the
        /// screen's bytes arrive as they are.
        view_format: wgpu::TextureFormat,
        blit: wgpu::RenderPipeline,
    }

    struct App {
        conf: Conf,
        main: Option<Pin<Box<dyn Future<Output = ()>>>>,
        shown: Option<Shown>,
        error: Option<Error>,
    }

    /// Opens the window and runs `main` in it, one step per frame, until it finishes. Fails when
    /// there is no graphics adapter to draw with (a software renderer doesn't count, see
    /// [`gpu::software_allowed`]).
    pub fn run(conf: Conf, main: impl Future<Output = ()> + 'static) -> Result<(), Error> {
        let event_loop = event_loop()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = App { conf, main: Some(Box::pin(main)), shown: None, error: None };
        event_loop.run_app(&mut app).map_err(|e| Error::Gpu(e.to_string()))?;
        // The program's future (and what it holds: textures, the game) goes before the device.
        app.main = None;
        match app.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// The event loop. On Linux a window of X11 where there is one (also under Wayland, through
    /// XWayland), as macroquad's were: a Wayland compositor stops handing out frames to a window
    /// that is covered, and a game waiting for the display then stands still (scripted runs with
    /// the window behind others never finish). `RUSTPG2D_WAYLAND=1` asks for a Wayland window;
    /// without an X server it is one anyway.
    fn event_loop() -> Result<EventLoop<()>, Error> {
        #[allow(unused_mut)]
        let mut builder = EventLoop::builder();
        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))]
        {
            use winit::platform::x11::EventLoopBuilderExtX11;
            let x11 = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
            let wayland = std::env::var_os("RUSTPG2D_WAYLAND").is_some_and(|v| v == "1");
            if x11 && !wayland {
                builder.with_x11();
            }
        }
        builder.build().map_err(|e| Error::Gpu(e.to_string()))
    }

    impl App {
        fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<Shown, Error> {
            let c = &self.conf;
            let mut attrs = Window::default_attributes()
                .with_title(c.window_title.clone())
                .with_inner_size(LogicalSize::new(c.window_width.max(1) as f64, c.window_height.max(1) as f64))
                .with_resizable(c.window_resizable);
            if c.fullscreen {
                attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
            }
            if let Some(icon) = &c.icon {
                attrs = attrs.with_window_icon(winit::window::Icon::from_rgba(icon.big.to_vec(), 64, 64).ok());
            }
            let window = Arc::new(event_loop.create_window(attrs).map_err(|e| Error::Gpu(e.to_string()))?);
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(event_loop.owned_display_handle())));
            let surface = instance.create_surface(window.clone()).map_err(|e| Error::Gpu(e.to_string()))?;
            let adapter = pollster::block_on(gpu::adapter(&instance, Some(&surface)))?;
            let (device, queue) = pollster::block_on(adapter.request_device(&gpu::device_descriptor(&adapter))).map_err(|e| Error::Gpu(e.to_string()))?;
            let size = window.inner_size();
            let caps = surface.get_capabilities(&adapter);
            // `RUSTPG2D_GFX_INFO=1`: what the game draws with (bug reports).
            if std::env::var_os("RUSTPG2D_GFX_INFO").is_some() {
                let i = adapter.get_info();
                eprintln!("[gfx] {} ({:?}, {:?}, driver {} {}); present modes {:?}; formats {:?}", i.name, i.backend, i.device_type, i.driver, i.driver_info, caps.present_modes, caps.formats);
            }
            let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or(caps.formats.first().copied()).ok_or_else(|| Error::Gpu("the window can't be drawn to".into()))?;
            let view_format = format.remove_srgb_suffix();
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: if c.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync },
                alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
                view_formats: if view_format != format { vec![view_format] } else { vec![] },
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            let blit = blit_pipeline(&device, view_format);
            crate::init(device, queue, size.width.max(1), size.height.max(1), c);
            get_context().screen.resize(size.width, size.height, window.scale_factor() as f32);
            Ok(Shown { window, surface, config, view_format, blit })
        }

        fn frame(&mut self, event_loop: &ActiveEventLoop) {
            let Some(shown) = &mut self.shown else { return };
            crate::begin_frame();
            let finished = match &mut self.main {
                Some(main) => super::poll_once(main.as_mut()),
                None => true,
            };
            crate::end_frame();
            if finished {
                self.main = None;
                event_loop.exit();
                return;
            }
            let ctx = get_context();
            // What the program asked for.
            let w = std::mem::take(&mut ctx.wishes);
            ctx.wishes.prevent_quit = w.prevent_quit;
            ctx.wishes.quit_requested = w.quit_requested;
            if let Some(f) = w.fullscreen {
                shown.window.set_fullscreen(f.then_some(Fullscreen::Borderless(None)));
            }
            if let Some(s) = w.show_mouse {
                shown.window.set_cursor_visible(s);
            }
            if let Some(g) = w.grab {
                let _ = shown.window.set_cursor_grab(if g { CursorGrabMode::Confined } else { CursorGrabMode::None });
            }
            if let Some((width, height)) = w.size {
                let _ = shown.window.request_inner_size(LogicalSize::new(width as f64, height as f64));
            }
            // The screen onto the window.
            let frame = match shown.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                _ => {
                    shown.surface.configure(&ctx.gpu.device, &shown.config);
                    return;
                }
            };
            let view = frame.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(shown.view_format), ..Default::default() });
            let screen = ctx.screen.texture().clone();
            screen.set_filter(crate::texture::FilterMode::Nearest);
            let mut enc = ctx.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gfx present") });
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gfx present"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                    ..Default::default()
                });
                pass.set_pipeline(&shown.blit);
                pass.set_bind_group(0, screen.bind_group(&ctx.gpu), &[]);
                pass.draw(0..3, 0..1);
            }
            ctx.gpu.queue.submit([enc.finish()]);
            shown.window.pre_present_notify();
            ctx.gpu.queue.present(frame);
        }

        fn resize(&mut self, size: PhysicalSize<u32>) {
            let Some(shown) = &mut self.shown else { return };
            if size.width == 0 || size.height == 0 {
                return;
            }
            let ctx = get_context();
            shown.config.width = size.width;
            shown.config.height = size.height;
            shown.surface.configure(&ctx.gpu.device, &shown.config);
            crate::flush();
            ctx.screen.resize(size.width, size.height, shown.window.scale_factor() as f32);
            ctx.gl.screen_size = ctx.screen.logical_size();
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.shown.is_some() {
                return;
            }
            match self.open(event_loop) {
                Ok(s) => self.shown = Some(s),
                Err(e) => {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if self.shown.is_none() {
                return;
            }
            let input = &mut get_context().input;
            match event {
                WindowEvent::CloseRequested => {
                    let w = &mut get_context().wishes;
                    if w.prevent_quit {
                        w.quit_requested = true;
                    } else {
                        self.main = None;
                        event_loop.exit();
                    }
                }
                WindowEvent::Resized(size) => self.resize(size),
                WindowEvent::ScaleFactorChanged { .. } => {
                    let size = self.shown.as_ref().unwrap().window.inner_size();
                    self.resize(size);
                }
                WindowEvent::Focused(false) => input.release_all(),
                WindowEvent::KeyboardInput { event, .. } => {
                    let key = key_code(&event.key_without_modifiers(), event.physical_key);
                    match event.state {
                        ElementState::Pressed => {
                            input.key_down(key, event.repeat);
                            if let Some(text) = &event.text {
                                for c in text.chars().filter(|c| !c.is_control()) {
                                    input.char(c);
                                }
                            }
                        }
                        ElementState::Released => input.key_up(key),
                    }
                }
                WindowEvent::CursorMoved { position, .. } => input.mouse_move(position.x as f32, position.y as f32),
                WindowEvent::MouseInput { state, button, .. } => {
                    let b = match button {
                        winit::event::MouseButton::Left => MouseButton::Left,
                        winit::event::MouseButton::Right => MouseButton::Right,
                        winit::event::MouseButton::Middle => MouseButton::Middle,
                        _ => MouseButton::Unknown,
                    };
                    input.mouse_button(b, state == ElementState::Pressed);
                }
                WindowEvent::MouseWheel { delta, .. } => match delta {
                    MouseScrollDelta::LineDelta(x, y) => input.wheel(x, y),
                    // Touchpads: about 40 pixels make a notch.
                    MouseScrollDelta::PixelDelta(p) => input.wheel(p.x as f32 / 40.0, p.y as f32 / 40.0),
                },
                WindowEvent::Touch(t) => {
                    let phase = match t.phase {
                        winit::event::TouchPhase::Started => TouchPhase::Started,
                        winit::event::TouchPhase::Moved => TouchPhase::Moved,
                        winit::event::TouchPhase::Ended => TouchPhase::Ended,
                        winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
                    };
                    input.touch(t.id, phase, t.location.x as f32, t.location.y as f32);
                }
                _ => {}
            }
        }

        /// A frame whenever the events are through: presenting waits for the display (vsync), so
        /// this paces itself. (Asking for redraws instead is throttled a second time by Wayland's
        /// frame callbacks, and every few frames one is missed.)
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            self.frame(event_loop);
        }
    }

    /// Draws a texture over the whole surface (one triangle).
    fn blit_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gfx present"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@group(0) @binding(0) var Texture: texture_2d<f32>;
@group(0) @binding(1) var TextureSampler: sampler;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    var out: Out;
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}
@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(Texture, TextureSampler, in.uv).rgb, 1.0);
}
"#
                .into(),
            ),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("gfx present"), entries: &gpu::texture_entries(1) });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("gfx present"), bind_group_layouts: &[Some(&layout)], ..Default::default() });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gfx present"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// The key as the keyboard's layout names it (letters, digits, punctuation), else by its place.
    fn key_code(logical: &Key, physical: PhysicalKey) -> KeyCode {
        use KeyCode as K;
        // Keys the layout doesn't tell apart: sides of modifiers, the keypad.
        if let PhysicalKey::Code(c) = physical {
            let k = match c {
                Physical::ShiftLeft => K::LeftShift,
                Physical::ShiftRight => K::RightShift,
                Physical::ControlLeft => K::LeftControl,
                Physical::ControlRight => K::RightControl,
                Physical::AltLeft => K::LeftAlt,
                Physical::AltRight => K::RightAlt,
                Physical::SuperLeft => K::LeftSuper,
                Physical::SuperRight => K::RightSuper,
                Physical::Numpad0 => K::Kp0,
                Physical::Numpad1 => K::Kp1,
                Physical::Numpad2 => K::Kp2,
                Physical::Numpad3 => K::Kp3,
                Physical::Numpad4 => K::Kp4,
                Physical::Numpad5 => K::Kp5,
                Physical::Numpad6 => K::Kp6,
                Physical::Numpad7 => K::Kp7,
                Physical::Numpad8 => K::Kp8,
                Physical::Numpad9 => K::Kp9,
                Physical::NumpadDecimal => K::KpDecimal,
                Physical::NumpadDivide => K::KpDivide,
                Physical::NumpadMultiply => K::KpMultiply,
                Physical::NumpadSubtract => K::KpSubtract,
                Physical::NumpadAdd => K::KpAdd,
                Physical::NumpadEnter => K::KpEnter,
                Physical::NumpadEqual => K::KpEqual,
                _ => K::Unknown,
            };
            if k != K::Unknown {
                return k;
            }
        }
        match logical {
            Key::Named(n) => match n {
                NamedKey::Space => K::Space,
                NamedKey::Escape => K::Escape,
                NamedKey::Enter => K::Enter,
                NamedKey::Tab => K::Tab,
                NamedKey::Backspace => K::Backspace,
                NamedKey::Insert => K::Insert,
                NamedKey::Delete => K::Delete,
                NamedKey::ArrowRight => K::Right,
                NamedKey::ArrowLeft => K::Left,
                NamedKey::ArrowDown => K::Down,
                NamedKey::ArrowUp => K::Up,
                NamedKey::PageUp => K::PageUp,
                NamedKey::PageDown => K::PageDown,
                NamedKey::Home => K::Home,
                NamedKey::End => K::End,
                NamedKey::CapsLock => K::CapsLock,
                NamedKey::ScrollLock => K::ScrollLock,
                NamedKey::NumLock => K::NumLock,
                NamedKey::PrintScreen => K::PrintScreen,
                NamedKey::Pause => K::Pause,
                NamedKey::ContextMenu => K::Menu,
                NamedKey::BrowserBack => K::Back,
                NamedKey::F1 => K::F1,
                NamedKey::F2 => K::F2,
                NamedKey::F3 => K::F3,
                NamedKey::F4 => K::F4,
                NamedKey::F5 => K::F5,
                NamedKey::F6 => K::F6,
                NamedKey::F7 => K::F7,
                NamedKey::F8 => K::F8,
                NamedKey::F9 => K::F9,
                NamedKey::F10 => K::F10,
                NamedKey::F11 => K::F11,
                NamedKey::F12 => K::F12,
                NamedKey::F13 => K::F13,
                NamedKey::F14 => K::F14,
                NamedKey::F15 => K::F15,
                NamedKey::F16 => K::F16,
                NamedKey::F17 => K::F17,
                NamedKey::F18 => K::F18,
                NamedKey::F19 => K::F19,
                NamedKey::F20 => K::F20,
                NamedKey::F21 => K::F21,
                NamedKey::F22 => K::F22,
                NamedKey::F23 => K::F23,
                NamedKey::F24 => K::F24,
                NamedKey::F25 => K::F25,
                _ => K::Unknown,
            },
            Key::Character(s) => {
                let mut chars = s.chars();
                match (chars.next().map(|c| c.to_ascii_lowercase()), chars.next()) {
                    (Some(c), None) => match c {
                        ' ' => K::Space,
                        '\'' => K::Apostrophe,
                        ',' => K::Comma,
                        '-' => K::Minus,
                        '.' => K::Period,
                        '/' => K::Slash,
                        ';' => K::Semicolon,
                        '=' => K::Equal,
                        '[' => K::LeftBracket,
                        '\\' => K::Backslash,
                        ']' => K::RightBracket,
                        '`' => K::GraveAccent,
                        '0' => K::Key0,
                        '1' => K::Key1,
                        '2' => K::Key2,
                        '3' => K::Key3,
                        '4' => K::Key4,
                        '5' => K::Key5,
                        '6' => K::Key6,
                        '7' => K::Key7,
                        '8' => K::Key8,
                        '9' => K::Key9,
                        'a' => K::A,
                        'b' => K::B,
                        'c' => K::C,
                        'd' => K::D,
                        'e' => K::E,
                        'f' => K::F,
                        'g' => K::G,
                        'h' => K::H,
                        'i' => K::I,
                        'j' => K::J,
                        'k' => K::K,
                        'l' => K::L,
                        'm' => K::M,
                        'n' => K::N,
                        'o' => K::O,
                        'p' => K::P,
                        'q' => K::Q,
                        'r' => K::R,
                        's' => K::S,
                        't' => K::T,
                        'u' => K::U,
                        'v' => K::V,
                        'w' => K::W,
                        'x' => K::X,
                        'y' => K::Y,
                        'z' => K::Z,
                        _ => K::Unknown,
                    },
                    _ => K::Unknown,
                }
            }
            _ => K::Unknown,
        }
    }
}
