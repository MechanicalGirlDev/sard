//! Particle system geometry
//!
//! Provides a particle system that simulates effects like fireworks, fire, and smoke.

use super::{Aabb, Geometry};
use crate::context::WgpuContext;
use crate::core::buffer::{IndexBuffer, VertexBuffer};
use crate::core::pipeline::Vertex;
use glam::Vec3;

/// Data describing a set of particles.
#[derive(Clone, Debug)]
pub struct ParticleData {
    /// Initial positions of each particle in world coordinates.
    pub start_positions: Vec<Vec3>,
    /// Initial velocities of each particle.
    pub start_velocities: Vec<Vec3>,
    /// Color for each particle (RGB).
    pub colors: Vec<[f32; 3]>,
}

impl ParticleData {
    /// Get the number of particles.
    pub fn count(&self) -> usize {
        self.start_positions.len()
    }
}

/// Particle system that simulates particle effects.
///
/// Particles move according to:
/// ```text
/// position = start_position + start_velocity * time + 0.5 * acceleration * time^2
/// ```
///
/// The particle positions are baked into the vertex buffer each time
/// `update` is called with the current time.
pub struct ParticleSystem {
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
    draw_count: u32,
    aabb: Aabb,
    data: ParticleData,
    acceleration: Vec3,
    quad_size: f32,
}

impl ParticleSystem {
    /// Create a new particle system.
    ///
    /// - `data`: The initial particle positions, velocities, and colors.
    /// - `acceleration`: Global acceleration applied to all particles (e.g., gravity).
    /// - `quad_size`: Half-extent of each particle quad.
    pub fn new(ctx: &WgpuContext, data: ParticleData, acceleration: Vec3, quad_size: f32) -> Self {
        let (vertices, indices) = Self::build_quads(&data, acceleration, 0.0, quad_size);
        let vertex_buffer = VertexBuffer::new(ctx, &vertices, Some("particle vertices"));
        let index_buffer = IndexBuffer::new_u32(ctx, &indices, Some("particle indices"));
        let draw_count = indices.len() as u32;
        let aabb = Self::compute_aabb(&data, acceleration, 0.0, quad_size);

        Self {
            vertex_buffer,
            index_buffer,
            draw_count,
            aabb,
            data,
            acceleration,
            quad_size,
        }
    }

    /// Update particle positions based on elapsed time.
    pub fn update(&mut self, ctx: &WgpuContext, time: f32) {
        let (vertices, _) = Self::build_quads(&self.data, self.acceleration, time, self.quad_size);
        self.vertex_buffer = VertexBuffer::new(ctx, &vertices, Some("particle vertices"));
        self.aabb = Self::compute_aabb(&self.data, self.acceleration, time, self.quad_size);
    }

    /// Set the acceleration (e.g., gravity).
    pub fn set_acceleration(&mut self, acceleration: Vec3) {
        self.acceleration = acceleration;
    }

    fn compute_position(start_pos: Vec3, start_vel: Vec3, acceleration: Vec3, time: f32) -> Vec3 {
        start_pos + start_vel * time + 0.5 * acceleration * time * time
    }

    fn compute_aabb(data: &ParticleData, acceleration: Vec3, time: f32, size: f32) -> Aabb {
        if data.count() == 0 {
            return Aabb::default();
        }
        let positions: Vec<Vec3> = data
            .start_positions
            .iter()
            .zip(data.start_velocities.iter())
            .map(|(sp, sv)| Self::compute_position(*sp, *sv, acceleration, time))
            .collect();
        let aabb1 = Aabb::from_points(positions.iter().map(|p| *p + Vec3::splat(size)));
        let aabb2 = Aabb::from_points(positions.iter().map(|p| *p - Vec3::splat(size)));
        aabb1.merge(&aabb2)
    }

    fn build_quads(
        data: &ParticleData,
        acceleration: Vec3,
        time: f32,
        size: f32,
    ) -> (Vec<Vertex>, Vec<u32>) {
        let count = data.count();
        let mut vertices = Vec::with_capacity(count * 4);
        let mut indices = Vec::with_capacity(count * 6);

        for i in 0..count {
            let pos = Self::compute_position(
                data.start_positions[i],
                data.start_velocities[i],
                acceleration,
                time,
            );
            let color = data.colors[i];
            let base = (i * 4) as u32;

            // Billboard quad - store center in position, offset in normal
            vertices.push(Vertex {
                position: pos.to_array(),
                normal: [-size, -size, 0.0],
                color,
            });
            vertices.push(Vertex {
                position: pos.to_array(),
                normal: [size, -size, 0.0],
                color,
            });
            vertices.push(Vertex {
                position: pos.to_array(),
                normal: [size, size, 0.0],
                color,
            });
            vertices.push(Vertex {
                position: pos.to_array(),
                normal: [-size, size, 0.0],
                color,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        (vertices, indices)
    }
}

impl Geometry for ParticleSystem {
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
