use super::ScrollState;
use crate::{
    gui::{Gui, Sense},
    scene::OffscreenTarget,
    window::event::{Event, Modifiers, MouseButton},
    ClearState, WgpuContext,
};

#[test]
fn nested_virtual_viewport_survives_following_ancestor_growth_without_replay() -> anyhow::Result<()>
{
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gui = Gui::new(&ctx, format);
    let mut outer = ScrollState::following_end();
    let mut inner = ScrollState::default();
    let mut rendered = Vec::new();
    for (frame, prefix) in [100.0, 100.0, 120.0, 120.0].into_iter().enumerate() {
        let events = [
            Event::MousePress {
                button: MouseButton::Left,
                position: (40.0, 115.0),
                modifiers: Modifiers::default(),
                handled: false,
            },
            Event::MouseRelease {
                button: MouseButton::Left,
                position: (40.0, 115.0),
                modifiers: Modifiers::default(),
                handled: false,
            },
        ];
        gui.begin_frame(if frame == 2 { &events } else { &[] }, 200, 100, 0.016);
        let mut callbacks = Vec::new();
        let mut actions = Vec::new();
        let _ = gui
            .root()
            .scroll_panel("outer", 0.0, 0.0, &mut outer, |_, ui| {
                ui.set_gap(0.0);
                let _ = ui.allocate_row(prefix);
                ui.virtual_rows(
                    "inner",
                    Some(100.0),
                    &mut inner,
                    10.0,
                    100_000,
                    |index| index,
                    |index, ui| {
                        callbacks.push(index);
                        let rect = ui.allocate_row(10.0);
                        if ui.interact(ui.make_id("row"), rect, Sense::CLICK).clicked {
                            actions.push(index);
                        }
                        ui.gui().paint_rect(
                            rect,
                            if index == 9 {
                                [0.0, 1.0, 0.0, 1.0]
                            } else {
                                [0.0, 0.0, 1.0, 1.0]
                            },
                        );
                    },
                );
            });
        if frame >= 2 {
            anyhow::ensure!(
                callbacks == (0..10).collect::<Vec<_>>(),
                "every local viewport row must be generated exactly once"
            );
        }
        anyhow::ensure!(
            actions.is_empty(),
            "deferred paint must not expand inherited input clipping"
        );
        let target = OffscreenTarget::new(&ctx, 200, 100, format);
        target
            .target(&ctx)
            .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
        let mut encoder = ctx.create_encoder(Some("nested virtual follow"));
        gui.render(&ctx, target.view(), &mut encoder)?;
        ctx.submit([encoder.finish()]);
        rendered.push(target.read_pixels(&ctx));
    }
    let at = (95 * 200 + 100) * 4;
    anyhow::ensure!(
        rendered[2][at..at + 3] == [0, 255, 0],
        "last virtual row must appear in the growth frame"
    );
    anyhow::ensure!(
        rendered[2] == rendered[3],
        "nested virtualization must not need a settling frame"
    );
    Ok(())
}
