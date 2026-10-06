# AGENTS.md

This file provides guidance to coding agents working in this repository.

## Project Overview

Sard is a 3D engine built on wgpu with ECS (hecs), physics simulation, and GPU compute. Primary use case is robotics visualization with URDF support. Architecture inspired by [three-d](https://github.com/asny/three-d).

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
5. **physics/** - Rigid body simulation, collision detection (feature="physics")
6. **ecs/** - hecs ECS integration, components, systems (feature="ecs")
7. **engine/** - Game loop with App trait, fixed/variable timestep (feature="engine")
8. **window/** - winit abstraction (feature="window")
9. **gui/** - Text rendering with glyphon (feature="gui")
10. **urdf/** - URDF robot model loading

Shaders are in `src/shaders/` as WGSL files, compiled at runtime. Compute shaders in `src/shaders/compute/`, effect shaders in `src/shaders/effects/`.

## Feature Flags

```toml
default = ["window"]
window = ["dep:winit"]
gui = ["dep:glyphon"]
compute = []
ecs = ["dep:hecs"]
physics = ["ecs"]           # physics requires ecs
gpu-physics = ["physics"]
engine = ["ecs", "window"]
full = ["engine", "physics", "gpu-physics", "gui"]
```

## Key Patterns

**Generic container pattern**: `Gm<G: Geometry, M: Material>` combines any geometry with any material. Implements `Object` trait for rendering dispatch.

**ECS game loop pattern** (feature="engine"):
```rust
struct MyApp;
impl App for MyApp {
    fn init(&mut self, ctx: &WgpuContext, world: &mut hecs::World) { /* setup */ }
    fn update(&mut self, world: &mut hecs::World, ctx: &SystemContext) { /* per-frame */ }
    fn fixed_update(&mut self, world: &mut hecs::World, dt: f32) { /* physics timestep */ }
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

## Physics Pipeline

Fixed-timestep simulation (default 1/60s):
1. Apply gravity → 2. Broadphase (sweep-and-prune AABB) → 3. Narrowphase (GJK/EPA, SAT) → 4. Integrate velocities → 5. Solve contacts (sequential impulse, 8 iterations) → 6. Integrate positions → 7. Sync transforms to ECS → 8. Clear forces

GPU physics (feature="gpu-physics"): GPU broadphase activated when body count >= 256. Narrowphase/solver remain on CPU.

Collider shapes: `Sphere`, `Box`, `Capsule`, `Cylinder`, `ConvexHull` - each with GJK support function.

## ECS Components

- **Transform/GlobalTransform/Parent/Children** - Transform hierarchy with propagation system
- **RigidBody** - Physics body (Dynamic/Static/Kinematic) with mass, velocity, damping, restitution
- **Collider** - Collision shape with offset and sensor flag
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

**New Collider Shape**: Add variant to `ColliderShape` enum, implement GJK support function in `collider.rs`, add narrowphase test in `narrowphase.rs`.

## Dependencies

- `wgpu` 30 - GPU backend
- `glam` 0.33 - Math (Vec3, Mat4, Quat, etc.)
- `hecs` 0.11 - ECS (optional)
- `urdf-rs` 0.9 - URDF parsing
- `winit` 0.30 - Window management (optional)
- `glyphon` 0.12 - Text rendering (optional)
