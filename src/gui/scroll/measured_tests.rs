use super::ScrollState;
use crate::{
    gui::{Gui, Rect},
    scene::OffscreenTarget,
    window::event::{Event, Modifiers},
    ClearState, WgpuContext,
};
use glam::Vec2;

fn measured(gui: &mut Gui, state: &mut ScrollState, rows: usize) {
    let mut calls = 0;
    let _ = gui
        .root()
        .scroll_panel("measured", 0.0, 0.0, state, |_, ui| {
            calls += 1;
            ui.set_gap(0.0);
            for row in 0..rows {
                let rect = ui.allocate_row(20.0);
                let color = if row + 1 == rows {
                    [0.0, 1.0, 0.0, 1.0]
                } else {
                    [0.0, 0.0, 1.0, 1.0]
                };
                ui.gui().push_clip(rect);
                ui.gui().paint_rect(rect, color);
                ui.gui()
                    .push_clip(Rect::new(rect.min.x + 8.0, rect.min.y + 2.0, 64.0, 16.0));
                ui.gui().paint_text(
                    &format!("Row {row}"),
                    rect.min + Vec2::new(8.0, 2.0),
                    12.0,
                    [1.0; 4],
                );
                ui.gui().pop_clip();
                ui.gui().pop_clip();
            }
        });
    assert_eq!(
        calls, 1,
        "a measured body must never be replayed to discover its height"
    );
}

#[test]
fn measured_following_updates_after_append_and_preserves_user_pause() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut state = ScrollState::following_end();
    for _ in 0..2 {
        gui.begin_frame(&[], 200, 100, 0.016);
        measured(&mut gui, &mut state, 10);
    }
    gui.begin_frame(&[], 200, 100, 0.016);
    measured(&mut gui, &mut state, 11);
    anyhow::ensure!(
        (state.offset() - 120.0).abs() < f32::EPSILON,
        "appended content must be followed this frame"
    );
    gui.begin_frame(
        &[Event::MouseWheel {
            delta: (0.0, 40.0),
            position: (40.0, 50.0),
            modifiers: Modifiers::default(),
            handled: false,
        }],
        200,
        100,
        0.016,
    );
    measured(&mut gui, &mut state, 12);
    anyhow::ensure!((state.offset() - 80.0).abs() < f32::EPSILON && !state.is_following_end());
    gui.begin_frame(&[], 200, 100, 0.016);
    measured(&mut gui, &mut state, 13);
    anyhow::ensure!(
        (state.offset() - 80.0).abs() < f32::EPSILON,
        "paused content must stay put"
    );
    gui.begin_frame(
        &[Event::MouseWheel {
            delta: (0.0, -1000.0),
            position: (40.0, 50.0),
            modifiers: Modifiers::default(),
            handled: false,
        }],
        200,
        100,
        0.016,
    );
    measured(&mut gui, &mut state, 14);
    anyhow::ensure!((state.offset() - 180.0).abs() < f32::EPSILON && state.is_following_end());
    Ok(())
}

#[test]
fn measured_append_paints_new_last_row_in_the_same_frame() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gui = Gui::new(&ctx, format);
    let mut state = ScrollState::following_end();
    for _ in 0..2 {
        gui.begin_frame(&[], 200, 100, 0.016);
        measured(&mut gui, &mut state, 10);
    }
    let mut frames = Vec::new();
    for rows in [11, 11, 3, 3] {
        gui.begin_frame(&[], 200, 100, 0.016);
        measured(&mut gui, &mut state, rows);
        let target = OffscreenTarget::new(&ctx, 200, 100, format);
        target
            .target(&ctx)
            .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
        let mut encoder = ctx.create_encoder(Some("measured follow-end"));
        gui.render(&ctx, target.view(), &mut encoder)?;
        ctx.submit([encoder.finish()]);
        frames.push(target.read_pixels(&ctx));
    }
    let pixel = (90 * 200 + 100) * 4;
    anyhow::ensure!(
        frames[0][pixel..pixel + 3] == [0, 255, 0],
        "the newly appended row must already be visible"
    );
    anyhow::ensure!(
        frames[0] == frames[1],
        "following must not require a second frame to settle"
    );
    let pixel = (50 * 200 + 100) * 4;
    anyhow::ensure!(
        frames[2][pixel..pixel + 3] == [0, 255, 0],
        "shrinking must restore content clipped above the old viewport"
    );
    anyhow::ensure!(
        frames[2] == frames[3],
        "nested clips must settle in the shrinking frame"
    );
    Ok(())
}
