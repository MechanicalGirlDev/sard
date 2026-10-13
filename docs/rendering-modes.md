# Rendering modes

`sard::Renderer` renders one opaque triangle scene using `RenderMode::Rasterization`,
`RenderMode::RayTracing`, or `RenderMode::PathTracing`. Switching modes does not
rebuild geometry or change camera ownership. The default is rasterization.

```rust,no_run
use sard::{Camera, RenderConfig, RenderMode, RenderObject, Renderer, SurfaceMaterial};
use sard::renderer::geometry::MeshData;
use sard::glam::{Mat4, Vec3};
# fn main() -> anyhow::Result<()> {
# let ctx = sard::WgpuContext::new_blocking(None)?;
# let format = sard::wgpu::TextureFormat::Rgba8UnormSrgb;
# let target = sard::scene::OffscreenTarget::new(&ctx, 320, 240, format);
let mesh = MeshData::cuboid(Vec3::ONE);
let mut renderer = Renderer::new(&ctx, format, RenderConfig::default())?;
renderer.set_scene(&[RenderObject {
    mesh: &mesh,
    transform: Mat4::IDENTITY,
    material: SurfaceMaterial::default(),
}])?;
let camera = Camera::new_perspective(
    Vec3::new(3.0, 2.0, 4.0), Vec3::ZERO, Vec3::Y, 45.0, 4.0 / 3.0, 0.1, 100.0,
);
renderer.set_mode(RenderMode::PathTracing);
renderer.render(&camera, &target.target(&ctx))?;
# Ok(()) }
```

Existing `Gm<Mesh, PbrMaterial>` objects expose `render_object()`. The snapshot
borrows their retained CPU mesh and copies the current transform and surface
parameters. `MeshData`, `MeshAsset`, and `MeshHandle` remain available through
`sard::scene` and are now also available through `sard::renderer::geometry`.

Call `set_scene` again after geometry, transforms, or materials change. Scene
uploads build a world-space BVH once; mode switches reuse it. A failed scene upload
leaves the old snapshot intact. `set_lighting` controls a directional sun and a
constant environment, using linear radiance in every mode.

| Mode | Rendering |
| --- | --- |
| Rasterization | Cook-Torrance metallic-roughness PBR, directional shadow mapping with 3x3 PCF, approximate ambient environment |
| RayTracing | GPU triangle/BVH intersections, exact ray visibility, direct PBR shading, deterministic mirror reflection bounces |
| PathTracing | GPU Monte Carlo diffuse/GGX transport, directional-light next-event estimation, emissive surfaces, environment and progressive accumulation |

Ray tracing and path tracing use portable WGSL GPU computation, not hardware
RT acceleration or vendor-specific extensions. No optional feature is required.
`compute` is not needed. The same APIs work with window and offscreen targets,
including a build without default features.

Path tracing adds one sample per `render` call. `accumulated_samples()` reports
progress. Scene uploads, mode changes, lighting changes, camera matrices/position
changes, target resizing, and bounce-count changes reset the accumulation.
`reset_accumulation()` explicitly restarts the seeded sequence. All modes honor
perspective/orthographic near and far clipping and write primary-surface depth
into the target's depth attachment, when supplied. Rendering covers the entire
target; use a separate target for an inset viewport.

The shared contract is opaque triangle meshes with base color, emission,
metallic, roughness, and ambient occlusion. UV textures, vertex colors, alpha
blending, instanced GPU geometry, lines, billboards, point/spot lights, and
environment maps are not inputs to this renderer. Existing material-driven,
retained-scene, ECS, and sensor rendering APIs remain usable for those workloads.
Ray-mode secondary reflections are mirrors; roughness shapes direct highlights.
Path tracing samples rough reflections and diffuse indirect illumination.
AO applies to raster ambient shading only; traced visibility replaces it.

`max_bounces` is configurable from 1 to 64 (default 6). Finite bounce counts
truncate transport. Low-sample path tracing is noisy and has no denoiser.
Meshes are double-sided; transforms must be finite, affine and invertible.
Extremely small geometry or very large world coordinates may exceed the tracer's
floating-point precision. Resolution and scene sizes are checked against device
storage/texture limits.

Run `cargo run -p pbr` and use **1**, **2**, **3** to switch modes. Orbit/zoom
controls change the same camera in every mode; stop moving to accumulate paths.
