//! Retained scene description: objects, lines, points, labels, grid and lighting.
//!
//! The scene is plain data. [`super::SceneRenderer`] draws it; [`Scene::pick`] ray-casts it.

use glam::{Mat4, Vec3};

use crate::core::vertex::VertexPC;
use crate::renderer::geometry::Aabb;

use super::mesh::MeshHandle;
use super::pick::Ray;

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
    /// Application-defined id returned by [`Scene::pick`]. `0` = not pickable.
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

/// Handle to an object inside a [`Scene`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(u32);

/// A batch of line segments (or a polyline) with per-vertex colour.
#[derive(Debug, Clone)]
pub struct LineSet {
    /// Vertices in local space. Pairs for `strip == false`, consecutive for a strip.
    pub vertices: Vec<VertexPC>,
    /// Local → world.
    pub transform: Mat4,
    /// Draw layer.
    pub layer: Layer,
    /// Hidden sets are skipped.
    pub visible: bool,
    /// Treat `vertices` as a polyline instead of independent segments.
    pub strip: bool,
}

impl LineSet {
    /// Independent segments (`vertices` in pairs).
    pub fn segments(vertices: Vec<VertexPC>) -> Self {
        Self {
            vertices,
            transform: Mat4::IDENTITY,
            layer: Layer::Scene,
            visible: true,
            strip: false,
        }
    }

    /// A polyline through `points` in one colour.
    pub fn strip(points: &[Vec3], color: [f32; 4]) -> Self {
        Self {
            vertices: points
                .iter()
                .map(|p| VertexPC::new(p.to_array(), color))
                .collect(),
            transform: Mat4::IDENTITY,
            layer: Layer::Scene,
            visible: true,
            strip: true,
        }
    }

    /// Builder: transform.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    /// Builder: layer.
    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }

    /// Expand into world-space segment vertices (two per segment).
    pub(crate) fn world_segments(&self) -> Vec<VertexPC> {
        let map = |v: &VertexPC| {
            VertexPC::new(
                self.transform
                    .transform_point3(Vec3::from(v.position))
                    .to_array(),
                v.color,
            )
        };
        if self.strip {
            self.vertices
                .windows(2)
                .flat_map(|w| [map(&w[0]), map(&w[1])])
                .collect()
        } else {
            let n = self.vertices.len() & !1;
            self.vertices[..n].iter().map(map).collect()
        }
    }
}

/// One camera-facing point sprite.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointInstance {
    /// Centre.
    pub position: [f32; 3],
    /// Diameter in world units.
    pub size: f32,
    /// RGBA.
    pub color: [f32; 4],
}

/// A batch of point sprites.
#[derive(Debug, Clone)]
pub struct PointSet {
    /// Points in local space.
    pub points: Vec<PointInstance>,
    /// Local → world.
    pub transform: Mat4,
    /// Draw layer.
    pub layer: Layer,
    /// Hidden sets are skipped.
    pub visible: bool,
}

impl PointSet {
    /// Points of one size and colour.
    pub fn uniform(positions: &[Vec3], size: f32, color: [f32; 4]) -> Self {
        Self {
            points: positions
                .iter()
                .map(|p| PointInstance {
                    position: p.to_array(),
                    size,
                    color,
                })
                .collect(),
            transform: Mat4::IDENTITY,
            layer: Layer::Scene,
            visible: true,
        }
    }

    /// Builder: transform.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub(crate) fn world_points(&self) -> Vec<PointInstance> {
        let scale = self
            .transform
            .to_scale_rotation_translation()
            .0
            .max_element();
        self.points
            .iter()
            .map(|p| PointInstance {
                position: self
                    .transform
                    .transform_point3(Vec3::from(p.position))
                    .to_array(),
                size: p.size * scale,
                color: p.color,
            })
            .collect()
    }
}

/// A text label anchored to a world position (drawn as a screen-space billboard).
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// Anchor in world space.
    pub position: Vec3,
    /// Text.
    pub text: String,
    /// Height of one text line in world units (perspective-scaled). `0` = fixed pixel size.
    pub height: f32,
    /// Pixel size used when `height == 0`.
    pub pixel_size: f32,
    /// RGBA.
    pub color: [f32; 4],
}

impl Label {
    /// A label `height` world units tall.
    pub fn new(position: Vec3, text: impl Into<String>, height: f32, color: [f32; 4]) -> Self {
        Self {
            position,
            text: text.into(),
            height,
            pixel_size: 14.0,
            color,
        }
    }
}

/// A label after projection to the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenLabel {
    /// Pixel x of the anchor (from the left).
    pub x: f32,
    /// Pixel y of the anchor (from the top).
    pub y: f32,
    /// Text height in pixels.
    pub size: f32,
    /// Text.
    pub text: String,
    /// RGBA.
    pub color: [f32; 4],
    /// Normalized depth (0 near … 1 far), for sorting.
    pub depth: f32,
}

/// Scene lighting: one directional "sun" plus a hemisphere ambient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// Direction the light travels **towards** (need not be normalized).
    pub sun_direction: Vec3,
    /// Sun colour × intensity.
    pub sun_color: [f32; 3],
    /// Ambient from the `up` hemisphere.
    pub sky: [f32; 3],
    /// Ambient from the opposite hemisphere.
    pub ground: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(-0.5, -0.3, -1.0),
            sun_color: [0.8; 3],
            sky: [0.55; 3],
            ground: [0.25; 3],
        }
    }
}

/// Ground grid on the plane perpendicular to the scene's `up` axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridParams {
    /// Offset of the plane along `up`.
    pub height: f32,
    /// The grid covers a square of ±`half_extent` (rounded up to a multiple of `step`).
    pub half_extent: f32,
    /// Cell size.
    pub step: f32,
    /// Every n-th line is a major line (`0` / `1` = all major).
    pub major_every: u32,
    /// Minor line colour.
    pub minor_color: [f32; 4],
    /// Major line colour.
    pub major_color: [f32; 4],
    /// Colour of the line along the first in-plane axis (world X for a Z-up scene).
    pub axis_u_color: [f32; 4],
    /// Colour of the line along the second in-plane axis (world Y for a Z-up scene).
    pub axis_v_color: [f32; 4],
}

impl Default for GridParams {
    fn default() -> Self {
        Self {
            height: 0.0,
            half_extent: 50.0,
            step: 1.0,
            major_every: 10,
            minor_color: [0.35, 0.35, 0.35, 0.5],
            major_color: [0.55, 0.55, 0.55, 0.8],
            axis_u_color: [0.9, 0.25, 0.25, 0.9],
            axis_v_color: [0.3, 0.85, 0.3, 0.9],
        }
    }
}

/// A hit reported by [`Scene::pick`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The object hit.
    pub object: ObjectId,
    /// Its `pick_id`.
    pub pick_id: u32,
    /// Distance along the ray.
    pub distance: f32,
    /// World-space hit point.
    pub point: Vec3,
}

/// Everything drawn in one frame.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    objects: Vec<Option<Object>>,
    free: Vec<u32>,
    /// Line batches.
    pub lines: Vec<LineSet>,
    /// Point batches.
    pub points: Vec<PointSet>,
    /// World-anchored text.
    pub labels: Vec<Label>,
    /// Ground grid, if any.
    pub grid: Option<GridParams>,
    /// Lights.
    pub lighting: Lighting,
    /// World up axis (hemisphere light and grid plane). Defaults to +Y like the rest of
    /// sard; a robotics scene sets +Z.
    pub up: Vec3,
    /// Background colour, linear RGBA.
    pub background: [f32; 4],
}

impl Scene {
    /// An empty Y-up scene.
    pub fn new() -> Self {
        Self {
            up: Vec3::Y,
            background: [0.12, 0.12, 0.13, 1.0],
            ..Default::default()
        }
    }

    /// An empty Z-up scene (URDF / ROS convention).
    pub fn new_z_up() -> Self {
        Self {
            up: Vec3::Z,
            lighting: Lighting {
                sun_direction: Vec3::new(-0.5, 0.3, -1.0),
                ..Lighting::default()
            },
            ..Self::new()
        }
    }

    /// Add an object.
    pub fn add(&mut self, object: Object) -> ObjectId {
        if let Some(i) = self.free.pop() {
            self.objects[i as usize] = Some(object);
            ObjectId(i)
        } else {
            self.objects.push(Some(object));
            ObjectId(self.objects.len() as u32 - 1)
        }
    }

    /// Remove an object (a stale id is ignored).
    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
        let slot = self.objects.get_mut(id.0 as usize)?;
        let obj = slot.take()?;
        self.free.push(id.0);
        Some(obj)
    }

    /// Borrow an object.
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(id.0 as usize).and_then(Option::as_ref)
    }

    /// Mutably borrow an object.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id.0 as usize).and_then(Option::as_mut)
    }

    /// Iterate live objects.
    pub fn objects(&self) -> impl Iterator<Item = (ObjectId, &Object)> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.as_ref().map(|o| (ObjectId(i as u32), o)))
    }

    /// Iterate live objects mutably.
    pub fn objects_mut(&mut self) -> impl Iterator<Item = (ObjectId, &mut Object)> {
        self.objects
            .iter_mut()
            .enumerate()
            .filter_map(|(i, o)| o.as_mut().map(|o| (ObjectId(i as u32), o)))
    }

    /// Number of live objects.
    pub fn len(&self) -> usize {
        self.objects.iter().filter(|o| o.is_some()).count()
    }

    /// Whether there are no objects.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove every object, line, point and label (grid / lighting / up are kept).
    pub fn clear(&mut self) {
        self.objects.clear();
        self.free.clear();
        self.lines.clear();
        self.points.clear();
        self.labels.clear();
    }

    /// Bounds of all visible objects on the `Scene` layer, if any.
    pub fn bounds(&self) -> Option<Aabb> {
        self.objects()
            .filter(|(_, o)| o.visible && o.layer == Layer::Scene)
            .map(|(_, o)| o.world_aabb())
            .reduce(|a, b| a.merge(&b))
    }

    /// Nearest visible object with a non-zero `pick_id` along `ray`. Overlay objects are
    /// tested first and win over scene objects regardless of distance (they are drawn on
    /// top, so that is what the user sees under the cursor).
    pub fn pick(&self, ray: &Ray) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        for layer in [Layer::Overlay, Layer::Scene, Layer::Ground] {
            for (id, o) in self.objects() {
                if !o.visible || o.pick_id == 0 || o.layer != layer {
                    continue;
                }
                if ray.hit_aabb(&o.world_aabb()).is_none() {
                    continue;
                }
                let Some(t) = ray.to_local(o.transform).hit_mesh(o.mesh.data()) else {
                    continue;
                };
                if best.is_none_or(|b| t < b.distance) {
                    best = Some(Hit {
                        object: id,
                        pick_id: o.pick_id,
                        distance: t,
                        point: ray.at(t),
                    });
                }
            }
            if best.is_some() {
                return best;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_expansion() {
        let strip = LineSet::strip(&[Vec3::ZERO, Vec3::X, Vec3::Y], [1.0; 4])
            .with_transform(Mat4::from_translation(Vec3::Z));
        let seg = strip.world_segments();
        assert_eq!(seg.len(), 4);
        assert_eq!(seg[1].position, [1.0, 0.0, 1.0]);
        assert_eq!(seg[2].position, [1.0, 0.0, 1.0]);

        let odd = LineSet::segments(vec![VertexPC::new([0.0; 3], [1.0; 4]); 3]);
        assert_eq!(odd.world_segments().len(), 2);
    }

    /// Slot reuse and picking need a device-backed mesh; see `scene::tests`.
    #[test]
    fn empty_scene_answers_safely() {
        let mut s = Scene::new();
        assert!(s.is_empty());
        assert!(s.remove(ObjectId(3)).is_none());
        assert!(s.get(ObjectId(0)).is_none());
        assert!(s.bounds().is_none());
        assert!(s.pick(&Ray::new(Vec3::ZERO, Vec3::X)).is_none());
    }
}
