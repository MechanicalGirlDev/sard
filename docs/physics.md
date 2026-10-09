# Native physics ownership

Physics uses Tessera coordinates (Z up), seconds, meters, and native `nalgebra`
types. Sard does not implement a collision detector, solver, material law, or
physics fallback.

## Articulated robots and ECS bodies

Load native URDF or MJCF XML through `physics::urdf` or `physics::mjcf`. Keep the
returned link/joint metadata. Mesh loading is an injected resolver rather than
a dependency on the application's directory layout. MJCF supports the native
rigid-body subset, not arbitrary MuJoCo models.

`PhysicsWorld::from_tessera(loaded.world)` accepts a robot world without existing
free scene bodies. `RigidBody` ECS components become its native free bodies and
share contacts with the robot. `step_with_efforts` and `step_gpu_with_efforts`
forward generalized efforts in native coordinate order. The immutable
`tessera_world` accessor exposes the solved robot state and `link_poses`.

For controllers that need direct native drive, reset, topology, or state mutation,
own the native world instead. Step it once and use `publish_poses` to publish its
current link/body poses into visual-only entities. This preserves visual scale.
Publication validates the whole batch before writing and rejects missing entities,
parents, non-finite poses, and competing `RigidBody` ownership.

Native scene/body/environment indices are dense indices. The owner must update
bindings after removal. No Sard component changes those indices into stable handles.

## Resident rigid bodies and independent environments

`physics::gpu::rigid` exports native primitive worlds and batches; `shape`,
`state`, `joints`, `ray_query`, and `point_query` export their native types.
These worlds integrate and solve on Tessera's device independently of
`PhysicsWorld`'s articulated GPU contact bridge.

Create isolated environments, queue native steps, explicitly read back the
required environments, then pass `(Entity, GpuRigidBodyState)` pairs to
`publish_gpu_poses`. Resetting one environment must not reset another. Update
visual bindings after native topology edits.

Environment-range edits preserve unrelated pending forces and sleep history.
The legacy single-body/primitive edit methods still discard pending forces,
as their native contract specifies. Contact warm-start caches are rebuilt.

Tessera uses wgpu 28; Sard rendering uses wgpu 30. Keep separate owned devices.
GPU simulation errors propagate rather than switching to a Sard solver.

## MPM and coupling

Enable `mpm` for `physics::mpm::{MpmWorld, MpmParticle, MaterialModel}` and native
emitters, chunks, removal/cutting, sampling, bounds, and CPU rigid synchronization.
Enable `gpu-mpm` for native transfers, resident sessions, and `GpuRigidMpmCoupler`.

`GpuMpmResidentSession::submit_steps` queues 1 through 64 fixed substeps per call.
`world()` still contains the last synchronized CPU snapshot. Call `synchronize`
before inspecting particles, editing topology, or converting them with
`mpm_particle_data`. The adapter omits disabled points, retains fixed points, and
zeros rendering velocities. Pass `Vec3::ZERO` as the acceleration to
`ParticleSystem::new(ctx, data, Vec3::ZERO, quad_size)` so rendering does not
simulate their motion again.

Rigid and MPM resident sessions used by a coupler must share the same native
device and queue. Coupling modes have different stepping responsibilities:

| Native operation | Responsibility |
| --- | --- |
| `submit_steps` / `step` on the coupler | Update obstacles and step MPM one-way; step rigid bodies separately first |
| `submit_two_way_step` / `step_two_way` | Transfer MPM reactions into rigid velocities; rigid pose integration is separate |
| `submit_coupled_step` / `step_coupled` | Transfer reactions and advance the rigid solver, including contacts and joints |
| `submit_mpm_owned_step` / `step_mpm_owned` | MPM advances rigid velocity and pose; do not also step those bodies with the rigid solver |

Recreate the coupler after body/shape/material/ground topology changes. Its CPU
obstacle snapshot is not automatically refreshed from live GPU poses. Native
resident errors invalidate or reject the operation; they do not silently fall
back. Tessera's direct transfer path separately documents its CPU treatment of
ill-conditioned material states.

## RGB, depth, and segmentation

`SensorCamera` owns offscreen targets but no physics state. Pass an explicit
scene of borrowed non-instanced triangle-list geometry with transforms and body
IDs. Share an ID across visuals belonging to the same body. ID zero is reserved
for background.

Standard `ParticleSystem`/`Sprites` billboard geometry is expanded using the
same camera-facing corner convention as the renderer. Its vertex normal XY
stores corner offsets, not surface normals. Custom geometry using that standard
layout must report `Geometry::is_billboard()`.

`SensorChannels` independently selects packed sRGB bytes, axial `f32` metric
depth, and exact `u32` segmentation. Outputs are top-to-bottom row-major; depth
and segmentation backgrounds are zero. Depth is not Euclidean ray length.
Occlusion and clipping use the same depth test for all channels.

Use `attach_to_body(world_from_body, body_from_camera)` after each solved pose
update. The mount is rigid, with -Z forward and +Y up. Resizing one camera does
not modify another; an invalid resize leaves its existing targets intact.

RGB uses opaque linear base colors and optional standard `VertexPNUC` UV image
textures (RGBA/BGRA8). This pass does not evaluate lighting, transparency, or
shadows. It is separate from material-driven PBR rendering.

## Published native dependency and local development

Both native crates use the reachable Tessera Git revision
`95213c183d4cf59f0f2d4b36c3e6a6e86a08e7d5`. It includes native IK,
Neo-Hookean sand, scene wrench/impulse methods, and topology preservation.
Ordinary builds require no local patch.

For local verification without a persistent manifest override, use both
command-scoped patches, replacing the paths with your Tessera checkout:

```sh
cargo check --all-features \
  --config "patch.'https://github.com/MechanicalGirlDev/tessera'.tessera-physics.path='C:/Users/nop/dev/mechanicalgirl/reiny-ecosystem/tessera/crates/tessera-physics'" \
  --config "patch.'https://github.com/MechanicalGirlDev/tessera'.tessera-mpm.path='C:/Users/nop/dev/mechanicalgirl/reiny-ecosystem/tessera/crates/tessera-mpm'"
```

When publishing future native changes, pin both crates to the same reachable
full Git SHA and regenerate the lockfile using that source. Do not publish a
lockfile resolved through the local patches.
