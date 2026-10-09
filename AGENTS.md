# AGENTS.md

This file provides guidance to coding agents working in this repository.

## Project Overview

Sard is a 3D engine built on wgpu with ECS (hecs), Tessera physics integration, and GPU compute. Primary use case is robotics visualization with URDF support. Architecture inspired by [three-d](https://github.com/asny/three-d).

## Build Commands

```bash
cargo build                        # Debug build (includes window feature)
cargo build --release              # Release build
cargo build --all-features         # All features
cargo build --no-default-features  # Minimal build (no window)
cargo build --features ecs         # ECS only
cargo build --features engine      # Engine (ECS + window)
cargo build --features full        # Everything

cargo test --lib                   # Run library tests
cargo test --all-features          # All tests
cargo test test_name               # Run specific test

cargo check --all-features         # Type checking
cargo clippy --all-features        # Linting
cargo fmt                          # Format code
```

Examples are workspace members in `examples/`:
```bash
cargo run -p hello_cube                    # Run an example
cargo run -p physics_demo                  # Physics example
cargo check --workspace                    # Check all examples and benchmarks
```

Benchmarks are in `benchmarks/`:
```bash
cargo bench -p sard-bench --bench physics       # Criterion benchmarks
cargo bench -p sard-bench --bench physics_iai   # IAI-Callgrind benchmarks
```

## Architecture

Ten-layer design from low to high level:

1. **context/** - wgpu wrapper (`WgpuContext` with Arc-wrapped Device/Queue)
2. **core/** - GPU primitives (buffers, textures, pipelines, vertex types)
3. **compute/** - Compute shader dispatch (`ComputeDispatcher`), GPU-CPU readback (`read_buffer_sync`, `read_back_async`)
4. **renderer/** - High-level rendering (cameras, materials, geometry, lights, shadows, culling)
5. **physics/** - Native Tessera world/ECS pose bridge (feature="physics")
6. **ecs/** - hecs ECS integration, components, systems (feature="ecs")
7. **engine/** - Game loop with App trait, fixed/variable timestep (feature="engine")
8. **window/** - winit abstraction (feature="window")
9. **gui/** - Text rendering with glyphon (feature="gui")
10. **urdf/** - URDF robot model loading

Rendering shaders are in `src/shaders/` as WGSL files, compiled at runtime; effect shaders are in `src/shaders/effects/`. Physics shaders and solvers belong to the Tessera dependency.

## Feature Flags

```toml
default = ["window"]
window = ["dep:winit"]
gui = ["dep:glyphon"]
compute = []
ecs = ["dep:hecs"]
physics = ["ecs", "dep:tessera-physics", "dep:nalgebra"]
gpu-physics = ["physics", "tessera-physics/gpu-contact"]
mpm = ["physics", "dep:tessera-mpm", "tessera-mpm/rigid-coupling"]
gpu-mpm = ["mpm", "gpu-physics", "tessera-mpm/gpu-rigid-coupling"]
engine = ["ecs", "window"]
full = ["engine", "physics", "gpu-physics", "gpu-mpm", "gui"]
```

## Key Patterns

**Generic container pattern**: `Gm<G: Geometry, M: Material>` combines any geometry with any material. Implements `Object` trait for rendering dispatch.

**ECS game loop pattern** (feature="engine"):
```rust
struct MyApp;
impl App for MyApp {
    fn init(&mut self, ctx: &WgpuContext, world: &mut hecs::World) -> anyhow::Result<()> { /* setup */ Ok(()) }
    fn update(&mut self, world: &mut hecs::World, ctx: &SystemContext) -> anyhow::Result<()> { /* per-frame */ Ok(()) }
    fn fixed_update(&mut self, world: &mut hecs::World, dt: f64) -> anyhow::Result<()> { /* physics timestep */ Ok(()) }
    fn post_render(&mut self, world: &mut hecs::World, ctx: &SystemContext) { /* GUI/debug */ }
}
run_app(WindowSettings::default(), GameLoopConfig::default(), MyApp)?;
```

`run_app` automatically runs: fixed_update → update → transform_system → culling_system → render_system → post_render.

**Uniform binding groups**:
- Group 0: Camera uniform (view, projection matrices, camera position, viewport)
- Group 1: Model uniform (model matrix, normal matrix)
- Group 2: Material-specific uniforms

**Vertex types** (in order of complexity):
- `VertexP` - position only (shadow mapping)
- `VertexPC` - position + color (lines)
- `VertexPN` - position + normal (lighting)
- `VertexPNUC` - full (position, normal, UV, color)

**Render loop pattern** (without engine):
```rust
window.render_loop(state, |state, frame| {
    // frame: FrameInput with ctx, viewport, delta_time, events, surface_format
    FrameOutput::default()
})
```

**ECS bridge**: `spawn_gm()` in `src/ecs/bridge.rs` converts `Gm<G,M>` into ECS entities with Transform, GlobalTransform, MeshRenderer, FrustumCullable, Visible components.

## Physics Integration

Tessera owns collision detection, contact solving, integration, and sleeping. Do not add a
Sard solver or physics compute shader. `PhysicsWorld` tracks native scene-body slots and
synchronizes solved Z-up world poses into optional rendering `Transform`/`GlobalTransform`.
`PhysicsConfig` is native `ArticulatedWorldParams`, including an implicit finite ground at Z=0.

Rigid-body components are native `SceneBody` values; they own collider geometry, pose,
inertia, velocities, and persistent force. An optional native `ColliderMaterial` overrides
all colliders on that entity. Use Tessera setters for external motion/material edits and
native add/remove APIs for topology changes so contact/sleep state remains consistent.

`gpu-physics` enables Tessera GPU contact detection and solving through its owned
`GpuContactDevice`, not Sard's rendering context. Their wgpu versions currently differ.
GPU errors propagate; there is no local physics fallback. Tests use serial execution
to avoid parallel device initialization. CI explicitly opts into software Vulkan computation.

Advanced native worlds and resident batches keep their own physics owner. Publish
their solved poses to visual-only entities with `publish_poses`/`publish_gpu_poses`;
do not attach competing `RigidBody` components. MPM sessions require explicit
synchronization before snapshot conversion. See `docs/physics.md` for coupling
step ownership and `docs/nexus-3d-coverage.md` for evidence and publication status.

`SensorCamera` renders explicit scenes into RGB, axial metric depth, and exact
body IDs. Its opaque color/UV-texture pass has no lighting or shadows. Test it
with `cargo test --test sensor_camera --all-features -- --test-threads=1`.

## ECS Components

- **Transform/GlobalTransform/Parent/Children** - Transform hierarchy with propagation system
- **RigidBody** - Native Tessera `SceneBody`; dynamic/static/kinematic state and owned colliders
- **ColliderMaterial** - Optional native Tessera contact material
- **MeshRenderer** - Holds `MeshHandle(Arc<dyn Geometry>)` + `MaterialHandle(Arc<dyn Material>)`
- **CameraComponent/LightComponent** - Rendering components
- **FrustumCullable/Visible** - Culling markers

## Adding New Components

**New Material**:
1. Create `src/renderer/material/my_material.rs` implementing `Material` trait (pipeline, bind groups, update_uniforms)
2. Add shader `src/shaders/my_material.wgsl`
3. Export from `src/renderer/material/mod.rs`

**New Geometry**: Implement `Geometry` trait (vertex_buffer, index_buffer, draw_count, aabb).

**New Effect**: Implement `Effect` trait in `src/effect/`, add shader in `src/shaders/effects/`, add to `EffectChain`.

**Physics changes**: Use Tessera's native scene-body/collider APIs. Physics algorithms and new shapes belong in Tessera, not Sard.

## Dependencies

- `wgpu` 30 - GPU backend
- `glam` 0.33 - Math (Vec3, Mat4, Quat, etc.)
- `hecs` 0.11 - ECS (optional)
- `tessera-physics`, `tessera-mpm` - Optional native physics, both pinned to published Git revision `95213c183d4cf59f0f2d4b36c3e6a6e86a08e7d5`
- `nalgebra` 0.30 - Native Tessera math types (optional)
- `urdf-rs` 0.9 - URDF parsing
- `winit` 0.30 - Window management (optional)
- `glyphon` 0.12 - Text rendering (optional)
