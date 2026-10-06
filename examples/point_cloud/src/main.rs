//! Point Cloud - Rendering a cloud of colored points
//!
//! Demonstrates rendering a point cloud using small meshes in an instanced fashion.
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, ColorMaterial, FrameOutput, Gm, InstanceData, InstancedMesh,
    Light, Mesh, Object, OrbitControl, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Point Cloud")
            .size(1024, 768),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        cloud: Option<Gm<InstancedMesh, ColorMaterial>>,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(5.0, 3.0, 5.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.33,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 2.0, 20.0),
        cloud: None,
    };

    window.render_loop(state, |state, frame| {
        if state.cloud.is_none() {
            let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");

            // Create a tiny sphere as the point primitive
            let point_mesh = Mesh::sphere(frame.ctx, 0.03, 6, 4, [1.0, 1.0, 1.0]);

            // Generate a point cloud: sphere distribution with color gradient
            let num_points = 2000;
            let mut instances = Vec::with_capacity(num_points);

            for i in 0..num_points {
                let t = i as f32 / num_points as f32;
                // Fibonacci sphere distribution
                let phi = (1.0 + 5.0f32.sqrt()) / 2.0;
                let theta = 2.0 * std::f32::consts::PI * i as f32 / phi;
                let cos_phi = 1.0 - 2.0 * (i as f32 + 0.5) / num_points as f32;
                let sin_phi = (1.0 - cos_phi * cos_phi).sqrt();
                let radius = 2.0 + (t * 10.0).sin() * 0.5;

                let position = Vec3::new(
                    radius * sin_phi * theta.cos(),
                    radius * cos_phi,
                    radius * sin_phi * theta.sin(),
                );

                let color = [
                    (t * std::f32::consts::TAU).sin() * 0.5 + 0.5,
                    (t * std::f32::consts::TAU + 2.0).sin() * 0.5 + 0.5,
                    (t * std::f32::consts::TAU + 4.0).sin() * 0.5 + 0.5,
                    1.0,
                ];

                let transform = Mat4::from_translation(position);
                instances.push(InstanceData::with_transform_and_color(transform, color));
            }

            let instanced = InstancedMesh::new(frame.ctx, point_mesh, &instances);
            state.cloud = Some(Gm::new(instanced, material));
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let angle = frame.elapsed_time as f32 * 0.2;
        if let Some(cloud) = &mut state.cloud {
            cloud.transform = Mat4::from_rotation_y(angle);
        }

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.02, 0.02, 0.05, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![];

            if let Some(cloud) = &state.cloud {
                cloud.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
