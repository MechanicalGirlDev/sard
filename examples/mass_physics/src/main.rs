//! Mass Physics Demo - Objects continuously spawning and falling
//!
//! CPU physics:  WAYLAND_DISPLAY="" WGPU_BACKEND=gl cargo run -p mass_physics
//! GPU physics:  WAYLAND_DISPLAY="" WGPU_BACKEND=gl cargo run -p mass_physics --features gpu-physics

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

/// Objects spawned per frame
const SPAWN_PER_FRAME: usize = 3;
/// Spawn area radius
const SPAWN_RADIUS: f32 = 8.0;
/// Spawn height
const SPAWN_HEIGHT: f32 = 15.0;

struct MassPhysicsApp {
    physics_world: Option<PhysicsWorld>,
    ground_spawned: bool,
    spawned_count: usize,
    cube_mesh: Option<Arc<dyn sard::Geometry + Send + Sync>>,
    sphere_mesh: Option<Arc<dyn sard::Geometry + Send + Sync>>,
    #[cfg(feature = "gpu-physics")]
    gpu_initialized: bool,
}

impl App for MassPhysicsApp {
    fn init(&mut self, _ctx: &WgpuContext, world: &mut hecs::World) {
        self.physics_world = Some(PhysicsWorld::new(PhysicsConfig::default()));

        // Camera
        let camera = Camera::new_perspective(
            Vec3::new(20.0, 25.0, 30.0),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::Y,
            45.0,
            1.0,
            0.1,
            200.0,
        );
        world.spawn((
            Transform::identity(),
            GlobalTransform::default(),
            CameraComponent {
                camera,
                active: true,
            },
        ));

        // Directional light
        world.spawn((
            Transform::from_position(Vec3::new(10.0, 20.0, 10.0)),
            GlobalTransform::default(),
            LightComponent {
                light_type: LightType::Directional,
                color: Vec3::ONE,
                intensity: 1.0,
            },
        ));
    }

    fn update(&mut self, world: &mut hecs::World, ctx: &SystemContext) {
        // First frame: spawn ground + init shared meshes
        if !self.ground_spawned {
            self.spawn_ground(world, ctx);
            self.cube_mesh = Some(Arc::new(Mesh::cube(ctx.ctx, 0.8, [0.8, 0.4, 0.3])));
            self.sphere_mesh = Some(Arc::new(Mesh::sphere(ctx.ctx, 0.4, 12, 8, [0.3, 0.5, 0.8])));
            self.ground_spawned = true;
        }

        // Initialize GPU physics on first frame (needs WgpuContext from SystemContext)
        #[cfg(feature = "gpu-physics")]
        if !self.gpu_initialized {
            if let Some(physics) = &mut self.physics_world {
                physics
                    .init_gpu(ctx.ctx, 4096)
                    .expect("Failed to init GPU physics");
            }
            self.gpu_initialized = true;
        }

        // Spawn a few objects each frame
        for _ in 0..SPAWN_PER_FRAME {
            self.spawn_object(world, ctx);
        }

        // Update camera viewport
        for (cam,) in world.query_mut::<(&mut CameraComponent,)>() {
            if cam.active {
                cam.camera.set_viewport(ctx.viewport);
            }
        }

        // Step physics (GPU or CPU)
        if let Some(physics) = &mut self.physics_world {
            #[cfg(feature = "gpu-physics")]
            {
                physics.step_gpu(world, ctx.delta_time, ctx.ctx);
            }
            #[cfg(not(feature = "gpu-physics"))]
            {
                physics.step(world, ctx.delta_time);
            }
        }
    }
}

impl MassPhysicsApp {
    fn spawn_ground(&self, world: &mut hecs::World, ctx: &SystemContext) {
        let ground_material = ColorMaterial::new(ctx.ctx, ctx.surface_format)
            .expect("Failed to create ground material");
        let ground_mesh = Mesh::quad(ctx.ctx, 40.0, 40.0, [0.35, 0.45, 0.35]);
        let ground_pos = Vec3::ZERO;

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
                    half_extents: Vec3::new(20.0, 5.0, 20.0),
                },
                offset: Vec3::new(0.0, -5.0, 0.0),
                is_sensor: false,
            },
        ));
    }

    fn spawn_object(&mut self, world: &mut hecs::World, ctx: &SystemContext) {
        let i = self.spawned_count;
        let is_sphere = i.is_multiple_of(2);

        let angle = (i * 137) as f32 * 0.01;
        let r = SPAWN_RADIUS * (((i * 73 + 17) % 100) as f32 / 100.0).sqrt();
        let height_jitter = (i % 5) as f32 * 0.6;
        let pos = Vec3::new(
            r * angle.cos(),
            SPAWN_HEIGHT + height_jitter,
            r * angle.sin(),
        );

        let material =
            ColorMaterial::new(ctx.ctx, ctx.surface_format).expect("Failed to create material");

        let (mesh, collider_shape) = if is_sphere {
            (
                MeshHandle(Arc::clone(self.sphere_mesh.as_ref().unwrap())),
                ColliderShape::Sphere { radius: 0.4 },
            )
        } else {
            (
                MeshHandle(Arc::clone(self.cube_mesh.as_ref().unwrap())),
                ColliderShape::Box {
                    half_extents: Vec3::splat(0.4),
                },
            )
        };

        world.spawn((
            Transform::from_position(pos),
            GlobalTransform(Mat4::from_translation(pos)),
            MeshRenderer {
                mesh,
                material: MaterialHandle(Arc::new(material)),
                visible: true,
                cast_shadow: true,
                receive_shadow: true,
            },
            FrustumCullable,
            Visible,
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: collider_shape,
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        self.spawned_count += 1;
    }
}

fn main() -> anyhow::Result<()> {
    let title = if cfg!(feature = "gpu-physics") {
        "Mass Physics Demo (GPU Broadphase)"
    } else {
        "Mass Physics Demo (CPU)"
    };
    let settings = WindowSettings::default().title(title);
    let config = GameLoopConfig::default();
    let app = MassPhysicsApp {
        physics_world: None,
        ground_spawned: false,
        spawned_count: 0,
        cube_mesh: None,
        sphere_mesh: None,
        #[cfg(feature = "gpu-physics")]
        gpu_initialized: false,
    };
    run_app(settings, config, app)
}
