use super::ScrollState;
use crate::{
    gui::{Gui, Id, Sense},
    scene::OffscreenTarget,
    window::event::{Event, Modifiers, MouseButton},
    ClearState, WgpuContext,
};
use glam::Vec2;

fn rows(gui: &mut Gui, state: &mut ScrollState, count: usize) -> bool {
    let popup = Id::new("anchored");
    let mut picked = false;
    let _ = gui.root().scroll_panel("input", 0.0, 0.0, state, |_, ui| {
        ui.set_gap(0.0);
        for index in 0..count {
            let rect = ui.allocate_row(20.0);
            let response = ui.interact(Id::new(index), rect, Sense::CLICK);
            ui.gui().paint_rect(rect, [0.0, 0.0, 1.0, 1.0]);
            if index == 9 {
                if response.clicked {
                    ui.gui().set_open(popup, true);
                }
                ui.popup(popup, rect, Vec2::new(90.0, 16.0), |ui| {
                    ui.set_next_height(10.0);
                    picked = ui.button("Choose").clicked;
                });
            }
        }
    });
    picked
}

fn click(y: f32) -> [Event; 2] {
    [
        Event::MousePress {
            button: MouseButton::Left,
            position: (20.0, y),
            modifiers: Modifiers::default(),
            handled: false,
        },
        Event::MouseRelease {
            button: MouseButton::Left,
            position: (20.0, y),
            modifiers: Modifiers::default(),
            handled: false,
        },
    ]
}

#[test]
fn default_measured_scroll_follows_after_wheel_and_reanchors_popup_before_present(
) -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gui = Gui::new(&ctx, format);
    let mut state = ScrollState::default();
    gui.begin_frame(&[], 200, 100, 0.016);
    let _ = rows(&mut gui, &mut state, 10);
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
    let _ = rows(&mut gui, &mut state, 10);
    anyhow::ensure!(
        state.is_following_end(),
        "default Inspector states can enter following through user input"
    );
    gui.begin_frame(&click(90.0), 200, 100, 0.016);
    let _ = rows(&mut gui, &mut state, 11);
    let target = OffscreenTarget::new(&ctx, 200, 100, format);
    target
        .target(&ctx)
        .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
    let mut encoder = ctx.create_encoder(Some("anchored popup during measured append"));
    gui.render(&ctx, target.view(), &mut encoder)?;
    ctx.submit([encoder.finish()]);
    let pixels = target.read_pixels(&ctx);
    let at = (83 * 200 + 2) * 4;
    let expected = gui
        .style()
        .palette
        .popup
        .map(|channel| (channel * 255.0).round() as u8);
    anyhow::ensure!(
        pixels[at..at + 4] == expected,
        "the corrected anchor now fits a popup below it"
    );
    gui.begin_frame(&click(89.0), 200, 100, 0.016);
    anyhow::ensure!(
        rows(&mut gui, &mut state, 11),
        "the corrected popup remains clickable on the next input frame"
    );
    Ok(())
}
