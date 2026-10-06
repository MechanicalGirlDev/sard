//! Hello Cube - Minimal example showing a rotating cube
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, ColorMaterial, FrameOutput, Gm, Mesh, Object, OrbitControl,
    Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("Hello Cube"))?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        cube: Option<Gm<Mesh, ColorMaterial>>,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(3.0, 2.0, 3.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.0,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 1.0, 20.0),
        cube: None,
    };

    window.render_loop(state, |state, frame| {
        // Initialize on first frame
        if state.cube.is_none() {
            let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let mesh = Mesh::cube(frame.ctx, 1.0, [0.2, 0.6, 0.9]);
            state.cube = Some(Gm::new(mesh, material));
        }

        // Update camera
        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Rotate cube
        let angle = frame.elapsed_time as f32;
        let rotation = Mat4::from_rotation_y(angle);
        if let Some(cube) = &mut state.cube {
            cube.transform = rotation;
        }

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.1, 0.1, 0.1, 1.0], 1.0),
            );
            if let Some(cube) = &state.cube {
                let lights: Vec<&dyn sard::Light> = vec![];
                cube.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
