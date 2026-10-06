//! Shapes 2D - GUI primitive rendering
//!
//! Demonstrates 2D rectangle and circle rendering using the PrimitiveRenderer.
//!
//! Run with: cargo run

use sard::{screen_target, ClearState, FrameOutput, Window, WindowSettings};

fn main() -> anyhow::Result<()> {
    let window = Window::new(WindowSettings::default().title("2D Shapes").size(800, 600))?;

    struct State {
        renderer: Option<sard::gui::PrimitiveRenderer>,
    }

    let state = State { renderer: None };

    window.render_loop(state, |state, frame| {
        if state.renderer.is_none() {
            state.renderer = Some(sard::gui::PrimitiveRenderer::new(
                frame.ctx,
                frame.surface_format,
            ));
        }

        let renderer = state.renderer.as_mut().unwrap();
        let vp = frame.viewport;
        let time = frame.elapsed_time as f32;

        // Draw animated rectangles
        for i in 0..5 {
            let t = i as f32 / 5.0;
            let x = 50.0 + (time * 0.5 + t * std::f32::consts::TAU).sin() * 100.0 + 150.0;
            let y = 100.0 + i as f32 * 80.0;
            let w = 60.0 + (time + t * 3.0).sin() * 20.0;
            let h = 40.0;
            let color = [t, 1.0 - t, 0.5, 0.8];
            renderer.draw_rect(x, y, w, h, color);
        }

        // Draw animated circles
        for i in 0..8 {
            let t = i as f32 / 8.0;
            let angle = t * std::f32::consts::TAU + time * 0.3;
            let cx = vp.width as f32 / 2.0 + angle.cos() * 150.0;
            let cy = vp.height as f32 / 2.0 + angle.sin() * 150.0;
            let radius = 20.0 + (time * 2.0 + t * 5.0).sin() * 10.0;
            let color = [
                (t * std::f32::consts::TAU).sin() * 0.5 + 0.5,
                (t * std::f32::consts::TAU + 2.0).sin() * 0.5 + 0.5,
                (t * std::f32::consts::TAU + 4.0).sin() * 0.5 + 0.5,
                0.9,
            ];
            renderer.draw_circle(cx - radius, cy - radius, radius, color);
        }

        // Draw a large centered rectangle
        let rect_w = 200.0;
        let rect_h = 100.0;
        let rect_x = (vp.width as f32 - rect_w) / 2.0;
        let rect_y = (vp.height as f32 - rect_h) / 2.0;
        renderer.draw_rect(rect_x, rect_y, rect_w, rect_h, [0.2, 0.3, 0.8, 0.6]);

        renderer.prepare(frame.ctx, vp.width, vp.height);

        let target = screen_target(&frame);
        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        {
            let mut pass = target.begin_render_pass(
                &mut encoder,
                ClearState::color_and_depth([0.1, 0.1, 0.15, 1.0], 1.0),
            );
            renderer.render(&mut pass);
        }

        frame.ctx.submit([encoder.finish()]);
        renderer.finish();

        FrameOutput::default()
    })
}
