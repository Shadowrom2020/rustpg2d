//! Pictures in memory ([`Image`]), on the GPU ([`Texture2D`]), and pictures drawn into
//! ([`RenderTarget`], [`Screen`]).

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use glam::{vec2, Vec2};

use crate::batch::{DrawMode, Vertex};
use crate::color::Color;
use crate::gpu::{Gpu, COLOR_FORMAT, DEPTH_FORMAT};
use crate::math::Rect;
use crate::{get_context, Error};

pub use image::ImageFormat;

/// How a texture is sampled between its pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum FilterMode {
    #[default]
    Linear = 0,
    Nearest = 1,
}

/// A picture in memory: RGBA, 8 bits each, rows from the top.
#[derive(Clone)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub width: u16,
    pub height: u16,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image").field("width", &self.width).field("height", &self.height).field("bytes.len()", &self.bytes.len()).finish()
    }
}

impl Image {
    pub const fn empty() -> Image {
        Image { width: 0, height: 0, bytes: vec![] }
    }

    /// Decodes a PNG or JPEG file's bytes (`None`: the format is guessed).
    pub fn from_file_with_format(bytes: &[u8], format: Option<ImageFormat>) -> Result<Image, Error> {
        let img = match format {
            Some(fmt) => image::load_from_memory_with_format(bytes, fmt),
            None => image::load_from_memory(bytes),
        }
        .map_err(|e| Error::Image(e.to_string()))?
        .to_rgba8();
        let (width, height) = (img.width() as u16, img.height() as u16);
        Ok(Image { width, height, bytes: img.into_raw() })
    }

    pub fn gen_image_color(width: u16, height: u16, color: Color) -> Image {
        let px: [u8; 4] = [(color.r * 255.) as u8, (color.g * 255.) as u8, (color.b * 255.) as u8, (color.a * 255.) as u8];
        let mut bytes = vec![0; width as usize * height as usize * 4];
        for p in bytes.as_chunks_mut::<4>().0 {
            *p = px;
        }
        Image { width, height, bytes }
    }

    pub fn update(&mut self, colors: &[Color]) {
        assert!(self.width as usize * self.height as usize == colors.len());
        for (p, c) in self.bytes.as_chunks_mut::<4>().0.iter_mut().zip(colors) {
            *p = [(c.r * 255.) as u8, (c.g * 255.) as u8, (c.b * 255.) as u8, (c.a * 255.) as u8];
        }
    }

    pub const fn width(&self) -> usize {
        self.width as usize
    }

    pub const fn height(&self) -> usize {
        self.height as usize
    }

    pub fn get_image_data(&self) -> &[[u8; 4]] {
        assert!(self.width as usize * self.height as usize * 4 == self.bytes.len());
        bytemuck::cast_slice(&self.bytes)
    }

    pub fn get_image_data_mut(&mut self) -> &mut [[u8; 4]] {
        assert!(self.width as usize * self.height as usize * 4 == self.bytes.len());
        bytemuck::cast_slice_mut(&mut self.bytes)
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        assert!(x < self.width as u32);
        assert!(y < self.height as u32);
        let width = self.width;
        self.get_image_data_mut()[(y * width as u32 + x) as usize] = color.into();
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        self.get_image_data()[(y * self.width as u32 + x) as usize].into()
    }

    pub fn sub_image(&self, rect: Rect) -> Image {
        let (width, height) = (rect.w as usize, rect.h as usize);
        let (x, y) = (rect.x as usize, rect.y as usize);
        let mut bytes = Vec::with_capacity(width * height * 4);
        for row in y..y + height {
            let at = row * self.width as usize * 4 + x * 4;
            bytes.extend_from_slice(&self.bytes[at..at + width * 4]);
        }
        Image { width: width as u16, height: height as u16, bytes }
    }

    /// Writes the picture as a PNG file, **bottom row first**: right for [`get_screen_data`], whose
    /// rows come bottom-up (as macroquad's did); a picture with its top row first is flipped.
    pub fn export_png(&self, path: &str) {
        let row = self.width as usize * 4;
        let mut bytes = Vec::with_capacity(self.bytes.len());
        for y in (0..self.height as usize).rev() {
            bytes.extend_from_slice(&self.bytes[y * row..(y + 1) * row]);
        }
        if let Err(e) = image::save_buffer(path, &bytes, self.width as _, self.height as _, image::ColorType::Rgba8) {
            eprintln!("[gfx] {path}: {e}");
        }
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct TexInner {
    pub id: u64,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    filter: AtomicU8,
    /// The bind groups of the texture with the linear and the nearest sampler, made when first drawn.
    bind: [OnceLock<wgpu::BindGroup>; 2],
}

/// A picture on the GPU. Cloning shares it; it is freed with its last clone (and the draw calls
/// that still show it).
#[derive(Clone)]
pub struct Texture2D(pub(crate) Arc<TexInner>);

impl PartialEq for Texture2D {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Texture2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Texture2D({}x{} #{})", self.0.width, self.0.height, self.0.id)
    }
}

impl Texture2D {
    pub(crate) fn create(gpu: &Gpu, width: u32, height: u32, target: bool, filter: FilterMode) -> Texture2D {
        let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC;
        if target {
            usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        }
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gfx texture"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Texture2D(Arc::new(TexInner {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            texture,
            view,
            width: width.max(1),
            height: height.max(1),
            filter: AtomicU8::new(filter as u8),
            bind: [OnceLock::new(), OnceLock::new()],
        }))
    }

    pub(crate) fn write(&self, gpu: &Gpu, x: u32, y: u32, width: u32, height: u32, bytes: &[u8]) {
        if width == 0 || height == 0 || bytes.len() < (width * height * 4) as usize {
            return;
        }
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.0.texture, mip_level: 0, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
            &bytes[..(width * height * 4) as usize],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    /// A one-pixel white texture.
    pub fn empty() -> Texture2D {
        get_context().gl.white_texture.clone()
    }

    /// A texture of a PNG / JPEG file's bytes. Panics when they can't be decoded (as macroquad did);
    /// decode with [`Image::from_file_with_format`] to handle the error.
    pub fn from_file_with_format(bytes: &[u8], format: Option<ImageFormat>) -> Texture2D {
        let img = Image::from_file_with_format(bytes, format).unwrap_or_else(|e| panic!("{e}"));
        Texture2D::from_image(&img)
    }

    pub fn from_image(image: &Image) -> Texture2D {
        Texture2D::from_rgba8(image.width, image.height, &image.bytes)
    }

    pub fn from_rgba8(width: u16, height: u16, bytes: &[u8]) -> Texture2D {
        let ctx = get_context();
        let t = Texture2D::create(&ctx.gpu, width as u32, height as u32, false, ctx.default_filter_mode);
        t.write(&ctx.gpu, 0, 0, width as u32, height as u32, bytes);
        t
    }

    /// Replaces the whole picture (same size).
    pub fn update(&self, image: &Image) {
        self.update_from_bytes(image.width as u32, image.height as u32, &image.bytes);
    }

    pub fn update_from_bytes(&self, width: u32, height: u32, bytes: &[u8]) {
        self.update_part_bytes(0, 0, width, height, bytes);
    }

    /// Replaces a part of the picture with `image`.
    pub fn update_part(&self, image: &Image, x_offset: i32, y_offset: i32, width: i32, height: i32) {
        self.update_part_bytes(x_offset.max(0) as u32, y_offset.max(0) as u32, width.max(0) as u32, height.max(0) as u32, &image.bytes);
    }

    fn update_part_bytes(&self, x: u32, y: u32, width: u32, height: u32, bytes: &[u8]) {
        let ctx = get_context();
        // Draw calls waiting for the GPU show the picture as it is now.
        if ctx.gl.uses(self) {
            crate::flush();
        }
        if x + width > self.0.width || y + height > self.0.height {
            eprintln!("[gfx] texture update outside the texture ({x},{y} {width}x{height} in {}x{})", self.0.width, self.0.height);
            return;
        }
        self.write(&ctx.gpu, x, y, width, height, bytes);
    }

    pub fn width(&self) -> f32 {
        self.0.width as f32
    }

    pub fn height(&self) -> f32 {
        self.0.height as f32
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.width(), self.height())
    }

    pub fn set_filter(&self, filter_mode: FilterMode) {
        self.0.filter.store(filter_mode as u8, Ordering::Relaxed);
    }

    pub fn filter(&self) -> FilterMode {
        if self.0.filter.load(Ordering::Relaxed) == FilterMode::Nearest as u8 {
            FilterMode::Nearest
        } else {
            FilterMode::Linear
        }
    }

    /// The same texture (macroquad told apart owning and weak clones; here every clone shares).
    pub fn weak_clone(&self) -> Texture2D {
        self.clone()
    }

    pub(crate) fn bind_group(&self, gpu: &Gpu) -> &wgpu::BindGroup {
        let filter = self.filter();
        self.0.bind[filter as usize].get_or_init(|| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gfx texture"),
                layout: &gpu.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.0.view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(if filter == FilterMode::Nearest { &gpu.nearest } else { &gpu.linear }) },
                ],
            })
        })
    }

    /// The texture's view, to show it with other wgpu code (an editor's UI).
    pub fn view(&self) -> &wgpu::TextureView {
        &self.0.view
    }

    /// The wgpu texture itself.
    pub fn raw(&self) -> &wgpu::Texture {
        &self.0.texture
    }

    /// Reads the picture back from the GPU (slow: waits for it), rows in memory order.
    pub fn get_texture_data(&self) -> Image {
        crate::flush();
        let gpu = &get_context().gpu;
        let (w, h) = (self.0.width, self.0.height);
        let row = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("gfx readback"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gfx readback") });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &self.0.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([enc.finish()]);
        let slice = buffer.slice(..);
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = done.clone();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            if r.is_ok() {
                flag.store(true, Ordering::SeqCst);
            }
        });
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let mut bytes = vec![0u8; (w * h * 4) as usize];
        let data = if done.load(Ordering::SeqCst) { slice.get_mapped_range().ok() } else { None };
        if let Some(data) = data {
            for y in 0..h as usize {
                let at = y * row as usize;
                bytes[y * w as usize * 4..(y + 1) * w as usize * 4].copy_from_slice(&data[at..at + w as usize * 4]);
            }
        } else {
            eprintln!("[gfx] reading a texture back failed");
        }
        buffer.unmap();
        Image { width: w as u16, height: h as u16, bytes }
    }
}

pub(crate) struct TargetInner {
    pub depth: Option<wgpu::TextureView>,
    /// Render targets are drawn upside down, as OpenGL's were: the engine's cameras and texture
    /// coordinates count on it. The screen is not.
    pub flip: bool,
}

/// A picture that is drawn into (`Camera2D::render_target`) and then used as a texture.
#[derive(Clone)]
pub struct RenderTarget {
    pub texture: Texture2D,
    pub(crate) inner: Arc<TargetInner>,
}

impl std::fmt::Debug for RenderTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RenderTarget({:?})", self.texture)
    }
}

impl PartialEq for RenderTarget {
    fn eq(&self, other: &Self) -> bool {
        self.texture == other.texture
    }
}

#[derive(Debug, Clone)]
pub struct RenderTargetParams {
    /// Only 1 is supported.
    pub sample_count: i32,
    pub depth: bool,
}

impl Default for RenderTargetParams {
    fn default() -> RenderTargetParams {
        RenderTargetParams { sample_count: 1, depth: false }
    }
}

pub fn render_target(width: u32, height: u32) -> RenderTarget {
    render_target_ex(width, height, RenderTargetParams::default())
}

pub fn render_target_ex(width: u32, height: u32, params: RenderTargetParams) -> RenderTarget {
    make_target(&get_context().gpu, width, height, params.depth, true)
}

pub(crate) fn make_target(gpu: &Gpu, width: u32, height: u32, depth: bool, flip: bool) -> RenderTarget {
    let texture = Texture2D::create(gpu, width, height, true, FilterMode::Linear);
    let depth = depth.then(|| {
        gpu.device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("gfx depth"),
                size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    });
    RenderTarget { texture, inner: Arc::new(TargetInner { depth, flip }) }
}

/// What "the screen" is: a picture of its own, with the scale between its pixels and the points the
/// engine lays things out in. The window copies it to its surface; an editor shows it in a panel.
pub struct Screen {
    pub(crate) target: RenderTarget,
    dpi: f32,
}

impl Screen {
    /// A screen of `width` × `height` pixels; `dpi_scale` pixels make one point of `screen_width()`.
    pub fn new(width: u32, height: u32, dpi_scale: f32) -> Screen {
        Screen::new_with(&get_context().gpu, width, height, dpi_scale)
    }

    pub(crate) fn new_with(gpu: &Gpu, width: u32, height: u32, dpi_scale: f32) -> Screen {
        Screen { target: make_target(gpu, width, height, true, false), dpi: if dpi_scale > 0.0 { dpi_scale } else { 1.0 } }
    }

    /// Changes the size (a new, empty picture) when it differs.
    pub fn resize(&mut self, width: u32, height: u32, dpi_scale: f32) {
        let (width, height) = (width.max(1), height.max(1));
        if self.target.texture.0.width != width || self.target.texture.0.height != height {
            self.target = make_target(&get_context().gpu, width, height, true, false);
        }
        if dpi_scale > 0.0 {
            self.dpi = dpi_scale;
        }
    }

    /// The picture, top row first.
    pub fn texture(&self) -> &Texture2D {
        &self.target.texture
    }

    /// Size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.target.texture.0.width, self.target.texture.0.height)
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi
    }

    /// Size in points: what `screen_width()` / `screen_height()` answer.
    pub fn logical_size(&self) -> (f32, f32) {
        (self.target.texture.0.width as f32 / self.dpi, self.target.texture.0.height as f32 / self.dpi)
    }
}

/// The screen as a picture, **bottom row first** (as macroquad's was; [`Image::export_png`] turns
/// it upright).
pub fn get_screen_data() -> Image {
    let mut img = get_context().screen.target.texture.clone().get_texture_data();
    let row = img.width as usize * 4;
    let mut flipped = Vec::with_capacity(img.bytes.len());
    for y in (0..img.height as usize).rev() {
        flipped.extend_from_slice(&img.bytes[y * row..(y + 1) * row]);
    }
    img.bytes = flipped;
    img
}

#[derive(Debug, Clone)]
pub struct DrawTextureParams {
    pub dest_size: Option<Vec2>,
    /// Part of the texture to draw, in pixels. `None`: all of it.
    pub source: Option<Rect>,
    /// Rotation in radians.
    pub rotation: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    /// The point the rotation turns around, in screen space. `None`: the middle.
    pub pivot: Option<Vec2>,
}

impl Default for DrawTextureParams {
    fn default() -> DrawTextureParams {
        DrawTextureParams { dest_size: None, source: None, rotation: 0., pivot: None, flip_x: false, flip_y: false }
    }
}

pub fn draw_texture(texture: &Texture2D, x: f32, y: f32, color: Color) {
    draw_texture_ex(texture, x, y, color, Default::default());
}

pub fn draw_texture_ex(texture: &Texture2D, x: f32, y: f32, color: Color, params: DrawTextureParams) {
    let context = get_context();
    let [width, height] = texture.size().to_array();
    let Rect { x: sx, y: sy, w: sw, h: sh } = params.source.unwrap_or(Rect { x: 0., y: 0., w: width, h: height });
    let (mut w, mut h) = match params.dest_size {
        Some(dst) => (dst.x, dst.y),
        _ => (sw, sh),
    };
    let (mut x, mut y) = (x, y);
    if params.flip_x {
        x += w;
        w = -w;
    }
    if params.flip_y {
        y += h;
        h = -h;
    }
    let pivot = params.pivot.unwrap_or(vec2(x + w / 2., y + h / 2.));
    let m = pivot;
    let p = [vec2(x, y) - pivot, vec2(x + w, y) - pivot, vec2(x + w, y + h) - pivot, vec2(x, y + h) - pivot];
    let r = params.rotation;
    let (sin, cos) = (r.sin(), r.cos());
    let p = p.map(|p| vec2(p.x * cos - p.y * sin, p.x * sin + p.y * cos) + m);
    #[rustfmt::skip]
    let vertices = [
        Vertex::new(p[0].x, p[0].y, 0.,  sx      /width,  sy      /height, color),
        Vertex::new(p[1].x, p[1].y, 0., (sx + sw)/width,  sy      /height, color),
        Vertex::new(p[2].x, p[2].y, 0., (sx + sw)/width, (sy + sh)/height, color),
        Vertex::new(p[3].x, p[3].y, 0.,  sx      /width, (sy + sh)/height, color),
    ];
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
    context.gl.texture(Some(texture));
    context.gl.draw_mode(DrawMode::Triangles);
    context.gl.geometry(&vertices, &indices);
}

/// The filter new textures get (`Conf::default_filter_mode` at the start).
pub fn set_default_filter_mode(filter: FilterMode) {
    get_context().default_filter_mode = filter;
}
