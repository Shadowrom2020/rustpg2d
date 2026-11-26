//! The draw list: every drawing function ends in [`Batch::geometry`], which collects triangles
//! into draw calls; [`Batch::flush`] sends them to the GPU as render passes, in order.
//!
//! A port of macroquad's `quad_gl.rs` in behaviour (what starts a new draw call, what a draw call
//! remembers), with two differences: a draw call keeps the projection it was made under, so the
//! camera can change without flushing, and clearing is an entry of the list.

use std::collections::HashMap;
use std::sync::Weak;

use glam::{Mat4, Vec2, Vec3, Vec4};

use crate::color::Color;
use crate::gpu::{Gpu, COLOR_FORMAT, DEPTH_FORMAT};
use crate::material::{Material, MaterialInner, PipelineParams};
use crate::texture::{RenderTarget, Screen, Texture2D};

/// One corner of a triangle. `color` is 0-255 per channel and reaches shaders as 0-1.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec3,
    pub uv: Vec2,
    pub color: [u8; 4],
    /// Free for shaders (the model shader's normal); zero from the 2D functions.
    pub normal: Vec4,
}

impl Vertex {
    pub fn new(x: f32, y: f32, z: f32, u: f32, v: f32, color: Color) -> Vertex {
        Vertex { position: Vec3::new(x, y, z), uv: Vec2::new(u, v), color: color.into(), normal: Vec4::ZERO }
    }

    pub fn new2(position: Vec3, uv: Vec2, color: Color) -> Vertex {
        Vertex { position, uv, color: color.into(), normal: Vec4::ZERO }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawMode {
    Triangles,
    Lines,
}

/// The bytes of one draw call's uniform block (and the step between blocks).
pub const UNIFORM_BLOCK: u64 = 256;
/// Where a material's own uniforms start in the block: after `Projection`, `Model`, `_Time`.
pub const UNIFORM_HEADER: usize = 144;

/// The shader of plain drawing: the texture times the vertex colour. A material that only blends
/// differently takes it as its source.
pub const DEFAULT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = u.Projection * u.Model * vec4<f32>(v.position, 1.0);
    out.color = v.color0;
    out.uv = v.texcoord;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color * textureSample(Texture, TextureSampler, in.uv);
}
"#;

#[derive(Clone)]
enum Pipe {
    /// Plain drawing; `true` with a 3D camera (depth tested and written).
    Default(bool),
    Material(Material),
}

impl Pipe {
    fn same(&self, other: &Pipe) -> bool {
        match (self, other) {
            (Pipe::Default(a), Pipe::Default(b)) => a == b,
            (Pipe::Material(a), Pipe::Material(b)) => a == b,
            _ => false,
        }
    }
}

struct DrawCall {
    vertices_start: usize,
    vertices_count: usize,
    indices_start: usize,
    indices_count: usize,
    clip: Option<(i32, i32, i32, i32)>,
    viewport: Option<(i32, i32, i32, i32)>,
    texture: Option<Texture2D>,
    model: Mat4,
    projection: Mat4,
    draw_mode: DrawMode,
    pipe: Pipe,
    /// The material's uniform block and textures as they were when the call started.
    uniforms: Option<Vec<u8>>,
    textures: Vec<Texture2D>,
    target: Option<RenderTarget>,
}

// (Draw calls are nearly all of the list: boxing them would cost an allocation each.)
#[allow(clippy::large_enum_variant)]
enum Cmd {
    Draw(DrawCall),
    Clear { target: Option<RenderTarget>, color: Color },
}

struct State {
    texture: Option<Texture2D>,
    draw_mode: DrawMode,
    clip: Option<(i32, i32, i32, i32)>,
    viewport: Option<(i32, i32, i32, i32)>,
    model_stack: Vec<Mat4>,
    pipeline: Option<Material>,
    depth_test_enable: bool,
    break_batching: bool,
    target: Option<RenderTarget>,
}

/// GPU buffers of a flush, kept and grown.
struct Buffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    uniform_bind: wgpu::BindGroup,
}

#[derive(PartialEq, Eq, Hash)]
struct PipelineKey {
    /// 0 = the default shader.
    material: u64,
    default_depth: bool,
    lines: bool,
    target_depth: bool,
}

pub struct Batch {
    cmds: Vec<Cmd>,
    state: State,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    /// `None` = the screen's pixel grid.
    pub camera_matrix: Option<Mat4>,
    /// The screen's size in points, for the default projection.
    pub screen_size: (f32, f32),
    pub white_texture: Texture2D,
    default_shader: wgpu::ShaderModule,
    default_layout: wgpu::PipelineLayout,
    pipelines: HashMap<PipelineKey, (Option<Weak<MaterialInner>>, wgpu::RenderPipeline)>,
    buffers: Option<Buffers>,
    /// Draw calls and passes of the last flushes (for `status` lines and tests).
    pub stats: Stats,
}

/// What the last frame cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub draw_calls: usize,
    pub passes: usize,
    pub vertices: usize,
}

impl Batch {
    pub fn new(gpu: &Gpu) -> Batch {
        let white_texture = Texture2D::create(gpu, 1, 1, false, crate::texture::FilterMode::Nearest);
        white_texture.write(gpu, 0, 0, 1, 1, &[255, 255, 255, 255]);
        let source = crate::material::shader_source(&[], &[], DEFAULT_SHADER);
        let default_shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("gfx default"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let default_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("gfx default"), bind_group_layouts: &[Some(&gpu.uniform_layout), Some(&gpu.texture_layout)], ..Default::default() });
        Batch {
            cmds: Vec::with_capacity(256),
            state: State { texture: None, draw_mode: DrawMode::Triangles, clip: None, viewport: None, model_stack: vec![Mat4::IDENTITY], pipeline: None, depth_test_enable: false, break_batching: false, target: None },
            vertices: Vec::with_capacity(16384),
            indices: Vec::with_capacity(32768),
            camera_matrix: None,
            screen_size: (1.0, 1.0),
            white_texture,
            default_shader,
            default_layout,
            pipelines: HashMap::new(),
            buffers: None,
            stats: Stats::default(),
        }
    }

    fn model(&self) -> Mat4 {
        *self.state.model_stack.last().unwrap()
    }

    pub(crate) fn projection(&self) -> Mat4 {
        self.camera_matrix.unwrap_or_else(|| Mat4::orthographic_rh_gl(0., self.screen_size.0, self.screen_size.1, 0., -1., 1.))
    }

    /// Back to the state a frame starts with (what was drawn stays in the list).
    pub fn reset(&mut self) {
        self.state.clip = None;
        self.state.viewport = None;
        self.state.texture = None;
        self.state.model_stack = vec![Mat4::IDENTITY];
        self.state.pipeline = None;
        self.state.depth_test_enable = false;
        self.state.target = None;
        self.state.draw_mode = DrawMode::Triangles;
        self.state.break_batching = true;
        self.camera_matrix = None;
    }

    /// Clears what is drawn into now (colour, and depth to the far end).
    pub fn clear(&mut self, color: Color) {
        self.cmds.push(Cmd::Clear { target: self.state.target.clone(), color });
        self.state.break_batching = true;
    }

    pub fn get_active_target(&self) -> Option<RenderTarget> {
        self.state.target.clone()
    }

    pub fn is_depth_test_enabled(&self) -> bool {
        self.state.depth_test_enable
    }

    pub fn render_target(&mut self, target: Option<RenderTarget>) {
        self.state.target = target;
    }

    pub fn depth_test(&mut self, enable: bool) {
        self.state.depth_test_enable = enable;
    }

    pub fn texture(&mut self, texture: Option<&Texture2D>) {
        if self.state.texture.as_ref() != texture {
            self.state.texture = texture.cloned();
        }
    }

    /// Clips drawing to a rectangle of the target, in pixels from its top left.
    pub fn scissor(&mut self, clip: Option<(i32, i32, i32, i32)>) {
        self.state.clip = clip;
    }

    pub fn viewport(&mut self, viewport: Option<(i32, i32, i32, i32)>) {
        self.state.viewport = viewport;
    }

    pub fn push_model_matrix(&mut self, matrix: Mat4) {
        self.state.model_stack.push(self.model() * matrix);
    }

    pub fn pop_model_matrix(&mut self) {
        if self.state.model_stack.len() > 1 {
            self.state.model_stack.pop();
        }
    }

    pub fn pipeline(&mut self, pipeline: Option<Material>) {
        if self.state.pipeline == pipeline {
            return;
        }
        self.state.break_batching = true;
        self.state.pipeline = pipeline;
    }

    /// A material's uniform or texture changed: the next geometry starts a new draw call.
    pub fn material_changed(&mut self) {
        self.state.break_batching = true;
    }

    pub fn draw_mode(&mut self, mode: DrawMode) {
        self.state.draw_mode = mode;
    }

    /// Does a waiting draw call show `texture`?
    pub fn uses(&self, texture: &Texture2D) -> bool {
        self.cmds.iter().any(|c| match c {
            Cmd::Draw(d) => d.texture.as_ref() == Some(texture) || d.textures.contains(texture),
            Cmd::Clear { .. } => false,
        })
    }

    pub fn geometry(&mut self, vertices: &[Vertex], indices: &[u16]) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        if vertices.len() > u16::MAX as usize {
            eprintln!("[gfx] a mesh of {} vertices is too big for one draw call (65535)", vertices.len());
            return;
        }
        let pipe = match &self.state.pipeline {
            Some(m) => Pipe::Material(m.clone()),
            None => Pipe::Default(self.state.depth_test_enable),
        };
        let (model, projection) = (self.model(), self.projection());
        let fits = match self.cmds.last() {
            Some(Cmd::Draw(dc)) => {
                !self.state.break_batching
                    && dc.texture == self.state.texture
                    && dc.clip == self.state.clip
                    && dc.viewport == self.state.viewport
                    && dc.model == model
                    && dc.projection == projection
                    && dc.pipe.same(&pipe)
                    && dc.target == self.state.target
                    && dc.draw_mode == self.state.draw_mode
                    && dc.vertices_count + vertices.len() <= u16::MAX as usize
            }
            _ => false,
        };
        if !fits {
            let (uniforms, textures) = match &pipe {
                Pipe::Material(m) => (Some(m.uniform_block()), m.textures()),
                Pipe::Default(_) => (None, Vec::new()),
            };
            self.cmds.push(Cmd::Draw(DrawCall {
                vertices_start: self.vertices.len(),
                vertices_count: 0,
                indices_start: self.indices.len(),
                indices_count: 0,
                clip: self.state.clip,
                viewport: self.state.viewport,
                texture: self.state.texture.clone(),
                model,
                projection,
                draw_mode: self.state.draw_mode,
                pipe,
                uniforms,
                textures,
                target: self.state.target.clone(),
            }));
            self.state.break_batching = false;
        }
        let Some(Cmd::Draw(dc)) = self.cmds.last_mut() else { return };
        let base = dc.vertices_count as u16;
        self.vertices.extend_from_slice(vertices);
        self.indices.extend(indices.iter().map(|i| *i + base));
        dc.vertices_count += vertices.len();
        dc.indices_count += indices.len();
    }

    fn pipeline_for(&mut self, gpu: &Gpu, pipe: &Pipe, mode: DrawMode, target_depth: bool) -> wgpu::RenderPipeline {
        let key = PipelineKey {
            material: match pipe {
                Pipe::Material(m) => m.id(),
                Pipe::Default(_) => 0,
            },
            default_depth: matches!(pipe, Pipe::Default(true)),
            lines: mode == DrawMode::Lines,
            target_depth,
        };
        if let Some((_, p)) = self.pipelines.get(&key) {
            return p.clone();
        }
        let alpha = crate::material::BlendState::new(crate::material::Equation::Add, crate::material::BlendFactor::Value(crate::material::BlendValue::SourceAlpha), crate::material::BlendFactor::OneMinusValue(crate::material::BlendValue::SourceAlpha));
        let (module, layout, params, weak) = match pipe {
            Pipe::Default(depth) => {
                let mut p = PipelineParams { color_blend: Some(alpha), ..Default::default() };
                if *depth {
                    p.depth_write = true;
                    p.depth_test = crate::material::Comparison::LessOrEqual;
                }
                (self.default_shader.clone(), self.default_layout.clone(), p, None)
            }
            Pipe::Material(m) => (m.module().clone(), m.layout().clone(), m.params(), Some(m.downgrade())),
        };
        let blend = wgpu::BlendState {
            color: params.color_blend.map_or(wgpu::BlendComponent::REPLACE, |b| b.component()),
            alpha: params.alpha_blend.or(params.color_blend).map_or(wgpu::BlendComponent::REPLACE, |b| b.component()),
        };
        let (cr, cg, cb, ca) = params.color_write;
        let mut write_mask = wgpu::ColorWrites::empty();
        write_mask.set(wgpu::ColorWrites::RED, cr);
        write_mask.set(wgpu::ColorWrites::GREEN, cg);
        write_mask.set(wgpu::ColorWrites::BLUE, cb);
        write_mask.set(wgpu::ColorWrites::ALPHA, ca);
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gfx pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4, 3 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: if mode == DrawMode::Lines { wgpu::PrimitiveTopology::LineList } else { wgpu::PrimitiveTopology::TriangleList },
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: target_depth.then(|| wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(params.depth_write),
                depth_compare: Some(params.depth_test.into()),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: Some(blend), write_mask })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(key, (weak, pipeline.clone()));
        pipeline
    }

    fn buffers(&mut self, gpu: &Gpu, vertices: u64, indices: u64, uniforms: u64) {
        let fits = self.buffers.as_ref().is_some_and(|b| b.vertices.size() >= vertices && b.indices.size() >= indices && b.uniforms.size() >= uniforms);
        if fits {
            return;
        }
        let grow = |want: u64, old: Option<u64>| want.max(old.unwrap_or(0)).next_power_of_two().max(4096);
        let old = self.buffers.as_ref();
        let buffer = |label, size, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let v = buffer("gfx vertices", grow(vertices, old.map(|b| b.vertices.size())), wgpu::BufferUsages::VERTEX);
        let i = buffer("gfx indices", grow(indices, old.map(|b| b.indices.size())), wgpu::BufferUsages::INDEX);
        let u = buffer("gfx uniforms", grow(uniforms, old.map(|b| b.uniforms.size())), wgpu::BufferUsages::UNIFORM);
        let uniform_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gfx uniforms"),
            layout: &gpu.uniform_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &u, offset: 0, size: std::num::NonZeroU64::new(UNIFORM_BLOCK) }) }],
        });
        self.buffers = Some(Buffers { vertices: v, indices: i, uniforms: u, uniform_bind });
    }

    /// Sends the list to the GPU: one render pass per run of draw calls into the same picture.
    pub fn flush(&mut self, gpu: &Gpu, screen: &Screen, time: f32) {
        if self.cmds.is_empty() {
            return;
        }
        let cmds = std::mem::take(&mut self.cmds);
        let draws = cmds.iter().filter(|c| matches!(c, Cmd::Draw(_))).count();

        // Uniform blocks, one per draw call.
        let mut uniform_bytes = vec![0u8; draws.max(1) * UNIFORM_BLOCK as usize];
        let time = [time, time.sin(), time.cos(), 0.0f32];
        let mut k = 0;
        for c in &cmds {
            let Cmd::Draw(dc) = c else { continue };
            let block = &mut uniform_bytes[k * UNIFORM_BLOCK as usize..(k + 1) * UNIFORM_BLOCK as usize];
            if let Some(u) = &dc.uniforms {
                block[..u.len()].copy_from_slice(u);
            }
            // OpenGL's clip space to wgpu's: depth -1..1 becomes 0..1, and render targets are
            // drawn upside down (see `TargetInner::flip`).
            let flipped = dc.target.as_ref().is_some_and(|t| t.inner.flip);
            // An edge exactly through pixel centres: OpenGL (rows counted from the bottom) kept the
            // row above it, wgpu keeps the row below. The UI's half-pixel lines were laid out for
            // OpenGL's choice, so the screen is drawn 1/128 pixel lower. Flipped targets count
            // their rows as OpenGL did and need nothing.
            let nudge = if flipped { 0.0 } else { -2.0 / 128.0 / dc.target.as_ref().map_or(screen.target.texture.0.height, |t| t.texture.0.height) as f32 };
            let fix = Mat4::from_cols(Vec4::new(1., 0., 0., 0.), Vec4::new(0., if flipped { -1.0 } else { 1.0 }, 0., 0.), Vec4::new(0., 0., 0.5, 0.), Vec4::new(0., nudge, 0.5, 1.));
            block[0..64].copy_from_slice(bytemuck::cast_slice(&(fix * dc.projection).to_cols_array()));
            block[64..128].copy_from_slice(bytemuck::cast_slice(&dc.model.to_cols_array()));
            block[128..144].copy_from_slice(bytemuck::cast_slice(&time));
            k += 1;
        }
        if self.indices.len() % 2 == 1 {
            self.indices.push(0);
        }
        let (vertex_len, index_len) = (std::mem::size_of_val(&self.vertices[..]) as u64, std::mem::size_of_val(&self.indices[..]) as u64);
        self.buffers(gpu, vertex_len, index_len, uniform_bytes.len() as u64);
        {
            let b = self.buffers.as_ref().unwrap();
            if vertex_len > 0 {
                gpu.queue.write_buffer(&b.vertices, 0, bytemuck::cast_slice(&self.vertices));
                gpu.queue.write_buffer(&b.indices, 0, bytemuck::cast_slice(&self.indices));
            }
            gpu.queue.write_buffer(&b.uniforms, 0, &uniform_bytes);
        }

        // Pipelines and material bind groups first (they need `&mut self`), then the passes.
        let mut ready: Vec<Option<(wgpu::RenderPipeline, Option<wgpu::BindGroup>)>> = Vec::with_capacity(cmds.len());
        for c in &cmds {
            ready.push(match c {
                Cmd::Draw(dc) => {
                    let target_depth = dc.target.as_ref().map_or(screen.target.inner.depth.is_some(), |t| t.inner.depth.is_some());
                    let pipeline = self.pipeline_for(gpu, &dc.pipe, dc.draw_mode, target_depth);
                    let extra = match &dc.pipe {
                        Pipe::Material(m) => m.bind_textures(gpu, &dc.textures, &self.white_texture),
                        Pipe::Default(_) => None,
                    };
                    Some((pipeline, extra))
                }
                Cmd::Clear { .. } => None,
            });
        }

        let b = self.buffers.take().unwrap();
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gfx frame") });
        let target_of = |c: &Cmd| match c {
            Cmd::Draw(d) => d.target.clone(),
            Cmd::Clear { target, .. } => target.clone(),
        };
        let (mut at, mut passes, mut drawn) = (0, 0, 0u32);
        while at < cmds.len() {
            // A pass: an optional clear, then the draw calls into the same picture that follow.
            let target = target_of(&cmds[at]);
            let mut clear = None;
            while at < cmds.len() {
                match &cmds[at] {
                    Cmd::Clear { target: t, color } if *t == target => {
                        clear = Some(*color);
                        at += 1;
                    }
                    _ => break,
                }
            }
            let start = at;
            while at < cmds.len() && matches!(&cmds[at], Cmd::Draw(d) if d.target == target) {
                at += 1;
            }
            let t = target.as_ref().unwrap_or(&screen.target);
            let (tw, th) = (t.texture.0.width, t.texture.0.height);
            let flipped = t.inner.flip;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gfx pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.texture.0.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: clear.map_or(wgpu::LoadOp::Load, |c| wgpu::LoadOp::Clear(wgpu::Color { r: c.r as f64, g: c.g as f64, b: c.b as f64, a: c.a as f64 })),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: t.inner.depth.as_ref().map(|view| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations { load: if clear.is_some() { wgpu::LoadOp::Clear(1.0) } else { wgpu::LoadOp::Load }, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            passes += 1;
            pass.set_vertex_buffer(0, b.vertices.slice(..));
            pass.set_index_buffer(b.indices.slice(..), wgpu::IndexFormat::Uint16);
            for (c, r) in cmds[start..at].iter().zip(&ready[start..at]) {
                let (Cmd::Draw(dc), Some((pipeline, extra))) = (c, r) else { continue };
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &b.uniform_bind, &[(drawn as u64 * UNIFORM_BLOCK) as u32]);
                pass.set_bind_group(1, dc.texture.as_ref().unwrap_or(&self.white_texture).bind_group(gpu), &[]);
                if let Some(extra) = extra {
                    pass.set_bind_group(2, extra, &[]);
                }
                // Rectangles come in OpenGL's terms: the viewport from the bottom left, the clip
                // from the top left (macroquad turned it over). A flipped target is OpenGL's way up.
                let fit = |(x, y, w, h): (i32, i32, i32, i32), from_bottom: bool| {
                    let y = if from_bottom != flipped { th as i32 - (y + h) } else { y };
                    let (x0, y0) = (x.clamp(0, tw as i32), y.clamp(0, th as i32));
                    let (x1, y1) = ((x + w).clamp(x0, tw as i32), (y + h).clamp(y0, th as i32));
                    (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
                };
                let (vx, vy, vw, vh) = dc.viewport.map_or((0, 0, tw, th), |v| fit(v, true));
                let (sx, sy, sw, sh) = dc.clip.map_or((0, 0, tw, th), |c| fit(c, false));
                drawn += 1;
                if vw == 0 || vh == 0 || sw == 0 || sh == 0 {
                    continue;
                }
                pass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(dc.indices_start as u32..(dc.indices_start + dc.indices_count) as u32, dc.vertices_start as i32, 0..1);
            }
        }
        gpu.queue.submit([encoder.finish()]);
        self.buffers = Some(b);
        self.stats = Stats { draw_calls: draws, passes, vertices: self.vertices.len() };
        self.vertices.clear();
        self.indices.clear();
        // Pipelines of materials that are gone.
        self.pipelines.retain(|_, (m, _)| m.as_ref().is_none_or(|m| m.strong_count() > 0));
    }
}
