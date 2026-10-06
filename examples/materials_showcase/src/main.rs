//! Materials Showcase - Display different material types side by side
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, AmbientLight, Camera, ClearState, ColorMaterial, DirectionalLight, FrameOutput,
    Gm, Light, Mesh, NormalMaterial, Object, OrbitControl, PhongMaterial, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Materials Showcase")
            .size(1200, 600),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        color_sphere: Option<Gm<Mesh, ColorMaterial>>,
        phong_sphere: Option<Gm<Mesh, PhongMaterial>>,
        normal_sphere: Option<Gm<Mesh, NormalMaterial>>,
        ambient_light: AmbientLight,
        directional_light: DirectionalLight,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(0.0, 2.0, 8.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            2.0,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 2.0, 30.0),
        color_sphere: None,
        phong_sphere: None,
        normal_sphere: None,
        ambient_light: AmbientLight::white(0.3),
        directional_light: DirectionalLight::white(1.0, Vec3::new(-1.0, -1.0, -1.0)),
    };

    window.render_loop(state, |state, frame| {
        // Initialize on first frame
        if state.color_sphere.is_none() {
            // ColorMaterial sphere (left)
            let color_mat = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let mesh = Mesh::sphere(frame.ctx, 1.0, 32, 24, [0.8, 0.2, 0.2]);
            state.color_sphere = Some(Gm::new(mesh, color_mat).with_position(-3.0, 0.0, 0.0));

            // PhongMaterial sphere (center)
            let phong_mat = PhongMaterial::new(
                frame.ctx,
                frame.surface_format,
                [0.1, 0.1, 0.1],
                [0.2, 0.8, 0.2],
                [1.0, 1.0, 1.0],
                32.0,
            )
            .expect("Failed to create phong material");
            let mesh = Mesh::sphere(frame.ctx, 1.0, 32, 24, [0.2, 0.8, 0.2]);
            state.phong_sphere = Some(Gm::new(mesh, phong_mat).with_position(0.0, 0.0, 0.0));

            // NormalMaterial sphere (right)
            let normal_mat = NormalMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create normal material");
            let mesh = Mesh::sphere(frame.ctx, 1.0, 32, 24, [1.0, 1.0, 1.0]);
            state.normal_sphere = Some(Gm::new(mesh, normal_mat).with_position(3.0, 0.0, 0.0));
        }

        // Update camera
        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Slow rotation
        let angle = frame.elapsed_time as f32 * 0.3;
        let rotation = Mat4::from_rotation_y(angle);

        if let Some(sphere) = &mut state.color_sphere {
            sphere.transform = Mat4::from_translation(Vec3::new(-3.0, 0.0, 0.0)) * rotation;
        }
        if let Some(sphere) = &mut state.phong_sphere {
            sphere.transform = rotation;
        }
        if let Some(sphere) = &mut state.normal_sphere {
            sphere.transform = Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)) * rotation;
        }

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.05, 0.05, 0.1, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![&state.ambient_light, &state.directional_light];

            if let Some(sphere) = &state.color_sphere {
                sphere.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
            if let Some(sphere) = &state.phong_sphere {
                sphere.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
            if let Some(sphere) = &state.normal_sphere {
                sphere.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
