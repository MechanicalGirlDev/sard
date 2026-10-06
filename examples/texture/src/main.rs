//! Texture - UV coordinate visualization
//!
//! Demonstrates the UVMaterial which shows texture coordinates as colors.
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, FrameOutput, Gm, Mesh, Object, OrbitControl, UVMaterial,
    Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("UV Texture Coordinates")
            .size(1024, 768),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        sphere: Option<Gm<Mesh, UVMaterial>>,
        cube: Option<Gm<Mesh, UVMaterial>>,
        quad: Option<Gm<Mesh, UVMaterial>>,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(0.0, 2.0, 6.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.33,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 2.0, 20.0),
        sphere: None,
        cube: None,
        quad: None,
    };

    window.render_loop(state, |state, frame| {
        if state.sphere.is_none() {
            let mat = UVMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create UV material");
            let mesh = Mesh::sphere(frame.ctx, 1.0, 32, 24, [1.0, 1.0, 1.0]);
            state.sphere = Some(Gm::new(mesh, mat).with_position(-3.0, 0.0, 0.0));

            let mat = UVMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create UV material");
            let mesh = Mesh::cube(frame.ctx, 1.5, [1.0, 1.0, 1.0]);
            state.cube = Some(Gm::new(mesh, mat).with_position(0.0, 0.0, 0.0));

            let mat = UVMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create UV material");
            let mesh = Mesh::quad(frame.ctx, 2.0, 2.0, [1.0, 1.0, 1.0]);
            state.quad = Some(Gm::new(mesh, mat).with_position(3.0, 0.0, 0.0));
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let angle = frame.elapsed_time as f32 * 0.3;
        let rotation = Mat4::from_rotation_y(angle);

        if let Some(obj) = &mut state.sphere {
            obj.transform = Mat4::from_translation(Vec3::new(-3.0, 0.0, 0.0)) * rotation;
        }
        if let Some(obj) = &mut state.cube {
            obj.transform = rotation;
        }
        if let Some(obj) = &mut state.quad {
            obj.transform = Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)) * rotation;
        }

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.05, 0.05, 0.1, 1.0], 1.0),
            );
            let lights: Vec<&dyn sard::Light> = vec![];

            if let Some(obj) = &state.sphere {
                obj.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
            if let Some(obj) = &state.cube {
                obj.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
            if let Some(obj) = &state.quad {
                obj.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
