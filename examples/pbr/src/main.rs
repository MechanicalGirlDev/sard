//! PBR - Physically Based Rendering showcase
//!
//! Displays spheres with varying metallic and roughness parameters.
//!
//! Run with: cargo run

use glam::Vec3;
use sard::{
    screen_target, AmbientLight, Camera, ClearState, DirectionalLight, FrameOutput, Gm, Light,
    Mesh, Object, OrbitControl, PbrMaterial, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("PBR Material")
            .size(1024, 768),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        spheres: Vec<Gm<Mesh, PbrMaterial>>,
        ambient_light: AmbientLight,
        directional_light: DirectionalLight,
        initialized: bool,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(0.0, 3.0, 10.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.33,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 3.0, 30.0),
        spheres: Vec::new(),
        ambient_light: AmbientLight::white(0.2),
        directional_light: DirectionalLight::white(1.0, Vec3::new(-1.0, -1.0, -1.0)),
        initialized: false,
    };

    window.render_loop(state, |state, frame| {
        if !state.initialized {
            // Create a grid of spheres: rows = roughness, columns = metallic
            let rows = 5;
            let cols = 5;

            for row in 0..rows {
                for col in 0..cols {
                    let roughness = row as f32 / (rows - 1) as f32;
                    let metallic = col as f32 / (cols - 1) as f32;

                    let material = PbrMaterial::with_params(
                        frame.ctx,
                        frame.surface_format,
                        [0.8, 0.2, 0.2, 1.0],
                        metallic,
                        roughness.max(0.05), // Avoid zero roughness
                        [0.0, 0.0, 0.0],
                        1.0,
                    )
                    .expect("Failed to create PBR material");

                    let mesh = Mesh::sphere(frame.ctx, 0.5, 32, 24, [0.8, 0.2, 0.2]);
                    let x = (col as f32 - (cols - 1) as f32 / 2.0) * 1.5;
                    let y = (row as f32 - (rows - 1) as f32 / 2.0) * 1.5;
                    let sphere = Gm::new(mesh, material).with_position(x, y, 0.0);
                    state.spheres.push(sphere);
                }
            }

            state.initialized = true;
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.05, 0.05, 0.1, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![&state.ambient_light, &state.directional_light];

            for sphere in &state.spheres {
                sphere.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
