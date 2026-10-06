//! Shared setup helpers for sard benchmarks.
//!
//! ## Running
//!
//! CPU physics (criterion):
//!   cargo bench --manifest-path benchmarks/Cargo.toml --bench physics
//!
//! GPU physics (criterion):
//!   cargo bench --manifest-path benchmarks/Cargo.toml --bench physics --features gpu-physics
//!
//! iai-callgrind (instruction counts, requires valgrind):
//!   cargo install iai-callgrind-runner
//!   cargo bench --manifest-path benchmarks/Cargo.toml --bench physics_iai
//!
//! Filter by group:
//!   cargo bench --manifest-path benchmarks/Cargo.toml --bench physics -- broadphase
//!   cargo bench --manifest-path benchmarks/Cargo.toml --bench physics -- gpu

use glam::{Mat4, Vec3};
use sard::ecs::components::physics::{Collider, ColliderShape, RigidBody, SleepInfo};
use sard::ecs::components::transform::{GlobalTransform, Transform};
use sard::physics::contact::{ContactManifold, ContactPoint};
use sard::physics::{PhysicsConfig, PhysicsWorld};

// ---------------------------------------------------------------------------
// Basic scenes
// ---------------------------------------------------------------------------

/// Spawn `n` dynamic sphere bodies in a grid layout so roughly half overlap.
pub fn setup_sphere_world(n: usize) -> hecs::World {
    let mut world = hecs::World::new();
    let cols = (n as f32).sqrt().ceil() as usize;

    for i in 0..n {
        let x = (i % cols) as f32 * 1.5;
        let z = (i / cols) as f32 * 1.5;
        let pos = Vec3::new(x, 0.0, z);

        world.spawn((
            Transform::from_position(pos),
            GlobalTransform(Mat4::from_translation(pos)),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 1.0 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));
    }
    world
}

/// Mixed scene: half dynamic spheres, half static boxes.
pub fn setup_mixed_world(n: usize) -> hecs::World {
    let mut world = hecs::World::new();
    let cols = (n as f32).sqrt().ceil() as usize;

    for i in 0..n {
        let x = (i % cols) as f32 * 1.5;
        let z = (i / cols) as f32 * 1.5;
        let pos = Vec3::new(x, 0.0, z);

        if i % 2 == 0 {
            world.spawn((
                Transform::from_position(pos),
                GlobalTransform(Mat4::from_translation(pos)),
                RigidBody::new_dynamic(1.0),
                Collider {
                    shape: ColliderShape::Sphere { radius: 1.0 },
                    offset: Vec3::ZERO,
                    is_sensor: false,
                },
            ));
        } else {
            world.spawn((
                Transform::from_position(pos),
                GlobalTransform(Mat4::from_translation(pos)),
                RigidBody::new_static(),
                Collider {
                    shape: ColliderShape::Box {
                        half_extents: Vec3::splat(0.5),
                    },
                    offset: Vec3::ZERO,
                    is_sensor: false,
                },
            ));
        }
    }
    world
}

/// Sparse scene: bodies spread far apart (no overlaps).
pub fn setup_sparse_world(n: usize) -> hecs::World {
    let mut world = hecs::World::new();
    let cols = (n as f32).sqrt().ceil() as usize;

    for i in 0..n {
        let x = (i % cols) as f32 * 10.0;
        let z = (i / cols) as f32 * 10.0;
        let pos = Vec3::new(x, 0.0, z);

        world.spawn((
            Transform::from_position(pos),
            GlobalTransform(Mat4::from_translation(pos)),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 0.5 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));
    }
    world
}

/// Ground plane + `n` dynamic bodies above it (mixed spheres/boxes).
pub fn setup_scene(n: usize) -> (hecs::World, PhysicsWorld) {
    let mut world = hecs::World::new();
    let physics = PhysicsWorld::new(PhysicsConfig::default());

    let ground_pos = Vec3::new(0.0, -0.5, 0.0);
    world.spawn((
        Transform::from_position(ground_pos),
        GlobalTransform(Mat4::from_translation(ground_pos)),
        RigidBody::new_static(),
        Collider {
            shape: ColliderShape::Box {
                half_extents: Vec3::new(100.0, 0.5, 100.0),
            },
            offset: Vec3::ZERO,
            is_sensor: false,
        },
    ));

    let cols = (n as f32).sqrt().ceil() as usize;
    for i in 0..n {
        let x = (i % cols) as f32 * 2.0 - (cols as f32);
        let z = (i / cols) as f32 * 2.0 - (cols as f32);
        let y = 1.0 + (i % 5) as f32 * 1.5;
        let pos = Vec3::new(x, y, z);

        if i % 2 == 0 {
            world.spawn((
                Transform::from_position(pos),
                GlobalTransform(Mat4::from_translation(pos)),
                RigidBody::new_dynamic(1.0),
                SleepInfo::default(),
                Collider {
                    shape: ColliderShape::Sphere { radius: 0.5 },
                    offset: Vec3::ZERO,
                    is_sensor: false,
                },
            ));
        } else {
            world.spawn((
                Transform::from_position(pos),
                GlobalTransform(Mat4::from_translation(pos)),
                RigidBody::new_dynamic(1.0),
                SleepInfo::default(),
                Collider {
                    shape: ColliderShape::Box {
                        half_extents: Vec3::splat(0.4),
                    },
                    offset: Vec3::ZERO,
                    is_sensor: false,
                },
            ));
        }
    }

    (world, physics)
}

// ---------------------------------------------------------------------------
// Solver setup
// ---------------------------------------------------------------------------

/// Stacked bodies with pre-built contact manifolds for solver benchmarks.
pub fn setup_contacts(n: usize) -> (hecs::World, Vec<ContactManifold>) {
    let mut world = hecs::World::new();
    let mut entities = Vec::with_capacity(n + 1);

    let ground_pos = Vec3::new(0.0, -0.5, 0.0);
    let ground = world.spawn((
        Transform::from_position(ground_pos),
        GlobalTransform(Mat4::from_translation(ground_pos)),
        RigidBody::new_static(),
        Collider {
            shape: ColliderShape::Box {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            offset: Vec3::ZERO,
            is_sensor: false,
        },
    ));
    entities.push(ground);

    for i in 0..n {
        let pos = Vec3::new(0.0, 0.5 + i as f32, 0.0);
        let entity = world.spawn((
            Transform::from_position(pos),
            GlobalTransform(Mat4::from_translation(pos)),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Box {
                    half_extents: Vec3::splat(0.5),
                },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));
        entities.push(entity);
    }

    let mut manifolds = Vec::new();
    for i in 0..n {
        let entity_a = entities[i];
        let entity_b = entities[i + 1];
        let contact_y = 0.5 * (i as f32 + (i + 1) as f32);

        manifolds.push(ContactManifold {
            entity_a,
            entity_b,
            normal: Vec3::Y,
            contacts: vec![ContactPoint {
                position: Vec3::new(0.0, contact_y, 0.0),
                penetration: 0.01,
                normal_impulse: 0.0,
                tangent_impulse: [0.0; 2],
            }],
        });
    }

    (world, manifolds)
}

// ---------------------------------------------------------------------------
// Mass physics scenario (mirrors the mass_physics demo)
// ---------------------------------------------------------------------------

const SPAWN_RADIUS: f32 = 8.0;
const SPAWN_HEIGHT: f32 = 15.0;

/// Spawn a single physics object at a deterministic position.
fn spawn_object(world: &mut hecs::World, index: usize) {
    let is_sphere = index.is_multiple_of(2);
    let angle = (index * 137) as f32 * 0.01;
    let r = SPAWN_RADIUS * (((index * 73 + 17) % 100) as f32 / 100.0).sqrt();
    let height_jitter = (index % 5) as f32 * 0.6;
    let pos = Vec3::new(
        r * angle.cos(),
        SPAWN_HEIGHT + height_jitter,
        r * angle.sin(),
    );

    let shape = if is_sphere {
        ColliderShape::Sphere { radius: 0.4 }
    } else {
        ColliderShape::Box {
            half_extents: Vec3::splat(0.4),
        }
    };

    world.spawn((
        Transform::from_position(pos),
        GlobalTransform(Mat4::from_translation(pos)),
        RigidBody::new_dynamic(1.0),
        SleepInfo::default(),
        Collider {
            shape,
            offset: Vec3::ZERO,
            is_sensor: false,
        },
    ));
}

/// Ground + `initial` pre-existing falling bodies.
pub fn setup_mass_scene(initial: usize) -> (hecs::World, PhysicsWorld) {
    let mut world = hecs::World::new();
    let physics = PhysicsWorld::new(PhysicsConfig::default());

    // Ground (same as mass_physics demo)
    let ground_pos = Vec3::ZERO;
    world.spawn((
        Transform::from_position(ground_pos),
        GlobalTransform(Mat4::from_translation(ground_pos)),
        RigidBody::new_static(),
        Collider {
            shape: ColliderShape::Box {
                half_extents: Vec3::new(20.0, 5.0, 20.0),
            },
            offset: Vec3::new(0.0, -5.0, 0.0),
            is_sensor: false,
        },
    ));

    for i in 0..initial {
        spawn_object(&mut world, i);
    }

    (world, physics)
}

/// Run `frames` frames, spawning `spawn_per_frame` objects each frame + physics step.
pub fn run_mass_physics(
    world: &mut hecs::World,
    physics: &mut PhysicsWorld,
    frames: usize,
    spawn_per_frame: usize,
    start_index: usize,
) {
    let mut idx = start_index;
    for _ in 0..frames {
        for _ in 0..spawn_per_frame {
            spawn_object(world, idx);
            idx += 1;
        }
        physics.step(world, 1.0 / 60.0);
    }
}

// ---------------------------------------------------------------------------
// GPU physics helpers
// ---------------------------------------------------------------------------

use sard::WgpuContext;

/// Create a headless WgpuContext for GPU benchmarks (no window needed).
/// Returns Err if no GPU adapter is available.
pub fn create_headless_context() -> anyhow::Result<WgpuContext> {
    WgpuContext::new_blocking(None)
}

/// Setup a GPU-enabled physics scene: ground + `n` bodies + GPU physics init.
pub fn setup_gpu_scene(ctx: &WgpuContext, n: usize) -> anyhow::Result<(hecs::World, PhysicsWorld)> {
    let (world, mut physics) = setup_scene(n);
    physics.init_gpu(ctx, n.max(256))?;
    Ok((world, physics))
}

/// Setup a GPU-enabled mass physics scene.
pub fn setup_gpu_mass_scene(
    ctx: &WgpuContext,
    initial: usize,
) -> anyhow::Result<(hecs::World, PhysicsWorld)> {
    let (world, mut physics) = setup_mass_scene(initial);
    physics.init_gpu(ctx, 4096)?;
    Ok((world, physics))
}

/// Run mass physics with GPU-accelerated step.
pub fn run_gpu_mass_physics(
    world: &mut hecs::World,
    physics: &mut PhysicsWorld,
    ctx: &WgpuContext,
    frames: usize,
    spawn_per_frame: usize,
    start_index: usize,
) {
    let mut idx = start_index;
    for _ in 0..frames {
        for _ in 0..spawn_per_frame {
            spawn_object(world, idx);
            idx += 1;
        }
        physics.step_gpu(world, 1.0 / 60.0, ctx);
    }
}
