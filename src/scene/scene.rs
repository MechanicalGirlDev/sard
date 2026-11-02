//! Retained scene description: objects, lines, points, labels, grid and lighting.
//!
//! The scene is plain data. [`super::SceneRenderer`] draws it; [`Scene::pick`] ray-casts it.

use glam::Vec3;

use crate::renderer::geometry::Aabb;

pub use super::lines::LineSet;
pub use super::object::{Layer, Object, Surface};
use super::pick::Ray;
pub use super::primitives::{GridParams, Label, Lighting, PointInstance, PointSet, ScreenLabel};

/// Handle to an object inside a [`Scene`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(u32);

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
