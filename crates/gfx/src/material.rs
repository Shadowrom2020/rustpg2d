//! Materials: a shader of your own with its uniforms, textures and blending.
//!
//! Shaders are WGSL. [`load_material`] writes the declarations in front of the source, so a
//! shader only holds its vertex output struct and the two entry points `vs_main` and `fs_main`:
//!
//! ```wgsl
//! struct Uniforms { Projection: mat4x4<f32>, Model: mat4x4<f32>, _Time: vec4<f32>, /* yours, in order */ };
//! @group(0) @binding(0) var<uniform> u: Uniforms;
//! @group(1) @binding(0) var Texture: texture_2d<f32>;      // the draw call's texture
//! @group(1) @binding(1) var TextureSampler: sampler;
//! @group(2) @binding(0) var Name: texture_2d<f32>;         // each of `MaterialParams::textures`
//! @group(2) @binding(1) var NameSampler: sampler;
//! struct VertexInput { position: vec3<f32>, texcoord: vec2<f32>, color0: vec4<f32>, normal: vec4<f32> };
//! ```
//!
//! `color0` is 0-1. Clip space is OpenGL's (y up, depth -1..1), as the projections of this crate
//! make it: the batch converts.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use crate::batch::UNIFORM_HEADER;
pub use crate::batch::DEFAULT_SHADER;
use crate::gpu::Gpu;
use crate::texture::Texture2D;
use crate::{get_context, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Equation {
    Add,
    Subtract,
    ReverseSubtract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendValue {
    SourceColor,
    SourceAlpha,
    DestinationColor,
    DestinationAlpha,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendFactor {
    Zero,
    One,
    Value(BlendValue),
    OneMinusValue(BlendValue),
    SourceAlphaSaturate,
}

/// result = source × `sfactor` (equation) destination × `dfactor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendState {
    equation: Equation,
    sfactor: BlendFactor,
    dfactor: BlendFactor,
}

impl BlendState {
    pub const fn new(equation: Equation, sfactor: BlendFactor, dfactor: BlendFactor) -> BlendState {
        BlendState { equation, sfactor, dfactor }
    }

    pub(crate) fn component(&self) -> wgpu::BlendComponent {
        let factor = |f: BlendFactor| match f {
            BlendFactor::Zero => wgpu::BlendFactor::Zero,
            BlendFactor::One => wgpu::BlendFactor::One,
            BlendFactor::Value(BlendValue::SourceColor) => wgpu::BlendFactor::Src,
            BlendFactor::Value(BlendValue::SourceAlpha) => wgpu::BlendFactor::SrcAlpha,
            BlendFactor::Value(BlendValue::DestinationColor) => wgpu::BlendFactor::Dst,
            BlendFactor::Value(BlendValue::DestinationAlpha) => wgpu::BlendFactor::DstAlpha,
            BlendFactor::OneMinusValue(BlendValue::SourceColor) => wgpu::BlendFactor::OneMinusSrc,
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha) => wgpu::BlendFactor::OneMinusSrcAlpha,
            BlendFactor::OneMinusValue(BlendValue::DestinationColor) => wgpu::BlendFactor::OneMinusDst,
            BlendFactor::OneMinusValue(BlendValue::DestinationAlpha) => wgpu::BlendFactor::OneMinusDstAlpha,
            BlendFactor::SourceAlphaSaturate => wgpu::BlendFactor::SrcAlphaSaturated,
        };
        wgpu::BlendComponent {
            src_factor: factor(self.sfactor),
            dst_factor: factor(self.dfactor),
            operation: match self.equation {
                Equation::Add => wgpu::BlendOperation::Add,
                Equation::Subtract => wgpu::BlendOperation::Subtract,
                Equation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Comparison {
    Never,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
    #[default]
    Always,
}

impl From<Comparison> for wgpu::CompareFunction {
    fn from(c: Comparison) -> Self {
        match c {
            Comparison::Never => wgpu::CompareFunction::Never,
            Comparison::Less => wgpu::CompareFunction::Less,
            Comparison::LessOrEqual => wgpu::CompareFunction::LessEqual,
            Comparison::Greater => wgpu::CompareFunction::Greater,
            Comparison::GreaterOrEqual => wgpu::CompareFunction::GreaterEqual,
            Comparison::Equal => wgpu::CompareFunction::Equal,
            Comparison::NotEqual => wgpu::CompareFunction::NotEqual,
            Comparison::Always => wgpu::CompareFunction::Always,
        }
    }
}

/// How a material's triangles reach the picture. The default: no depth test, no blending (the
/// source replaces what is there), every channel written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipelineParams {
    pub depth_test: Comparison,
    pub depth_write: bool,
    /// `None`: no blending.
    pub color_blend: Option<BlendState>,
    /// `None`: as `color_blend`.
    pub alpha_blend: Option<BlendState>,
    /// Which of red, green, blue, alpha are written.
    pub color_write: (bool, bool, bool, bool),
}

impl Default for PipelineParams {
    fn default() -> PipelineParams {
        PipelineParams { depth_test: Comparison::Always, depth_write: false, color_blend: None, alpha_blend: None, color_write: (true, true, true, true) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformType {
    Float1,
    Float2,
    Float3,
    Float4,
    Int1,
    Int2,
    Int3,
    Int4,
    Mat4,
}

impl UniformType {
    /// Bytes of a value.
    pub fn size(self) -> usize {
        match self {
            UniformType::Float1 | UniformType::Int1 => 4,
            UniformType::Float2 | UniformType::Int2 => 8,
            UniformType::Float3 | UniformType::Int3 => 12,
            UniformType::Float4 | UniformType::Int4 => 16,
            UniformType::Mat4 => 64,
        }
    }

    /// WGSL's alignment of the type in a uniform struct.
    fn align(self) -> usize {
        match self {
            UniformType::Float1 | UniformType::Int1 => 4,
            UniformType::Float2 | UniformType::Int2 => 8,
            _ => 16,
        }
    }

    fn wgsl(self) -> &'static str {
        match self {
            UniformType::Float1 => "f32",
            UniformType::Float2 => "vec2<f32>",
            UniformType::Float3 => "vec3<f32>",
            UniformType::Float4 => "vec4<f32>",
            UniformType::Int1 => "i32",
            UniformType::Int2 => "vec2<i32>",
            UniformType::Int3 => "vec3<i32>",
            UniformType::Int4 => "vec4<i32>",
            UniformType::Mat4 => "mat4x4<f32>",
        }
    }
}

#[derive(Clone, Debug)]
pub struct UniformDesc {
    pub name: String,
    pub uniform_type: UniformType,
}

impl UniformDesc {
    pub fn new(name: &str, uniform_type: UniformType) -> UniformDesc {
        UniformDesc { name: name.to_string(), uniform_type }
    }
}

/// A shader's text.
pub enum ShaderSource<'a> {
    /// The vertex output struct, `vs_main` and `fs_main` (see the module's description).
    Wgsl { source: &'a str },
}

#[derive(Default)]
pub struct MaterialParams {
    pub pipeline_params: PipelineParams,
    pub uniforms: Vec<UniformDesc>,
    /// Names of the material's own textures (set with [`Material::set_texture`]).
    pub textures: Vec<String>,
}

/// The whole WGSL of a material: the declarations, then `body`.
pub(crate) fn shader_source(uniforms: &[UniformDesc], textures: &[String], body: &str) -> String {
    let mut s = String::from("struct Uniforms {\n    Projection: mat4x4<f32>,\n    Model: mat4x4<f32>,\n    _Time: vec4<f32>,\n");
    for u in uniforms {
        s.push_str(&format!("    {}: {},\n", u.name, u.uniform_type.wgsl()));
    }
    s.push_str("};\n@group(0) @binding(0) var<uniform> u: Uniforms;\n@group(1) @binding(0) var Texture: texture_2d<f32>;\n@group(1) @binding(1) var TextureSampler: sampler;\n");
    for (k, t) in textures.iter().enumerate() {
        s.push_str(&format!("@group(2) @binding({}) var {t}: texture_2d<f32>;\n@group(2) @binding({}) var {t}Sampler: sampler;\n", k * 2, k * 2 + 1));
    }
    s.push_str("struct VertexInput {\n    @location(0) position: vec3<f32>,\n    @location(1) texcoord: vec2<f32>,\n    @location(2) color0: vec4<f32>,\n    @location(3) normal: vec4<f32>,\n};\n");
    s.push_str(body);
    s
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Slot {
    name: String,
    kind: UniformType,
    offset: usize,
}

struct Values {
    /// The uniform block: the header's place, then this material's values at their offsets.
    block: Vec<u8>,
    textures: Vec<Option<Texture2D>>,
    /// The bind group of the textures last drawn with (ids and filters).
    bound: Option<(Vec<(u64, u8)>, wgpu::BindGroup)>,
}

pub(crate) struct MaterialInner {
    id: u64,
    module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    textures_layout: Option<wgpu::BindGroupLayout>,
    params: PipelineParams,
    slots: Vec<Slot>,
    texture_names: Vec<String>,
    values: Mutex<Values>,
}

/// A loaded material. Clones share its uniforms and textures.
#[derive(Clone)]
pub struct Material(Arc<MaterialInner>);

impl PartialEq for Material {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Material {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Material(#{})", self.0.id)
    }
}

impl Material {
    /// Sets a uniform declared in [`MaterialParams::uniforms`]: an `f32`, `Vec2`, `Vec3`, `Vec4`,
    /// `Mat4` or the like, of exactly the declared size.
    pub fn set_uniform<T: bytemuck::NoUninit>(&self, name: &str, uniform: T) {
        let Some(slot) = self.0.slots.iter().find(|s| s.name == name) else {
            eprintln!("[gfx] the material has no uniform {name}");
            return;
        };
        let bytes = bytemuck::bytes_of(&uniform);
        if bytes.len() != slot.kind.size() {
            eprintln!("[gfx] uniform {name} takes {} bytes, got {}", slot.kind.size(), bytes.len());
            return;
        }
        let mut v = self.0.values.lock().unwrap_or_else(|e| e.into_inner());
        if v.block[slot.offset..slot.offset + bytes.len()] != *bytes {
            v.block[slot.offset..slot.offset + bytes.len()].copy_from_slice(bytes);
            drop(v);
            get_context().gl.material_changed();
        }
    }

    /// Sets one of the textures declared in [`MaterialParams::textures`].
    pub fn set_texture(&self, name: &str, texture: Texture2D) {
        let Some(k) = self.0.texture_names.iter().position(|n| n == name) else {
            eprintln!("[gfx] the material has no texture {name} (it has {:?})", self.0.texture_names);
            return;
        };
        let mut v = self.0.values.lock().unwrap_or_else(|e| e.into_inner());
        if v.textures[k].as_ref() != Some(&texture) {
            v.textures[k] = Some(texture);
            drop(v);
            get_context().gl.material_changed();
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.0.id
    }

    pub(crate) fn module(&self) -> &wgpu::ShaderModule {
        &self.0.module
    }

    pub(crate) fn layout(&self) -> &wgpu::PipelineLayout {
        &self.0.layout
    }

    pub(crate) fn params(&self) -> PipelineParams {
        self.0.params
    }

    pub(crate) fn downgrade(&self) -> Weak<MaterialInner> {
        Arc::downgrade(&self.0)
    }

    pub(crate) fn uniform_block(&self) -> Vec<u8> {
        self.0.values.lock().unwrap_or_else(|e| e.into_inner()).block.clone()
    }

    /// The textures as set now (unset ones are filled in when drawn).
    pub(crate) fn textures(&self) -> Vec<Texture2D> {
        let v = self.0.values.lock().unwrap_or_else(|e| e.into_inner());
        let white = &get_context().gl.white_texture;
        v.textures.iter().map(|t| t.clone().unwrap_or_else(|| white.clone())).collect()
    }

    /// The bind group of a draw call's material textures (`None` for a material without any).
    pub(crate) fn bind_textures(&self, gpu: &Gpu, textures: &[Texture2D], white: &Texture2D) -> Option<wgpu::BindGroup> {
        let layout = self.0.textures_layout.as_ref()?;
        let key: Vec<(u64, u8)> = textures.iter().map(|t| (t.0.id, t.filter() as u8)).collect();
        let mut v = self.0.values.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, group)) = &v.bound {
            if *k == key {
                return Some(group.clone());
            }
        }
        let mut entries = Vec::with_capacity(textures.len() * 2);
        for k in 0..self.0.texture_names.len() {
            let t = textures.get(k).unwrap_or(white);
            entries.push(wgpu::BindGroupEntry { binding: k as u32 * 2, resource: wgpu::BindingResource::TextureView(&t.0.view) });
            entries.push(wgpu::BindGroupEntry { binding: k as u32 * 2 + 1, resource: wgpu::BindingResource::Sampler(if t.filter() == crate::texture::FilterMode::Nearest { &gpu.nearest } else { &gpu.linear }) });
        }
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("gfx material textures"), layout, entries: &entries });
        v.bound = Some((key, group.clone()));
        Some(group)
    }
}

/// Compiles a material. Fails when the shader doesn't validate (the error names the line of the
/// whole source, declarations included).
pub fn load_material(shader: ShaderSource, params: MaterialParams) -> Result<Material, Error> {
    let gpu = &get_context().gpu;
    let ShaderSource::Wgsl { source } = shader;
    let text = shader_source(&params.uniforms, &params.textures, source);

    // Where each uniform sits, by WGSL's layout rules.
    let mut offset = UNIFORM_HEADER;
    let mut slots = Vec::with_capacity(params.uniforms.len());
    for u in &params.uniforms {
        offset = offset.next_multiple_of(u.uniform_type.align());
        slots.push(Slot { name: u.name.clone(), kind: u.uniform_type, offset });
        offset += u.uniform_type.size();
    }
    let size = offset.next_multiple_of(16);
    if size as u64 > crate::batch::UNIFORM_BLOCK {
        return Err(Error::Shader(format!("the uniforms take {size} bytes, more than a block's {}", crate::batch::UNIFORM_BLOCK)));
    }

    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("gfx material"), source: wgpu::ShaderSource::Wgsl(text.into()) });
    if let Some(e) = crate::window::block_on(scope.pop()) {
        return Err(Error::Shader(e.to_string()));
    }
    let textures_layout = (!params.textures.is_empty())
        .then(|| gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("gfx material textures"), entries: &crate::gpu::texture_entries(params.textures.len()) }));
    let mut groups = vec![Some(&gpu.uniform_layout), Some(&gpu.texture_layout)];
    if let Some(l) = &textures_layout {
        groups.push(Some(l));
    }
    let layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("gfx material"), bind_group_layouts: &groups, ..Default::default() });
    Ok(Material(Arc::new(MaterialInner {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        module,
        layout,
        textures_layout,
        params: params.pipeline_params,
        slots,
        values: Mutex::new(Values { block: vec![0; size], textures: vec![None; params.textures.len()], bound: None }),
        texture_names: params.textures,
    })))
}

/// What is drawn from here on goes through `material`.
pub fn gl_use_material(material: &Material) {
    get_context().gl.pipeline(Some(material.clone()));
}

/// Back to plain drawing.
pub fn gl_use_default_material() {
    get_context().gl.pipeline(None);
}
