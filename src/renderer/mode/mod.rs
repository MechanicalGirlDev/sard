//! Switchable rendering of the same opaque triangle scene.
//!
//! Rasterization uses Cook-Torrance PBR and a directional shadow map. Ray tracing
//! uses GPU BVH traversal, ray shadows and specular reflections. Path tracing
//! progressively samples diffuse and glossy light transport on the same GPU.

mod bvh;
mod raster;
mod trace;

use crate::context::WgpuContext;
use crate::core::{DepthTexture, RenderTarget};
use crate::renderer::geometry::MeshData;
use crate::renderer::{Gm, Mesh, PbrMaterial, Viewer};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

/// The rendering algorithm, independent of scene and camera ownership.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// PBR shading with a directional-light shadow map.
    #[default]
    Rasterization,
    /// Deterministic ray shadows and specular reflection bounces.
    RayTracing,
    /// Progressive Monte Carlo diffuse and glossy global illumination.
    PathTracing,
}

/// Opaque metallic-roughness surface shared by all rendering algorithms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceMaterial {
    /// Linear RGB albedo.
    pub base_color: [f32; 3],
    /// Linear emitted radiance.
    pub emissive: [f32; 3],
    /// Metal fraction in [0, 1].
    pub metallic: f32,
    /// Perceptual roughness in [0, 1].
    pub roughness: f32,
    /// Ambient occlusion in [0, 1].
    pub ao: f32,
}

impl Default for SurfaceMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0; 3],
            emissive: [0.0; 3],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
        }
    }
}

impl From<&PbrMaterial> for SurfaceMaterial {
    fn from(material: &PbrMaterial) -> Self {
        Self {
            base_color: [
                material.base_color[0],
                material.base_color[1],
                material.base_color[2],
            ],
            emissive: material.emissive,
            metallic: material.metallic,
            roughness: material.roughness,
            ao: material.ao,
        }
    }
}

/// A borrowed opaque triangle mesh, world transform, and surface.
///
/// Mesh data is shared with Sard's retained-scene API. Textures, transparency,
/// lines and billboards are not part of this triangle-surface contract.
pub struct RenderObject<'a> {
    pub mesh: &'a MeshData,
    pub transform: Mat4,
    pub material: SurfaceMaterial,
}

impl Gm<Mesh, PbrMaterial> {
    /// Borrow this object's current geometry, transform and PBR parameters.
    pub fn render_object(&self) -> RenderObject<'_> {
        RenderObject {
            mesh: self.geometry.mesh_data(),
            transform: self.transform,
            material: SurfaceMaterial::from(&self.material),
        }
    }
}

/// Lighting shared by rasterization, ray tracing, and path tracing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderLighting {
    /// Direction in which directional-light photons travel (need not be unit).
    pub sun_direction: Vec3,
    /// Linear directional-light radiance, including intensity.
    pub sun_color: [f32; 3],
    /// Constant linear environment radiance and background.
    pub environment: [f32; 3],
}

impl Default for RenderLighting {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(-1.0, -2.0, -1.0),
            sun_color: [3.0; 3],
            environment: [0.05; 3],
        }
    }
}

/// Renderer resource and sampling configuration.
#[derive(Debug, Clone, Copy)]
pub struct RenderConfig {
    pub mode: RenderMode,
    /// Maximum traced surface bounces, from 1 to 64.
    pub max_bounces: u32,
    /// Square directional shadow map size.
    pub shadow_resolution: u32,
    /// Reproducible path-sampling seed.
    pub seed: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            mode: RenderMode::Rasterization,
            max_bounces: 6,
            shadow_resolution: 2048,
            seed: 0,
        }
    }
}

/// Errors at the scene, configuration, or render-target boundary.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("invalid render configuration: {0}")]
    Configuration(&'static str),
    #[error("object {object}: {reason}")]
    Scene { object: usize, reason: &'static str },
    #[error("render resources exceed device limits")]
    DeviceLimit,
    #[error("render target format does not match the renderer")]
    TargetFormat,
    #[error("camera matrices must be finite and invertible")]
    Camera,
    #[error(transparent)]
    Pipeline(#[from] anyhow::Error),
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GpuTriangle {
    positions: [[f32; 4]; 3],
    normals: [[f32; 4]; 3],
    base_color: [f32; 4],
    emissive_metallic: [f32; 4],
    parameters: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Frame {
    view_proj: [[f32; 4]; 4],
    inverse_view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    sun_direction: [f32; 4],
    sun_color: [f32; 4],
    environment: [f32; 4],
    settings: [u32; 4],
    counts: [u32; 4],
}

const _: () = assert!(size_of::<GpuTriangle>() == 144);
const _: () = assert!(size_of::<Frame>() == 288);

/// Owns the algorithm backends, but not the application's camera or scene.
///
/// Upload a scene with [`Self::set_scene`], then call [`Self::render`] each frame.
/// Mode changes keep the uploaded scene. Path tracing adds one sample per call;
/// scene, lighting, camera, size, bounce-count, and mode changes reset accumulation.
pub struct Renderer {
    ctx: WgpuContext,
    format: wgpu::TextureFormat,
    config: RenderConfig,
    lighting: RenderLighting,
    triangles: wgpu::Buffer,
    nodes: wgpu::Buffer,
    triangle_count: u32,
    node_count: u32,
    bounds: Option<crate::renderer::Aabb>,
    frame_buffer: wgpu::Buffer,
    raster: raster::RasterBackend,
    trace: trace::TraceBackend,
    depth: Option<DepthTexture>,
    size: [u32; 2],
    samples: u32,
    last_camera: Option<[u32; 20]>,
}

impl Renderer {
    pub fn new(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        config: RenderConfig,
    ) -> Result<Self, RenderError> {
        if !(1..=64).contains(&config.max_bounces) {
            return Err(RenderError::Configuration("max_bounces must be in 1..=64"));
        }
        if config.shadow_resolution == 0
            || config.shadow_resolution > ctx.device.limits().max_texture_dimension_2d
        {
            return Err(RenderError::Configuration("invalid shadow_resolution"));
        }
        let frame_buffer = buffer(
            ctx,
            "render frame",
            &[0; size_of::<Frame>()],
            wgpu::BufferUsages::UNIFORM,
        );
        let triangles = buffer(
            ctx,
            "render triangles",
            &[0; size_of::<GpuTriangle>()],
            wgpu::BufferUsages::STORAGE,
        );
        let nodes = buffer(
            ctx,
            "render BVH",
            &[0; size_of::<bvh::GpuNode>()],
            wgpu::BufferUsages::STORAGE,
        );
        Ok(Self {
            ctx: ctx.clone(),
            format,
            config,
            lighting: RenderLighting::default(),
            raster: raster::RasterBackend::new(ctx, format, config.shadow_resolution)?,
            trace: trace::TraceBackend::new(ctx, format)?,
            triangles,
            nodes,
            triangle_count: 0,
            node_count: 0,
            bounds: None,
            frame_buffer,
            depth: None,
            size: [0; 2],
            samples: 0,
            last_camera: None,
        })
    }

    pub const fn mode(&self) -> RenderMode {
        self.config.mode
    }

    pub fn set_mode(&mut self, mode: RenderMode) {
        if self.config.mode != mode {
            self.config.mode = mode;
            self.reset_accumulation();
        }
    }

    pub fn set_max_bounces(&mut self, max_bounces: u32) -> Result<(), RenderError> {
        if !(1..=64).contains(&max_bounces) {
            return Err(RenderError::Configuration("max_bounces must be in 1..=64"));
        }
        if self.config.max_bounces != max_bounces {
            self.config.max_bounces = max_bounces;
            self.reset_accumulation();
        }
        Ok(())
    }

    pub fn set_lighting(&mut self, lighting: RenderLighting) -> Result<(), RenderError> {
        let direction_length = lighting.sun_direction.length_squared();
        if !lighting.sun_direction.is_finite()
            || !direction_length.is_finite()
            || direction_length < 1e-12
            || !valid_color(lighting.sun_color)
            || !valid_color(lighting.environment)
        {
            return Err(RenderError::Configuration("invalid lighting"));
        }
        if self.lighting != lighting {
            self.lighting = lighting;
            self.reset_accumulation();
        }
        Ok(())
    }

    /// Upload a scene snapshot. Call again after geometry, transforms or materials change.
    /// Failed uploads leave the previously uploaded scene and accumulation intact.
    pub fn set_scene(&mut self, objects: &[RenderObject<'_>]) -> Result<(), RenderError> {
        let mut triangles = scene_triangles(objects)?;
        let triangle_count =
            u32::try_from(triangles.len()).map_err(|_| RenderError::DeviceLimit)?;
        let limit = self.ctx.device.limits().max_storage_buffer_binding_size;
        // A binary BVH has at most two nodes per triangle.
        if triangle_count > u32::MAX / 3
            || size_of_val(triangles.as_slice()) as u64 > limit
            || u64::from(triangle_count) * 2 * size_of::<bvh::GpuNode>() as u64 > limit
        {
            return Err(RenderError::DeviceLimit);
        }
        let bounds = (!triangles.is_empty()).then(|| {
            crate::renderer::Aabb::from_points(
                triangles
                    .iter()
                    .flat_map(|triangle| triangle.positions)
                    .map(|p| Vec3::new(p[0], p[1], p[2])),
            )
        });
        let nodes = bvh::build(&mut triangles);
        self.triangles = buffer(
            &self.ctx,
            "render triangles",
            if triangles.is_empty() {
                &[0; size_of::<GpuTriangle>()]
            } else {
                bytemuck::cast_slice(&triangles)
            },
            wgpu::BufferUsages::STORAGE,
        );
        self.nodes = buffer(
            &self.ctx,
            "render BVH",
            if nodes.is_empty() {
                &[0; size_of::<bvh::GpuNode>()]
            } else {
                bytemuck::cast_slice(&nodes)
            },
            wgpu::BufferUsages::STORAGE,
        );
        self.triangle_count = triangle_count;
        self.node_count = u32::try_from(nodes.len()).map_err(|_| RenderError::DeviceLimit)?;
        self.bounds = bounds;
        self.reset_accumulation();
        Ok(())
    }

    /// Number of accumulated path-tracing samples (zero in other modes).
    pub const fn accumulated_samples(&self) -> u32 {
        self.samples
    }

    pub const fn reset_accumulation(&mut self) {
        self.samples = 0;
    }

    /// Render and submit one frame into a window or offscreen target.
    /// Zero-sized targets are a no-op, allowing minimized windows.
    pub fn render(
        &mut self,
        viewer: &dyn Viewer,
        target: &RenderTarget<'_>,
    ) -> Result<(), RenderError> {
        if target.format() != self.format {
            return Err(RenderError::TargetFormat);
        }
        let size = [target.width(), target.height()];
        if size.contains(&0) {
            return Ok(());
        }
        let limits = self.ctx.device.limits();
        if size.iter().any(|&n| n > limits.max_texture_dimension_2d)
            || u64::from(size[0]) * u64::from(size[1]) * 16 > limits.max_storage_buffer_binding_size
        {
            return Err(RenderError::DeviceLimit);
        }
        let view_proj = viewer.view_projection_matrix();
        let inverse = view_proj.inverse();
        let eye = viewer.position();
        if !view_proj.is_finite() || !inverse.is_finite() || !eye.is_finite() {
            return Err(RenderError::Camera);
        }
        let mut camera = [0; 20];
        camera[..16].copy_from_slice(&view_proj.to_cols_array().map(f32::to_bits));
        camera[16..].copy_from_slice(&eye.extend(1.0).to_array().map(f32::to_bits));
        if self.last_camera != Some(camera) {
            self.reset_accumulation();
            self.last_camera = Some(camera);
        }
        if self.size != size {
            self.depth = Some(DepthTexture::new(
                &self.ctx,
                size[0],
                size[1],
                Some("mode depth"),
            ));
            self.trace.resize(&self.ctx, size);
            self.size = size;
            self.reset_accumulation();
        }
        let mode = match self.config.mode {
            RenderMode::Rasterization => 0,
            RenderMode::RayTracing => 1,
            RenderMode::PathTracing => 2,
        };
        let frame = Frame {
            view_proj: view_proj.to_cols_array_2d(),
            inverse_view_proj: inverse.to_cols_array_2d(),
            light_view_proj: light_matrix(self.bounds, self.lighting.sun_direction)
                .to_cols_array_2d(),
            eye: eye.extend(1.0).to_array(),
            sun_direction: self
                .lighting
                .sun_direction
                .normalize()
                .extend(0.0)
                .to_array(),
            sun_color: Vec3::from_array(self.lighting.sun_color)
                .extend(0.0)
                .to_array(),
            environment: Vec3::from_array(self.lighting.environment)
                .extend(0.0)
                .to_array(),
            settings: [size[0], size[1], self.samples, self.config.max_bounces],
            counts: [mode, self.triangle_count, self.node_count, self.config.seed],
        };
        self.ctx
            .queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&frame));
        let mut encoder = self.ctx.create_encoder(Some("render mode"));
        let depth = target
            .depth_view()
            .or_else(|| self.depth.as_ref().map(DepthTexture::view));
        match self.config.mode {
            RenderMode::Rasterization => {
                self.raster.render(
                    &self.ctx,
                    &mut encoder,
                    &self.frame_buffer,
                    &self.triangles,
                    self.triangle_count,
                    target,
                    depth,
                    self.lighting.environment,
                );
            }
            RenderMode::RayTracing | RenderMode::PathTracing => {
                self.trace.render(
                    &self.ctx,
                    &mut encoder,
                    &self.frame_buffer,
                    &self.triangles,
                    &self.nodes,
                    target,
                    depth,
                    size,
                );
            }
        }
        self.ctx.submit([encoder.finish()]);
        if self.config.mode == RenderMode::PathTracing {
            self.samples = self.samples.saturating_add(1);
        }
        Ok(())
    }
}

fn buffer(ctx: &WgpuContext, label: &str, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    ctx.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytes,
            usage: usage | wgpu::BufferUsages::COPY_DST,
        })
}

fn valid_color(color: [f32; 3]) -> bool {
    color.iter().all(|n| n.is_finite() && *n >= 0.0)
}

fn scene_triangles(objects: &[RenderObject<'_>]) -> Result<Vec<GpuTriangle>, RenderError> {
    let mut triangles = Vec::new();
    for (object, item) in objects.iter().enumerate() {
        let invalid = |reason| RenderError::Scene { object, reason };
        let surface = item.material;
        if !valid_color(surface.base_color)
            || !valid_color(surface.emissive)
            || [surface.metallic, surface.roughness, surface.ao]
                .iter()
                .any(|n| !n.is_finite() || !(0.0..=1.0).contains(n))
        {
            return Err(invalid("invalid surface parameters"));
        }
        let normal_matrix = item.transform.inverse().transpose();
        if !item.transform.is_finite()
            || !normal_matrix.is_finite()
            || !item.transform.row(3).abs_diff_eq(glam::Vec4::W, 0.0)
        {
            return Err(invalid("transform must be finite, affine and invertible"));
        }
        let mesh = item.mesh;
        if mesh.positions.len() != mesh.normals.len() || mesh.indices.len() % 3 != 0 {
            return Err(invalid(
                "expected aligned normals and triangle-list indices",
            ));
        }
        for indices in mesh.indices.chunks_exact(3) {
            let mut positions = [[0.0; 4]; 3];
            let mut normals = [[0.0; 4]; 3];
            for (corner, &index) in indices.iter().enumerate() {
                let position = mesh
                    .positions
                    .get(index as usize)
                    .ok_or_else(|| invalid("vertex index out of range"))?;
                let normal = mesh
                    .normals
                    .get(index as usize)
                    .ok_or_else(|| invalid("normal index out of range"))?;
                let position = item.transform.transform_point3(*position);
                let normal = normal_matrix.transform_vector3(*normal).normalize_or_zero();
                if !position.is_finite() || !normal.is_finite() || normal.length_squared() < 0.5 {
                    return Err(invalid("non-finite positions or invalid normals"));
                }
                positions[corner] = position.extend(1.0).to_array();
                normals[corner] = normal.extend(0.0).to_array();
            }
            triangles.push(GpuTriangle {
                positions,
                normals,
                base_color: Vec3::from_array(surface.base_color).extend(1.0).to_array(),
                emissive_metallic: Vec3::from_array(surface.emissive)
                    .extend(surface.metallic)
                    .to_array(),
                parameters: [surface.roughness.max(0.045), surface.ao, 0.0, 0.0],
            });
        }
    }
    Ok(triangles)
}

fn light_matrix(bounds: Option<crate::renderer::Aabb>, direction: Vec3) -> Mat4 {
    let (center, radius) = bounds.map_or((Vec3::ZERO, 1.0), |b| {
        (b.center(), (b.size().length() * 0.5).max(0.1))
    });
    let direction = direction.normalize();
    let up = if direction.dot(Vec3::Y).abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = glam::camera::rh::view::look_at_mat4(center - direction * radius * 2.0, center, up);
    let projection = glam::camera::rh::proj::directx::orthographic(
        -radius,
        radius,
        -radius,
        radius,
        radius * 0.5,
        radius * 3.5,
    );
    projection * view
}

fn buffer_layout(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BufferBindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn depth_attachment(view: &wgpu::TextureView) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    }
}
