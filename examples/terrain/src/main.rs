//! Terrain - Procedural terrain generation with height function
//!
//! Demonstrates the Terrain geometry with procedural height maps.
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, ColorMaterial, DirectionalLight, FrameOutput, Gm, Light,
    Object, OrbitControl, Terrain, TerrainLod, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("Terrain").size(1024, 768))?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        terrain: Option<Gm<Terrain, ColorMaterial>>,
        directional_light: DirectionalLight,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(15.0, 12.0, 15.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.33,
            0.1,
            200.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 3.0, 60.0),
        terrain: None,
        directional_light: DirectionalLight::white(0.8, Vec3::new(-1.0, -1.0, -0.5)),
    };

    window.render_loop(state, |state, frame| {
        if state.terrain.is_none() {
            let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");

            // Procedural height function: rolling hills with sine waves
            let height_fn = |x: f32, z: f32| -> f32 {
                (x * 0.3).sin() * (z * 0.3).cos() * 2.0
                    + (x * 0.1 + 0.5).sin() * 3.0
                    + (z * 0.15).cos() * 2.5
            };

            let terrain = Terrain::new(
                frame.ctx,
                40.0,
                40.0,
                128,
                &height_fn,
                TerrainLod::High,
                [0.3, 0.6, 0.2],
            );

            state.terrain = Some(Gm::new(terrain, material));
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Slow rotation
        let angle = frame.elapsed_time as f32 * 0.1;
        if let Some(terrain) = &mut state.terrain {
            terrain.transform = Mat4::from_rotation_y(angle);
        }

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.5, 0.7, 0.9, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![&state.directional_light];

            if let Some(terrain) = &state.terrain {
                terrain.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
