//! Physics Demo - Demonstrates physics simulation with falling boxes
//!
//! Run with: cargo run -p physics_demo

use std::sync::Arc;

use glam::{Mat4, Vec3};
use sard::ecs::components::physics::{Collider, ColliderShape, RigidBody};
use sard::ecs::components::rendering::{
    CameraComponent, FrustumCullable, LightComponent, MaterialHandle, MeshHandle, MeshRenderer,
    Visible,
};
use sard::ecs::components::transform::{GlobalTransform, Transform};
use sard::engine::{run_app, App, GameLoopConfig, SystemContext};
use sard::physics::{PhysicsConfig, PhysicsWorld};
use sard::renderer::light::LightType;
use sard::{Camera, ColorMaterial, Mesh, WgpuContext, WindowSettings};

struct PhysicsApp {
    physics_world: Option<PhysicsWorld>,
    scene_spawned: bool,
}

impl App for PhysicsApp {
    fn init(&mut self, _ctx: &WgpuContext, world: &mut hecs::World) {
        // Create physics world
        self.physics_world = Some(PhysicsWorld::new(PhysicsConfig::default()));

        // Camera
        let camera = Camera::new_perspective(
            Vec3::new(5.0, 5.0, 8.0),
            Vec3::ZERO,
            Vec3::Y,
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
            Transform::from_position(Vec3::new(5.0, 10.0, 5.0)),
            GlobalTransform::default(),
            LightComponent {
                light_type: LightType::Directional,
                color: Vec3::ONE,
                intensity: 1.0,
            },
        ));
    }

    fn update(&mut self, world: &mut hecs::World, ctx: &SystemContext) {
        // Spawn scene on first update (surface_format is available here)
        if !self.scene_spawned {
            // Ground (static rigid body)
            let ground_material = ColorMaterial::new(ctx.ctx, ctx.surface_format)
                .expect("Failed to create ground material");
            let ground_mesh = Mesh::quad(ctx.ctx, 20.0, 20.0, [0.4, 0.5, 0.4]);
            let ground_pos = Vec3::new(0.0, -0.5, 0.0);

            world.spawn((
                Transform::from_position(ground_pos),
                GlobalTransform(Mat4::from_translation(ground_pos)),
                MeshRenderer {
                    mesh: MeshHandle(Arc::new(ground_mesh)),
                    material: MaterialHandle(Arc::new(ground_material)),
                    visible: true,
                    cast_shadow: false,
                    receive_shadow: true,
                },
                FrustumCullable,
                Visible,
                RigidBody::new_static(),
                Collider {
                    shape: ColliderShape::Box {
                        half_extents: Vec3::new(10.0, 0.01, 10.0),
                    },
                    offset: Vec3::ZERO,
                    is_sensor: false,
                },
            ));

            // Falling box (dynamic rigid body)
            let box_material = ColorMaterial::new(ctx.ctx, ctx.surface_format)
                .expect("Failed to create box material");
            let box_mesh = Mesh::cube(ctx.ctx, 1.0, [0.8, 0.2, 0.2]);
            let box_pos = Vec3::new(0.0, 5.0, 0.0);

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
                RigidBody::new_dynamic(1.0),
                Collider {
                    shape: ColliderShape::Box {
                        half_extents: Vec3::splat(0.5),
                    },
                    offset: Vec3::ZERO,
                    is_sensor: false,
                },
            ));

            self.scene_spawned = true;
        }

        // Update camera viewport
        for (cam,) in world.query_mut::<(&mut CameraComponent,)>() {
            if cam.active {
                cam.camera.set_viewport(ctx.viewport);
            }
        }

        // Step physics
        if let Some(physics) = &mut self.physics_world {
            physics.step(world, ctx.delta_time);
        }
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
