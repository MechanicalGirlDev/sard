//! Wireframe - Line rendering showcase
//!
//! Demonstrates Lines and LineStrip geometries for wireframe rendering.
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, FrameOutput, Geometry, LineMaterial, LineStrip, Lines,
    OrbitControl, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("Wireframe").size(1024, 768))?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        line_material: Option<LineMaterial>,
        cube_lines: Option<Lines>,
        spiral: Option<LineStrip>,
        cube_transform: Mat4,
        spiral_transform: Mat4,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(5.0, 4.0, 5.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.33,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 2.0, 20.0),
        line_material: None,
        cube_lines: None,
        spiral: None,
        cube_transform: Mat4::IDENTITY,
        spiral_transform: Mat4::IDENTITY,
    };

    window.render_loop(state, |state, frame| {
        if state.line_material.is_none() {
            state.line_material = Some(
                LineMaterial::new(frame.ctx, frame.surface_format)
                    .expect("Failed to create line material"),
            );

            // Wireframe cube edges
            let s = 1.0f32;
            let edges: Vec<(Vec3, [f32; 4], Vec3, [f32; 4])> = vec![
                // Bottom face (red)
                (
                    Vec3::new(-s, -s, -s),
                    [1.0, 0.3, 0.3, 1.0],
                    Vec3::new(s, -s, -s),
                    [1.0, 0.3, 0.3, 1.0],
                ),
                (
                    Vec3::new(s, -s, -s),
                    [1.0, 0.3, 0.3, 1.0],
                    Vec3::new(s, -s, s),
                    [1.0, 0.3, 0.3, 1.0],
                ),
                (
                    Vec3::new(s, -s, s),
                    [1.0, 0.3, 0.3, 1.0],
                    Vec3::new(-s, -s, s),
                    [1.0, 0.3, 0.3, 1.0],
                ),
                (
                    Vec3::new(-s, -s, s),
                    [1.0, 0.3, 0.3, 1.0],
                    Vec3::new(-s, -s, -s),
                    [1.0, 0.3, 0.3, 1.0],
                ),
                // Top face (green)
                (
                    Vec3::new(-s, s, -s),
                    [0.3, 1.0, 0.3, 1.0],
                    Vec3::new(s, s, -s),
                    [0.3, 1.0, 0.3, 1.0],
                ),
                (
                    Vec3::new(s, s, -s),
                    [0.3, 1.0, 0.3, 1.0],
                    Vec3::new(s, s, s),
                    [0.3, 1.0, 0.3, 1.0],
                ),
                (
                    Vec3::new(s, s, s),
                    [0.3, 1.0, 0.3, 1.0],
                    Vec3::new(-s, s, s),
                    [0.3, 1.0, 0.3, 1.0],
                ),
                (
                    Vec3::new(-s, s, s),
                    [0.3, 1.0, 0.3, 1.0],
                    Vec3::new(-s, s, -s),
                    [0.3, 1.0, 0.3, 1.0],
                ),
                // Vertical edges (blue)
                (
                    Vec3::new(-s, -s, -s),
                    [0.3, 0.3, 1.0, 1.0],
                    Vec3::new(-s, s, -s),
                    [0.3, 0.3, 1.0, 1.0],
                ),
                (
                    Vec3::new(s, -s, -s),
                    [0.3, 0.3, 1.0, 1.0],
                    Vec3::new(s, s, -s),
                    [0.3, 0.3, 1.0, 1.0],
                ),
                (
                    Vec3::new(s, -s, s),
                    [0.3, 0.3, 1.0, 1.0],
                    Vec3::new(s, s, s),
                    [0.3, 0.3, 1.0, 1.0],
                ),
                (
                    Vec3::new(-s, -s, s),
                    [0.3, 0.3, 1.0, 1.0],
                    Vec3::new(-s, s, s),
                    [0.3, 0.3, 1.0, 1.0],
                ),
            ];
            state.cube_lines = Some(Lines::new(frame.ctx, &edges, Some("wireframe cube")));

            // Spiral using LineStrip
            let num_points = 200;
            let points: Vec<(Vec3, [f32; 4])> = (0..num_points)
                .map(|i| {
                    let t = i as f32 / num_points as f32;
                    let angle = t * std::f32::consts::TAU * 4.0;
                    let radius = 1.5;
                    let height = t * 3.0 - 1.5;
                    let pos = Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
                    let color = [t, 1.0 - t, 0.5, 1.0];
                    (pos, color)
                })
                .collect();
            state.spiral = Some(LineStrip::new(frame.ctx, &points, Some("spiral")));
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let angle = frame.elapsed_time as f32 * 0.3;
        let rotation = Mat4::from_rotation_y(angle);
        state.cube_transform = Mat4::from_translation(Vec3::new(-2.0, 0.0, 0.0)) * rotation;
        state.spiral_transform = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)) * rotation;

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.02, 0.02, 0.05, 1.0], 1.0),
            );

            if let Some(line_mat) = &state.line_material {
                // Render wireframe cube
                if let Some(lines) = &state.cube_lines {
                    line_mat.update_uniforms(frame.ctx, &state.camera, state.cube_transform);
                    pass.set_pipeline(line_mat.pipeline());
                    pass.set_bind_group(0, line_mat.camera_bind_group(), &[]);
                    pass.set_bind_group(1, line_mat.model_bind_group(), &[]);
                    pass.set_vertex_buffer(0, lines.vertex_buffer().slice());
                    pass.draw(0..lines.draw_count(), 0..1);
                }

                // Render spiral
                if let Some(strip) = &state.spiral {
                    line_mat.update_uniforms(frame.ctx, &state.camera, state.spiral_transform);
                    pass.set_pipeline(line_mat.pipeline());
                    pass.set_bind_group(0, line_mat.camera_bind_group(), &[]);
                    pass.set_bind_group(1, line_mat.model_bind_group(), &[]);
                    pass.set_vertex_buffer(0, strip.vertex_buffer().slice());
                    pass.draw(0..strip.draw_count(), 0..1);
                }
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
