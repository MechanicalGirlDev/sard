//! Scene pipeline construction.

use core::mem::size_of;

use super::gpu::{
    bind_uniform, uniform_layout, FrameUniform, GridUniform, GrowBuffer, ObjectUniform,
};
use super::SceneRenderer;
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::core::vertex::VertexPN;
use crate::scene::lines::LineInstance;
use crate::scene::PointInstance;

impl SceneRenderer {
    /// Build the pipelines for a colour target of `format`.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let frame_layout =
            uniform_layout(ctx, "scene frame", false, size_of::<FrameUniform>() as u64);
        let frame_buffer =
            RawUniformBuffer::new(ctx, size_of::<FrameUniform>() as u64, Some("scene frame"));
        let frame_bind_group = bind_uniform(ctx, &frame_layout, frame_buffer.buffer(), None);

        let align = u64::from(ctx.device.limits().min_uniform_buffer_offset_alignment);
        let object_stride = (size_of::<ObjectUniform>() as u64).div_ceil(align) * align;
        let object_layout =
            uniform_layout(ctx, "scene object", true, size_of::<ObjectUniform>() as u64);
        let object_buffer = GrowBuffer::new(
            ctx,
            object_stride * 256,
            wgpu::BufferUsages::UNIFORM,
            "scene objects",
        );
        let object_bind_group = bind_uniform(
            ctx,
            &object_layout,
            &object_buffer.buffer,
            Some(size_of::<ObjectUniform>() as u64),
        );

        let grid_layout = uniform_layout(ctx, "scene grid", false, size_of::<GridUniform>() as u64);
        let grid_buffer =
            RawUniformBuffer::new(ctx, size_of::<GridUniform>() as u64, Some("scene grid"));
        let grid_bind_group = bind_uniform(ctx, &grid_layout, grid_buffer.buffer(), None);

        let line_buffer =
            GrowBuffer::new(ctx, 64 * 1024, wgpu::BufferUsages::VERTEX, "scene lines");
        let point_buffer =
            GrowBuffer::new(ctx, 64 * 1024, wgpu::BufferUsages::VERTEX, "scene points");

        let mesh_shader = include_str!("../../shaders/scene_mesh.wgsl");
        let mesh = |label, depth, blend| {
            PipelineBuilder::new(ctx)
                .label(label)
                .shader(mesh_shader)
                .vertex_layout(VertexPN::layout())
                .bind_group_layout(&frame_layout)
                .bind_group_layout(&object_layout)
                .color_format(format)
                .depth(depth)
                .blend(blend)
                .cull(CullState::None)
                .build()
        };
        let mesh_opaque = mesh(
            "scene mesh opaque",
            DepthState::read_write(),
            BlendState::Opaque,
        )?;
        let mesh_blend = mesh(
            "scene mesh blend",
            DepthState::read_only(),
            BlendState::Alpha,
        )?;
        let mesh_overlay = mesh(
            "scene mesh overlay",
            DepthState::disabled(),
            BlendState::Alpha,
        )?;

        let line_shader = include_str!("../../shaders/scene_line.wgsl");
        let line_pipe = |label, depth| {
            PipelineBuilder::new(ctx)
                .label(label)
                .shader(line_shader)
                .vertex_layout(LineInstance::layout())
                .bind_group_layout(&frame_layout)
                .color_format(format)
                .depth(depth)
                .blend(BlendState::Alpha)
                .cull(CullState::None)
                .build()
        };
        let line = line_pipe("scene lines", DepthState::read_write())?;
        let line_overlay = line_pipe("scene lines overlay", DepthState::disabled())?;

        let point_shader = include_str!("../../shaders/scene_point.wgsl");
        let point_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<PointInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4],
        };
        let point_pipe = |label, depth| {
            PipelineBuilder::new(ctx)
                .label(label)
                .shader(point_shader)
                .vertex_layout(point_layout.clone())
                .bind_group_layout(&frame_layout)
                .color_format(format)
                .depth(depth)
                .blend(BlendState::Alpha)
                .cull(CullState::None)
                .build()
        };
        let point = point_pipe("scene points", DepthState::read_write())?;
        let point_overlay = point_pipe("scene points overlay", DepthState::disabled())?;

        let grid = PipelineBuilder::new(ctx)
            .label("scene grid")
            .shader(include_str!("../../shaders/scene_grid.wgsl"))
            .bind_group_layout(&frame_layout)
            .bind_group_layout(&grid_layout)
            .color_format(format)
            .depth(DepthState {
                write: false,
                compare: wgpu::CompareFunction::LessEqual,
            })
            .depth_bias(-2, -1.0)
            .blend(BlendState::Alpha)
            .cull(CullState::None)
            .build()?;

        let depth_reset = PipelineBuilder::new(ctx)
            .label("scene depth reset")
            .shader(include_str!("../../shaders/scene_depth_reset.wgsl"))
            .color_format(format)
            .depth(DepthState {
                write: true,
                compare: wgpu::CompareFunction::Always,
            })
            // Colour writes are neutralised by the blend state: destination stays as is.
            .blend(BlendState::KeepDestination)
            .cull(CullState::None)
            .build()?;

        Ok(Self {
            format,
            frame_buffer,
            frame_bind_group,
            object_layout,
            object_buffer,
            object_bind_group,
            object_stride,
            grid_buffer,
            grid_bind_group,
            line_buffer,
            point_buffer,
            mesh_opaque,
            mesh_blend,
            mesh_overlay,
            line,
            line_overlay,
            point,
            point_overlay,
            grid,
            depth_reset,
        })
    }
}
