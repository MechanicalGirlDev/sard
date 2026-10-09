//! Sprite geometry for billboard rendering
//!
//! Provides a set of camera-facing quads (billboards/sprites).

use super::{Aabb, Geometry};
use crate::context::WgpuContext;
use crate::core::buffer::{IndexBuffer, VertexBuffer};
use crate::core::pipeline::Vertex;
use glam::Vec3;

/// A set of sprites (billboard quads) that orient toward the camera.
///
/// Each sprite is defined by a center position and rendered as a quad
/// that always faces the camera. Optionally constrained to rotate
/// around a fixed axis (e.g., Y-up for tree billboards).
pub struct Sprites {
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
    draw_count: u32,
    aabb: Aabb,
}

impl Sprites {
    /// Create sprites at the given center positions.
    ///
    /// Each sprite is a unit quad that will be scaled/oriented by the material's
    /// billboard shader. `size` controls the half-extent of each quad.
    pub fn new(ctx: &WgpuContext, centers: &[Vec3], size: f32, color: [f32; 3]) -> Self {
        let mut vertices = Vec::with_capacity(centers.len() * 4);
        let mut indices = Vec::with_capacity(centers.len() * 6);

        for (i, center) in centers.iter().enumerate() {
            let base = (i * 4) as u32;

            // Four corners of a quad centered at the sprite position.
            // The billboard orientation is handled by the vertex shader;
            // here we store offsets in the normal field (x,y = corner offset, z unused).
            vertices.push(Vertex {
                position: center.to_array(),
                normal: [-size, -size, 0.0],
                color,
            });
            vertices.push(Vertex {
                position: center.to_array(),
                normal: [size, -size, 0.0],
                color,
            });
            vertices.push(Vertex {
                position: center.to_array(),
                normal: [size, size, 0.0],
                color,
            });
            vertices.push(Vertex {
                position: center.to_array(),
                normal: [-size, size, 0.0],
                color,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let vertex_buffer = VertexBuffer::new(ctx, &vertices, Some("sprites vertices"));
        let index_buffer = IndexBuffer::new_u32(ctx, &indices, Some("sprites indices"));
        let draw_count = indices.len() as u32;

        let aabb = Aabb::from_points(centers.iter().map(|c| *c + Vec3::splat(size)));
        let aabb2 = Aabb::from_points(centers.iter().map(|c| *c - Vec3::splat(size)));
        let aabb = aabb.merge(&aabb2);

        Self {
            vertex_buffer,
            index_buffer,
            draw_count,
            aabb,
        }
    }
}

impl Geometry for Sprites {
    fn is_billboard(&self) -> bool {
        true
    }

    fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    fn index_buffer(&self) -> Option<&IndexBuffer> {
        Some(&self.index_buffer)
    }

    fn draw_count(&self) -> u32 {
        self.draw_count
    }

    fn aabb(&self) -> Aabb {
        self.aabb
    }
}
