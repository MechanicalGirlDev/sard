# Architecture

Sard is a layered library. Dependencies should point downward through this list:

1. `context` owns the wgpu device and queue.
2. `core` provides buffers, textures, render targets, pipelines, and vertex layouts.
3. `compute` and `effect` build reusable GPU operations on `context` and `core`.
4. `renderer` provides material-driven geometry, lighting, cameras, and controls.
5. `scene` provides a retained scene optimized for telemetry viewers.
6. `ecs` bridges renderer resources into hecs components and systems.
7. `physics` operates on ECS rigid bodies and colliders, with optional GPU acceleration.
8. `window`, `gui`, and `engine` provide application-facing integration.
9. `urdf` loads robot models into renderer or ECS-facing representations.

## Rendering APIs

Use `renderer` when objects own a `Geometry` and `Material`, require specialized materials, or are
managed by the ECS bridge. Its central abstraction is `Gm<G, M>`.

Use `scene` when a viewer rebuilds a large, heterogeneous scene from telemetry. It shares pipelines
per primitive type, retains CPU mesh data for picking, and can render into a window, off-screen
target, or XR eye.

The two APIs intentionally share foundational types such as `Viewer`, `RenderTarget`, and `Aabb`.
New low-level rendering utilities should be placed in `core` or a private common module instead of
being duplicated between `renderer` and `scene`.

## Repository layout

- `src/` contains the published `sard` library.
- `examples/*` are runnable workspace packages and share the root `Cargo.lock`.
- `benchmarks/` is the `sard-bench` workspace package.
- `src/shaders/` contains runtime-compiled WGSL, grouped by general, effect, and compute usage.

Workspace packages deliberately use one dependency resolution so examples exercise the same wgpu
stack as the library.
