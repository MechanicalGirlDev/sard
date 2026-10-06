//! URDF Robot - Load and display a robot model from URDF
//!
//! Run with: cargo run
//!
//! Note: Requires a URDF file. Create a simple one or download from ROS packages.

use glam::Vec3;
use sard::{
    screen_target, AmbientLight, Camera, ClearState, DirectionalLight, FrameOutput, Light,
    OrbitControl, RobotModel, Window, WindowSettings,
};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("URDF Robot Viewer")
            .size(1000, 700),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        robot: Option<RobotModel>,
        ambient_light: AmbientLight,
        directional_light: DirectionalLight,
        load_attempted: bool,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(2.0, 1.5, 2.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::Y,
            45.0,
            1.43,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::new(0.0, 0.5, 0.0), 0.5, 20.0),
        robot: None,
        ambient_light: AmbientLight::white(0.4),
        directional_light: DirectionalLight::white(0.8, Vec3::new(-1.0, -1.0, -0.5)),
        load_attempted: false,
    };

    window.render_loop(state, |state, frame| {
        // Try to load robot on first frame
        if !state.load_attempted {
            state.load_attempted = true;

            // Try common URDF locations
            let urdf_paths = ["robot.urdf", "examples/robot.urdf", "assets/robot.urdf"];

            for path in &urdf_paths {
                if Path::new(path).exists() {
                    match RobotModel::from_urdf(frame.ctx, path, frame.surface_format) {
                        Ok(robot) => {
                            println!("Loaded robot from: {}", path);
                            state.robot = Some(robot);
                            break;
                        }
                        Err(e) => {
                            eprintln!("Failed to load {}: {}", path, e);
                        }
                    }
                }
            }

            if state.robot.is_none() {
                println!("No URDF file found. Place a robot.urdf file in the project root.");
                println!("The viewer will show an empty scene with a grid.");
            }
        }

        // Update camera
        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        // Animate robot joints if loaded
        if let Some(robot) = &mut state.robot {
            let time = frame.elapsed_time as f32;
            // Simple sine wave animation for demonstration
            let angle = (time * 0.5).sin() * 0.5;
            let angles = [angle, -angle * 0.5, angle * 0.3, 0.0, 0.0, 0.0, 0.0, 0.0];
            robot.update_joints(&angles, &angles);
        }

        // Render
        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.1, 0.12, 0.15, 1.0], 1.0),
            );
            let lights: Vec<&dyn Light> = vec![&state.ambient_light, &state.directional_light];

            if let Some(robot) = &state.robot {
                robot.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
