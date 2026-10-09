//! Offscreen robot cameras. Camera local axes are -Z forward and +Y up.
//!
//! Each call borrows an explicit scene, so cameras and scenes have no shared
//! registration state. Geometry must be non-instanced triangle lists with a
//! position at byte offset zero (the engine's standard vertex layouts).
//! RGB uses an opaque linear base color and optional UV-mapped sRGB texture.
//! This sensor pass does not evaluate scene lighting or shadows.

use super::geometry::Geometry;
use crate::context::WgpuContext;
use crate::core::Texture2D;
use glam::Mat4;
use std::collections::HashMap;
use std::time::Duration;
use wgpu::util::DeviceExt;

/// Independently requested channels. Unrequested channels are `None`.
#[derive(Debug, Clone, Copy)]
pub struct SensorChannels {
    pub rgb: bool,
    pub depth: bool,
    pub segmentation: bool,
}

impl Default for SensorChannels {
    fn default() -> Self {
        Self {
            rgb: true,
            depth: false,
            segmentation: false,
        }
    }
}

/// Top-to-bottom row-major sensor data, matching Nexus's channel conventions.
#[derive(Debug)]
pub struct SensorFrame {
    pub width: u32,
    pub height: u32,
    /// Packed RGB, three bytes per pixel.
    pub rgb: Option<Vec<u8>>,
    /// Axial distance in meters, not Euclidean ray distance. Background is zero.
    pub depth: Option<Vec<f32>>,
    /// Exact caller-defined body IDs. Zero is reserved for background.
    pub segmentation: Option<Vec<u32>>,
}

/// Borrowed sensor geometry. Multiple visuals of one body should share an ID.
pub struct SensorObject<'a> {
    pub geometry: &'a dyn Geometry,
    pub transform: Mat4,
    pub color: [f32; 3],
    pub segmentation_id: u32,
    /// Optional image texture. Requires the standard `VertexPNUC` UV layout.
    pub texture: Option<&'a Texture2D>,
}

/// Perspective parameters. Vertical FOV is in degrees, distances in meters.
#[derive(Debug, Clone, Copy)]
pub struct SensorCameraConfig {
    pub width: u32,
    pub height: u32,
    pub fov_y_deg: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Default for SensorCameraConfig {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            fov_y_deg: 60.0,
            znear: 0.01,
            zfar: 100.0,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SensorError {
    #[error("invalid camera size, field of view or clipping planes")]
    InvalidCamera,
    #[error("camera and body poses must be finite rigid transforms")]
    InvalidPose,
    #[error("invalid object transform, base color, geometry or zero segmentation ID")]
    InvalidObject,
    #[error("unsupported vertex stride {0}; expected an engine position-first vertex layout")]
    UnsupportedVertexStride(u64),
    #[error("GPU readback failed: {0}")]
    Readback(String),
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    eye: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SensorModel {
    transform: [[f32; 4]; 4],
    color: [f32; 4],
    segmentation_id: u32,
    textured: u32,
    billboard: u32,
    padding: [u32; 1],
}

/// Owns its GPU targets and camera pose. Select a camera by borrowing that
/// instance; dropping it releases its targets. No physics owner is hidden here.
pub struct SensorCamera {
    ctx: WgpuContext,
    config: SensorCameraConfig,
    pose: Mat4,
    background: [f32; 3],
    camera_layout: wgpu::BindGroupLayout,
    model_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    white_texture: Texture2D,
    pipelines: HashMap<u64, wgpu::RenderPipeline>,
    targets: [wgpu::Texture; 3],
    zbuffer: wgpu::Texture,
}

impl SensorCamera {
    pub fn new(ctx: &WgpuContext, config: SensorCameraConfig) -> Result<Self, SensorError> {
        validate_config(ctx, config)?;
        let (targets, zbuffer) = create_targets(ctx, config);
        Ok(Self {
            ctx: ctx.clone(),
            config,
            pose: Mat4::IDENTITY,
            background: [0.0; 3],
            camera_layout: uniform_layout(ctx),
            model_layout: uniform_layout(ctx),
            texture_layout: ctx
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("sensor image layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                }),
            white_texture: Texture2D::from_rgba8(ctx, 1, 1, &[255; 4], Some("sensor white")),
            pipelines: HashMap::new(),
            targets,
            zbuffer,
        })
    }

    pub fn config(&self) -> SensorCameraConfig {
        self.config
    }
    pub fn pose(&self) -> Mat4 {
        self.pose
    }

    pub fn set_pose(&mut self, world_from_camera: Mat4) -> Result<(), SensorError> {
        if !rigid_pose(world_from_camera) {
            return Err(SensorError::InvalidPose);
        }
        self.pose = world_from_camera;
        Ok(())
    }

    /// Call after each authoritative body-pose update. The mount maps camera
    /// local coordinates into body local coordinates: world * body_from_camera.
    pub fn attach_to_body(
        &mut self,
        world_from_body: Mat4,
        body_from_camera: Mat4,
    ) -> Result<(), SensorError> {
        if !rigid_pose(world_from_body) || !rigid_pose(body_from_camera) {
            return Err(SensorError::InvalidPose);
        }
        self.set_pose(world_from_body * body_from_camera)
    }

    pub fn set_background(&mut self, linear_rgb: [f32; 3]) -> Result<(), SensorError> {
        if !valid_color(linear_rgb) {
            return Err(SensorError::InvalidObject);
        }
        self.background = linear_rgb;
        Ok(())
    }

    /// Reallocates only this camera's targets; an invalid size leaves it intact.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), SensorError> {
        let config = SensorCameraConfig {
            width,
            height,
            ..self.config
        };
        validate_config(&self.ctx, config)?;
        (self.targets, self.zbuffer) = create_targets(&self.ctx, config);
        self.config = config;
        Ok(())
    }

    /// Render an explicit scene and synchronously read only requested channels.
    /// Uses map completion callbacks and a bounded device wait, never polling
    /// delays. All per-draw uniforms are immutable and distinct for this frame.
    pub fn render(
        &mut self,
        scene: &[SensorObject<'_>],
        channels: SensorChannels,
    ) -> Result<SensorFrame, SensorError> {
        for object in scene {
            if !object.transform.is_finite()
                || !object.transform.row(3).abs_diff_eq(glam::Vec4::W, 1.0e-5)
                || object.transform.determinant().abs() < 1.0e-8
                || !valid_color(object.color)
                || object.segmentation_id == 0
                || object.geometry.draw_count() % 3 != 0
            {
                return Err(SensorError::InvalidObject);
            }
            let stride = object.geometry.vertex_buffer().stride();
            if !matches!(stride, 12 | 24 | 28 | 36 | 48) {
                return Err(SensorError::UnsupportedVertexStride(stride));
            }
            if object.geometry.is_billboard() && stride != 36 {
                return Err(SensorError::InvalidObject);
            }
            if let Some(texture) = object.texture {
                if stride != 48
                    || !texture
                        .texture
                        .usage()
                        .contains(wgpu::TextureUsages::TEXTURE_BINDING)
                    || !matches!(
                        texture.format(),
                        wgpu::TextureFormat::Rgba8Unorm
                            | wgpu::TextureFormat::Rgba8UnormSrgb
                            | wgpu::TextureFormat::Bgra8Unorm
                            | wgpu::TextureFormat::Bgra8UnormSrgb
                    )
                {
                    return Err(SensorError::InvalidObject);
                }
            }
            if let Some(index) = object.geometry.index_buffer() {
                if object.geometry.draw_count() > index.count() {
                    return Err(SensorError::InvalidObject);
                }
            } else if object.geometry.draw_count() > object.geometry.vertex_buffer().count() {
                return Err(SensorError::InvalidObject);
            }
        }
        let mut frame = SensorFrame {
            width: self.config.width,
            height: self.config.height,
            rgb: None,
            depth: None,
            segmentation: None,
        };
        if !channels.rgb && !channels.depth && !channels.segmentation {
            return Ok(frame);
        }
        for object in scene {
            let stride = object.geometry.vertex_buffer().stride();
            if !self.pipelines.contains_key(&stride) {
                self.pipelines.insert(stride, self.create_pipeline(stride));
            }
        }
        let camera = CameraUniform {
            view: self.pose.inverse().to_cols_array_2d(),
            projection: glam::camera::rh::proj::directx::perspective(
                self.config.fov_y_deg.to_radians(),
                self.config.width as f32 / self.config.height as f32,
                self.config.znear,
                self.config.zfar,
            )
            .to_cols_array_2d(),
            eye: self.pose.w_axis.to_array(),
        };
        let camera_group = uniform_group(&self.ctx, &self.camera_layout, &camera);
        let models: Vec<_> = scene
            .iter()
            .map(|object| {
                uniform_group(
                    &self.ctx,
                    &self.model_layout,
                    &SensorModel {
                        transform: object.transform.to_cols_array_2d(),
                        color: [object.color[0], object.color[1], object.color[2], 1.0],
                        segmentation_id: object.segmentation_id,
                        textured: u32::from(object.texture.is_some()),
                        billboard: u32::from(object.geometry.is_billboard()),
                        padding: [0; 1],
                    },
                )
            })
            .collect();
        let textures: Vec<_> = scene
            .iter()
            .map(|object| {
                let texture = object.texture.unwrap_or(&self.white_texture);
                self.ctx
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("sensor image"),
                        layout: &self.texture_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(texture.view()),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(texture.sampler()),
                            },
                        ],
                    })
            })
            .collect();
        let views = self
            .targets
            .each_ref()
            .map(|target| target.create_view(&wgpu::TextureViewDescriptor::default()));
        let zview = self
            .zbuffer
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.ctx.create_encoder(Some("sensor frame"));
        {
            let backgrounds = [
                wgpu::Color {
                    r: f64::from(self.background[0]),
                    g: f64::from(self.background[1]),
                    b: f64::from(self.background[2]),
                    a: 1.0,
                },
                wgpu::Color::TRANSPARENT,
                wgpu::Color::TRANSPARENT,
            ];
            let attachments = std::array::from_fn::<_, 3, _>(|i| {
                Some(wgpu::RenderPassColorAttachment {
                    view: &views[i],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(backgrounds[i]),
                        store: wgpu::StoreOp::Store,
                    },
                })
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sensor scene"),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &zview,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &camera_group, &[]);
            for ((object, model), texture) in scene.iter().zip(&models).zip(&textures) {
                pass.set_pipeline(&self.pipelines[&object.geometry.vertex_buffer().stride()]);
                pass.set_bind_group(1, model, &[]);
                pass.set_bind_group(2, texture, &[]);
                pass.set_vertex_buffer(0, object.geometry.vertex_buffer().slice());
                if let Some(index) = object.geometry.index_buffer() {
                    pass.set_index_buffer(index.slice(), index.format());
                    pass.draw_indexed(0..object.geometry.draw_count(), 0, 0..1);
                } else {
                    pass.draw(0..object.geometry.draw_count(), 0..1);
                }
            }
        }
        let row_bytes = self.config.width * 4;
        let padded_row = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let requested = [channels.rgb, channels.depth, channels.segmentation];
        let staging: Vec<_> = requested
            .iter()
            .enumerate()
            .filter(|(_, enabled)| **enabled)
            .map(|(i, _)| {
                let buffer = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("sensor readback"),
                    size: u64::from(padded_row) * u64::from(self.config.height),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.targets[i],
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row),
                            rows_per_image: Some(self.config.height),
                        },
                    },
                    wgpu::Extent3d {
                        width: self.config.width,
                        height: self.config.height,
                        depth_or_array_layers: 1,
                    },
                );
                (i, buffer)
            })
            .collect();
        let submission = self.ctx.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        for (_, buffer) in &staging {
            let tx = tx.clone();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });
        }
        self.ctx
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .map_err(|error| SensorError::Readback(error.to_string()))?;
        for _ in &staging {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|error| SensorError::Readback(error.to_string()))?
                .map_err(|error| SensorError::Readback(error.to_string()))?;
        }
        for (i, buffer) in staging {
            let mapped = buffer
                .slice(..)
                .get_mapped_range()
                .map_err(|error| SensorError::Readback(error.to_string()))?;
            let bytes: Vec<u8> = mapped
                .chunks_exact(padded_row as usize)
                .flat_map(|row| row[..row_bytes as usize].iter().copied())
                .collect();
            match i {
                0 => {
                    frame.rgb = Some(
                        bytes
                            .chunks_exact(4)
                            .flat_map(|pixel| pixel[..3].iter().copied())
                            .collect(),
                    );
                }
                1 => {
                    frame.depth = Some(
                        bytes
                            .chunks_exact(4)
                            .map(|pixel| {
                                f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]])
                            })
                            .collect(),
                    );
                }
                2 => {
                    frame.segmentation = Some(
                        bytes
                            .chunks_exact(4)
                            .map(|pixel| {
                                u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]])
                            })
                            .collect(),
                    );
                }
                _ => unreachable!(),
            }
            drop(mapped);
            buffer.unmap();
        }
        Ok(frame)
    }

    fn create_pipeline(&self, stride: u64) -> wgpu::RenderPipeline {
        let shader = self
            .ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sensor shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sensor.wgsl").into()),
            });
        let layout = self
            .ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sensor pipeline layout"),
                bind_group_layouts: &[
                    Some(&self.camera_layout),
                    Some(&self.model_layout),
                    Some(&self.texture_layout),
                ],
                immediate_size: 0,
            });
        let targets = sensor_formats().map(|format| {
            Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        });
        self.ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sensor pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: stride,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: if stride == 48 { 24 } else { 0 },
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: if stride == 36 { 12 } else { 0 },
                                shader_location: 2,
                            },
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
    }
}

fn valid_color(color: [f32; 3]) -> bool {
    color
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
}

fn rigid_pose(pose: Mat4) -> bool {
    let rotation = glam::Mat3::from_mat4(pose);
    pose.is_finite()
        && (rotation.transpose() * rotation).abs_diff_eq(glam::Mat3::IDENTITY, 1.0e-4)
        && (rotation.determinant() - 1.0).abs() < 1.0e-4
        && pose.row(3).abs_diff_eq(glam::Vec4::W, 1.0e-5)
}

fn validate_config(ctx: &WgpuContext, config: SensorCameraConfig) -> Result<(), SensorError> {
    let limits = ctx.device.limits();
    if config.width == 0
        || config.height == 0
        || config.width > limits.max_texture_dimension_2d
        || config.height > limits.max_texture_dimension_2d
        || !config.fov_y_deg.is_finite()
        || !(0.0..180.0).contains(&config.fov_y_deg)
        || config.fov_y_deg == 0.0
        || !config.znear.is_finite()
        || !config.zfar.is_finite()
        || config.znear <= 0.0
        || config.zfar <= config.znear
    {
        return Err(SensorError::InvalidCamera);
    }
    let padded = u64::from(config.width) * 4;
    let readback_size = padded.div_ceil(256) * 256 * u64::from(config.height);
    if readback_size > limits.max_buffer_size {
        return Err(SensorError::InvalidCamera);
    }
    Ok(())
}

fn sensor_formats() -> [wgpu::TextureFormat; 3] {
    [
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::R32Float,
        wgpu::TextureFormat::R32Uint,
    ]
}

fn create_targets(
    ctx: &WgpuContext,
    config: SensorCameraConfig,
) -> ([wgpu::Texture; 3], wgpu::Texture) {
    let create = |format, usage| {
        ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sensor target"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    (
        sensor_formats().map(|format| {
            create(
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            )
        }),
        create(
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ),
    )
}

fn uniform_layout(ctx: &WgpuContext) -> wgpu::BindGroupLayout {
    ctx.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sensor uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
}

fn uniform_group<T: bytemuck::Pod>(
    ctx: &WgpuContext,
    layout: &wgpu::BindGroupLayout,
    value: &T,
) -> wgpu::BindGroup {
    let buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sensor immutable uniform"),
            contents: bytemuck::bytes_of(value),
            usage: wgpu::BufferUsages::UNIFORM,
        });
    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sensor uniform group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}
