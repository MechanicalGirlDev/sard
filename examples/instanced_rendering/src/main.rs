//! Instanced Rendering - Efficiently render many objects
//!
//! Demonstrates InstancedMesh for rendering thousands of cubes
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, AmbientLight, Camera, ClearState, ColorMaterial, DirectionalLight, FrameOutput,
    Gm, InstanceData, InstancedMesh, Light, Mesh, Object, OrbitControl, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Instanced Rendering - 1000 cubes")
            .size(1000, 700),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        instanced_cubes: Option<Gm<InstancedMesh, ColorMaterial>>,
        ambient_light: AmbientLight,
        directional_light: DirectionalLight,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(15.0, 12.0, 15.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.43,
            0.1,
            200.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 5.0, 100.0),
        instanced_cubes: None,
        ambient_light: AmbientLight::white(0.3),
        directional_light: DirectionalLight::white(0.8, Vec3::new(-1.0, -1.0, -0.5)),
    };

    window.render_loop(state, |state, frame| {
        // Initialize on first frame
        if state.instanced_cubes.is_none() {
            let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let base_mesh = Mesh::cube(frame.ctx, 0.8, [1.0, 1.0, 1.0]);

            // Create instance data for a 10x10x10 grid of cubes
            let mut instances = Vec::new();
            let grid_size = 10;
            let spacing = 2.0;
            let offset = (grid_size as f32 - 1.0) * spacing / 2.0;

            for x in 0..grid_size {
                for y in 0..grid_size {
                    for z in 0..grid_size {
                        let position = Vec3::new(
                            x as f32 * spacing - offset,
                            y as f32 * spacing - offset,
                            z as f32 * spacing - offset,
                        );

                        // Color based on position
                        let color = [
                            x as f32 / grid_size as f32,
                            y as f32 / grid_size as f32,
                            z as f32 / grid_size as f32,
                            1.0,
                        ];

                        let transform = Mat4::from_translation(position);
                        instances.push(InstanceData::with_transform_and_color(transform, color));
                    }
                }
            }

            let instanced_mesh = InstancedMesh::new(frame.ctx, base_mesh, &instances);
            state.instanced_cubes = Some(Gm::new(instanced_mesh, material));
        }

        // Update camera
        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Rotate the entire grid slowly
        let angle = frame.elapsed_time as f32 * 0.1;
        if let Some(cubes) = &mut state.instanced_cubes {
            cubes.transform = Mat4::from_rotation_y(angle);
        }

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.08, 0.08, 0.12, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![&state.ambient_light, &state.directional_light];

            if let Some(cubes) = &state.instanced_cubes {
                cubes.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
