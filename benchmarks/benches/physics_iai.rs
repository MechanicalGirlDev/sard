//! Physics engine benchmarks (iai-callgrind - instruction counts).
//!
//! Prerequisites:
//!   cargo install iai-callgrind-runner
//!   sudo dnf install valgrind   # Fedora/WSL2
//!
//! Run all:    cargo bench --manifest-path benchmarks/Cargo.toml --bench physics_iai
//! Filter:     cargo bench --manifest-path benchmarks/Cargo.toml --bench physics_iai -- broadphase

use std::hint::black_box;

use glam::{Mat4, Vec3};
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use sard::ecs::components::physics::ColliderShape;
use sard::ecs::components::transform::GlobalTransform;
use sard::physics::broadphase::SweepAndPrune;
use sard::physics::narrowphase::{detect_collision, gjk_intersection, sat_box_box, sphere_sphere};
use sard::physics::solver::solve_contacts;
use sard_bench::*;

// ---------------------------------------------------------------------------
// Broadphase
// ---------------------------------------------------------------------------

#[library_benchmark]
fn broadphase_100() {
    let world = setup_sphere_world(black_box(100));
    let mut bp = SweepAndPrune::new();
    black_box(bp.find_pairs(&world));
}

#[library_benchmark]
fn broadphase_500() {
    let world = setup_sphere_world(black_box(500));
    let mut bp = SweepAndPrune::new();
    black_box(bp.find_pairs(&world));
}

#[library_benchmark]
fn broadphase_1000() {
    let world = setup_sphere_world(black_box(1000));
    let mut bp = SweepAndPrune::new();
    black_box(bp.find_pairs(&world));
}

#[library_benchmark]
fn broadphase_mixed_500() {
    let world = setup_mixed_world(black_box(500));
    let mut bp = SweepAndPrune::new();
    black_box(bp.find_pairs(&world));
}

#[library_benchmark]
fn broadphase_sparse_500() {
    let world = setup_sparse_world(black_box(500));
    let mut bp = SweepAndPrune::new();
    black_box(bp.find_pairs(&world));
}

library_benchmark_group!(
    name = broadphase_group;
    benchmarks =
        broadphase_100,
        broadphase_500,
        broadphase_1000,
        broadphase_mixed_500,
        broadphase_sparse_500
);

// ---------------------------------------------------------------------------
// Narrowphase
// ---------------------------------------------------------------------------

#[library_benchmark]
fn narrowphase_sphere_sphere_hit() {
    let shape = ColliderShape::Sphere { radius: 1.0 };
    let ta = GlobalTransform(Mat4::IDENTITY);
    let tb = GlobalTransform(Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0)));
    black_box(sphere_sphere(&shape, &ta, &shape, &tb));
}

#[library_benchmark]
fn narrowphase_sphere_sphere_miss() {
    let shape = ColliderShape::Sphere { radius: 1.0 };
    let ta = GlobalTransform(Mat4::IDENTITY);
    let tb = GlobalTransform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
    black_box(sphere_sphere(&shape, &ta, &shape, &tb));
}

#[library_benchmark]
fn narrowphase_box_box_hit() {
    let half = Vec3::splat(1.0);
    let ta = Mat4::IDENTITY;
    let tb = Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0));
    black_box(sat_box_box(half, ta, half, tb));
}

#[library_benchmark]
fn narrowphase_box_box_miss() {
    let half = Vec3::splat(1.0);
    let ta = Mat4::IDENTITY;
    let tb = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    black_box(sat_box_box(half, ta, half, tb));
}

#[library_benchmark]
fn narrowphase_gjk_hit() {
    let shape = ColliderShape::Sphere { radius: 1.0 };
    let ta = GlobalTransform(Mat4::IDENTITY);
    let tb = GlobalTransform(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)));
    black_box(gjk_intersection(&shape, &ta, &shape, &tb));
}

#[library_benchmark]
fn narrowphase_gjk_miss() {
    let shape = ColliderShape::Sphere { radius: 1.0 };
    let ta = GlobalTransform(Mat4::IDENTITY);
    let tb = GlobalTransform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
    black_box(gjk_intersection(&shape, &ta, &shape, &tb));
}

#[library_benchmark]
fn narrowphase_dispatch_all() {
    let ta = GlobalTransform(Mat4::IDENTITY);
    let tb = GlobalTransform(Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0)));
    let sphere = ColliderShape::Sphere { radius: 1.0 };
    let bbox = ColliderShape::Box {
        half_extents: Vec3::splat(1.0),
    };
    black_box(detect_collision(&sphere, &ta, &sphere, &tb));
    black_box(detect_collision(&bbox, &ta, &bbox, &tb));
    black_box(detect_collision(&bbox, &ta, &sphere, &tb));
    black_box(detect_collision(&sphere, &ta, &bbox, &tb));
}

library_benchmark_group!(
    name = narrowphase_group;
    benchmarks =
        narrowphase_sphere_sphere_hit,
        narrowphase_sphere_sphere_miss,
        narrowphase_box_box_hit,
        narrowphase_box_box_miss,
        narrowphase_gjk_hit,
        narrowphase_gjk_miss,
        narrowphase_dispatch_all
);

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

#[library_benchmark]
fn solver_10_contacts() {
    let (mut world, mut manifolds) = setup_contacts(black_box(10));
    solve_contacts(&mut manifolds, &mut world, 8);
}

#[library_benchmark]
fn solver_100_contacts() {
    let (mut world, mut manifolds) = setup_contacts(black_box(100));
    solve_contacts(&mut manifolds, &mut world, 8);
}

#[library_benchmark]
fn solver_100_contacts_16iter() {
    let (mut world, mut manifolds) = setup_contacts(black_box(100));
    solve_contacts(&mut manifolds, &mut world, 16);
}

library_benchmark_group!(
    name = solver_group;
    benchmarks =
        solver_10_contacts,
        solver_100_contacts,
        solver_100_contacts_16iter
);

// ---------------------------------------------------------------------------
// Full pipeline
// ---------------------------------------------------------------------------

#[library_benchmark]
fn pipeline_step_100() {
    let (mut world, mut physics) = setup_scene(black_box(100));
    physics.step(&mut world, 1.0 / 60.0);
}

#[library_benchmark]
fn pipeline_step_500() {
    let (mut world, mut physics) = setup_scene(black_box(500));
    physics.step(&mut world, 1.0 / 60.0);
}

#[library_benchmark]
fn pipeline_sustained_100() {
    let (mut world, mut physics) = setup_scene(black_box(100));
    for _ in 0..10 {
        physics.step(&mut world, 1.0 / 60.0);
    }
    black_box(&world);
}

library_benchmark_group!(
    name = pipeline_group;
    benchmarks =
        pipeline_step_100,
        pipeline_step_500,
        pipeline_sustained_100
);

// ---------------------------------------------------------------------------
// Mass physics
// ---------------------------------------------------------------------------

#[library_benchmark]
fn mass_physics_60frames_3spawn() {
    let (mut world, mut physics) = setup_mass_scene(black_box(0));
    run_mass_physics(&mut world, &mut physics, 60, 3, 0);
    black_box(&world);
}

#[library_benchmark]
fn mass_physics_60frames_10spawn() {
    let (mut world, mut physics) = setup_mass_scene(black_box(0));
    run_mass_physics(&mut world, &mut physics, 60, 10, 0);
    black_box(&world);
}

#[library_benchmark]
fn mass_physics_initial_500() {
    let (mut world, mut physics) = setup_mass_scene(black_box(500));
    run_mass_physics(&mut world, &mut physics, 60, 3, 500);
    black_box(&world);
}

library_benchmark_group!(
    name = mass_physics_group;
    benchmarks =
        mass_physics_60frames_3spawn,
        mass_physics_60frames_10spawn,
        mass_physics_initial_500
);

// ---------------------------------------------------------------------------
// Sleep effect
// ---------------------------------------------------------------------------

#[library_benchmark]
fn sleep_settled_scene_100() {
    let (mut world, mut physics) = setup_scene(black_box(100));
    // Settle bodies
    for _ in 0..300 {
        physics.step(&mut world, 1.0 / 60.0);
    }
    // Measure stepping a settled scene
    for _ in 0..60 {
        physics.step(&mut world, 1.0 / 60.0);
    }
    black_box(&world);
}

library_benchmark_group!(
    name = sleep_group;
    benchmarks =
        sleep_settled_scene_100
);

// ---------------------------------------------------------------------------
// GPU physics
// ---------------------------------------------------------------------------

#[library_benchmark]
fn gpu_pipeline_step_500() {
    let ctx = sard_bench::create_headless_context().expect("GPU context");
    let (mut world, mut physics) =
        sard_bench::setup_gpu_scene(&ctx, black_box(500)).expect("GPU scene setup");
    physics.step_gpu(&mut world, 1.0 / 60.0, &ctx);
}

#[library_benchmark]
fn gpu_pipeline_step_1000() {
    let ctx = sard_bench::create_headless_context().expect("GPU context");
    let (mut world, mut physics) =
        sard_bench::setup_gpu_scene(&ctx, black_box(1000)).expect("GPU scene setup");
    physics.step_gpu(&mut world, 1.0 / 60.0, &ctx);
}

#[library_benchmark]
fn gpu_mass_physics_60frames() {
    let ctx = sard_bench::create_headless_context().expect("GPU context");
    let (mut world, mut physics) =
        sard_bench::setup_gpu_mass_scene(&ctx, black_box(0)).expect("GPU mass scene");
    sard_bench::run_gpu_mass_physics(&mut world, &mut physics, &ctx, 60, 3, 0);
    black_box(&world);
}

library_benchmark_group!(
    name = gpu_group;
    benchmarks =
        gpu_pipeline_step_500,
        gpu_pipeline_step_1000,
        gpu_mass_physics_60frames
);

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

main!(
    library_benchmark_groups = broadphase_group,
    narrowphase_group,
    solver_group,
    pipeline_group,
    mass_physics_group,
    sleep_group,
    gpu_group
);
