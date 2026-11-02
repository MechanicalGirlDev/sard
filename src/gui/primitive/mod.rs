//! Primitive 2D rendering
//!
//! Provides rendering for 2D primitives like rectangles and circles.

use crate::context::WgpuContext;
use bytemuck::{Pod, Zeroable};

/// Vertex structure for 2D primitives.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub mode: u32,
}

/// A contiguous run of vertices sharing one scissor (clip) rectangle.
#[derive(Clone, Copy)]
struct DrawBatch {
    /// First vertex index covered by this batch.
    start: u32,
    /// Unrounded clip bounds `[left, top, right, bottom]`, or `None` for no clip.
    scissor: Option<[f32; 4]>,
}

/// Renderer for 2D primitives.
pub struct PrimitiveRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
    screen_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    screen_size: [f32; 2],
    // Scissor batching: each batch records where it starts and which clip applies.
    batches: Vec<DrawBatch>,
    clip_stack: Vec<[f32; 4]>,
}

mod geometry;
mod pipeline;
mod render;
#[cfg(feature = "window")]
mod scope;
#[cfg(feature = "window")]
pub(crate) use scope::{PrimitivePaintRange, PrimitivePaintScope};

impl PrimitiveRenderer {
    /// Push a clip rectangle. Drawing is scissored to the intersection of this
    /// rectangle with any clip already on the stack. Balance every call with
    /// [`pop_clip`](Self::pop_clip).
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut x0 = x;
        let mut y0 = y;
        let mut x1 = (x + w).max(x0);
        let mut y1 = (y + h).max(y0);
        if let Some(&[left, top, right, bottom]) = self.clip_stack.last() {
            x0 = x0.max(left);
            y0 = y0.max(top);
            x1 = x1.min(right);
            y1 = y1.min(bottom);
        }
        self.clip_stack.push([x0, y0, x1.max(x0), y1.max(y0)]);
    }

    /// Pop the most recently pushed clip rectangle.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Start a new batch if the active clip differs from the open batch's clip.
    pub(super) fn begin_geometry(&mut self) {
        let scissor = self.clip_stack.last().copied();
        let start = self.vertices.len() as u32;
        match self.batches.last() {
            Some(b)
                if b.scissor.map(|r| r.map(f32::to_bits))
                    == scissor.map(|r| r.map(f32::to_bits)) => {}
            _ => self.batches.push(DrawBatch { start, scissor }),
        }
    }
}
