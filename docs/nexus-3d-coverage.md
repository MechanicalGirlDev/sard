# Nexus 3D capability coverage

Reference: [dimforge/nexus](https://github.com/dimforge/nexus/tree/ca2cc91d6a3cabbed1ccff9b7f08b22d76c7865d),
revision `ca2cc91d6a3cabbed1ccff9b7f08b22d76c7865d` (workspace version 0.6.0).

This is an implementation and verification ledger, not a claim that every row
already works. Physics algorithms belong to Tessera. Sard owns API exposure,
ECS/rendering synchronization, and rendered sensor output.

## Scope

The target is the reference's actual 3D RBD and MPM contracts, including their
Rust/Python robot and state-control entry points. Internal kernel names and
backend-specific optimizations are not separate physics features. CUDA/Metal
graph capture does not imply that a WebGPU implementation must pretend to
support those backends.

The Nexus source contains an unimplemented GPU heightfield conversion
(`src_rbd/shapes/shape.rs`), while its MPM heightfield example uses sampled
geometry. It exposes no general rigid-body CCD or contact event stream in the
inspected GPU entry points. It does expose a multibody contact sensor: up to four
selected links report sums of normal constraint impulses, excluding friction.
That sensor is a required capability, not a Rapier CPU reexport.

## Inventory

The native APIs below are reachable through physics::tessera, physics::gpu, or
physics::mpm. The native additions are published at the shared Git pin below.
Coverage does not mean bit-identical trajectories across different solvers.

| Capability | Nexus evidence | Tessera/Sard status |
| --- | --- | --- |
| Dynamic, static, kinematic bodies and editable poses/velocities | `src_rbd/dynamics/body.rs`, `src/state.rs` | Native bodies and ECS synchronization; CPU and resident GPU suites |
| Mass and rotational inertia | `BodyDesc`, `LocalMassProperties`, `WorldMassProperties` | Native mass properties and world-frame inertia; CPU tests |
| Linear/angular forces and impulses | `src_rbd/lib.rs`, shader dynamics and Rapier world controls | Added native scene wrench/impulse methods and Python access; persistent torque and rotated-inertia tests |
| Friction, restitution and contact resolution | RBD solver and Rapier collider conversion | Native materials and CPU/GPU contact solvers |
| Spheres and boxes | `shape_from_parry` | Native scene and resident GPU shapes |
| Capsules | `shape_from_parry` | Native scene and resident GPU shapes |
| Cones and cylinders | `shape_from_parry` | Native scene and resident GPU shapes |
| Convex geometry | `shape_from_parry` | Native prepared hulls and resident GPU hull contact tests |
| Polylines | `shape_from_parry` | Native indexed segments and resident contact fixtures |
| Indexed triangle meshes | `shape_from_parry` | Native indexed geometry/BVH; resident tests cover primitive partners and shared edges |
| Multiple body-local colliders / compound bodies | `rbd_compound3.rs`, Rapier collider mapping | Native scene/link colliders with body-local transforms; CPU and Python scene tests |
| Spherical/ball joints | `dynamics/joint.rs`, `rbd_joint_ball3.rs` | Native articulated and resident constraints; CPU, Python, and GPU suites |
| Fixed joints | `dynamics/joint.rs`, `rbd_joint_fixed3.rs` | Native articulated and resident constraints; CPU, Python, and GPU suites |
| Prismatic joints | `dynamics/joint.rs`, `rbd_joint_prismatic3.rs` | Native articulated and resident constraints; Sard robot/ECS effort fixture |
| Revolute joints | `dynamics/joint.rs`, `rbd_joint_revolute3.rs` | Native articulated and resident constraints; CPU and GPU suites |
| Joint limits, PD/velocity motors and force limits | `convert_joint_motor`, robot controls | Native drives/limits and effort APIs; CPU/Python fixtures |
| Articulated trees, free/fixed roots and loop-closing joints | `dynamics/multibody`, `rbd_joints3.rs` | Native articulations and point/fixed closure constraints; CPU dynamics/loader tests |
| Armature, joint damping and friction loss | Robot dynamics and RBD multibody state | Native passive/coupled dynamics; CPU/MJCF tests |
| Resident GPU rigid-body integration | `RbdPipeline`, `RbdState` | Native resident owner plus explicit publish_gpu_poses; separate from the articulated hybrid bridge |
| Isolated batched environments | `NexusCapacities`, `add_environment` | Native primitive batches; Sard real-GPU fixture verifies overlapping environments and independent reset |
| Live body insertion/removal and topology updates | `insertion_removal.rs`, `add_rigid_bodies` | Added CPU cache remapping and GPU force/sleep preservation; native mutation fixtures |
| State readback, teleports and environment reset | `src/state.rs`, multibody reset templates | Native readback/reset/setters; Python and Sard GPU reset fixtures |
| Fixed timesteps, substeps and deterministic execution | `src/state.rs`, 3D determinism tests | Native fixed/substep APIs; CPU repeatability and resident sequential/batched fixtures |
| URDF loading and mesh resolution | 3D URDF example and Python loaders | Native XML loader, injected mesh resolver, joint/link metadata; CPU/Python and Sard robot fixtures |
| MJCF loading | MuJoCo Menagerie example and Python loaders | Native rigid-body subset with typed unsupported errors; CPU/Python fixtures, not arbitrary MuJoCo models |
| Robot forward/inverse kinematics and joint state/targets | Python robot API | Added pure FK/DLS IK, axis masks, reduced limits, floating/spherical state, coupling, and Python methods |
| Linear elastic MPM | `ParticleModel::ElasticLinear` | Native corotated law, matching the reference shader family; CPU/GPU material fixtures |
| Neo-Hookean elastic MPM | `ParticleModel::ElasticNeoHookean` | Native material and direct/resident GPU fixtures |
| Linear and cohesive Drucker-Prager sand | `Sand`, `cohesive_sand` | Native plasticity/cohesion and CPU/GPU fixtures |
| Neo-Hookean Drucker-Prager sand | `SandNeoHookean` | Added material, hardening/plasticity, GPU tag 6, Python variant; nine CPU/Vulkan/DX12 tests |
| Tait fluid, viscosity and tensile stiffness | `ParticleModel::Fluid` | Native fluid law, viscosity, and tensile parameters; CPU/GPU fixtures |
| Snow yield and compaction hardening | `ParticleModel::Snow` | Native projection/hardening and CPU/GPU fixtures |
| Particle chunks, emitters and removal/cutting | `add_particles`, `extend_chunk`, chunk removal | Native APIs exported by mpm; CPU/resident chunk and emitter tests |
| Mesh/terrain particle sampling | MPM sampling and heightfield example | Native surface/closed-volume sampling and mesh/terrain obstacles; MPM tests |
| Grid boundary handling, CPIC and substep/CFL controls | MPM pipeline/state | Native bounds, transfer colors/CPIC groups, sparse topology, and CFL; CPU/direct/resident fixtures |
| One-way rigid/MPM coupling | `BodyCoupling::OneWay` | Native CPU synchronization and live device-side obstacles; MPM coupling tests |
| Two-way rigid/MPM coupling and rigid MPM particles | `BodyCoupling::TwoWays`, MPM impulses/integration | Native reaction-only, coupled rigid-solver, and MPM-owned modes; distinct stepping contracts |
| Resident GPU MPM, queued stepping and readback | `MpmPipeline`, particle buffers | Exported resident sessions; Sard real-GPU fixture synchronizes four steps before frozen render conversion |
| Python access to native state and controls | `crates/nexus_python3d` | Native UniFFI physics bindings plus FK/IK, sand, wrench/impulse controls; 37 binding tests do not establish complete Nexus Python method parity |
| Offscreen RGB, metric depth and body segmentation | Nexus Python viewer sensor API | Added Sard camera attachments, explicit scenes, resize, optional channels, and UV textures; real-GPU fixture |
| Selected-link normal contact impulses | `multibody_set.rs:1194-1226`, `contact_sensor.rs` | Added host observations and native resident output; three GPU kernel/fault regressions passed |
| Motor targets from GPU action buffers | `multibody_set.rs:553-582` | Added native position/velocity scatter wired into resident stepping; required GPU control regression passed |
| Per-environment actuator substep delay | `multibody_set.rs:1094-1191` | Added device-side counters and resident delay-buffer updates; old targets held for exactly the configured substeps |
| Selective resident articulated reset templates | `rbd_state.rs:1063-1204`, `multibody/env_reset.rs` | Added immutable device snapshots, selected reset, floating offsets/twists, quaternion and drive restoration; four real-GPU owner/layout/fault regressions passed |
| Configurable resident recovery and warmstarting | `RbdSimParams`, native contact shader | Added recovery gain, speed cap, penetration tolerance, and cached-impulse multiplier; actual recovery-speed fixture passed |
| MJCF actuator metadata and ordered controls | Python `loaders.rs:131-178`, `nexus.rs:357-413` | Added native and Python metadata/ordered controls; CPU and binding regressions passed; feedback reevaluates each native substep |

Algorithm-specific solver parameters are not interchangeable. Nexus TGS iteration
count controls substeps; native contact iterations control constraint sweeps.
Native repeated resident steps and maximum CPU substep duration supply temporal
resolution. Resident recovery/warmstart parameters are meaningful native controls,
not compatibility fields pretending to implement Nexus frequency/damping formulas.

The current Nexus `main` revision was checked again on 2026-10-09 and remains
`ca2cc91d6a3cabbed1ccff9b7f08b22d76c7865d`. The new rows above correct omissions in
the original comparison. These additions were verified against the local Tessera
checkout, then published in `e4fc887` and `196f983`. Sard now pins the published
`196f9830e74fbecc92c89e22d4539ef14b130d09` source. Local implementation evidence and
ordinary Git-dependency verification are tracked separately.

The actuator subset follows the actual reference converter
[`rapier3d-mjcf 0.36.0 runtime`](https://github.com/dimforge/rapier/blob/v0.36.0/crates/rapier3d-mjcf/src/loader/runtime.rs):
motor, position, velocity, damper, and restoring/non-restoring general branches.
The reference does not automatically drive intvelocity/muscle/tendon controls.
Native imported controls remain distinct from the resident scalar motor mapping.
For contact sensors, native normal impulses include its fused restitution solve,
whereas Nexus samples before a separate restitution pass; friction is excluded.

## Verification rules

- Existing implementations need behavioral evidence, not just public reexports.
- CPU and GPU results are separate claims. Real GPU computation must run, not
  skip or silently switch to a Sard implementation.
- New tests use deterministic inputs and event-based async completion, without
  sleeps or timing-dependent assertions.
- Mutation preserves surviving body state and environment-local mappings. CPU
  insertion/removal remaps unrelated warm-start entries. GPU topology rebuilds
  preserve sleep history but invalidate contact warm-start caches explicitly.
  Environment edits preserve unrelated queued forces; legacy single-body edits
  discard pending forces. Pose/material edits may also invalidate caches.
- Tessera's existing user edits to licensing notices remain outside this work.

## Current baseline

Sard `627655b` uses Tessera revision `9fea04e` and only registers free scene
bodies in a zero-joint articulated world. Its basic CPU and GPU contact tests
are verified, but that does not expose all native robot, resident-batch, or MPM
features. The native checkout inspected for this work is `3c33cd0`.

## Executed evidence

These results are local to this checkout, not remote CI or published binaries.

| Check | Result |
| --- | --- |
| Native CPU physics library | 205 passed, including geometry/loaders, dynamics, IK, wrench/impulse, and cache remapping |
| Native Python 3D bindings | 37 passed, including GPU controls and state/reset, FK/IK, sand, and scene loads |
| Masked IK local-point regression | Failed before the fix; CPU suite and two Python IK tests passed after it |
| Native Neo-Hookean sand regression group | 9 passed; direct/resident GPU parity on required Vulkan and DX12 adapters |
| Native MPM full library run | 95 passed and one new sand failure; the failure was fixed and its nine-test group rerun successfully |
| Native topology force/sleep preservation | Required real-GPU regression passed |
| Sard all-feature library | 67 passed, including resident rigid environment isolation/reset and four-step MPM snapshot conversion |
| Sard bridge against local native additions | 15 passed after native-state upload change |
| Sard sensor camera | Three required real-GPU fixtures passed; rigid-body and resident MPM snapshots visually inspected from three directions; frozen billboard rendering, solved depth, exact IDs above 2^24, occlusion, attachments, channels, UV textures, and resize verified |
| Native strict Clippy | Physics, MPM, and Python crates: all targets/features passed with `-D warnings` |
| Sard strict Clippy against local additions | Entire workspace/all targets/features passed with `-D warnings` |
| Sard against published Tessera `95213c1` | Without local patches: 67 library and 3 sensor tests passed; strict workspace/all-target/all-feature Clippy passed |
| Sard required feature matrix | All eight configurations passed against published `95213c1`, including no-default-feature CPU/GPU MPM |
| Native resident rigid-world regression suite | Initial run: 136 passed, one failed. After preserving the legacy single-body force-clearing contract, all 22 topology-related tests passed, including the original failure and one-time force consumption |

The sand failure was not hidden by loosening tolerances. An exactly rotated
cohesive particle selected the isotropic plastic branch in f64 but the elastic
branch after f32 GPU upload. Both implementations now use the same f32-aware
isotropic classification, with an additional exact-versus-uploaded regression.

The masked IK regression fixes the point residual and Jacobian, rather than
assuming the target orientation is attained when its rotational axes are
disabled. A revolute-only robot now reaches a link-local point's position
without a rotation target or an unintended root translation.

MPM's existing direct-transfer path can treat ill-conditioned stress/plasticity
on CPU. Resident execution rejects unsupported states rather than silently
switching backend. The native behavior is exposed as such, not advertised as
unconditional GPU residency.

The final MPM capture uses the real native resident session and the engine's
billboard vertex representation, with explicit zero rendering acceleration.
Advancing rendering time leaves all three sensor channels unchanged. Side and
world-up-pole camera views verify that particle quads do not collapse to points.

## Additional verification on 2026-10-09

The new native additions passed strict physics/Python all-target/all-feature
Clippy. Native CPU physics passed all 215 tests. Eleven focused real-GPU tests
passed: resident motor scatter/delay/reset, floating quaternion/drive template
restoration, selected reset layouts and fault preservation, normal-impulse
observations and mass faults, existing two-environment CPU parity, actual support
impulses, and configurable recovery speed. All 38 native Python binding tests passed.

With command-scoped local native patches, Sard passed strict workspace
all-target/all-feature Clippy, all 67 library tests, six new physics integration
tests, three rendered sensor tests, and one doctest. The control fixture uses
resident delay/action buffers on one encoder and confirms selective reset does
not cancel another environment's pending action. The MJCF fixture publishes
the actuated link's actual moving pose into Sard's visual components.
All eight required feature configurations passed. No test was skipped or weakened.

These results verify source implementation and local integration. The native
additions have subsequently been published; the final lockfile is resolved from
their Git source, not the command-scoped local patches.

Final verification against published `196f983`, without local patches:
`cargo fmt --all --check`, locked strict workspace/all-target/all-feature Clippy,
the locked complete test suite (77 passed), and all eight locked feature checks
passed. Both native lockfile entries resolve to the full published Git SHA.

## Published dependency

Both Sard native dependencies pin the reachable Tessera revision
`196f9830e74fbecc92c89e22d4539ef14b130d09`, published to `main`.
It includes the previous IK, Neo-Hookean sand, scene torque/impulse controls, and
mutation preservation, plus MJCF actuator controls, resident motor targets/delays,
selected articulated reset templates, contact observations, and contact policy.

The additional publication contains 30 source/test files in two verified commits:
`e4fc887` for MJCF/Python controls and `196f983` for resident GPU contracts.
Optional command-scoped patches for future local development are documented in
[physics ownership and usage](physics.md); none is required for the pinned source.
