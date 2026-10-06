//! Window management module
//!
//! Provides a convenient abstraction over winit for window creation and event handling.

pub mod event;
pub mod frame_io;
pub mod settings;

pub use event::{Cursor, Event, Key, Modifiers, MouseButton};
pub use frame_io::{FrameInput, FrameOutput, ResizeEdge, Viewport, WindowCommand};
pub use settings::WindowSettings;

use crate::context::WgpuContext;
use crate::core::texture::DepthTexture;
use crate::core::RenderTarget;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

/// A window with GPU rendering context.
pub struct Window {
    settings: WindowSettings,
}

impl Window {
    /// Create a new window with the given settings.
    pub fn new(settings: WindowSettings) -> anyhow::Result<Self> {
        Ok(Self { settings })
    }

    /// Run the render loop with a callback.
    ///
    /// The callback receives a `FrameInput` and should return a `FrameOutput`.
    pub fn render_loop<F, S>(self, state_init: S, callback: F) -> anyhow::Result<()>
    where
        F: FnMut(&mut S, FrameInput<'_>) -> FrameOutput + 'static,
        S: 'static,
    {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App {
            settings: self.settings,
            state: Some(state_init),
            callback: Some(callback),
            graphics: None,
            events: Vec::new(),
            start_time: std::time::Instant::now(),
            last_frame_time: std::time::Instant::now(),
            mouse_position: (0.0, 0.0),
            modifiers: Modifiers::default(),
            cursor: Cursor::Default,
        };

        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

struct Graphics {
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    ctx: WgpuContext,
    depth_texture: DepthTexture,
}

struct App<S, F> {
    settings: WindowSettings,
    state: Option<S>,
    callback: Option<F>,
    graphics: Option<Graphics>,
    events: Vec<Event>,
    start_time: std::time::Instant,
    last_frame_time: std::time::Instant,
    mouse_position: (f32, f32),
    modifiers: Modifiers,
    cursor: Cursor,
}

impl<S, F> ApplicationHandler for App<S, F>
where
    F: FnMut(&mut S, FrameInput<'_>) -> FrameOutput + 'static,
    S: 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }

        let window_attrs = winit::window::WindowAttributes::default()
            .with_title(&self.settings.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.settings.size.0,
                self.settings.size.1,
            ))
            .with_resizable(self.settings.resizable)
            .with_decorations(self.settings.decorations)
            .with_maximized(self.settings.maximized)
            .with_fullscreen(
                self.settings
                    .fullscreen
                    .then_some(winit::window::Fullscreen::Borderless(None)),
            );

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        // Create wgpu instance and surface
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(event_loop.owned_display_handle()),
            ));

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("Failed to find suitable GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("sard device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .expect("Failed to create device");

        let ctx = WgpuContext::new(device, queue);

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: if self.settings.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&ctx.device, &config);

        let depth_texture =
            DepthTexture::new(&ctx, config.width, config.height, Some("depth texture"));

        self.graphics = Some(Graphics {
            window,
            surface,
            config,
            ctx,
            depth_texture,
        });

        self.start_time = std::time::Instant::now();
        self.last_frame_time = std::time::Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(graphics) = &mut self.graphics else {
            return;
        };

        let modifiers = self.modifiers;

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(m) => {
                let st = m.state();
                self.modifiers = Modifiers {
                    shift: st.shift_key(),
                    ctrl: st.control_key(),
                    alt: st.alt_key(),
                };
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    graphics.config.width = size.width;
                    graphics.config.height = size.height;
                    graphics
                        .surface
                        .configure(&graphics.ctx.device, &graphics.config);
                    graphics
                        .depth_texture
                        .resize(&graphics.ctx, size.width, size.height);
                }
                self.events.push(Event::Resize {
                    width: size.width,
                    height: size.height,
                });
            }
            WindowEvent::CursorMoved { position, .. } => {
                let old_position = self.mouse_position;
                self.mouse_position = (position.x as f32, position.y as f32);
                let delta = (
                    self.mouse_position.0 - old_position.0,
                    self.mouse_position.1 - old_position.1,
                );
                self.events.push(Event::MouseMotion {
                    delta,
                    position: self.mouse_position,
                    modifiers,
                    handled: false,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                match state {
                    winit::event::ElementState::Pressed => {
                        self.events.push(Event::MousePress {
                            button,
                            position: self.mouse_position,
                            modifiers,
                            handled: false,
                        });
                    }
                    winit::event::ElementState::Released => {
                        self.events.push(Event::MouseRelease {
                            button,
                            position: self.mouse_position,
                            modifiers,
                            handled: false,
                        });
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };
                self.events.push(Event::MouseWheel {
                    delta,
                    position: self.mouse_position,
                    modifiers,
                    handled: false,
                });
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                let pressed = matches!(key_event.state, winit::event::ElementState::Pressed);
                if let Some(key) = Key::from_winit(&key_event.logical_key) {
                    if pressed {
                        self.events.push(Event::KeyPress {
                            key,
                            modifiers,
                            handled: false,
                        });
                    } else {
                        self.events.push(Event::KeyRelease {
                            key,
                            modifiers,
                            handled: false,
                        });
                    }
                }

                // Emit composed text (shift/layout-correct) for text fields. Filter
                // control characters so Backspace/Enter stay on the `KeyPress` path.
                if pressed {
                    if let Some(text) = key_event.text.as_ref() {
                        let text: String = text.chars().filter(|c| !c.is_control()).collect();
                        if !text.is_empty() {
                            self.events.push(Event::Text { text });
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let elapsed_time = (now - self.start_time).as_secs_f64();
                let delta_time = (now - self.last_frame_time).as_secs_f64();
                self.last_frame_time = now;

                let surface_texture = match graphics.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                    wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                        graphics
                            .surface
                            .configure(&graphics.ctx.device, &graphics.config);
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Timeout
                    | wgpu::CurrentSurfaceTexture::Occluded => return,
                    wgpu::CurrentSurfaceTexture::Validation => {
                        tracing::error!("surface validation error");
                        return;
                    }
                };

                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let viewport = Viewport {
                    x: 0,
                    y: 0,
                    width: graphics.config.width,
                    height: graphics.config.height,
                };

                let events = std::mem::take(&mut self.events);

                let frame_input = FrameInput {
                    events,
                    elapsed_time,
                    delta_time,
                    viewport,
                    ctx: &graphics.ctx,
                    surface_view: &view,
                    depth_texture: &graphics.depth_texture,
                    surface_format: graphics.config.format,
                    maximized: graphics.window.is_maximized(),
                    scale_factor: graphics.window.scale_factor(),
                };

                let state = self.state.as_mut().expect("State should exist");
                let callback = self.callback.as_mut().expect("Callback should exist");
                let output = (callback)(state, frame_input);

                graphics.ctx.queue.present(surface_texture);

                if let Some(cursor) = output.cursor {
                    if cursor != self.cursor {
                        self.cursor = cursor;
                        graphics.window.set_cursor(cursor.to_winit());
                    }
                }
                for command in output.window {
                    let w = &graphics.window;
                    match command {
                        WindowCommand::DragMove | WindowCommand::DragResize(_) => {
                            let r = match command {
                                WindowCommand::DragResize(edge) => {
                                    w.drag_resize_window(edge.to_winit())
                                }
                                _ => w.drag_window(),
                            };
                            if let Err(e) = r {
                                tracing::debug!("window drag: {e}");
                            }
                            // The OS move / size loop eats the button release, so the
                            // frame callback would otherwise see the button held forever.
                            self.events.push(Event::MouseRelease {
                                button: MouseButton::Left,
                                position: self.mouse_position,
                                modifiers: self.modifiers,
                                handled: false,
                            });
                        }
                        WindowCommand::Minimize => w.set_minimized(true),
                        WindowCommand::ToggleMaximize => w.set_maximized(!w.is_maximized()),
                        WindowCommand::Close => event_loop.exit(),
                    }
                }

                if output.exit {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(graphics) = &self.graphics {
            graphics.window.request_redraw();
        }
    }
}

/// Create a render target from frame input.
pub fn screen_target<'a>(input: &'a FrameInput<'a>) -> RenderTarget<'a> {
    RenderTarget::from_surface(
        input.ctx,
        input.surface_view,
        Some(input.depth_texture),
        input.viewport.width,
        input.viewport.height,
        input.surface_format,
    )
}
