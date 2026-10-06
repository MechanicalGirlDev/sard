//! Particles - Particle system simulation
//!
//! Demonstrates the ParticleSystem geometry with physics-based particle movement.
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, FrameOutput, Gm, Object, OrbitControl, ParticleData,
    ParticleSystem, SpriteMaterial, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Particle System")
            .size(1024, 768),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        particles: Option<Gm<ParticleSystem, SpriteMaterial>>,
        last_reset: f64,
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
        control: OrbitControl::new(Vec3::ZERO, 2.0, 30.0),
        particles: None,
        last_reset: 0.0,
    };

    window.render_loop(state, |state, frame| {
        let time = frame.elapsed_time;

        if state.particles.is_none() {
            let material = SpriteMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create sprite material");

            // Create a fountain-like particle system
            let num_particles = 500;
            let mut start_positions = Vec::with_capacity(num_particles);
            let mut start_velocities = Vec::with_capacity(num_particles);
            let mut colors = Vec::with_capacity(num_particles);

            for i in 0..num_particles {
                let t = i as f32 / num_particles as f32;
                let angle = t * std::f32::consts::TAU * 5.0;
                let spread = 0.5;

                start_positions.push(Vec3::ZERO);
                start_velocities.push(Vec3::new(
                    angle.cos() * spread + (t * 13.0).sin() * 0.3,
                    2.0 + (t * 7.0).sin() * 1.0,
                    angle.sin() * spread + (t * 17.0).cos() * 0.3,
                ));
                colors.push([1.0, 0.3 + t * 0.5, 0.1]);
            }

            let data = ParticleData {
                start_positions,
                start_velocities,
                colors,
            };

            let particles = ParticleSystem::new(
                frame.ctx,
                data,
                Vec3::new(0.0, -3.0, 0.0), // gravity
                0.05,
            );

            state.particles = Some(Gm::new(particles, material));
            state.last_reset = time;
        }

        // Update particle positions
        let local_time = (time - state.last_reset) as f32;
        // Reset particles every 3 seconds
        let cycle_time = local_time % 3.0;
        if let Some(gm) = &mut state.particles {
            gm.geometry.update(frame.ctx, cycle_time);
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let angle = time as f32 * 0.1;
        if let Some(particles) = &mut state.particles {
            particles.transform = Mat4::from_rotation_y(angle);
        }

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.02, 0.02, 0.05, 1.0], 1.0),
            );
            let lights: Vec<&dyn sard::Light> = vec![];

            if let Some(particles) = &state.particles {
                particles.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
