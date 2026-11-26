//! The wgpu device and the few objects everything shares.

use std::sync::Arc;

use crate::Error;

/// Every colour picture this crate draws into or samples from.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Depth buffers of render targets and screens.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Gpu {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub nearest: wgpu::Sampler,
    pub linear: wgpu::Sampler,
    /// Group 1 of every pipeline: the draw call's texture and its sampler.
    pub texture_layout: wgpu::BindGroupLayout,
    /// Group 0 of every pipeline: the uniform block, at a moving offset.
    pub uniform_layout: wgpu::BindGroupLayout,
}

impl Gpu {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Gpu {
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("gfx sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let nearest = sampler(wgpu::FilterMode::Nearest);
        let linear = sampler(wgpu::FilterMode::Linear);
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("gfx texture"), entries: &texture_entries(1) });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gfx uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: None },
                count: None,
            }],
        });
        Gpu { device, queue, nearest, linear, texture_layout, uniform_layout }
    }
}

/// Layout entries of `n` textures, each followed by its sampler (bindings 2k and 2k + 1).
pub fn texture_entries(n: usize) -> Vec<wgpu::BindGroupLayoutEntry> {
    let mut v = Vec::with_capacity(n * 2);
    for k in 0..n as u32 {
        v.push(wgpu::BindGroupLayoutEntry {
            binding: k * 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        });
        v.push(wgpu::BindGroupLayoutEntry { binding: k * 2 + 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None });
    }
    v
}

/// May a software renderer (llvmpipe, lavapipe, WARP) be used? Players get a message instead; tests
/// and CI set `RUSTPG2D_ALLOW_SOFTWARE=1`.
pub fn software_allowed() -> bool {
    std::env::var("RUSTPG2D_ALLOW_SOFTWARE").is_ok_and(|v| v != "0" && !v.is_empty())
}

/// What the device must be able to do. Kept to what WebGL2 offers, so every backend qualifies.
pub fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        label: Some("gfx device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    }
}

/// Picks the adapter for `surface` (or any, without one). A software adapter is an error unless
/// [`software_allowed`].
pub async fn adapter(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<wgpu::Adapter, Error> {
    let real = instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false, compatible_surface: surface, apply_limit_buckets: false }).await;
    let is_cpu = |a: &wgpu::Adapter| a.get_info().device_type == wgpu::DeviceType::Cpu;
    match real {
        Ok(a) if !is_cpu(&a) || software_allowed() => Ok(a),
        Ok(a) => Err(Error::Gpu(format!("only a software renderer was found ({}); a graphics driver is needed", a.get_info().name))),
        Err(e) if software_allowed() => {
            instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::None, force_fallback_adapter: true, compatible_surface: surface, apply_limit_buckets: false }).await.map_err(|e2| Error::Gpu(format!("{e}; {e2}")))
        }
        Err(e) => Err(Error::Gpu(e.to_string())),
    }
}

/// A device without a window, for drawing off screen.
#[cfg(not(target_arch = "wasm32"))]
pub fn headless_device() -> Result<(wgpu::Device, wgpu::Queue), Error> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    pollster::block_on(async {
        let adapter = adapter(&instance, None).await?;
        adapter.request_device(&device_descriptor(&adapter)).await.map_err(|e| Error::Gpu(e.to_string()))
    })
}

#[cfg(target_arch = "wasm32")]
pub fn headless_device() -> Result<(wgpu::Device, wgpu::Queue), Error> {
    Err(Error::Gpu("no drawing without a canvas in the browser".into()))
}
