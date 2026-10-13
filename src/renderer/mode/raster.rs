use crate::context::WgpuContext;
use crate::core::{CullState, DepthState, DepthTexture, PipelineBuilder, RenderTarget};

pub(super) struct RasterBackend {
    scene_layout: wgpu::BindGroupLayout,
    shadow_group: wgpu::BindGroup,
    shadow: DepthTexture,
    depth_pipeline: wgpu::RenderPipeline,
    color_pipeline: wgpu::RenderPipeline,
}

impl RasterBackend {
    pub(super) fn new(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        resolution: u32,
    ) -> anyhow::Result<Self> {
        let scene_layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("raster scene layout"),
                entries: &[
                    super::buffer_layout(
                        0,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        wgpu::BufferBindingType::Uniform,
                    ),
                    super::buffer_layout(
                        1,
                        wgpu::ShaderStages::VERTEX,
                        wgpu::BufferBindingType::Storage { read_only: true },
                    ),
                ],
            });
        let shadow_layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("raster shadow layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ],
            });
        let shadow = DepthTexture::new(ctx, resolution, resolution, Some("mode shadow"));
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mode shadow sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mode shadow group"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = concat!(
            include_str!("../../shaders/mode_common.wgsl"),
            "\n",
            include_str!("../../shaders/mode_raster.wgsl")
        );
        let depth_pipeline = PipelineBuilder::new(ctx)
            .label("mode shadow pipeline")
            .shader(shader)
            .vertex_entry("vs_shadow")
            .fragment_entry("fs_shadow")
            .bind_group_layout(&scene_layout)
            .cull(CullState::None)
            .depth_bias(2, 2.0)
            .build_depth_only()?;
        let color_pipeline = PipelineBuilder::new(ctx)
            .label("mode PBR pipeline")
            .shader(shader)
            .bind_group_layout(&scene_layout)
            .bind_group_layout(&shadow_layout)
            .color_format(format)
            .depth(DepthState::read_write())
            .cull(CullState::None)
            .build()?;
        Ok(Self {
            scene_layout,
            shadow_group,
            shadow,
            depth_pipeline,
            color_pipeline,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "explicit frame resources borrowed for one render submission"
    )]
    pub(super) fn render(
        &self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Buffer,
        triangles: &wgpu::Buffer,
        triangle_count: u32,
        target: &RenderTarget<'_>,
        depth: Option<&wgpu::TextureView>,
        environment: [f32; 3],
    ) {
        let group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mode raster scene"),
            layout: &self.scene_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: triangles.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mode shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(super::depth_attachment(self.shadow.view())),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.depth_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..triangle_count * 3, 0..1);
        }
        let background = environment.map(|c| f64::from(c / (1.0 + c)));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mode PBR pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view(),
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: background[0],
                        g: background[1],
                        b: background[2],
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth.map(super::depth_attachment),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.color_pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.set_bind_group(1, &self.shadow_group, &[]);
        pass.draw(0..triangle_count * 3, 0..1);
    }
}
