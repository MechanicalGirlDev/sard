//! Same scene in three rendering modes: 1 raster, 2 ray tracing, 3 path tracing.

use glam::Vec3;
use sard::{
    screen_target, Camera, Event, FrameOutput, Gm, Key, Mesh, OrbitControl, PbrMaterial,
    RenderConfig, RenderMode, Renderer, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    struct State {
        camera: Camera,
        control: OrbitControl,
        renderer: Option<Renderer>,
    }

    let window = Window::new(
        WindowSettings::default()
            .title("PBR: 1 Raster | 2 Ray Tracing | 3 Path Tracing")
            .size(960, 640),
    )?;
    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(4.0, 3.0, 7.0),
            Vec3::new(0.0, 0.7, 0.0),
            Vec3::Y,
            45.0,
            1.5,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::new(0.0, 0.7, 0.0), 2.0, 30.0),
        renderer: None,
    };
    let failure = std::rc::Rc::new(std::cell::Cell::new(None));
    let callback_failure = std::rc::Rc::clone(&failure);
    window.render_loop(state, move |state, frame| {
        let result = (|| -> anyhow::Result<()> {
            if state.renderer.is_none() {
                let mut objects = Vec::new();
                for (x, color, metallic, roughness) in [
                    (-1.5, [0.8, 0.12, 0.04], 0.0, 0.65),
                    (0.0, [0.85, 0.65, 0.25], 1.0, 0.08),
                    (1.5, [0.08, 0.3, 0.85], 0.4, 0.3),
                ] {
                    let material = PbrMaterial::with_params(
                        frame.ctx,
                        frame.surface_format,
                        [color[0], color[1], color[2], 1.0],
                        metallic,
                        roughness,
                        [0.0; 3],
                        1.0,
                    )?;
                    objects.push(
                        Gm::new(Mesh::sphere(frame.ctx, 0.7, 32, 24, [1.0; 3]), material)
                            .with_position(x, 0.7, 0.0),
                    );
                }
                objects.push(Gm::new(
                    Mesh::quad(frame.ctx, 12.0, 12.0, [1.0; 3]),
                    PbrMaterial::with_params(
                        frame.ctx,
                        frame.surface_format,
                        [0.65, 0.65, 0.65, 1.0],
                        0.0,
                        0.8,
                        [0.0; 3],
                        1.0,
                    )?,
                ));
                let mut renderer =
                    Renderer::new(frame.ctx, frame.surface_format, RenderConfig::default())?;
                renderer.set_scene(&objects.iter().map(Gm::render_object).collect::<Vec<_>>())?;
                state.renderer = Some(renderer);
            }
            state.camera.set_viewport(frame.viewport);
            let mut events = frame.events.clone();
            state.control.handle_events(&mut state.camera, &mut events);
            if let Some(renderer) = &mut state.renderer {
                for event in &events {
                    if let Event::KeyPress { key, .. } = event {
                        match key {
                            Key::Key1 => renderer.set_mode(RenderMode::Rasterization),
                            Key::Key2 => renderer.set_mode(RenderMode::RayTracing),
                            Key::Key3 => renderer.set_mode(RenderMode::PathTracing),
                            _ => {}
                        }
                    }
                }
                renderer.render(&state.camera, &screen_target(&frame))?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => FrameOutput::default(),
            Err(error) => {
                callback_failure.set(Some(error));
                FrameOutput {
                    exit: true,
                    ..FrameOutput::default()
                }
            }
        }
    })?;
    match failure.take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
