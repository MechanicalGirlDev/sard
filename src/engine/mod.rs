//! Game engine module with App trait and game loop.
//!
//! Provides a high-level API for ECS-based applications with automatic
//! system scheduling (transform propagation, culling, rendering).

use crate::context::WgpuContext;
use crate::window::event::Event;
use crate::window::frame_io::Viewport;

/// Game loop configuration.
pub struct GameLoopConfig {
    /// Fixed timestep for physics (seconds). Default: 1/60.
    pub fixed_timestep: f64,
    /// Maximum physics substeps per frame. Default: 4.
    pub max_substeps: u32,
}

impl Default for GameLoopConfig {
    fn default() -> Self {
        Self {
            fixed_timestep: 1.0 / 60.0,
            max_substeps: 4,
        }
    }
}

/// System execution context passed to App callbacks.
pub struct SystemContext<'a> {
    /// The wgpu context.
    pub ctx: &'a WgpuContext,
    /// Time since last frame (seconds).
    pub delta_time: f64,
    /// Fixed timestep interval (seconds).
    pub fixed_delta_time: f64,
    /// Time since application start (seconds).
    pub elapsed_time: f64,
    /// Current viewport dimensions.
    pub viewport: Viewport,
    /// Events that occurred this frame.
    pub events: &'a [Event],
    /// The surface texture format.
    pub surface_format: wgpu::TextureFormat,
}

/// Trait for ECS-based game applications.
///
/// Implement this trait and pass it to [`run_app`] to run a game
/// with automatic ECS system scheduling.
pub trait App {
    /// Called once on the first frame. Set up ECS world and load resources.
    fn init(&mut self, ctx: &WgpuContext, world: &mut hecs::World) -> anyhow::Result<()>;

    /// Called each frame (variable timestep). Handle input and game logic.
    fn update(&mut self, world: &mut hecs::World, ctx: &SystemContext) -> anyhow::Result<()>;

    /// Called at fixed timestep intervals. Use for physics logic. Optional.
    fn fixed_update(&mut self, _world: &mut hecs::World, _dt: f64) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called after rendering. Use for GUI and debug overlays. Optional.
    fn post_render(&mut self, _world: &mut hecs::World, _ctx: &SystemContext) {}
}

/// Run a game application with automatic ECS system scheduling.
///
/// This wraps [`Window::render_loop`] and automatically runs:
/// 1. `App::fixed_update` at fixed timestep intervals
/// 2. `App::update` each frame
/// 3. `transform_system` (hierarchy propagation)
/// 4. `culling_system` (frustum culling)
/// 5. `render_system` (draw visible entities)
/// 6. `App::post_render`
pub fn run_app<A: App + 'static>(
    settings: crate::window::WindowSettings,
    config: GameLoopConfig,
    app: A,
) -> anyhow::Result<()> {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::core::ClearState;
    use crate::ecs::systems::{culling_system, render_system, transform_system};
    use crate::window::{screen_target, FrameOutput, Window};

    struct EngineState<A: App> {
        app: A,
        world: hecs::World,
        config: GameLoopConfig,
        initialized: bool,
        accumulator: f64,
    }

    let window = Window::new(settings)?;

    let state = EngineState {
        app,
        world: hecs::World::new(),
        config,
        initialized: false,
        accumulator: 0.0,
    };

    let failure = Rc::new(Cell::new(None));
    let callback_failure = Rc::clone(&failure);
    window.render_loop(state, move |state, frame| {
        let result = (|| -> anyhow::Result<FrameOutput> {
            let ctx = frame.ctx;

            // Initialize on first frame (GPU context is now available)
            if !state.initialized {
                state.app.init(ctx, &mut state.world)?;
                state.initialized = true;
            }

            // Fixed timestep loop
            state.accumulator += frame.delta_time;
            let mut substeps = 0_u32;
            while state.accumulator >= state.config.fixed_timestep
                && substeps < state.config.max_substeps
            {
                state
                    .app
                    .fixed_update(&mut state.world, state.config.fixed_timestep)?;
                state.accumulator -= state.config.fixed_timestep;
                substeps += 1;
            }

            // Variable timestep update
            let sys_ctx = SystemContext {
                ctx,
                delta_time: frame.delta_time,
                fixed_delta_time: state.config.fixed_timestep,
                elapsed_time: frame.elapsed_time,
                viewport: frame.viewport,
                events: &frame.events,
                surface_format: frame.surface_format,
            };
            state.app.update(&mut state.world, &sys_ctx)?;

            // ECS systems
            transform_system(&mut state.world);
            culling_system(&mut state.world);

            // Rendering
            let target = screen_target(&frame);
            let mut encoder = ctx.create_encoder(Some("engine frame"));
            {
                let clear = ClearState::color_and_depth([0.1, 0.1, 0.1, 1.0], 1.0);
                let mut pass = target.begin_render_pass(&mut encoder, clear);
                render_system(&state.world, ctx, &mut pass);
            }
            ctx.submit([encoder.finish()]);

            // Post-render
            state.app.post_render(&mut state.world, &sys_ctx);

            Ok(FrameOutput::default())
        })();
        match result {
            Ok(output) => output,
            Err(error) => {
                callback_failure.set(Some(error));
                FrameOutput::exit()
            }
        }
    })?;
    match failure.take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
