use crate::context::WgpuContext;
use crate::core::{CullState, DepthState, PipelineBuilder, RenderTarget};

pub(super) struct TraceBackend {
    compute_layout: wgpu::BindGroupLayout,
    display_layout: wgpu::BindGroupLayout,
    compute: wgpu::ComputePipeline,
    display: wgpu::RenderPipeline,
    pixels: wgpu::Buffer,
}

impl TraceBackend {
    pub(super) fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let storage = wgpu::BufferBindingType::Storage { read_only: true };
        let compute_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("mode trace layout"),
                    entries: &[
                        super::buffer_layout(
                            0,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::BufferBindingType::Uniform,
                        ),
                        super::buffer_layout(1, wgpu::ShaderStages::COMPUTE, storage),
                        super::buffer_layout(2, wgpu::ShaderStages::COMPUTE, storage),
                        super::buffer_layout(
                            3,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::BufferBindingType::Storage { read_only: false },
                        ),
                    ],
                });
        let display_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("mode trace display layout"),
                    entries: &[
                        super::buffer_layout(
                            0,
                            wgpu::ShaderStages::FRAGMENT,
                            wgpu::BufferBindingType::Uniform,
                        ),
                        super::buffer_layout(1, wgpu::ShaderStages::FRAGMENT, storage),
                    ],
                });
        let module = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mode ray and path tracing"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("../../shaders/mode_common.wgsl"),
                        "\n",
                        include_str!("../../shaders/mode_trace.wgsl")
                    )
                    .into(),
                ),
            });
        let layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mode tracing pipeline layout"),
                bind_group_layouts: &[Some(&compute_layout)],
                immediate_size: 0,
            });
        let compute = ctx
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("mode ray and path tracing"),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("cs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
        let display = PipelineBuilder::new(ctx)
            .label("mode trace display")
            .shader(concat!(
                include_str!("../../shaders/mode_common.wgsl"),
                "\n",
                include_str!("../../shaders/mode_display.wgsl")
            ))
            .bind_group_layout(&display_layout)
            .color_format(format)
            .depth(DepthState {
                write: true,
                compare: wgpu::CompareFunction::Always,
            })
            .cull(CullState::None)
            .build()?;
        let pixels = super::buffer(ctx, "trace pixels", &[0; 16], wgpu::BufferUsages::STORAGE);
        Ok(Self {
            compute_layout,
            display_layout,
            compute,
            display,
            pixels,
        })
    }

    pub(super) fn resize(&mut self, ctx: &WgpuContext, size: [u32; 2]) {
        self.pixels = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trace accumulated pixels"),
            size: u64::from(size[0]) * u64::from(size[1]) * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "explicit resources shared with raster backend for one frame"
    )]
    pub(super) fn render(
        &self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Buffer,
        triangles: &wgpu::Buffer,
        nodes: &wgpu::Buffer,
        target: &RenderTarget<'_>,
        depth: Option<&wgpu::TextureView>,
        size: [u32; 2],
    ) {
        let compute_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trace scene and pixels"),
            layout: &self.compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: triangles.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: nodes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.pixels.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ray and path tracing pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute);
            pass.set_bind_group(0, &compute_group, &[]);
            pass.dispatch_workgroups(size[0].div_ceil(8), size[1].div_ceil(8), 1);
        }
        let display_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trace display pixels"),
            layout: &self.display_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.pixels.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("trace display pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view(),
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth.map(super::depth_attachment),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.display);
        pass.set_bind_group(0, &display_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
