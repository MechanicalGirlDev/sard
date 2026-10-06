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
| `physics` | No | CPU rigid-body simulation; enables `ecs` |
| `gpu-physics` | No | GPU-assisted physics; enables `physics` |
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
