use super::*;
use crate::{
    window::event::{Event, Modifiers},
    WgpuContext,
};

fn wheel(y: f32, delta: f32) -> Event {
    Event::MouseWheel {
        delta: (0.0, delta),
        position: (40.0, y),
        modifiers: Modifiers::default(),
        handled: false,
    }
}

fn visible(gui: &mut crate::gui::Gui, state: &mut ScrollState, rows: usize) -> Vec<usize> {
    let mut seen = Vec::new();
    gui.root().virtual_rows(
        "rows",
        None,
        state,
        20.0,
        rows,
        |index| index,
        |index, ui| {
            seen.push(index);
            let _ = ui.button("Row");
        },
    );
    seen
}

#[test]
fn virtual_rows_follow_append_pause_on_wheel_and_clamp_after_shrink() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = crate::gui::Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut state = ScrollState::following_end();
    gui.begin_frame(&[], 200, 100, 0.016);
    anyhow::ensure!((visible(&mut gui, &mut state, 1000)) == ((995..1000).collect::<Vec<_>>()));
    gui.begin_frame(&[wheel(20.0, 40.0)], 200, 100, 0.016);
    anyhow::ensure!((visible(&mut gui, &mut state, 1000)) == ((993..998).collect::<Vec<_>>()));
    gui.begin_frame(&[], 200, 100, 0.016);
    anyhow::ensure!((visible(&mut gui, &mut state, 1001)) == ((993..998).collect::<Vec<_>>()));
    gui.begin_frame(&[], 200, 100, 0.016);
    anyhow::ensure!((visible(&mut gui, &mut state, 3)) == (vec![0, 1, 2]));
    Ok(())
}

#[test]
fn scroll_panel_keeps_header_footer_fixed_and_clips_body_interactions() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = crate::gui::Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut state = ScrollState::default();
    let draw = |gui: &mut crate::gui::Gui, state: &mut ScrollState| {
        let mut body_rows = Vec::new();
        let regions =
            gui.root()
                .scroll_panel("panel", 20.0, 20.0, state, |region, ui| match region {
                    ScrollRegion::Header => {
                        let _ = ui.label("Header");
                    }
                    ScrollRegion::Footer => {
                        let _ = ui.button("Footer");
                    }
                    ScrollRegion::Body => {
                        ui.set_gap(0.0);
                        for _ in 0..20 {
                            body_rows.push(ui.button("Item"));
                        }
                    }
                });
        (regions, body_rows)
    };
    gui.begin_frame(&[], 200, 100, 0.016);
    let (before, _) = draw(&mut gui, &mut state);
    gui.begin_frame(&[wheel(30.0, -44.0)], 200, 100, 0.016);
    let (after, rows) = draw(&mut gui, &mut state);
    anyhow::ensure!((before.header) == (after.header));
    anyhow::ensure!((before.footer) == (after.footer));
    anyhow::ensure!((rows[0].rect.min.y + 24.0).abs() < f32::EPSILON);
    anyhow::ensure!(!rows[0].hovered);
    anyhow::ensure!(rows[2].hovered);
    anyhow::ensure!((after.body) == (Rect::new(0.0, 20.0, 200.0, 60.0)));
    Ok(())
}

#[test]
fn tiny_viewports_and_empty_virtual_lists_do_not_panic() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = crate::gui::Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut state = ScrollState::default();
    gui.begin_frame(&[], 10, 8, 0.016);
    anyhow::ensure!((visible(&mut gui, &mut state, 20)) == (vec![0]));
    gui.begin_frame(&[], 10, 8, 0.016);
    anyhow::ensure!(visible(&mut gui, &mut state, 0).is_empty());
    Ok(())
}
