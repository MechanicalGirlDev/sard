//! Shared scene uniforms and growable GPU buffers.

use crate::context::WgpuContext;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FrameUniform {
    pub(super) view_proj: [[f32; 4]; 4],
    pub(super) view: [[f32; 4]; 4],
    pub(super) eye: [f32; 4],
    pub(super) sun_dir: [f32; 4],
    pub(super) sun_color: [f32; 4],
    pub(super) sky: [f32; 4],
    pub(super) ground: [f32; 4],
    pub(super) up: [f32; 4],
    pub(super) cam_right: [f32; 4],
    pub(super) cam_up: [f32; 4],
    pub(super) viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ObjectUniform {
    pub(super) model: [[f32; 4]; 4],
    pub(super) normal_matrix: [[f32; 4]; 4],
    pub(super) color: [f32; 4],
    pub(super) emissive: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GridUniform {
    pub(super) origin: [f32; 4],
    pub(super) axis_u: [f32; 4],
    pub(super) axis_v: [f32; 4],
    pub(super) params: [f32; 4],
    pub(super) minor_color: [f32; 4],
    pub(super) major_color: [f32; 4],
    pub(super) axis_u_color: [f32; 4],
    pub(super) axis_v_color: [f32; 4],
}

/// A GPU buffer that grows to fit whatever is written each frame.
pub(super) struct GrowBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) capacity: u64,
    pub(super) usage: wgpu::BufferUsages,
    pub(super) label: &'static str,
}

impl GrowBuffer {
    pub(super) fn new(
        ctx: &WgpuContext,
        capacity: u64,
        usage: wgpu::BufferUsages,
        label: &'static str,
    ) -> Self {
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity,
            usage,
            label,
        }
    }

    /// Write `bytes`; returns `true` when the buffer had to be re-created.
    pub(super) fn write(&mut self, ctx: &WgpuContext, bytes: &[u8]) -> bool {
        let needed = bytes.len() as u64;
        let recreated = needed > self.capacity;
        if recreated {
            let capacity = needed.next_power_of_two().max(self.capacity * 2);
            *self = Self::new(ctx, capacity, self.usage, self.label);
        }
        if !bytes.is_empty() {
            ctx.queue.write_buffer(&self.buffer, 0, bytes);
        }
        recreated
    }
}

pub(super) fn uniform_layout(
    ctx: &WgpuContext,
    label: &str,
    dynamic: bool,
    min_size: u64,
) -> wgpu::BindGroupLayout {
    ctx.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: dynamic,
                    min_binding_size: wgpu::BufferSize::new(min_size),
                },
                count: None,
            }],
        })
}

pub(super) fn bind_uniform(
    ctx: &WgpuContext,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    size: Option<u64>,
) -> wgpu::BindGroup {
    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: size.and_then(wgpu::BufferSize::new),
            }),
        }],
    })
}
