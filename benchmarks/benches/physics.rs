//! Physics engine benchmarks (criterion - wall-clock time).
//!
//! Run all:    cargo bench --manifest-path benchmarks/Cargo.toml --bench physics
//! Filter:     cargo bench --manifest-path benchmarks/Cargo.toml --bench physics -- broadphase

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use glam::{Mat4, Vec3};
use sard::ecs::components::physics::ColliderShape;
use sard::ecs::components::transform::GlobalTransform;
use sard::physics::broadphase::SweepAndPrune;
use sard::physics::narrowphase::{
    box_sphere, detect_collision, gjk_intersection, sat_box_box, sphere_sphere,
};
use sard::physics::solver::solve_contacts;
use sard_bench::*;

// ---------------------------------------------------------------------------
// Broadphase
// ---------------------------------------------------------------------------

fn bench_broadphase(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("broadphase/uniform_spheres");
        for &n in &[100, 500, 1000, 2000] {
            let world = setup_sphere_world(n);
            let mut broadphase = SweepAndPrune::new();
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| broadphase.find_pairs(&world));
            });
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("broadphase/mixed_shapes");
        for &n in &[100, 500, 1000, 2000] {
            let world = setup_mixed_world(n);
            let mut broadphase = SweepAndPrune::new();
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| broadphase.find_pairs(&world));
            });
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("broadphase/sparse");
        for &n in &[100, 500, 1000, 2000] {
            let world = setup_sparse_world(n);
            let mut broadphase = SweepAndPrune::new();
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| broadphase.find_pairs(&world));
            });
        }
        group.finish();
    }
}

// ---------------------------------------------------------------------------
// Narrowphase
// ---------------------------------------------------------------------------

fn bench_narrowphase(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("narrowphase/sphere_sphere");
        let shape = ColliderShape::Sphere { radius: 1.0 };
        let ta = GlobalTransform(Mat4::IDENTITY);

        let tb_hit = GlobalTransform(Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0)));
        group.bench_function("intersecting", |b| {
            b.iter(|| sphere_sphere(&shape, &ta, &shape, &tb_hit));
        });

        let tb_miss = GlobalTransform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
        group.bench_function("separated", |b| {
            b.iter(|| sphere_sphere(&shape, &ta, &shape, &tb_miss));
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("narrowphase/box_box");
        let half = Vec3::splat(1.0);
        let ta = Mat4::IDENTITY;

        let tb_hit = Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0));
        group.bench_function("intersecting", |b| {
            b.iter(|| sat_box_box(half, ta, half, tb_hit));
        });

        let tb_miss = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        group.bench_function("separated", |b| {
            b.iter(|| sat_box_box(half, ta, half, tb_miss));
        });

        let tb_rot =
            Mat4::from_rotation_y(0.785) * Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0));
        group.bench_function("rotated", |b| {
            b.iter(|| sat_box_box(half, ta, half, tb_rot));
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("narrowphase/box_sphere");
        let half = Vec3::splat(1.0);
        let box_t = GlobalTransform(Mat4::IDENTITY);

        let sphere_t_hit = GlobalTransform(Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0)));
        group.bench_function("intersecting", |b| {
            b.iter(|| box_sphere(half, &box_t, 1.0, &sphere_t_hit));
        });

        let sphere_t_miss = GlobalTransform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
        group.bench_function("separated", |b| {
            b.iter(|| box_sphere(half, &box_t, 1.0, &sphere_t_miss));
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("narrowphase/gjk");
        let shape = ColliderShape::Sphere { radius: 1.0 };
        let ta = GlobalTransform(Mat4::IDENTITY);

        let tb_hit = GlobalTransform(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)));
        group.bench_function("intersecting", |b| {
            b.iter(|| gjk_intersection(&shape, &ta, &shape, &tb_hit));
        });

        let tb_miss = GlobalTransform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
        group.bench_function("separated", |b| {
            b.iter(|| gjk_intersection(&shape, &ta, &shape, &tb_miss));
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("narrowphase/dispatch");
        let ta = GlobalTransform(Mat4::IDENTITY);
        let tb = GlobalTransform(Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0)));

        let sphere = ColliderShape::Sphere { radius: 1.0 };
        group.bench_function("sphere_sphere", |b| {
            b.iter(|| detect_collision(&sphere, &ta, &sphere, &tb));
        });

        let bbox = ColliderShape::Box {
            half_extents: Vec3::splat(1.0),
        };
        group.bench_function("box_box", |b| {
            b.iter(|| detect_collision(&bbox, &ta, &bbox, &tb));
        });
        group.bench_function("box_sphere", |b| {
            b.iter(|| detect_collision(&bbox, &ta, &sphere, &tb));
        });
        group.bench_function("sphere_box", |b| {
            b.iter(|| detect_collision(&sphere, &ta, &bbox, &tb));
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("narrowphase/batch");
        for &n in &[100, 500, 1000] {
            let pairs: Vec<_> = (0..n)
                .map(|i| {
                    let x = (i as f32) * 3.0;
                    let shape = ColliderShape::Sphere { radius: 1.0 };
                    let ta = GlobalTransform(Mat4::from_translation(Vec3::new(x, 0.0, 0.0)));
                    let tb = GlobalTransform(Mat4::from_translation(Vec3::new(x + 1.5, 0.0, 0.0)));
                    (shape.clone(), ta, shape, tb)
                })
                .collect();

            group.bench_with_input(BenchmarkId::from_parameter(n), &pairs, |b, pairs| {
                b.iter(|| {
                    for (sa, ta, sb, tb) in pairs {
                        detect_collision(sa, ta, sb, tb);
                    }
                });
            });
        }
        group.finish();
    }
}

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

fn bench_solver(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("solver/contact_count");
        for &n in &[10, 50, 100, 500] {
            let (mut world, manifolds) = setup_contacts(n);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter_batched(
                    || manifolds.clone(),
                    |mut m| solve_contacts(&mut m, &mut world, 8),
                    criterion::BatchSize::SmallInput,
                );
            });
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("solver/iterations");
        let (mut world, manifolds) = setup_contacts(100);
        for &iters in &[1, 4, 8, 16, 32] {
            group.bench_with_input(BenchmarkId::from_parameter(iters), &iters, |b, &iters| {
                b.iter_batched(
                    || manifolds.clone(),
                    |mut m| solve_contacts(&mut m, &mut world, iters),
                    criterion::BatchSize::SmallInput,
                );
            });
        }
        group.finish();
    }
}

// ---------------------------------------------------------------------------
// Full pipeline
// ---------------------------------------------------------------------------

fn bench_pipeline(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("pipeline/step");
        group.sample_size(30);
        for &n in &[50, 100, 500, 1000] {
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_batched(
                    || setup_scene(n),
                    |(mut world, mut physics)| {
                        physics.step(&mut world, 1.0 / 60.0);
                    },
                    criterion::BatchSize::LargeInput,
                );
            });
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("pipeline/sustained_10steps");
        group.sample_size(20);
        for &n in &[100, 500] {
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_batched(
                    || setup_scene(n),
                    |(mut world, mut physics)| {
                        for _ in 0..10 {
                            physics.step(&mut world, 1.0 / 60.0);
                        }
                    },
                    criterion::BatchSize::LargeInput,
                );
            });
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("pipeline/stages");
        let n = 500;
        let (world, _) = setup_scene(n);

        let mut broadphase = SweepAndPrune::new();
        group.bench_function("broadphase_500", |b| {
            b.iter(|| broadphase.find_pairs(&world));
        });

        group.bench_function("integrate_500", |b| {
            b.iter_batched(
                || {
                    let (w, _) = setup_scene(n);
                    w
                },
                |mut w| {
                    sard::physics::rigid_body::apply_gravity(&mut w, Vec3::new(0.0, -9.81, 0.0));
                    sard::physics::rigid_body::integrate_velocities(&mut w, 1.0 / 60.0);
                    sard::physics::rigid_body::integrate_positions(&mut w, 1.0 / 60.0);
                },
                criterion::BatchSize::LargeInput,
            );
        });

        group.bench_function("sync_transforms_500", |b| {
            b.iter_batched(
                || {
                    let (w, _) = setup_scene(n);
                    w
                },
                |mut w| {
                    sard::physics::rigid_body::sync_transforms(&mut w);
                },
                criterion::BatchSize::LargeInput,
            );
        });
        group.finish();
    }
}

// ---------------------------------------------------------------------------
// Mass physics (continuous spawn + step, mirrors the mass_physics demo)
// ---------------------------------------------------------------------------

fn bench_mass_physics(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("mass_physics/spawn_rate");
        group.sample_size(10);
        for &spawn_per_frame in &[1, 3, 10] {
            group.bench_with_input(
                BenchmarkId::from_parameter(spawn_per_frame),
                &spawn_per_frame,
                |b, &spf| {
                    b.iter_batched(
                        || setup_mass_scene(0),
                        |(mut world, mut physics)| {
                            run_mass_physics(&mut world, &mut physics, 60, spf, 0);
                        },
                        criterion::BatchSize::LargeInput,
                    );
                },
            );
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("mass_physics/initial_bodies");
        group.sample_size(10);
        for &initial in &[0, 100, 500] {
            group.bench_with_input(
                BenchmarkId::from_parameter(initial),
                &initial,
                |b, &init| {
                    b.iter_batched(
                        || setup_mass_scene(init),
                        |(mut world, mut physics)| {
                            run_mass_physics(&mut world, &mut physics, 60, 3, init);
                        },
                        criterion::BatchSize::LargeInput,
                    );
                },
            );
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("mass_physics/stress");
        group.sample_size(10);
        group.bench_function("300frames_3spawn", |b| {
            b.iter_batched(
                || setup_mass_scene(0),
                |(mut world, mut physics)| {
                    run_mass_physics(&mut world, &mut physics, 300, 3, 0);
                },
                criterion::BatchSize::LargeInput,
            );
        });
        group.finish();
    }
}

// ---------------------------------------------------------------------------
// Sleep effect (stable scene where bodies settle and sleep)
// ---------------------------------------------------------------------------

fn bench_sleep_effect(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("sleep/settled_scene");
        group.sample_size(10);
        for &n in &[100, 500] {
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_batched(
                    || {
                        // Pre-run the scene so bodies settle and start sleeping
                        let (mut world, mut physics) = setup_scene(n);
                        for _ in 0..300 {
                            physics.step(&mut world, 1.0 / 60.0);
                        }
                        (world, physics)
                    },
                    |(mut world, mut physics)| {
                        // Measure stepping a settled scene (most bodies should be sleeping)
                        for _ in 0..60 {
                            physics.step(&mut world, 1.0 / 60.0);
                        }
                    },
                    criterion::BatchSize::LargeInput,
                );
            });
        }
        group.finish();
    }
}

// ---------------------------------------------------------------------------
// GPU physics
// ---------------------------------------------------------------------------

fn bench_gpu_physics(c: &mut Criterion) {
    use sard_bench::{
        create_headless_context, run_gpu_mass_physics, setup_gpu_mass_scene, setup_gpu_scene,
    };

    let ctx = match create_headless_context() {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("GPU benchmarks skipped: {e}");
            return;
        }
    };

    // GPU vs CPU pipeline step
    {
        let mut group = c.benchmark_group("gpu/pipeline_step");
        group.sample_size(20);
        for &n in &[100, 500, 1000] {
            // GPU step
            group.bench_with_input(BenchmarkId::new("gpu", n), &n, |b, &n| {
                b.iter_batched(
                    || setup_gpu_scene(&ctx, n).expect("GPU scene setup"),
                    |(mut world, mut physics)| {
                        physics.step_gpu(&mut world, 1.0 / 60.0, &ctx);
                    },
                    criterion::BatchSize::LargeInput,
                );
            });
            // CPU step for comparison
            group.bench_with_input(BenchmarkId::new("cpu", n), &n, |b, &n| {
                b.iter_batched(
                    || setup_scene(n),
                    |(mut world, mut physics)| {
                        physics.step(&mut world, 1.0 / 60.0);
                    },
                    criterion::BatchSize::LargeInput,
                );
            });
        }
        group.finish();
    }

    // GPU broadphase stage only
    {
        let mut group = c.benchmark_group("gpu/broadphase");
        group.sample_size(20);
        for &n in &[256, 500, 1000, 2000] {
            let (world, physics) = setup_gpu_scene(&ctx, n).expect("GPU scene setup");
            let gpu_physics = physics
                .gpu_physics_ref()
                .expect("GPU physics not initialized");

            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| {
                    let (body_count, _entity_map, _max_extent) =
                        gpu_physics.upload_aabbs(&ctx, &world);
                    gpu_physics.dispatch_broadphase(&ctx, body_count);
                    gpu_physics.readback_pairs(&ctx)
                });
            });
        }
        group.finish();
    }

    // GPU mass physics
    {
        let mut group = c.benchmark_group("gpu/mass_physics");
        group.sample_size(10);

        // GPU
        group.bench_function("gpu_60frames_3spawn", |b| {
            b.iter_batched(
                || setup_gpu_mass_scene(&ctx, 0).expect("GPU mass scene"),
                |(mut world, mut physics)| {
                    run_gpu_mass_physics(&mut world, &mut physics, &ctx, 60, 3, 0);
                },
                criterion::BatchSize::LargeInput,
            );
        });

        // CPU for comparison
        group.bench_function("cpu_60frames_3spawn", |b| {
            b.iter_batched(
                || setup_mass_scene(0),
                |(mut world, mut physics)| {
                    run_mass_physics(&mut world, &mut physics, 60, 3, 0);
                },
                criterion::BatchSize::LargeInput,
            );
        });

        group.finish();
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_broadphase,
    bench_narrowphase,
    bench_solver,
    bench_pipeline,
    bench_mass_physics,
    bench_sleep_effect,
    bench_gpu_physics,
);
criterion_main!(benches);
