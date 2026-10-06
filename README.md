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
| `engine` | No | ECS game loop; enables `ecs` and `window` |
| `full` | No | Engine, physics, GPU physics, and GUI |

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
pinned to Git revision `9fea04efe38d9eb02541057962848532e755b3e4`.
Sard contains no collision detector, contact solver, integrator, or physics compute shader.
Its `PhysicsWorld` only registers native Tessera `SceneBody` ECS components, steps Tessera,
and copies solved poses into optional rendering transforms.

`PhysicsConfig` is Tessera's `ArticulatedWorldParams`; its default world is Z-up with a
finite ground at Z = 0. `RigidBody` is its `SceneBody`, which owns colliders, mass, inertia,
velocities, force, and pose. Edit the body's native pose to teleport it. `ColliderMaterial`
is an optional ECS component applied to that body's colliders. Physics entities use
world-space transforms and should not have an ECS parent.

The previous Sard collider, sensor, damping, torque-accumulator, solver, and GPU-initialization
APIs have been removed. Use native types through `sard::physics::tessera` and
`sard::physics::nalgebra`. Tessera's scene bodies do not currently expose trigger sensors,
external torque, or damping. Native collision geometry includes spheres, boxes, capsules,
cylinders, and prepared convex hulls, including body-local offsets and rotations.

GPU physics requires an owned `sard::physics::GpuContactDevice`; it does not reuse Sard's
render device because Tessera currently uses wgpu 28 while rendering uses wgpu 30.
Device and simulation errors propagate, without a Sard CPU fallback. This Tessera path
uses GPU contact detection and solving with CPU world preparation and pose integration.
App `init`, `update`, and `fixed_update` callbacks return `anyhow::Result<()>`; `run_app`
exits and returns callback errors rather than continuing after failed physics. Physics demos
step Tessera in the engine's bounded fixed-update loop, using durations in `f64` seconds.

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
