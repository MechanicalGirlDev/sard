//! Mass Physics Demo - Objects continuously spawning and falling
//!
//! CPU physics:  WAYLAND_DISPLAY="" WGPU_BACKEND=gl cargo run -p mass_physics
//! GPU physics:  WAYLAND_DISPLAY="" WGPU_BACKEND=gl cargo run -p mass_physics --features gpu-physics

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
#[cfg(feature = "gpu-physics")]
use sard::physics::GpuContactDevice;
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
    gpu_device: GpuContactDevice,
}

impl App for MassPhysicsApp {
    fn init(&mut self, _ctx: &WgpuContext, world: &mut hecs::World) -> anyhow::Result<()> {
        self.physics_world = Some(PhysicsWorld::new(PhysicsConfig {
            ground_half_extent: 20.0,
            max_substep: 1.0 / 120.0,
            ..PhysicsConfig::default()
        })?);

        // Camera
        let camera = Camera::new_perspective(
            Vec3::new(20.0, 30.0, 25.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::Z,
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
            Transform::from_position(Vec3::new(10.0, 10.0, 20.0)),
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
        // First frame: spawn ground + init shared meshes
        if !self.ground_spawned {
            self.spawn_ground(world, ctx)?;
            self.ground_spawned = true;
        }

        let cube_mesh = Arc::clone(
            self.cube_mesh
                .get_or_insert_with(|| Arc::new(Mesh::cube(ctx.ctx, 0.8, [0.8, 0.4, 0.3]))),
        );
        let sphere_mesh =
            Arc::clone(self.sphere_mesh.get_or_insert_with(|| {
                Arc::new(Mesh::sphere(ctx.ctx, 0.4, 12, 8, [0.3, 0.5, 0.8]))
            }));

        // Spawn a few objects each frame
        for _ in 0..SPAWN_PER_FRAME {
            self.spawn_object(world, ctx, &cube_mesh, &sphere_mesh)?;
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
            #[cfg(feature = "gpu-physics")]
            {
                physics.step_gpu(world, dt, &self.gpu_device)?;
            }
            #[cfg(not(feature = "gpu-physics"))]
            {
                physics.step(world, dt)?;
            }
        }
        Ok(())
    }
}

impl MassPhysicsApp {
    fn spawn_ground(&self, world: &mut hecs::World, ctx: &SystemContext) -> anyhow::Result<()> {
        let ground_material = ColorMaterial::new(ctx.ctx, ctx.surface_format)?;
        let ground_mesh = Mesh::quad(ctx.ctx, 40.0, 40.0, [0.35, 0.45, 0.35]);
        // Render Tessera's implicit finite ground at z=0.
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
        Ok(())
    }

    fn spawn_object(
        &mut self,
        world: &mut hecs::World,
        ctx: &SystemContext,
        cube_mesh: &Arc<dyn sard::Geometry + Send + Sync>,
        sphere_mesh: &Arc<dyn sard::Geometry + Send + Sync>,
    ) -> anyhow::Result<()> {
        let i = self.spawned_count;
        let is_sphere = i.is_multiple_of(2);

        let angle = (i * 137) as f32 * 0.01;
        let r = SPAWN_RADIUS * (((i * 73 + 17) % 100) as f32 / 100.0).sqrt();
        let height_jitter = (i % 5) as f32 * 0.6;
        let pos = Vec3::new(
            r * angle.cos(),
            r * angle.sin(),
            SPAWN_HEIGHT + height_jitter,
        );

        let material = ColorMaterial::new(ctx.ctx, ctx.surface_format)?;

        let (mesh, collider, inertia) = if is_sphere {
            (
                MeshHandle(Arc::clone(sphere_mesh)),
                SceneCollider::Sphere {
                    center: Vector3::zeros(),
                    radius: 0.4,
                },
                Matrix3::identity() * (2.0 / 5.0 * 0.4 * 0.4),
            )
        } else {
            (
                MeshHandle(Arc::clone(cube_mesh)),
                SceneCollider::Box {
                    origin: Isometry3::identity(),
                    half_extents: Vector3::repeat(0.4),
                },
                Matrix3::identity() * (0.8 * 0.8 / 6.0),
            )
        };
        let body = RigidBody::new(
            Isometry3::translation(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z)),
            1.0,
            inertia,
            vec![collider],
        )?;

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
            body,
        ));

        self.spawned_count += 1;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let title = if cfg!(feature = "gpu-physics") {
        "Mass Physics Demo (Tessera GPU)"
    } else {
        "Mass Physics Demo (CPU)"
    };
    let settings = WindowSettings::default().title(title);
    let config = GameLoopConfig::default();
    #[cfg(feature = "gpu-physics")]
    let gpu_device = GpuContactDevice::new()?;
    let app = MassPhysicsApp {
        physics_world: None,
        ground_spawned: false,
        spawned_count: 0,
        cube_mesh: None,
        sphere_mesh: None,
        #[cfg(feature = "gpu-physics")]
        gpu_device,
    };
    run_app(settings, config, app)
}
