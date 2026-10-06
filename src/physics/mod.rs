//! Tessera physics with an ECS-to-rendering bridge.
//!
//! Tessera owns collision detection, contact solving, integration, and sleeping.
//! Bodies use its native Z-up coordinates and collider types. The optional GPU
//! path uses Tessera's own compute device, independently of Sard's render device.

use std::collections::HashMap;

use glam::{DQuat, DVec3, Mat4};
use nalgebra::{Isometry3, Matrix3, Vector3};
use tessera_physics::articulated_world::ArticulatedWorld;
use tessera_physics::articulation::{Articulation, LinkSpec};

use crate::ecs::components::physics::{ColliderMaterial, RigidBody};
use crate::ecs::components::transform::{GlobalTransform, Transform};

pub use nalgebra;
pub use tessera_physics as tessera;
pub use tessera_physics::articulated_world::{
    ArticulatedWorldError as PhysicsError, ArticulatedWorldParams as PhysicsConfig, SceneCollider,
};
#[cfg(feature = "gpu-physics")]
pub use tessera_physics::gpu_contact_pipeline::GpuContactDevice;

/// Synchronizes native Tessera bodies with one ECS world.
///
/// Rigid-body poses are authoritative. `Transform` and `GlobalTransform` are
/// optional rendering outputs, in world space; physics entities should not have
/// an ECS parent. Shape, mass, or inertia edits recreate the corresponding
/// Tessera body, while ordinary stepping preserves its contact and sleep state.
#[derive(Debug)]
pub struct PhysicsWorld {
    tessera: ArticulatedWorld,
    entities: Vec<hecs::Entity>,
}

impl PhysicsWorld {
    /// Create a Tessera scene with a fixed inertial root and no robot joints.
    ///
    /// Native configuration includes a finite ground plane at Z = 0.
    pub fn new(config: PhysicsConfig) -> Result<Self, PhysicsError> {
        let root = Articulation::new(
            vec![LinkSpec {
                mass: 0.0,
                center_of_mass: Vector3::zeros(),
                inertia: Matrix3::zeros(),
            }],
            vec![],
            0,
        )?;
        Ok(Self {
            tessera: ArticulatedWorld::new(root, Isometry3::identity(), vec![], config)?,
            entities: Vec::new(),
        })
    }

    /// Inspect Tessera's world, including native contact and sleep diagnostics.
    pub const fn tessera_world(&self) -> &ArticulatedWorld {
        &self.tessera
    }

    /// Advance native CPU physics by a positive duration, then publish its poses.
    ///
    /// Tessera owns timestep subdivision through `PhysicsConfig::max_substep`.
    pub fn step(&mut self, world: &mut hecs::World, delta_time: f64) -> Result<(), PhysicsError> {
        let slots = self.upload(world)?;
        self.tessera.step(delta_time, &[])?;
        self.download(world, &slots);
        Ok(())
    }

    /// Use Tessera's GPU contact detection and solving, then publish its poses.
    ///
    /// GPU initialization and computation errors propagate; there is no Sard
    /// solver or automatic switch to a CPU implementation.
    #[cfg(feature = "gpu-physics")]
    pub fn step_gpu(
        &mut self,
        world: &mut hecs::World,
        delta_time: f64,
        device: &GpuContactDevice,
    ) -> Result<(), PhysicsError> {
        let slots = self.upload(world)?;
        self.tessera.step_gpu(delta_time, &[], device)?;
        self.download(world, &slots);
        Ok(())
    }

    fn upload(
        &mut self,
        world: &hecs::World,
    ) -> Result<HashMap<hecs::Entity, usize>, PhysicsError> {
        // Tessera removal shifts dense slots. Remove in reverse order, and use
        // its API so material, sleep, and contact side tables move with bodies.
        for index in (0..self.entities.len()).rev() {
            let keep = world
                .get::<&RigidBody>(self.entities[index])
                .is_ok_and(|body| {
                    let previous = &self.tessera.scene_bodies[index];
                    body.mass.to_bits() == previous.mass.to_bits()
                        && body.inertia == previous.inertia
                        && body.colliders == previous.colliders
                        && body.kinematic == previous.kinematic
                });
            if !keep {
                self.tessera.remove_scene_body(index);
                self.entities.remove(index);
            }
        }

        let mut slots: HashMap<_, _> = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (*entity, index))
            .collect();
        let default_material = ColliderMaterial::new(
            self.tessera.params().friction,
            self.tessera.params().restitution,
        );
        for (entity, body, material) in
            &mut world.query::<(hecs::Entity, &RigidBody, Option<&ColliderMaterial>)>()
        {
            let index = match slots.entry(entity) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let index = *entry.get();
                    if body.pose != self.tessera.scene_bodies[index].pose {
                        self.tessera.set_scene_body_pose(index, body.pose)?;
                    }
                    let previous = &self.tessera.scene_bodies[index];
                    if body.linear_velocity != previous.linear_velocity
                        || body.angular_velocity != previous.angular_velocity
                    {
                        if body.kinematic {
                            self.tessera.set_scene_body_kinematic_motion(
                                index,
                                Some((body.linear_velocity, body.angular_velocity)),
                            )?;
                        } else {
                            self.tessera.set_scene_body_velocity(
                                index,
                                body.linear_velocity,
                                body.angular_velocity,
                            )?;
                        }
                    }
                    // Native scene force is persistent; Tessera also owns its
                    // validation and automatic wake behavior.
                    self.tessera.scene_bodies[index].force = body.force;
                    index
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let index = self.tessera.add_scene_body(body.clone());
                    self.entities.push(entity);
                    entry.insert(index);
                    index
                }
            };
            let material = material.copied().unwrap_or(default_material);
            for collider in 0..body.colliders.len() {
                if self.tessera.scene_collider_material(index, collider) != Some(material) {
                    self.tessera
                        .set_scene_collider_material(index, collider, material)?;
                }
            }
        }
        Ok(slots)
    }

    fn download(&self, world: &mut hecs::World, slots: &HashMap<hecs::Entity, usize>) {
        for (entity, body, transform, global) in world.query_mut::<(
            hecs::Entity,
            &mut RigidBody,
            Option<&mut Transform>,
            Option<&mut GlobalTransform>,
        )>() {
            let solved = &self.tessera.scene_bodies[slots[&entity]];
            body.pose = solved.pose;
            body.linear_velocity = solved.linear_velocity;
            body.angular_velocity = solved.angular_velocity;
            let position = DVec3::new(
                solved.pose.translation.x,
                solved.pose.translation.y,
                solved.pose.translation.z,
            )
            .as_vec3();
            let quaternion = solved.pose.rotation.quaternion();
            let rotation =
                DQuat::from_xyzw(quaternion.i, quaternion.j, quaternion.k, quaternion.w).as_quat();
            let matrix = if let Some(transform) = transform {
                transform.position = position;
                transform.rotation = rotation;
                transform.to_matrix()
            } else {
                Mat4::from_rotation_translation(rotation, position)
            };
            if let Some(global) = global {
                global.0 = matrix;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn sphere(position: Vector3<f64>) -> RigidBody {
        RigidBody::new(
            Isometry3::translation(position.x, position.y, position.z),
            1.0,
            Matrix3::identity(),
            vec![SceneCollider::Sphere {
                center: Vector3::zeros(),
                radius: 0.5,
            }],
        )
        .unwrap()
    }

    #[test]
    fn test_physics_world_free_fall() {
        // Given: a native Tessera body and rendering transforms with scale.
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig::default()).unwrap();
        let entity = world.spawn((
            sphere(Vector3::new(0.0, 0.0, 10.0)),
            Transform {
                scale: Vec3::splat(2.0),
                ..Transform::default()
            },
            GlobalTransform::default(),
        ));

        // When: Tessera advances one second.
        for _ in 0..60 {
            physics.step(&mut world, 1.0 / 60.0).unwrap();
        }

        // Then: solved motion reaches both rendering components without changing scale.
        let body = world.get::<&RigidBody>(entity).unwrap();
        let transform = world.get::<&Transform>(entity).unwrap();
        let global = world.get::<&GlobalTransform>(entity).unwrap();
        assert!(body.pose.translation.z < 6.0);
        assert!(body.pose.translation.z > 4.0);
        assert!(body.linear_velocity.z < -9.0);
        assert!((f64::from(transform.position.z) - body.pose.translation.z).abs() < 1e-5);
        assert_eq!(transform.scale, Vec3::splat(2.0));
        assert_eq!(global.0, transform.to_matrix());
    }

    #[test]
    fn test_physics_world_collision() {
        // Given: a box above an elevated static box, not the implicit ground.
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig {
            max_substep: 1.0 / 120.0,
            ..PhysicsConfig::default()
        })
        .unwrap();
        let box_shape = |half_extents| SceneCollider::Box {
            origin: Isometry3::identity(),
            half_extents,
        };
        let dynamic = world.spawn((RigidBody::new(
            Isometry3::translation(0.0, 0.0, 4.0),
            1.0,
            Matrix3::identity(),
            vec![box_shape(Vector3::repeat(0.5))],
        )
        .unwrap(),));
        let ground = world.spawn((RigidBody::new(
            Isometry3::translation(0.0, 0.0, 1.0),
            0.0,
            Matrix3::zeros(),
            vec![box_shape(Vector3::new(5.0, 5.0, 0.5))],
        )
        .unwrap(),));

        // When: the native collision solver advances three seconds.
        for _ in 0..180 {
            physics.step(&mut world, 1.0 / 60.0).unwrap();
        }

        // Then: the box rests on the explicit static body, which remains still.
        let body = world.get::<&RigidBody>(dynamic).unwrap();
        assert!((body.pose.translation.z - 2.0).abs() < 0.1);
        assert!(body.linear_velocity.norm() < 0.2);
        assert_eq!(
            world.get::<&RigidBody>(ground).unwrap().pose.translation.z,
            1.0
        );
    }

    #[test]
    fn test_physics_config_default() {
        let config = PhysicsConfig::default();
        assert_eq!(config.gravity, [0.0, 0.0, -9.81]);
        assert!((config.max_substep - 0.001).abs() < f64::EPSILON);
        assert_eq!(config.solver_iterations, 12);
        assert!(config.sleep.enabled);
    }

    #[test]
    fn bridge_matches_direct_tessera_steps() {
        // Given: identical native inputs in a direct Tessera world and the bridge.
        let mut world = hecs::World::new();
        let config = PhysicsConfig {
            max_substep: 1.0 / 120.0,
            ..PhysicsConfig::default()
        };
        let mut physics = PhysicsWorld::new(config).unwrap();
        let mut direct = PhysicsWorld::new(config).unwrap().tessera;
        let body = sphere(Vector3::new(0.0, 0.0, 2.0));
        let entity = world.spawn((body.clone(),));
        direct.add_scene_body(body);

        // When: both paths receive the same simulation steps.
        for _ in 0..60 {
            physics.step(&mut world, 1.0 / 60.0).unwrap();
            direct.step(1.0 / 60.0, &[]).unwrap();
        }

        // Then: ECS receives Tessera's exact pose and velocities.
        assert_eq!(
            *world.get::<&RigidBody>(entity).unwrap(),
            direct.scene_bodies[0]
        );
    }

    #[test]
    fn despawn_and_shape_edits_preserve_other_entity_bindings() {
        // Given: three distinct bodies, already registered with Tessera.
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig {
            gravity: [0.0; 3],
            ..PhysicsConfig::default()
        })
        .unwrap();
        let removed = world.spawn((sphere(Vector3::new(-4.0, 0.0, 3.0)),));
        let changed = world.spawn((sphere(Vector3::new(0.0, 0.0, 3.0)),));
        let retained = world.spawn((sphere(Vector3::new(4.0, 0.0, 3.0)),));
        physics.step(&mut world, 0.01).unwrap();

        // When: a dense slot is removed and another body's shape is replaced.
        world.despawn(removed).unwrap();
        world.get::<&mut RigidBody>(changed).unwrap().colliders = vec![SceneCollider::Box {
            origin: Isometry3::identity(),
            half_extents: Vector3::repeat(0.5),
        }];
        physics.step(&mut world, 0.01).unwrap();

        // Then: both surviving entities still own the right native bodies.
        assert_eq!(physics.tessera.scene_bodies.len(), 2);
        assert_eq!(
            world
                .get::<&RigidBody>(retained)
                .unwrap()
                .pose
                .translation
                .x,
            4.0
        );
        assert!(matches!(
            world.get::<&RigidBody>(changed).unwrap().colliders[0],
            SceneCollider::Box { .. }
        ));
    }

    #[test]
    fn material_removal_restores_the_native_default() {
        // Given: an entity overriding Tessera's contact material.
        let mut world = hecs::World::new();
        let config = PhysicsConfig::default();
        let mut physics = PhysicsWorld::new(config).unwrap();
        let material = ColliderMaterial::new(0.2, 0.4);
        let entity = world.spawn((sphere(Vector3::new(0.0, 0.0, 3.0)), material));
        physics.step(&mut world, 0.01).unwrap();
        assert_eq!(
            physics.tessera.scene_collider_material(0, 0),
            Some(material)
        );

        // When: the ECS override is removed.
        world.remove_one::<ColliderMaterial>(entity).unwrap();
        physics.step(&mut world, 0.01).unwrap();

        // Then: Tessera's previous override does not leak into later steps.
        assert_eq!(
            physics.tessera.scene_collider_material(0, 0),
            Some(ColliderMaterial::new(config.friction, config.restitution))
        );
    }

    #[test]
    fn externally_changed_velocity_wakes_a_sleeping_native_body() {
        // Given: a sleeping native body away from any contact.
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig {
            gravity: [0.0; 3],
            ..PhysicsConfig::default()
        })
        .unwrap();
        let entity = world.spawn((sphere(Vector3::new(0.0, 0.0, 3.0)),));
        physics.step(&mut world, 0.01).unwrap();
        physics.tessera.sleep_scene_body(0).unwrap();

        // When: ECS requests motion below the automatic wake threshold.
        world
            .get::<&mut RigidBody>(entity)
            .unwrap()
            .linear_velocity
            .x = 0.001;
        physics.step(&mut world, 0.01).unwrap();

        // Then: the native setter wakes the body and its pose advances.
        assert_eq!(physics.tessera.scene_body_is_sleeping(0), Some(false));
        assert!(world.get::<&RigidBody>(entity).unwrap().pose.translation.x > 0.0);
    }

    #[test]
    fn native_static_velocity_edits_propagate_tessera_error() {
        // Given: a stationary native scene body already registered in Tessera.
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig::default()).unwrap();
        let body = RigidBody::new(
            Isometry3::translation(0.0, 0.0, 3.0),
            0.0,
            Matrix3::zeros(),
            vec![SceneCollider::Sphere {
                center: Vector3::zeros(),
                radius: 0.5,
            }],
        )
        .unwrap();
        let entity = world.spawn((body,));
        physics.step(&mut world, 0.01).unwrap();

        // When: ECS requests dynamic velocity without opting into kinematic motion.
        world
            .get::<&mut RigidBody>(entity)
            .unwrap()
            .linear_velocity
            .x = 1.0;
        let result = physics.step(&mut world, 0.01);

        // Then: native input rejection is returned, not silently discarded.
        assert!(matches!(result, Err(PhysicsError::InvalidInput)));
    }

    #[test]
    fn native_pose_edits_publish_rotation_without_a_local_transform() {
        // Given: a body with only a world rendering transform.
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig {
            gravity: [0.0; 3],
            ..PhysicsConfig::default()
        })
        .unwrap();
        let entity = world.spawn((
            sphere(Vector3::new(0.0, 0.0, 3.0)),
            GlobalTransform::default(),
        ));
        physics.step(&mut world, 0.01).unwrap();

        // When: its native pose is externally changed.
        world.get::<&mut RigidBody>(entity).unwrap().pose =
            Isometry3::new(Vector3::new(2.0, 1.0, 3.0), Vector3::z() * 0.5);
        physics.step(&mut world, 0.01).unwrap();

        // Then: the rendering matrix includes both the new translation and rotation.
        let expected =
            Mat4::from_rotation_translation(Quat::from_rotation_z(0.5), Vec3::new(2.0, 1.0, 3.0));
        assert!(world
            .get::<&GlobalTransform>(entity)
            .unwrap()
            .0
            .abs_diff_eq(expected, 1e-5));
    }

    #[cfg(feature = "gpu-physics")]
    #[test]
    fn tessera_gpu_contacts_support_a_body_against_gravity() {
        // Given: a real Tessera compute device and a sphere above its ground.
        let device = GpuContactDevice::new().unwrap();
        eprintln!("Tessera GPU adapter: {:?}", device.adapter_info());
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig {
            max_substep: 1.0 / 60.0,
            ..PhysicsConfig::default()
        })
        .unwrap();
        let entity = world.spawn((sphere(Vector3::new(0.0, 0.0, 1.0)), Transform::default()));

        // When: real GPU collision detection and solving advance two seconds.
        for _ in 0..120 {
            physics.step_gpu(&mut world, 1.0 / 60.0, &device).unwrap();
        }

        // Then: native contacts stop the fall and the rendering pose follows.
        let body = world.get::<&RigidBody>(entity).unwrap();
        assert!((body.pose.translation.z - 0.5).abs() < 0.1);
        assert!(body.linear_velocity.norm() < 0.2);
        let transform = world.get::<&Transform>(entity).unwrap();
        assert!((f64::from(transform.position.z) - body.pose.translation.z).abs() < 1e-5);
    }
}
