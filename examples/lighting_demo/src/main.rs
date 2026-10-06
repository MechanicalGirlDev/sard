//! Lighting Demo - Display different light types
//!
//! Shows ambient, directional, and point lights affecting spheres
//!
//! Run with: cargo run

use glam::Vec3;
use sard::{
    screen_target, AmbientLight, Camera, ClearState, ColorMaterial, DirectionalLight, FrameOutput,
    Gm, Light, Mesh, Object, OrbitControl, PointLight, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Lighting Demo")
            .size(1000, 700),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        spheres: Vec<Gm<Mesh, ColorMaterial>>,
        floor: Option<Gm<Mesh, ColorMaterial>>,
        ambient_light: AmbientLight,
        directional_light: DirectionalLight,
        point_lights: Vec<PointLight>,
        initialized: bool,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(0.0, 8.0, 12.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.43,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 3.0, 30.0),
        spheres: Vec::new(),
        floor: None,
        ambient_light: AmbientLight::white(0.15),
        directional_light: DirectionalLight::white(0.6, Vec3::new(-0.5, -1.0, -0.3)),
        point_lights: vec![
            PointLight::new(
                2.0,
                [1.0, 0.3, 0.3],
                Vec3::new(-3.0, 2.0, 0.0),
                [1.0, 0.14, 0.07],
            ),
            PointLight::new(
                2.0,
                [0.3, 1.0, 0.3],
                Vec3::new(3.0, 2.0, 0.0),
                [1.0, 0.14, 0.07],
            ),
            PointLight::new(
                2.0,
                [0.3, 0.3, 1.0],
                Vec3::new(0.0, 2.0, 3.0),
                [1.0, 0.14, 0.07],
            ),
        ],
        initialized: false,
    };

    window.render_loop(state, |state, frame| {
        // Initialize on first frame
        if !state.initialized {
            // Create a grid of spheres
            for x in -2..=2 {
                for z in -2..=2 {
                    let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                        .expect("Failed to create material");
                    let mesh = Mesh::sphere(frame.ctx, 0.4, 24, 16, [0.9, 0.9, 0.9]);
                    let sphere =
                        Gm::new(mesh, material).with_position(x as f32 * 1.5, 0.5, z as f32 * 1.5);
                    state.spheres.push(sphere);
                }
            }

            // Floor
            let floor_material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let floor_mesh = Mesh::quad(frame.ctx, 15.0, 15.0, [0.25, 0.25, 0.3]);
            state.floor = Some(Gm::new(floor_mesh, floor_material));

            state.initialized = true;
        }

        // Update camera
        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Animate point lights in circles
        let time = frame.elapsed_time as f32;
        state.point_lights[0].position = Vec3::new(
            (time * 0.5).cos() * 4.0,
            2.0 + (time * 0.7).sin() * 0.5,
            (time * 0.5).sin() * 4.0,
        );
        state.point_lights[1].position = Vec3::new(
            (time * 0.5 + 2.0).cos() * 4.0,
            2.0 + (time * 0.9).sin() * 0.5,
            (time * 0.5 + 2.0).sin() * 4.0,
        );
        state.point_lights[2].position = Vec3::new(
            (time * 0.5 + 4.0).cos() * 4.0,
            2.0 + (time * 1.1).sin() * 0.5,
            (time * 0.5 + 4.0).sin() * 4.0,
        );

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.02, 0.02, 0.05, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![
                &state.ambient_light,
                &state.directional_light,
                &state.point_lights[0],
                &state.point_lights[1],
                &state.point_lights[2],
            ];

            if let Some(floor) = &state.floor {
                floor.render(frame.ctx, &state.camera, &lights, &mut pass);
            }

            for sphere in &state.spheres {
                sphere.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
