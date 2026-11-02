use super::ScrollState;
use crate::{
    gui::{Gui, Id, Sense},
    scene::OffscreenTarget,
    window::event::{Event, Modifiers, MouseButton},
    ClearState, WgpuContext,
};

fn click(button: MouseButton, position: (f32, f32)) -> [Event; 2] {
    [
        Event::MousePress {
            button,
            position,
            modifiers: Modifiers::default(),
            handled: false,
        },
        Event::MouseRelease {
            button,
            position,
            modifiers: Modifiers::default(),
            handled: false,
        },
    ]
}

fn rows(gui: &mut Gui, state: &mut ScrollState, count: usize) -> (Option<usize>, bool) {
    let mut selected = None;
    let mut menu_picked = false;
    let _ = gui.root().scroll_panel("input", 0.0, 0.0, state, |_, ui| {
        ui.set_gap(0.0);
        for index in 0..count {
            let rect = ui.allocate_row(20.0);
            let response = ui.interact(Id::new(index), rect, Sense::CLICK);
            ui.gui().paint_rect(rect, [0.0, 0.0, 1.0, 1.0]);
            if response.clicked {
                selected = Some(index);
            }
            ui.context_menu(&response, |menu| menu_picked |= menu.item("Choose"));
        }
    });
    (selected, menu_picked)
}

#[test]
fn measured_following_hits_the_displayed_row_then_the_corrected_row() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut state = ScrollState::following_end();
    for _ in 0..2 {
        gui.begin_frame(&[], 200, 100, 0.016);
        let _ = rows(&mut gui, &mut state, 10);
    }
    // Input arrives for the old visible bottom row while a new row is appended.
    gui.begin_frame(&click(MouseButton::Left, (40.0, 90.0)), 200, 100, 0.016);
    anyhow::ensure!(rows(&mut gui, &mut state, 11).0 == Some(9));
    anyhow::ensure!((state.offset() - 120.0).abs() < f32::EPSILON);
    // The next input uses the new bottom row, now actually displayed.
    gui.begin_frame(&click(MouseButton::Left, (40.0, 90.0)), 200, 100, 0.016);
    anyhow::ensure!(rows(&mut gui, &mut state, 11).0 == Some(10));
    Ok(())
}

#[test]
fn measured_following_preserves_popup_paint_and_input_in_window_space() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gui = Gui::new(&ctx, format);
    let mut state = ScrollState::following_end();
    for _ in 0..2 {
        gui.begin_frame(&[], 200, 100, 0.016);
        let _ = rows(&mut gui, &mut state, 10);
    }
    gui.begin_frame(&click(MouseButton::Right, (20.0, 10.0)), 200, 100, 0.016);
    let _ = rows(&mut gui, &mut state, 11);
    let target = OffscreenTarget::new(&ctx, 200, 100, format);
    target
        .target(&ctx)
        .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
    let mut encoder = ctx.create_encoder(Some("popup during measured append"));
    gui.render(&ctx, target.view(), &mut encoder)?;
    ctx.submit([encoder.finish()]);
    let pixels = target.read_pixels(&ctx);
    let at = (15 * 200 + 25) * 4;
    let expected = gui
        .style()
        .palette
        .popup
        .map(|channel| (channel * 255.0).round() as u8);
    anyhow::ensure!(
        pixels[at..at + 4] == expected,
        "popup painting must not follow the body translation"
    );
    gui.begin_frame(&click(MouseButton::Left, (30.0, 20.0)), 200, 100, 0.016);
    anyhow::ensure!(
        rows(&mut gui, &mut state, 11).1,
        "popup hit rectangle and persistent position must match its painting"
    );
    Ok(())
}
