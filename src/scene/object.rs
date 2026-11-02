//! Mesh instances and their draw layers.

use super::MeshHandle;
use crate::renderer::geometry::Aabb;
use glam::Mat4;

/// Draw layer. Layers are drawn in this order; `Overlay` ignores depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Layer {
    /// Drawn first (ground grid, floor decals).
    Ground,
    /// Ordinary depth-tested content.
    #[default]
    Scene,
    /// Drawn last without depth testing (gizmos, selection outlines).
    Overlay,
}

/// How an object's surface is shaded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    /// Base colour, linear RGBA. `a < 1` makes the object transparent (drawn back to front).
    pub color: [f32; 4],
    /// Added after lighting (selection tint, glow).
    pub emissive: [f32; 3],
    /// Skip lighting entirely.
    pub unlit: bool,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            color: [0.8, 0.8, 0.8, 1.0],
            emissive: [0.0; 3],
            unlit: false,
        }
    }
}

impl Surface {
    /// Opaque lit surface of one colour.
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self {
            color: [r, g, b, 1.0],
            ..Default::default()
        }
    }

    /// Lit surface with alpha.
    pub fn rgba(color: [f32; 4]) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }

    /// Whether the surface needs blending.
    pub fn is_transparent(&self) -> bool {
        self.color[3] < 1.0
    }
}

/// A mesh instance.
#[derive(Debug, Clone)]
pub struct Object {
    /// Shared mesh.
    pub mesh: MeshHandle,
    /// Local → world.
    pub transform: Mat4,
    /// Shading.
    pub surface: Surface,
    /// Draw layer.
    pub layer: Layer,
    /// Hidden objects are neither drawn nor picked.
    pub visible: bool,
    /// Application-defined id returned by [`super::Scene::pick`]. `0` = not pickable.
    pub pick_id: u32,
}

impl Object {
    /// A visible, pickable-less object on the `Scene` layer.
    pub fn new(mesh: MeshHandle) -> Self {
        Self {
            mesh,
            transform: Mat4::IDENTITY,
            surface: Surface::default(),
            layer: Layer::Scene,
            visible: true,
            pick_id: 0,
        }
    }

    /// Builder: transform.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    /// Builder: surface.
    pub fn with_surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// Builder: layer.
    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }

    /// Builder: pick id.
    pub fn with_pick_id(mut self, pick_id: u32) -> Self {
        self.pick_id = pick_id;
        self
    }

    /// World-space bounds (the transformed local box).
    pub fn world_aabb(&self) -> Aabb {
        let local = self.mesh.aabb();
        Aabb::from_points(
            local
                .corners()
                .iter()
                .map(|c| self.transform.transform_point3(*c)),
        )
    }
}
