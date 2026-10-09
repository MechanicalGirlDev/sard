//! Sard 3D Engine
//!
//! A 3D engine built on wgpu with ECS, physics, and GPU compute.
//!
//! # Architecture
//!
//! The library is organized into layers:
//!
//! 1. **context** - Core wgpu wrapper (Device, Queue)
//! 2. **core** - GPU primitives (buffers, textures, pipelines)
//! 3. **compute** - Compute shader utilities
//! 4. **renderer** - High-level rendering (cameras, materials, geometry, lights)
//! 5. **physics** - Rigid body simulation, collision detection (feature = "physics")
//! 6. **ecs** - hecs ECS integration (feature = "ecs")
//! 7. **engine** - Game loop with App trait (feature = "engine")
//! 8. **window** - Window management with winit (feature = "window")
//! 9. **gui** - Text rendering with glyphon (feature = "gui")
//! 10. **urdf** - URDF robot model loading

// Test code may freely panic, print, and compare floats exactly — these patterns
// would be warnings in production code but are idiomatic in `#[test]` functions.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::float_cmp,
        clippy::print_stderr,
        clippy::print_stdout,
        reason = "lints intentionally disabled inside tests",
    )
)]

pub mod context;
pub mod core;
pub mod effect;
pub mod renderer;
pub mod scene;
pub mod urdf;

#[cfg(feature = "window")]
pub mod window;

#[cfg(feature = "gui")]
pub mod gui;

pub mod compute;

#[cfg(feature = "ecs")]
pub mod ecs;

#[cfg(feature = "engine")]
pub mod engine;

#[cfg(feature = "physics")]
pub mod physics;

// Re-export commonly used types
pub use context::WgpuContext;

pub use core::{
    BlendState, ClearState, ComputePipelineBuilder, CullState, DepthState, DepthTexture,
    IndexBuffer, InstanceBuffer, InstanceData, PipelineBuilder, RawUniformBuffer, RenderTarget,
    StorageBuffer, Texture2D, Texture2DArray, TextureCubeMap, UniformBuffer, VertexBuffer, VertexP,
    VertexPC, VertexPN, VertexPNUC,
};

#[cfg(feature = "window")]
pub use renderer::{
    Control2D, FirstPersonControl, FlyControl, FreeOrbitControl, OrbitControl, Viewer,
};

pub use renderer::{
    Aabb, AmbientLight, Attenuation, Axes, BoundingBoxMesh, Camera, Circle, ColorMaterial,
    DepthMaterial, DirectionalLight, DirectionalShadow, Frustum, FrustumCuller, Geometry, Gm,
    GridMaterial, InstancedMesh, Intersection, Light, LineMaterial, LineStrip, Lines, Material,
    Mesh, ModelUniform, NormalMaterial, Object, ParticleData, ParticleSystem, PbrMaterial,
    PhongMaterial, Plane, PointLight, PositionMaterial, Projection, Rectangle, SensorCamera,
    SensorCameraConfig, SensorChannels, SensorError, SensorFrame, SensorObject, ShadowConfig,
    ShadowMap, ShadowUniform, Skybox, SpotLight, SpriteMaterial, Sprites, Terrain, TerrainLod,
    TerrainMaterial, TerrainUniform, UVMaterial, UnlitMaterial,
};

pub use urdf::{RobotModel, UrdfLoader};

pub use effect::{CopyEffect, Effect, EffectChain, FogEffect, FogMode, FullscreenQuad, FxaaEffect};

#[cfg(feature = "window")]
pub use window::{
    screen_target, Cursor, Event, FrameInput, FrameOutput, Key, Modifiers, MouseButton, ResizeEdge,
    Viewport, Window, WindowCommand, WindowSettings,
};

#[cfg(feature = "gui")]
pub use gui::{TextBuilder, TextRenderer};

pub use compute::{compute_workgroup_count, read_back, ComputeDispatcher};

#[cfg(feature = "ecs")]
pub use ecs::prelude::*;

#[cfg(feature = "engine")]
pub use engine::{run_app, App, GameLoopConfig, SystemContext};

#[cfg(feature = "physics")]
pub use physics::{PhysicsConfig, PhysicsWorld};

// Re-export glam for convenience
pub use glam;

// Re-export wgpu so downstream crates name `TextureFormat` etc. without pinning their own copy.
pub use wgpu;
