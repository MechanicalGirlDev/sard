//! Camera Controls - Demonstrate OrbitControl mouse interaction
//!
//! Controls:
//! - Left mouse drag: Rotate camera around target
//! - Right mouse drag: Pan camera
//! - Mouse wheel: Zoom in/out
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, AmbientLight, Axes, Camera, ClearState, ColorMaterial, DirectionalLight,
    FrameOutput, Geometry, Gm, Light, LineMaterial, Mesh, Object, OrbitControl, Window,
    WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Camera Controls - Drag to rotate, scroll to zoom")
            .size(800, 600),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        cube: Option<Gm<Mesh, ColorMaterial>>,
        floor: Option<Gm<Mesh, ColorMaterial>>,
        axes: Option<Axes>,
        line_material: Option<LineMaterial>,
        ambient_light: AmbientLight,
        directional_light: DirectionalLight,
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
        control: OrbitControl::new(Vec3::ZERO, 2.0, 50.0),
        cube: None,
        floor: None,
        axes: None,
        line_material: None,
        ambient_light: AmbientLight::white(0.4),
        directional_light: DirectionalLight::white(0.8, Vec3::new(-1.0, -2.0, -1.0)),
    };

    window.render_loop(state, |state, frame| {
        // Initialize on first frame
        if state.cube.is_none() {
            let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let mesh = Mesh::cube(frame.ctx, 1.5, [0.3, 0.5, 0.8]);
            state.cube = Some(Gm::new(mesh, material).with_position(0.0, 0.75, 0.0));

            let floor_material = ColorMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create material");
            let floor_mesh = Mesh::quad(frame.ctx, 10.0, 10.0, [0.3, 0.3, 0.35]);
            state.floor = Some(Gm::new(floor_mesh, floor_material));

            state.axes = Some(Axes::new(frame.ctx, 2.0));
            state.line_material = Some(
                LineMaterial::new(frame.ctx, frame.surface_format)
                    .expect("Failed to create line material"),
            );
        }

        // Update camera
        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.15, 0.15, 0.18, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![&state.ambient_light, &state.directional_light];

            if let Some(floor) = &state.floor {
                floor.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
            if let Some(cube) = &state.cube {
                cube.render(frame.ctx, &state.camera, &lights, &mut pass);
            }

            // Render axes with line material
            if let (Some(axes), Some(line_mat)) = (&state.axes, &state.line_material) {
                line_mat.update_uniforms(frame.ctx, &state.camera, Mat4::IDENTITY);
                pass.set_pipeline(line_mat.pipeline());
                pass.set_bind_group(0, line_mat.camera_bind_group(), &[]);
                pass.set_bind_group(1, line_mat.model_bind_group(), &[]);
                pass.set_vertex_buffer(0, axes.vertex_buffer().slice());
                pass.draw(0..axes.draw_count(), 0..1);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
