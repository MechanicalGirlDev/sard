//! Geometry abstractions
//!
//! Provides mesh and primitive geometry types including lines and axes.

mod axes;
mod bounds;
mod circle;
mod instanced;
mod lines;
mod mesh;
mod particles;
mod rectangle;
pub mod skybox;
mod sprites;
mod terrain;

pub use axes::Axes;
pub use bounds::BoundingBoxMesh;
pub use circle::Circle;
pub use instanced::InstancedMesh;
pub use lines::{LineStrip, Lines};
pub use mesh::Mesh;
pub use particles::{ParticleData, ParticleSystem};
pub use rectangle::Rectangle;
pub use skybox::Skybox;
pub use sprites::Sprites;
pub use terrain::{Terrain, TerrainLod};

use crate::core::buffer::{IndexBuffer, VertexBuffer};
use glam::Vec3;

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Create a new AABB.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from a set of points.
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for p in points {
            min = min.min(p);
            max = max.max(p);
        }

        Self { min, max }
    }

    /// Get the center of the AABB.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Get the size of the AABB.
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Get all 8 corners of the AABB.
    pub fn corners(&self) -> [Vec3; 8] {
        [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }

    /// Check if a point is inside the AABB.
    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Merge two AABBs.
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

impl Default for Aabb {
    fn default() -> Self {
        Self {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
        }
    }
}

/// Trait for geometry that can be rendered.
pub trait Geometry {
    /// Get the vertex buffer.
    fn vertex_buffer(&self) -> &VertexBuffer;

    /// Get the index buffer if available.
    fn index_buffer(&self) -> Option<&IndexBuffer>;

    /// Get the number of primitives to draw.
    fn draw_count(&self) -> u32;

    /// Get the axis-aligned bounding box.
    fn aabb(&self) -> Aabb;

    /// Whether standard `Vertex` positions store centers and normal XY stores
    /// camera-facing corner offsets in world units rather than surface normals.
    fn is_billboard(&self) -> bool {
        false
    }

    /// Draw the geometry using the given render pass.
    fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer().slice());
        if let Some(index_buffer) = self.index_buffer() {
            render_pass.set_index_buffer(index_buffer.slice(), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.draw_count(), 0, 0..1);
        } else {
            render_pass.draw(0..self.draw_count(), 0..1);
        }
    }
}
