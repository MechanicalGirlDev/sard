//! Physics Demo - Demonstrates physics simulation with falling boxes
//!
//! Run with: cargo run -p physics_demo

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use sard::ecs::components::physics::RigidBody;
use sard::ecs::components::rendering::{
    CameraComponent, FrustumCullable, LightComponent, MaterialHandle, MeshHandle, MeshRenderer,
    Visible,
};
use sard::ecs::components::transform::{GlobalTransform, Transform};
use sard::engine::{run_app, App, GameLoopConfig, SystemContext};
use sard::physics::nalgebra::{Isometry3, Matrix3, Vector3};
use sard::physics::tessera::articulated_world::SceneCollider;
use sard::physics::{PhysicsConfig, PhysicsWorld};
use sard::renderer::light::LightType;
use sard::{Camera, ColorMaterial, Mesh, WgpuContext, WindowSettings};

struct PhysicsApp {
    physics_world: Option<PhysicsWorld>,
    scene_spawned: bool,
}

impl App for PhysicsApp {
    fn init(&mut self, _ctx: &WgpuContext, world: &mut hecs::World) -> anyhow::Result<()> {
        // Create physics world
        self.physics_world = Some(PhysicsWorld::new(PhysicsConfig {
            ground_half_extent: 10.0,
            max_substep: 1.0 / 120.0,
            ..PhysicsConfig::default()
        })?);

        // Camera
        let camera = Camera::new_perspective(
            Vec3::new(5.0, 8.0, 5.0),
            Vec3::ZERO,
            Vec3::Z,
            45.0,
            1.0,
            0.1,
            100.0,
        );
        world.spawn((
            Transform::identity(),
            GlobalTransform::default(),
            CameraComponent {
                camera,
                active: true,
            },
        ));

        // Light
        world.spawn((
            Transform::from_position(Vec3::new(5.0, 5.0, 10.0)),
            GlobalTransform::default(),
            LightComponent {
                light_type: LightType::Directional,
                color: Vec3::ONE,
                intensity: 1.0,
            },
        ));
        Ok(())
    }

    fn update(&mut self, world: &mut hecs::World, ctx: &SystemContext) -> anyhow::Result<()> {
        // Spawn scene on first update (surface_format is available here)
        if !self.scene_spawned {
            // Render Tessera's implicit finite ground at z=0.
            let ground_material = ColorMaterial::new(ctx.ctx, ctx.surface_format)?;
            let ground_mesh = Mesh::quad(ctx.ctx, 20.0, 20.0, [0.4, 0.5, 0.4]);
            let ground_transform = Transform {
                rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                // Quad winding opposes its +Y normal; mirror X so the +Z face is visible.
                scale: Vec3::new(-1.0, 1.0, 1.0),
                ..Transform::identity()
            };

            world.spawn((
                ground_transform,
                GlobalTransform(ground_transform.to_matrix()),
                MeshRenderer {
                    mesh: MeshHandle(Arc::new(ground_mesh)),
                    material: MaterialHandle(Arc::new(ground_material)),
                    visible: true,
                    cast_shadow: false,
                    receive_shadow: true,
                },
                FrustumCullable,
                Visible,
            ));

            // Falling box (dynamic rigid body)
            let box_material = ColorMaterial::new(ctx.ctx, ctx.surface_format)?;
            let box_mesh = Mesh::cube(ctx.ctx, 1.0, [0.8, 0.2, 0.2]);
            let box_pos = Vec3::new(0.0, 0.0, 5.0);
            let body = RigidBody::new(
                Isometry3::translation(0.0, 0.0, 5.0),
                1.0,
                Matrix3::identity() * (1.0 / 6.0),
                vec![SceneCollider::Box {
                    origin: Isometry3::identity(),
                    half_extents: Vector3::repeat(0.5),
                }],
            )?;

            world.spawn((
                Transform::from_position(box_pos),
                GlobalTransform(Mat4::from_translation(box_pos)),
                MeshRenderer {
                    mesh: MeshHandle(Arc::new(box_mesh)),
                    material: MaterialHandle(Arc::new(box_material)),
                    visible: true,
                    cast_shadow: true,
                    receive_shadow: true,
                },
                FrustumCullable,
                Visible,
                body,
            ));

            self.scene_spawned = true;
        }

        // Update camera viewport
        for (cam,) in world.query_mut::<(&mut CameraComponent,)>() {
            if cam.active {
                cam.camera.set_viewport(ctx.viewport);
            }
        }

        Ok(())
    }

    fn fixed_update(&mut self, world: &mut hecs::World, dt: f64) -> anyhow::Result<()> {
        if let Some(physics) = &mut self.physics_world {
            physics.step(world, dt)?;
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let settings = WindowSettings::default().title("Physics Demo");
    let config = GameLoopConfig::default();
    let app = PhysicsApp {
        physics_world: None,
        scene_spawned: false,
    };
    run_app(settings, config, app)
}
