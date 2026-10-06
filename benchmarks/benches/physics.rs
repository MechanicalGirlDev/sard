//! Real Tessera world stepping (Criterion wall-clock measurements).

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion};
use sard_bench::{run_mass_physics, run_steps, setup_scene, Scene};

// A failed simulation must abort measurement, not produce a successful sample.
fn measured<T>(result: anyhow::Result<T>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("Tessera benchmark failed: {error:#}"),
    }
}

fn benchmarks(c: &mut Criterion) -> anyhow::Result<()> {
    for (name, scene, sizes, frames, presteps) in [
        (
            "cpu/uniform_spheres",
            Scene::Spheres,
            &[100, 500, 1000, 2000][..],
            1,
            0,
        ),
        (
            "cpu/mixed_shapes",
            Scene::Mixed,
            &[100, 500, 1000, 2000][..],
            1,
            0,
        ),
        (
            "cpu/sparse",
            Scene::Sparse,
            &[100, 500, 1000, 2000][..],
            1,
            0,
        ),
        ("cpu/stack", Scene::Stack, &[10, 50, 100, 500][..], 1, 0),
        (
            "cpu/pipeline_step",
            Scene::Falling,
            &[50, 100, 500, 1000][..],
            1,
            0,
        ),
        (
            "cpu/sustained_10steps",
            Scene::Falling,
            &[100, 500][..],
            10,
            0,
        ),
        (
            "cpu/after_300steps",
            Scene::Falling,
            &[100, 500][..],
            60,
            300,
        ),
    ] {
        let mut group = c.benchmark_group(name);
        group.sample_size(10);
        for &n in sizes {
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_batched(
                    || {
                        let (mut world, mut physics) = measured(setup_scene(scene, n));
                        measured(run_steps(&mut world, &mut physics, presteps));
                        (world, physics)
                    },
                    |(mut world, mut physics)| {
                        measured(run_steps(&mut world, &mut physics, frames));
                        black_box(world);
                    },
                    BatchSize::PerIteration,
                );
            });
        }
        group.finish();
    }

    let mut group = c.benchmark_group("cpu/mass_physics");
    group.sample_size(10);
    for (name, initial, frames, spawn) in [
        ("60frames_1spawn", 0, 60, 1),
        ("60frames_3spawn", 0, 60, 3),
        ("60frames_10spawn", 0, 60, 10),
        ("initial_100", 100, 60, 3),
        ("initial_500", 500, 60, 3),
        ("300frames_3spawn", 0, 300, 3),
    ] {
        group.bench_function(name, |b| {
            b.iter_batched(
                || measured(setup_scene(Scene::Mass, initial)),
                |(mut world, mut physics)| {
                    measured(run_mass_physics(
                        &mut world,
                        &mut physics,
                        frames,
                        spawn,
                        initial,
                    ));
                    black_box(world);
                },
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();

    #[cfg(feature = "gpu-physics")]
    {
        use sard::physics::GpuContactDevice;
        use sard_bench::{run_gpu_mass_physics, run_gpu_steps};

        // Tessera owns its GPU device; adapter/setup errors propagate to main.
        let device = GpuContactDevice::new()?;
        let mut group = c.benchmark_group("gpu/pipeline_step");
        group.sample_size(10);
        for &n in &[100, 256, 500, 1000, 2000] {
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_batched(
                    || measured(setup_scene(Scene::Falling, n)),
                    |(mut world, mut physics)| {
                        measured(run_gpu_steps(&mut world, &mut physics, &device, 1));
                        black_box(world);
                    },
                    BatchSize::PerIteration,
                );
            });
        }
        group.finish();
        let mut group = c.benchmark_group("gpu/mass_physics");
        group.sample_size(10);
        group.bench_function("60frames_3spawn", |b| {
            b.iter_batched(
                || measured(setup_scene(Scene::Mass, 0)),
                |(mut world, mut physics)| {
                    measured(run_gpu_mass_physics(
                        &mut world,
                        &mut physics,
                        &device,
                        60,
                        3,
                        0,
                    ));
                    black_box(world);
                },
                BatchSize::PerIteration,
            );
        });
        group.finish();
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut criterion = Criterion::default().configure_from_args();
    benchmarks(&mut criterion)?;
    criterion.final_summary();
    Ok(())
}
