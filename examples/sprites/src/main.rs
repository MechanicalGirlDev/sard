//! Sprites - Billboard rendering
//!
//! Demonstrates the Sprites geometry which renders camera-facing quads.
//!
//! Run with: cargo run

use glam::{Mat4, Vec3};
use sard::{
    screen_target, Camera, ClearState, FrameOutput, Gm, Object, OrbitControl, SpriteMaterial,
    Sprites, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("Sprites").size(1024, 768))?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        sprites: Option<Gm<Sprites, SpriteMaterial>>,
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
        sprites: None,
    };

    window.render_loop(state, |state, frame| {
        if state.sprites.is_none() {
            let material = SpriteMaterial::new(frame.ctx, frame.surface_format)
                .expect("Failed to create sprite material");

            // Create sprite centers scattered around
            let mut centers = Vec::new();
            for i in 0..50 {
                let t = i as f32 / 50.0;
                let angle = t * std::f32::consts::TAU * 3.0;
                let radius = 2.0 + t * 2.0;
                let y = (t * 5.0).sin() * 1.5;
                centers.push(Vec3::new(angle.cos() * radius, y, angle.sin() * radius));
            }

            let sprites = Sprites::new(frame.ctx, &centers, 0.3, [0.2, 0.8, 0.4]);
            state.sprites = Some(Gm::new(sprites, material));
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let angle = frame.elapsed_time as f32 * 0.2;
        if let Some(sprites) = &mut state.sprites {
            sprites.transform = Mat4::from_rotation_y(angle);
        }

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.02, 0.02, 0.05, 1.0], 1.0),
            );
            let lights: Vec<&dyn sard::Light> = vec![];

            if let Some(sprites) = &state.sprites {
                sprites.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
