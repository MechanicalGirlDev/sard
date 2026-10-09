//! Mesh geometry
//!
//! Provides mesh and primitive geometry types.

use super::{Aabb, Geometry};
use crate::context::WgpuContext;
use crate::core::buffer::{IndexBuffer, VertexBuffer};
use crate::core::pipeline::Vertex;
use crate::core::vertex::VertexPNUC;
use glam::Vec3;

/// A mesh with vertex and index data.
pub struct Mesh {
    vertex_buffer: VertexBuffer,
    index_buffer: Option<IndexBuffer>,
    draw_count: u32,
    aabb: Aabb,
}

impl Mesh {
    /// Create a new mesh from vertices and indices.
    pub fn new(
        ctx: &WgpuContext,
        vertices: &[Vertex],
        indices: Option<&[u32]>,
        label: Option<&str>,
    ) -> Self {
        let vertex_buffer = VertexBuffer::new(ctx, vertices, label);

        let (index_buffer, draw_count) = if let Some(indices) = indices {
            let ib = IndexBuffer::new_u32(ctx, indices, label);
            let count = indices.len() as u32;
            (Some(ib), count)
        } else {
            (None, vertices.len() as u32)
        };

        let aabb = Aabb::from_points(vertices.iter().map(|v| Vec3::from(v.position)));

        Self {
            vertex_buffer,
            index_buffer,
            draw_count,
            aabb,
        }
    }

    /// Create a mesh with explicit UV coordinates and RGBA vertex colors.
    pub fn new_textured(
        ctx: &WgpuContext,
        vertices: &[VertexPNUC],
        indices: Option<&[u32]>,
        label: Option<&str>,
    ) -> Self {
        let vertex_buffer = VertexBuffer::new(ctx, vertices, label);
        let (index_buffer, draw_count) = if let Some(indices) = indices {
            let buffer = IndexBuffer::new_u32(ctx, indices, label);
            (Some(buffer), indices.len() as u32)
        } else {
            (None, vertices.len() as u32)
        };
        Self {
            vertex_buffer,
            index_buffer,
            draw_count,
            aabb: Aabb::from_points(vertices.iter().map(|vertex| Vec3::from(vertex.position))),
        }
    }

    /// Create a cube mesh.
    pub fn cube(ctx: &WgpuContext, size: f32, color: [f32; 3]) -> Self {
        let half = size / 2.0;
        let vertices = cube_vertices(half, color);
        let indices = cube_indices();
        Self::new(ctx, &vertices, Some(&indices), Some("cube"))
    }

    /// Create a sphere mesh.
    pub fn sphere(
        ctx: &WgpuContext,
        radius: f32,
        segments: u32,
        rings: u32,
        color: [f32; 3],
    ) -> Self {
        let (vertices, indices) = sphere_vertices(radius, segments, rings, color);
        Self::new(ctx, &vertices, Some(&indices), Some("sphere"))
    }

    /// Create a cylinder mesh.
    pub fn cylinder(
        ctx: &WgpuContext,
        radius: f32,
        height: f32,
        segments: u32,
        color: [f32; 3],
    ) -> Self {
        let (vertices, indices) = cylinder_vertices(radius, height, segments, color);
        Self::new(ctx, &vertices, Some(&indices), Some("cylinder"))
    }

    /// Create a quad mesh (XZ plane).
    pub fn quad(ctx: &WgpuContext, width: f32, depth: f32, color: [f32; 3]) -> Self {
        let hw = width / 2.0;
        let hd = depth / 2.0;

        let vertices = vec![
            Vertex {
                position: [-hw, 0.0, -hd],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            Vertex {
                position: [hw, 0.0, -hd],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            Vertex {
                position: [hw, 0.0, hd],
                normal: [0.0, 1.0, 0.0],
                color,
            },
            Vertex {
                position: [-hw, 0.0, hd],
                normal: [0.0, 1.0, 0.0],
                color,
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        Self::new(ctx, &vertices, Some(&indices), Some("quad"))
    }
}

impl Geometry for Mesh {
    fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    fn index_buffer(&self) -> Option<&IndexBuffer> {
        self.index_buffer.as_ref()
    }

    fn draw_count(&self) -> u32 {
        self.draw_count
    }

    fn aabb(&self) -> Aabb {
        self.aabb
    }
}

// Helper functions for generating primitive geometry

fn cube_vertices(half: f32, color: [f32; 3]) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(24);

    // Front face (+Z)
    let normal = [0.0, 0.0, 1.0];
    vertices.push(Vertex {
        position: [-half, -half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, -half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, half, half],
        normal,
        color,
    });

    // Back face (-Z)
    let normal = [0.0, 0.0, -1.0];
    vertices.push(Vertex {
        position: [half, -half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, -half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, half, -half],
        normal,
        color,
    });

    // Top face (+Y)
    let normal = [0.0, 1.0, 0.0];
    vertices.push(Vertex {
        position: [-half, half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, half, -half],
        normal,
        color,
    });

    // Bottom face (-Y)
    let normal = [0.0, -1.0, 0.0];
    vertices.push(Vertex {
        position: [-half, -half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, -half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, -half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, -half, half],
        normal,
        color,
    });

    // Right face (+X)
    let normal = [1.0, 0.0, 0.0];
    vertices.push(Vertex {
        position: [half, -half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, -half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [half, half, half],
        normal,
        color,
    });

    // Left face (-X)
    let normal = [-1.0, 0.0, 0.0];
    vertices.push(Vertex {
        position: [-half, -half, -half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, -half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, half, half],
        normal,
        color,
    });
    vertices.push(Vertex {
        position: [-half, half, -half],
        normal,
        color,
    });

    vertices
}

fn cube_indices() -> Vec<u32> {
    let mut indices = Vec::with_capacity(36);
    for face in 0..6 {
        let base = face * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    indices
}

fn sphere_vertices(
    radius: f32,
    segments: u32,
    rings: u32,
    color: [f32; 3],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        let y = phi.cos();
        let ring_radius = phi.sin();

        for segment in 0..=segments {
            let theta = 2.0 * std::f32::consts::PI * segment as f32 / segments as f32;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();

            let position = [x * radius, y * radius, z * radius];
            let normal = [x, y, z];

            vertices.push(Vertex {
                position,
                normal,
                color,
            });
        }
    }

    for ring in 0..rings {
        for segment in 0..segments {
            let current = ring * (segments + 1) + segment;
            let next = current + segments + 1;

            indices.push(current);
            indices.push(next);
            indices.push(current + 1);

            indices.push(current + 1);
            indices.push(next);
            indices.push(next + 1);
        }
    }

    (vertices, indices)
}

fn cylinder_vertices(
    radius: f32,
    height: f32,
    segments: u32,
    color: [f32; 3],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let half_height = height / 2.0;

    // Side vertices
    for i in 0..=segments {
        let theta = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
        let x = theta.cos();
        let z = theta.sin();

        // Bottom vertex
        vertices.push(Vertex {
            position: [x * radius, -half_height, z * radius],
            normal: [x, 0.0, z],
            color,
        });

        // Top vertex
        vertices.push(Vertex {
            position: [x * radius, half_height, z * radius],
            normal: [x, 0.0, z],
            color,
        });
    }

    // Side indices
    for i in 0..segments {
        let base = i * 2;
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);

        indices.push(base + 2);
        indices.push(base + 1);
        indices.push(base + 3);
    }

    // Top cap center
    let top_center = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, half_height, 0.0],
        normal: [0.0, 1.0, 0.0],
        color,
    });

    // Top cap vertices
    for i in 0..=segments {
        let theta = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
        let x = theta.cos();
        let z = theta.sin();
        vertices.push(Vertex {
            position: [x * radius, half_height, z * radius],
            normal: [0.0, 1.0, 0.0],
            color,
        });
    }

    // Top cap indices
    for i in 0..segments {
        indices.push(top_center);
        indices.push(top_center + 1 + i);
        indices.push(top_center + 2 + i);
    }

    // Bottom cap center
    let bottom_center = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, -half_height, 0.0],
        normal: [0.0, -1.0, 0.0],
        color,
    });

    // Bottom cap vertices
    for i in 0..=segments {
        let theta = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
        let x = theta.cos();
        let z = theta.sin();
        vertices.push(Vertex {
            position: [x * radius, -half_height, z * radius],
            normal: [0.0, -1.0, 0.0],
            color,
        });
    }

    // Bottom cap indices (reversed winding)
    for i in 0..segments {
        indices.push(bottom_center);
        indices.push(bottom_center + 2 + i);
        indices.push(bottom_center + 1 + i);
    }

    (vertices, indices)
}
