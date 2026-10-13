//! Render pipeline builder
//!
//! Provides a builder pattern for creating wgpu render pipelines.

use crate::context::WgpuContext;
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::core::texture::DepthTexture;

/// Builder for creating render pipelines.
pub struct PipelineBuilder<'a> {
    ctx: &'a WgpuContext,
    label: Option<&'a str>,
    shader_source: Option<&'a str>,
    vertex_entry: &'a str,
    fragment_entry: &'a str,
    vertex_layouts: Vec<Option<wgpu::VertexBufferLayout<'a>>>,
    bind_group_layouts: Vec<Option<&'a wgpu::BindGroupLayout>>,
    color_format: wgpu::TextureFormat,
    depth_state: Option<DepthState>,
    blend_state: BlendState,
    cull_state: CullState,
    topology: wgpu::PrimitiveTopology,
    depth_bias: wgpu::DepthBiasState,
}

impl<'a> PipelineBuilder<'a> {
    /// Create a new pipeline builder.
    pub fn new(ctx: &'a WgpuContext) -> Self {
        Self {
            ctx,
            label: None,
            shader_source: None,
            vertex_entry: "vs_main",
            fragment_entry: "fs_main",
            vertex_layouts: Vec::new(),
            bind_group_layouts: Vec::new(),
            color_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_state: None,
            blend_state: BlendState::Opaque,
            cull_state: CullState::Back,
            topology: wgpu::PrimitiveTopology::TriangleList,
            depth_bias: wgpu::DepthBiasState::default(),
        }
    }

    /// Constant and slope-scaled depth bias (negative values pull towards the viewer).
    pub fn depth_bias(mut self, constant: i32, slope_scale: f32) -> Self {
        self.depth_bias = wgpu::DepthBiasState {
            constant,
            slope_scale,
            clamp: 0.0,
        };
        self
    }

    /// Set the pipeline label.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the shader source (WGSL).
    pub fn shader(mut self, source: &'a str) -> Self {
        self.shader_source = Some(source);
        self
    }

    /// Set the vertex shader entry point.
    pub fn vertex_entry(mut self, entry: &'a str) -> Self {
        self.vertex_entry = entry;
        self
    }

    /// Set the fragment shader entry point.
    pub fn fragment_entry(mut self, entry: &'a str) -> Self {
        self.fragment_entry = entry;
        self
    }

    /// Add a vertex buffer layout.
    pub fn vertex_layout(mut self, layout: wgpu::VertexBufferLayout<'a>) -> Self {
        self.vertex_layouts.push(Some(layout));
        self
    }

    /// Add a bind group layout.
    pub fn bind_group_layout(mut self, layout: &'a wgpu::BindGroupLayout) -> Self {
        self.bind_group_layouts.push(Some(layout));
        self
    }

    /// Set the color target format.
    pub fn color_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.color_format = format;
        self
    }

    /// Enable depth testing.
    pub fn depth(mut self, state: DepthState) -> Self {
        self.depth_state = Some(state);
        self
    }

    /// Set the blend state.
    pub fn blend(mut self, state: BlendState) -> Self {
        self.blend_state = state;
        self
    }

    /// Set the cull state.
    pub fn cull(mut self, state: CullState) -> Self {
        self.cull_state = state;
        self
    }

    /// Set the primitive topology.
    pub fn topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    /// Build a depth-only render pipeline (no color output).
    /// Used for shadow map generation.
    pub fn build_depth_only(self) -> anyhow::Result<wgpu::RenderPipeline> {
        let shader_source = self
            .shader_source
            .ok_or_else(|| anyhow::anyhow!("Shader source is required"))?;

        let shader_module = self
            .ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: self.label,
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let pipeline_layout =
            self.ctx
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: self.label,
                    bind_group_layouts: &self.bind_group_layouts,
                    immediate_size: 0,
                });

        let depth_stencil = self.depth_state.map_or_else(
            || DepthState::default().to_wgpu(DepthTexture::FORMAT),
            |state| state.to_wgpu(DepthTexture::FORMAT),
        );
        let depth_stencil = wgpu::DepthStencilState {
            bias: self.depth_bias,
            ..depth_stencil
        };

        let pipeline = self
            .ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: self.label,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: Some(self.vertex_entry),
                    buffers: &self.vertex_layouts,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: Some(self.fragment_entry),
                    targets: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: self.topology,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: self.cull_state.to_wgpu(),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(depth_stencil),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        Ok(pipeline)
    }

    /// Build the render pipeline.
    pub fn build(self) -> anyhow::Result<wgpu::RenderPipeline> {
        let shader_source = self
            .shader_source
            .ok_or_else(|| anyhow::anyhow!("Shader source is required"))?;

        let shader_module = self
            .ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: self.label,
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let pipeline_layout =
            self.ctx
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: self.label,
                    bind_group_layouts: &self.bind_group_layouts,
                    immediate_size: 0,
                });

        let depth_stencil = self.depth_state.map(|state| wgpu::DepthStencilState {
            bias: self.depth_bias,
            ..state.to_wgpu(DepthTexture::FORMAT)
        });

        let pipeline = self
            .ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: self.label,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: Some(self.vertex_entry),
                    buffers: &self.vertex_layouts,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: Some(self.fragment_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.color_format,
                        blend: self.blend_state.to_wgpu(),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: self.topology,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: self.cull_state.to_wgpu(),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        Ok(pipeline)
    }
}

/// Standard vertex type with position, normal, and color.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// Builder for creating compute pipelines.
pub struct ComputePipelineBuilder<'a> {
    ctx: &'a WgpuContext,
    label: Option<&'a str>,
    shader_source: Option<&'a str>,
    entry_point: &'a str,
    bind_group_layouts: Vec<Option<&'a wgpu::BindGroupLayout>>,
}

impl<'a> ComputePipelineBuilder<'a> {
    /// Create a new compute pipeline builder.
    pub fn new(ctx: &'a WgpuContext) -> Self {
        Self {
            ctx,
            label: None,
            shader_source: None,
            entry_point: "cs_main",
            bind_group_layouts: Vec::new(),
        }
    }

    /// Set the pipeline label.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the shader source (WGSL).
    pub fn shader(mut self, source: &'a str) -> Self {
        self.shader_source = Some(source);
        self
    }

    /// Set the compute shader entry point.
    pub fn entry_point(mut self, entry: &'a str) -> Self {
        self.entry_point = entry;
        self
    }

    /// Add a bind group layout.
    pub fn bind_group_layout(mut self, layout: &'a wgpu::BindGroupLayout) -> Self {
        self.bind_group_layouts.push(Some(layout));
        self
    }

    /// Build the compute pipeline.
    pub fn build(self) -> anyhow::Result<wgpu::ComputePipeline> {
        let shader_source = self
            .shader_source
            .ok_or_else(|| anyhow::anyhow!("Shader source is required"))?;

        let shader_module = self
            .ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: self.label,
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let pipeline_layout =
            self.ctx
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: self.label,
                    bind_group_layouts: &self.bind_group_layouts,
                    immediate_size: 0,
                });

        Ok(self
            .ctx
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: self.label,
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some(self.entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            }))
    }
}

impl Vertex {
    /// Get the vertex buffer layout for this vertex type.
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // color
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}
