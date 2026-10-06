//! Native CPU world stepping (IAI instruction counts; requires Valgrind).
//! Optional GPU cases count host instructions, including device setup, not GPU time.

use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use sard::physics::PhysicsWorld;
use sard_bench::{run_mass_physics, run_steps, setup_scene, Scene};

fn fixture(scene: Scene, n: usize) -> (hecs::World, PhysicsWorld) {
    // Only the fixed cases below call this helper. Their finite poses, positive
    // radii/extents and positive-definite solid inertias satisfy SceneBody::new;
    // native default world parameters are valid and require no external device.
    setup_scene(scene, n).expect("deterministic valid native benchmark fixture")
}

fn check_result(result: anyhow::Result<()>) {
    // IAI's generated main cannot propagate Result. A computation error must
    // fail the benchmark process, never become a black-boxed successful sample.
    if let Err(error) = result {
        panic!("native physics benchmark failed: {error:#}");
    }
}

#[library_benchmark(teardown = check_result)]
#[bench::spheres_100(args = (Scene::Spheres, 100), setup = fixture)]
#[bench::spheres_500(args = (Scene::Spheres, 500), setup = fixture)]
#[bench::spheres_1000(args = (Scene::Spheres, 1000), setup = fixture)]
#[bench::spheres_2000(args = (Scene::Spheres, 2000), setup = fixture)]
#[bench::mixed_500(args = (Scene::Mixed, 500), setup = fixture)]
#[bench::sparse_500(args = (Scene::Sparse, 500), setup = fixture)]
#[bench::stack_10(args = (Scene::Stack, 10), setup = fixture)]
#[bench::stack_100(args = (Scene::Stack, 100), setup = fixture)]
#[bench::stack_500(args = (Scene::Stack, 500), setup = fixture)]
#[bench::falling_100(args = (Scene::Falling, 100), setup = fixture)]
#[bench::falling_500(args = (Scene::Falling, 500), setup = fixture)]
fn world_step((mut world, mut physics): (hecs::World, PhysicsWorld)) -> anyhow::Result<()> {
    run_steps(&mut world, &mut physics, black_box(1))?;
    black_box(world);
    Ok(())
}

#[library_benchmark(teardown = check_result)]
#[bench::bodies_100(args = (Scene::Falling, 100), setup = fixture)]
#[bench::bodies_500(args = (Scene::Falling, 500), setup = fixture)]
fn sustained_10steps((mut world, mut physics): (hecs::World, PhysicsWorld)) -> anyhow::Result<()> {
    run_steps(&mut world, &mut physics, black_box(10))?;
    black_box(world);
    Ok(())
}

#[library_benchmark(teardown = check_result)]
#[bench::spawn_3(args = (0, 3))]
#[bench::spawn_10(args = (0, 10))]
#[bench::initial_500(args = (500, 3))]
fn mass_physics(initial: usize, spawn: usize) -> anyhow::Result<()> {
    let (mut world, mut physics) = fixture(Scene::Mass, initial);
    run_mass_physics(&mut world, &mut physics, black_box(60), spawn, initial)?;
    black_box(world);
    Ok(())
}

#[library_benchmark(teardown = check_result)]
fn after_300steps() -> anyhow::Result<()> {
    let (mut world, mut physics) = fixture(Scene::Falling, 100);
    run_steps(&mut world, &mut physics, black_box(300))?;
    run_steps(&mut world, &mut physics, black_box(60))?;
    black_box(world);
    Ok(())
}

library_benchmark_group!(
    name = physics_group;
    benchmarks = world_step, sustained_10steps, mass_physics, after_300steps
);

#[cfg(feature = "gpu-physics")]
#[library_benchmark(teardown = check_result)]
#[bench::bodies_500(500)]
#[bench::bodies_1000(1000)]
fn gpu_world_step(n: usize) -> anyhow::Result<()> {
    let device = sard::physics::GpuContactDevice::new()?;
    let (mut world, mut physics) = setup_scene(Scene::Falling, n)?;
    sard_bench::run_gpu_steps(&mut world, &mut physics, &device, black_box(1))?;
    black_box(world);
    Ok(())
}

#[cfg(feature = "gpu-physics")]
#[library_benchmark(teardown = check_result)]
fn gpu_mass_physics() -> anyhow::Result<()> {
    let device = sard::physics::GpuContactDevice::new()?;
    let (mut world, mut physics) = setup_scene(Scene::Mass, 0)?;
    sard_bench::run_gpu_mass_physics(&mut world, &mut physics, &device, black_box(60), 3, 0)?;
    black_box(world);
    Ok(())
}

#[cfg(feature = "gpu-physics")]
library_benchmark_group!(
    name = gpu_group;
    benchmarks = gpu_world_step, gpu_mass_physics
);

#[cfg(not(feature = "gpu-physics"))]
main!(library_benchmark_groups = physics_group);

#[cfg(feature = "gpu-physics")]
main!(library_benchmark_groups = physics_group, gpu_group);
