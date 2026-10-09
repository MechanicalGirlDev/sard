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
/// Native CPU/GPU particles, materials, emitters, resident sessions, and couplers.
#[cfg(feature = "mpm")]
pub use tessera_mpm as mpm;
pub use tessera_physics as tessera;
pub use tessera_physics::{articulated_world, articulation, batch, mjcf, urdf};
#[cfg(feature = "mpm")]
mod mpm_render;
#[cfg(feature = "mpm")]
pub use mpm_render::{mpm_particle_data, ParticleRenderError};
pub use tessera_physics::articulated_world::{
    ArticulatedWorldError as PhysicsError, ArticulatedWorldParams as PhysicsConfig, SceneCollider,
};
#[cfg(feature = "gpu-physics")]
pub use tessera_physics::gpu_contact_pipeline::GpuContactDevice;

/// Native GPU-resident bodies and environments, independent of the ECS bridge.
///
/// Stepping retains state on Tessera's device. Read back explicitly and call
/// [`publish_poses`] to update visual entities; do not attach `RigidBody` copies.
#[cfg(feature = "gpu-physics")]
pub mod gpu {
    pub use tessera_physics::{
        gpu_point_query as point_query, gpu_ray_query as ray_query, gpu_rigid_ball_joint as joints,
        gpu_rigid_shape as shape, gpu_rigid_sphere_world as rigid, gpu_rigid_state as state,
    };
}

/// A visual binding cannot accept the supplied authoritative pose.
#[derive(Debug, thiserror::Error)]
pub enum PosePublicationError {
    #[error("visual entity {0:?} does not exist")]
    MissingEntity(hecs::Entity),
    #[error("visual entity {0:?} has a competing rigid-body owner")]
    CompetingBody(hecs::Entity),
    #[error("visual entity {0:?} has an ECS parent, but physics poses are world-space")]
    ParentedEntity(hecs::Entity),
    #[error("visual entity {0:?} has no rendering transform")]
    MissingTransform(hecs::Entity),
    #[error("pose for visual entity {0:?} cannot be represented by rendering transforms")]
    InvalidPose(hecs::Entity),
}

/// Publish authoritative native world poses without stepping or editing physics.
///
/// Visual bindings must have a rendering transform, no parent, and no
/// `RigidBody` component. The topology owner supplies the current entity/pose
/// pairs after native mutation or GPU readback. Validation precedes all writes.
///
/// # Errors
///
/// Returns [`PosePublicationError`] for stale entities, conflicting ownership,
/// parented entities, missing transforms, or non-finite rendering poses.
pub fn publish_poses(
    world: &mut hecs::World,
    poses: &[(hecs::Entity, Isometry3<f64>)],
) -> Result<(), PosePublicationError> {
    for &(entity, pose) in poses {
        if !world.contains(entity) {
            return Err(PosePublicationError::MissingEntity(entity));
        }
        if world.get::<&RigidBody>(entity).is_ok() {
            return Err(PosePublicationError::CompetingBody(entity));
        }
        if world
            .get::<&crate::ecs::components::transform::Parent>(entity)
            .is_ok()
        {
            return Err(PosePublicationError::ParentedEntity(entity));
        }
        if world.get::<&Transform>(entity).is_err()
            && world.get::<&GlobalTransform>(entity).is_err()
        {
            return Err(PosePublicationError::MissingTransform(entity));
        }
        let (position, rotation) = rendering_pose(pose);
        if !position.is_finite() || !rotation.is_finite() {
            return Err(PosePublicationError::InvalidPose(entity));
        }
    }
    for &(entity, pose) in poses {
        let mut query =
            world.query_one::<(Option<&mut Transform>, Option<&mut GlobalTransform>)>(entity);
        let (transform, global) = query
            .get()
            .map_err(|_| PosePublicationError::MissingEntity(entity))?;
        publish_pose(pose, transform, global);
    }
    Ok(())
}

/// Publish an explicitly read-back native resident GPU snapshot.
///
/// Bindings are supplied by the owner after any body/environment index remap.
/// This performs no GPU submission, simulation step, or ECS-to-physics upload.
#[cfg(feature = "gpu-physics")]
pub fn publish_gpu_poses(
    world: &mut hecs::World,
    states: &[(hecs::Entity, gpu::state::GpuRigidBodyState)],
) -> Result<(), PosePublicationError> {
    let poses = states
        .iter()
        .map(|(entity, state)| {
            if !state.is_valid() {
                return Err(PosePublicationError::InvalidPose(*entity));
            }
            let [x, y, z, _] = state.position_inverse_mass;
            let [qx, qy, qz, qw] = state.orientation;
            let pose = Isometry3::from_parts(
                nalgebra::Translation3::new(f64::from(x), f64::from(y), f64::from(z)),
                nalgebra::UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
                    f64::from(qw),
                    f64::from(qx),
                    f64::from(qy),
                    f64::from(qz),
                )),
            );
            Ok((*entity, pose))
        })
        .collect::<Result<Vec<_>, _>>()?;
    publish_poses(world, &poses)
}

fn rendering_pose(pose: Isometry3<f64>) -> (glam::Vec3, glam::Quat) {
    let position = DVec3::new(pose.translation.x, pose.translation.y, pose.translation.z).as_vec3();
    let quaternion = pose.rotation.quaternion();
    let rotation =
        DQuat::from_xyzw(quaternion.i, quaternion.j, quaternion.k, quaternion.w).as_quat();
    (position, rotation)
}

fn publish_pose(
    pose: Isometry3<f64>,
    transform: Option<&mut Transform>,
    global: Option<&mut GlobalTransform>,
) {
    let (position, rotation) = rendering_pose(pose);
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
        Self::from_tessera(ArticulatedWorld::new(
            root,
            Isometry3::identity(),
            vec![],
            config,
        )?)
    }

    /// Attach an articulated robot world to ECS-owned free scene bodies.
    ///
    /// URDF/MJCF worlds retain their native links, drives, and joint state.
    /// Keep loader metadata separately. Existing native free bodies cannot be
    /// adopted because this bridge owns its scene-body/entity correspondence.
    ///
    /// # Errors
    ///
    /// Returns [`PhysicsError::InvalidInput`] if scene bodies already exist.
    pub fn from_tessera(tessera: ArticulatedWorld) -> Result<Self, PhysicsError> {
        if !tessera.scene_bodies.is_empty() {
            return Err(PhysicsError::InvalidInput);
        }
        Ok(Self {
            tessera,
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
        self.step_with_efforts(world, delta_time, &[])
    }

    /// Step a robot and ECS free bodies together with native generalized efforts.
    ///
    /// The effort slice must contain exactly `articulation.dof()` values.
    /// Link poses are available through [`Self::tessera_world`] and can be
    /// published to separate visual entities with [`publish_poses`].
    pub fn step_with_efforts(
        &mut self,
        world: &mut hecs::World,
        delta_time: f64,
        efforts: &[f64],
    ) -> Result<(), PhysicsError> {
        let slots = self.upload(world)?;
        self.tessera.step(delta_time, efforts)?;
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
        self.step_gpu_with_efforts(world, delta_time, &[], device)
    }

    /// Use native GPU contacts for a robot and ECS bodies with explicit efforts.
    #[cfg(feature = "gpu-physics")]
    pub fn step_gpu_with_efforts(
        &mut self,
        world: &mut hecs::World,
        delta_time: f64,
        efforts: &[f64],
        device: &GpuContactDevice,
    ) -> Result<(), PhysicsError> {
        let slots = self.upload(world)?;
        self.tessera.step_gpu(delta_time, efforts, device)?;
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
                    // Motion setters handled cache invalidation above. Forward
                    // remaining native state (including persistent loads), only
                    // cloning geometry when an external edit actually differs.
                    let previous = &mut self.tessera.scene_bodies[index];
                    if *previous != *body {
                        previous.clone_from(body);
                    }
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
            publish_pose(solved.pose, transform, global);
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
    fn authoritative_pose_publication_preserves_visual_scale() {
        // Given: a visual entity without a competing physics owner.
        let mut world = hecs::World::new();
        let entity = world.spawn((
            Transform {
                scale: Vec3::new(2.0, 3.0, 4.0),
                ..Transform::default()
            },
            GlobalTransform::default(),
        ));
        let pose = Isometry3::new(Vector3::new(1.0, 2.0, 3.0), Vector3::z() * 0.5);

        // When: a native owner publishes its authoritative world pose.
        publish_poses(&mut world, &[(entity, pose)]).unwrap();

        // Then: rendering receives translation/rotation without losing geometry scale.
        let transform = world.get::<&Transform>(entity).unwrap();
        assert!(transform
            .position
            .abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-6));
        assert!(transform
            .rotation
            .abs_diff_eq(Quat::from_rotation_z(0.5), 1e-6));
        assert_eq!(transform.scale, Vec3::new(2.0, 3.0, 4.0));
        assert!(world
            .get::<&GlobalTransform>(entity)
            .unwrap()
            .0
            .abs_diff_eq(transform.to_matrix(), 1e-6));
    }

    #[test]
    fn pose_publication_rejects_competing_owners_before_any_write() {
        // Given: one visual binding followed by an ECS-owned native body.
        let mut world = hecs::World::new();
        let visual = world.spawn((Transform::default(),));
        let body = world.spawn((sphere(Vector3::new(0.0, 0.0, 3.0)), Transform::default()));
        let pose = Isometry3::translation(4.0, 5.0, 6.0);

        // When: publication tries to introduce a second owner for that body.
        let result = publish_poses(&mut world, &[(visual, pose), (body, pose)]);

        // Then: the complete publication is rejected, including the earlier visual.
        assert!(matches!(result, Err(PosePublicationError::CompetingBody(e)) if e == body));
        assert_eq!(
            world.get::<&Transform>(visual).unwrap().position,
            Vec3::ZERO
        );
    }

    #[test]
    fn bridge_rejects_unbound_native_scene_bodies() {
        // Given: a native owner with a scene body but no corresponding ECS entity.
        let mut native = PhysicsWorld::new(PhysicsConfig::default()).unwrap().tessera;
        native.add_scene_body(sphere(Vector3::new(0.0, 0.0, 3.0)));

        // When: that owner is passed to the ECS bridge.
        let result = PhysicsWorld::from_tessera(native);

        // Then: native dense slots cannot be mistaken for ECS bindings.
        assert!(matches!(result, Err(PhysicsError::InvalidInput)));
    }

    #[test]
    fn robot_efforts_and_ecs_contacts_match_direct_native_world() {
        // Given: identical loaded robots beside identical free bodies.
        let xml = r#"<robot name="slider">
          <link name="base"/>
          <link name="slider">
            <inertial><mass value="1"/><inertia ixx="1" ixy="0" ixz="0" iyy="1" iyz="0" izz="1"/></inertial>
            <collision><geometry><sphere radius="0.5"/></geometry></collision>
          </link>
          <joint name="slide" type="prismatic">
            <parent link="base"/><child link="slider"/><origin xyz="0 0 3"/>
            <axis xyz="1 0 0"/><limit lower="-2" upper="2" effort="10" velocity="10"/>
          </joint>
        </robot>"#;
        let options = urdf::UrdfLoadOptions {
            world: PhysicsConfig {
                gravity: [0.0; 3],
                max_substep: 0.005,
                ..PhysicsConfig::default()
            },
            ..urdf::UrdfLoadOptions::default()
        };
        let loaded = urdf::load_urdf_str(xml, options.clone()).unwrap();
        let mut direct = urdf::load_urdf_str(xml, options).unwrap().world;
        let mut physics = PhysicsWorld::from_tessera(loaded.world).unwrap();
        let mut world = hecs::World::new();
        let body = sphere(Vector3::new(0.75, 0.0, 3.0));
        let entity = world.spawn((body.clone(), Transform::default()));
        direct.add_scene_body(body);
        let visual = world.spawn((Transform::default(),));

        // When: the robot and ECS free body share contacts and nonzero efforts.
        for _ in 0..40 {
            physics
                .step_with_efforts(&mut world, 0.005, &[2.0])
                .unwrap();
            direct.step(0.005, &[2.0]).unwrap();
        }
        let link_pose = physics.tessera_world().link_poses().unwrap()[1];
        publish_poses(&mut world, &[(visual, link_pose)]).unwrap();

        // Then: both generalized and free-body states match actual native dynamics.
        assert_eq!(physics.tessera_world().positions, direct.positions);
        assert_eq!(physics.tessera_world().velocities, direct.velocities);
        assert!(physics.tessera_world().positions[0].abs() > 1e-5);
        assert_eq!(
            *world.get::<&RigidBody>(entity).unwrap(),
            direct.scene_bodies[0]
        );
        assert!(world.get::<&RigidBody>(entity).unwrap().pose.translation.x > 0.75);
        assert!(
            (f64::from(world.get::<&Transform>(visual).unwrap().position.x)
                - link_pose.translation.x)
                .abs()
                < 1e-6
        );
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
    fn resident_primitive_environments_publish_and_reset_independently() {
        // Given: overlapping world coordinates in two native GPU environments.
        use gpu::rigid::{
            GpuRigidPrimitiveBatch, GpuRigidPrimitiveEnvironment, GpuRigidSphereWorldConfig,
        };
        use gpu::shape::GpuRigidShape;
        use gpu::state::GpuRigidBodyState;
        let device = GpuContactDevice::new().unwrap();
        let stationary = GpuRigidBodyState {
            position_inverse_mass: [0.0, 0.0, 3.0, 1.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [0.0; 4],
            angular_velocity: [0.0; 4],
            inverse_inertia_sleep: [1.0, 1.0, 1.0, 0.0],
        };
        let moving = GpuRigidBodyState {
            linear_velocity: [1.0, 0.0, 0.0, 0.0],
            ..stationary
        };
        let first = [stationary];
        let second = [moving];
        let shape = [GpuRigidShape::Sphere { radius: 0.2 }];
        let mut batch = GpuRigidPrimitiveBatch::new_primitives(
            device.device(),
            device.queue(),
            &[
                GpuRigidPrimitiveEnvironment {
                    states: &first,
                    shapes: &shape,
                },
                GpuRigidPrimitiveEnvironment {
                    states: &second,
                    shapes: &shape,
                },
            ],
            GpuRigidSphereWorldConfig {
                gravity: [0.0; 3],
                ground_half_extent: None,
                ..GpuRigidSphereWorldConfig::default()
            },
        )
        .unwrap();
        let mut world = hecs::World::new();
        let first_visual = world.spawn((Transform::default(),));
        let second_visual = world.spawn((Transform::default(), GlobalTransform::default()));

        // When: resident stepping runs without ECS uploads, followed by explicit readback.
        for _ in 0..10 {
            let _contacts = batch.step(0.01).unwrap();
        }
        publish_gpu_poses(
            &mut world,
            &[
                (first_visual, batch.readback_environment(0).unwrap()[0]),
                (second_visual, batch.readback_environment(1).unwrap()[0]),
            ],
        )
        .unwrap();
        batch.reset_environment(0, &first).unwrap();

        // Then: overlap across environments never becomes contact or a shared reset.
        assert!(
            world
                .get::<&Transform>(first_visual)
                .unwrap()
                .position
                .x
                .abs()
                < 1e-6
        );
        assert!((world.get::<&Transform>(second_visual).unwrap().position.x - 0.1).abs() < 1e-6);
        assert!(
            (batch.readback_environment(1).unwrap()[0].position_inverse_mass[0] - 0.1).abs() < 1e-6
        );
        assert!(world
            .get::<&GlobalTransform>(second_visual)
            .unwrap()
            .0
            .abs_diff_eq(
                world.get::<&Transform>(second_visual).unwrap().to_matrix(),
                1e-6
            ));
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
