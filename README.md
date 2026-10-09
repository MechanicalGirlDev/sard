# sard

3D rendering library built on [wgpu](https://github.com/gfx-rs/wgpu).

## Features

- **wgpu backend** - Cross-platform graphics with WebGPU
- **PBR materials** - Physically-based rendering with Phong and PBR materials
- **Shadow mapping** - Directional light shadows
- **Post-processing effects** - FXAA, fog, and extensible effect chain
- **Camera controls** - Orbit, fly, and first-person controls
- **URDF support** - Load robot models from URDF files
- **Text rendering** - Optional GUI text rendering with glyphon
- **Instanced rendering** - Efficient rendering of many identical objects
- **Tessera physics** - Optional native rigid bodies, contacts, and GPU contact solving
- **Native MPM** - Elasticity, sand, fluid, snow, resident GPU particles, and rigid coupling
- **Robot cameras** - Offscreen RGB, metric depth, and exact body segmentation

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sard = "0.1.1"
```

### Feature Flags

| Feature  | Default | Description |
|----------|---------|-------------|
| `window` | Yes     | Window management with winit |
| `gui`    | No      | Text rendering with glyphon |
| `compute` | No     | GPU compute dispatch and readback utilities |
| `ecs` | No | hecs components and rendering systems |
| `physics` | No | Tessera CPU physics and ECS pose synchronization; enables `ecs` |
| `gpu-physics` | No | Tessera GPU contact detection and solving; enables `physics` |
| `mpm` | No | Native MPM worlds, materials, sampling, and rigid coupling; enables `physics` |
| `gpu-mpm` | No | Native resident MPM and device-side rigid coupling; enables `mpm` and `gpu-physics` |
| `engine` | No | ECS game loop; enables `ecs` and `window` |
| `full` | No | Engine, rigid/MPM CPU and GPU physics, and GUI |

## Architecture

The library is organized into layers:

1. **context** - Core wgpu wrapper (Device, Queue)
2. **core** - Mid-level abstractions (buffers, textures, pipelines)
3. **renderer** - High-level rendering (cameras, materials, objects, lights)
4. **window** - Window management with winit (optional)
5. **gui** - Text rendering (optional)
6. **urdf** - URDF robot model support

Compute, ECS, physics, and the application loop are optional higher-level layers. Sard also
provides two rendering styles: `renderer` for material-driven objects and `scene` for retained,
telemetry-oriented scenes. See [Architecture](docs/architecture.md) for the dependency boundaries
and guidance on choosing between them.

### Physics

Physics is provided by [Tessera](https://github.com/MechanicalGirlDev/tessera),
pinned to Git revision `95213c183d4cf59f0f2d4b36c3e6a6e86a08e7d5`.
Sard contains no collision detector, contact solver, integrator, or physics compute shader.
Its `PhysicsWorld` registers native Tessera `SceneBody` ECS components, steps Tessera,
and copies solved poses into optional rendering transforms. `from_tessera` also accepts
a loaded articulated robot, and `step_with_efforts` forwards native joint efforts.

`PhysicsConfig` is Tessera's `ArticulatedWorldParams`; its default world is Z-up with a
finite ground at Z = 0. `RigidBody` is its `SceneBody`, which owns colliders, mass, inertia,
velocities, force, and pose. Edit the body's native pose to teleport it. `ColliderMaterial`
is an optional ECS component applied to that body's colliders. Physics entities use
world-space transforms and should not have an ECS parent.

Use native types through `sard::physics::tessera` and `sard::physics::nalgebra`.
For direct ownership of articulated worlds or resident GPU batches, publish their
authoritative snapshots with `publish_poses` or `publish_gpu_poses` into visual-only
entities. Do not attach competing `RigidBody` components to those visuals. Native
collision geometry includes spheres, boxes, capsules, cones, cylinders, convex hulls,
polylines, triangle meshes, and compound body-local colliders.

GPU physics requires an owned `sard::physics::GpuContactDevice`; it does not reuse Sard's
render device because Tessera currently uses wgpu 28 while rendering uses wgpu 30.
Device and simulation errors propagate, without a Sard CPU fallback. This Tessera path
uses GPU contact detection and solving with CPU world preparation and pose integration.
App `init`, `update`, and `fixed_update` callbacks return `anyhow::Result<()>`; `run_app`
exits and returns callback errors rather than continuing after failed physics. Physics demos
step Tessera in the engine's bounded fixed-update loop, using durations in `f64` seconds.

`sard::physics::mpm` exposes the native MPM API with the `mpm` feature. With `gpu-mpm`,
it also exposes `GpuMpmResidentSession` and `GpuRigidMpmCoupler`. Explicitly synchronize
resident sessions before converting their current particles with `mpm_particle_data`.
The render adapter zeros velocities; construct `ParticleSystem` with zero acceleration
to display a frozen snapshot instead of extrapolating it with another solver.

`SensorCamera` borrows an explicit scene of `SensorObject` values. It provides packed
sRGB bytes, axial depth in meters, and exact `u32` body IDs; zero denotes background.
It uses opaque base colors and optional UV-mapped textures, without scene lighting
or shadows. Camera axes are -Z forward and +Y up. Attach it to the current solved body
pose and a rigid mounting transform with `attach_to_body`.

See [physics ownership and usage](docs/physics.md) and the
[Nexus capability and verification ledger](docs/nexus-3d-coverage.md). The additional
native IK, Neo-Hookean sand, scene torque/impulse controls, and topology preservation
are included in the published Tessera dependency pin.

## Examples

Examples and benchmarks are members of the repository workspace and share the root lockfile:

```bash
cargo run -p hello_cube
cargo run -p physics_demo
cargo check --workspace
```

## Example

```rust
use sard::{Window, WindowSettings, FrameOutput};
use sard::renderer::{Camera, OrbitControl};
use sard::urdf::RobotModel;
use sard::core::ClearState;
use glam::Vec3;

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("Robot Viewer"))?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        robot: Option<RobotModel>,
    }

    // Initialize and run your render loop...
    Ok(())
}
```

## Acknowledgments

This project is heavily inspired by [three-d](https://github.com/asny/three-d), a fantastic 3D rendering library for Rust. The architecture, API design, and many implementation patterns in sard are based on three-d's excellent work. We are deeply grateful to the three-d authors and contributors for creating such a well-designed and educational codebase.

## License

MIT License - see [LICENSE](LICENSE) for details.
