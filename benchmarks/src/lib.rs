//! Shared native Tessera fixtures for wall-clock and instruction-count benchmarks.
//!
//! CPU: cargo bench -p sard-bench --bench physics
//! GPU: cargo bench -p sard-bench --bench physics --features gpu-physics -- gpu
//! IAI: cargo bench -p sard-bench --bench physics_iai
//! IAI requires iai-callgrind-runner and Valgrind. Its optional GPU cases count
//! host instructions, including device setup; Criterion measures GPU wall time.

use sard::ecs::components::physics::RigidBody;
use sard::physics::nalgebra::{Isometry3, Matrix3, Vector3};
use sard::physics::tessera::articulated_world::SceneCollider;
use sard::physics::{PhysicsConfig, PhysicsWorld};

pub const DT: f64 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug)]
pub enum Scene {
    Spheres,
    Mixed,
    Sparse,
    Falling,
    Stack,
    Mass,
}

fn spawn_body(
    world: &mut hecs::World,
    position: Vector3<f64>,
    sphere: bool,
    size: f64,
    mass: f64,
) -> anyhow::Result<()> {
    // Exact solid-sphere and solid-cube inertia about their centers of mass.
    let (collider, inertia) = if sphere {
        (
            SceneCollider::Sphere {
                center: Vector3::zeros(),
                radius: size,
            },
            Matrix3::identity() * (0.4 * mass * size * size),
        )
    } else {
        (
            SceneCollider::Box {
                origin: Isometry3::identity(),
                half_extents: Vector3::repeat(size),
            },
            Matrix3::identity() * (2.0 * mass * size * size / 3.0),
        )
    };
    world.spawn((RigidBody::new(
        Isometry3::translation(position.x, position.y, position.z),
        mass,
        inertia,
        vec![collider],
    )?,));
    Ok(())
}

/// Z-up fixtures use the native world's default ground at z=0.
/// No rendering transforms are needed: SceneBody owns the authoritative pose.
pub fn setup_scene(scene: Scene, n: usize) -> anyhow::Result<(hecs::World, PhysicsWorld)> {
    let mut world = hecs::World::new();
    let physics = PhysicsWorld::new(PhysicsConfig::default())?;
    let n = u32::try_from(n)?;
    let cols = n.isqrt().max(1);
    let cols = if n.div_ceil(cols) > cols {
        cols + 1
    } else {
        cols
    };
    for i in 0..n {
        if matches!(scene, Scene::Mass) {
            spawn_object(&mut world, usize::try_from(i)?)?;
            continue;
        }
        let spacing = match scene {
            Scene::Sparse => 10.0,
            Scene::Spheres => 2.5,
            Scene::Mixed | Scene::Falling | Scene::Stack | Scene::Mass => 1.5,
        };
        let position = if matches!(scene, Scene::Stack) {
            Vector3::new(0.0, 0.0, 0.5 + f64::from(i))
        } else {
            Vector3::new(
                f64::from(i % cols) * spacing,
                f64::from(i / cols) * spacing,
                if matches!(scene, Scene::Falling) {
                    1.0 + f64::from(i % 5) * 1.5
                } else if matches!(scene, Scene::Spheres | Scene::Mixed) {
                    1.0
                } else {
                    0.5
                },
            )
        };
        let sphere = matches!(scene, Scene::Spheres | Scene::Sparse)
            || (!matches!(scene, Scene::Stack) && i % 2 == 0);
        let size = if matches!(scene, Scene::Spheres) || (matches!(scene, Scene::Mixed) && sphere) {
            1.0
        } else {
            0.5
        };
        let mass = if matches!(scene, Scene::Mixed) && !sphere {
            0.0
        } else {
            1.0
        };
        spawn_body(&mut world, position, sphere, size, mass)?;
    }
    Ok((world, physics))
}

fn spawn_object(world: &mut hecs::World, index: usize) -> anyhow::Result<()> {
    let index = u32::try_from(index)?;
    let angle = f64::from(index) * 137.0 * 0.01;
    let radius = 8.0 * (f64::from(index.wrapping_mul(73).wrapping_add(17) % 100) / 100.0).sqrt();
    spawn_body(
        world,
        Vector3::new(
            radius * angle.cos(),
            radius * angle.sin(),
            15.0 + f64::from(index % 5) * 0.6,
        ),
        index % 2 == 0,
        0.4,
        1.0,
    )
}

/// Advance a fixed number of real native CPU simulation steps.
pub fn run_steps(
    world: &mut hecs::World,
    physics: &mut PhysicsWorld,
    frames: usize,
) -> anyhow::Result<()> {
    for _ in 0..frames {
        physics.step(world, DT)?;
    }
    Ok(())
}

/// Continuous insertion plus stepping, preserving the mass-physics workload.
pub fn run_mass_physics(
    world: &mut hecs::World,
    physics: &mut PhysicsWorld,
    frames: usize,
    spawn_per_frame: usize,
    start_index: usize,
) -> anyhow::Result<()> {
    let mut index = start_index;
    for _ in 0..frames {
        for _ in 0..spawn_per_frame {
            spawn_object(world, index)?;
            index += 1;
        }
        physics.step(world, DT)?;
    }
    Ok(())
}

#[cfg(feature = "gpu-physics")]
pub fn run_gpu_steps(
    world: &mut hecs::World,
    physics: &mut PhysicsWorld,
    device: &sard::physics::GpuContactDevice,
    frames: usize,
) -> anyhow::Result<()> {
    for _ in 0..frames {
        physics.step_gpu(world, DT, device)?;
    }
    Ok(())
}

#[cfg(feature = "gpu-physics")]
pub fn run_gpu_mass_physics(
    world: &mut hecs::World,
    physics: &mut PhysicsWorld,
    device: &sard::physics::GpuContactDevice,
    frames: usize,
    spawn_per_frame: usize,
    start_index: usize,
) -> anyhow::Result<()> {
    let mut index = start_index;
    for _ in 0..frames {
        for _ in 0..spawn_per_frame {
            spawn_object(world, index)?;
            index += 1;
        }
        physics.step_gpu(world, DT, device)?;
    }
    Ok(())
}
