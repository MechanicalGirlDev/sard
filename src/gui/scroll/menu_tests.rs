use super::ScrollState;
use crate::{
    gui::Gui,
    scene::OffscreenTarget,
    window::event::{Event, Modifiers, MouseButton},
    ClearState, WgpuContext,
};

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

fn menu(gui: &mut Gui, state: &mut ScrollState, prefix: f32) -> (bool, usize) {
    let mut picked = false;
    let mut callbacks = 0;
    let _ = gui.root().scroll_panel("menu", 0.0, 0.0, state, |_, ui| {
        ui.set_gap(0.0);
        let _ = ui.allocate_row(prefix);
        ui.menu("File", |menu| {
            callbacks += 1;
            picked = menu.item("Choose");
        });
        let rest = ui.allocate_row(80.0);
        ui.gui().paint_rect(rest, [0.0, 0.0, 1.0, 1.0]);
    });
    (picked, callbacks)
}

#[test]
fn menu_dropdown_follows_corrected_title_with_matching_paint_and_hit_bounds() -> anyhow::Result<()>
{
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gui = Gui::new(&ctx, format);
    let mut state = ScrollState::following_end();
    for _ in 0..2 {
        gui.begin_frame(&[], 200, 100, 0.016);
        let _ = menu(&mut gui, &mut state, 100.0);
    }
    gui.begin_frame(&click(10.0), 200, 100, 0.016);
    anyhow::ensure!(menu(&mut gui, &mut state, 100.0).1 == 1);
    gui.begin_frame(&[], 200, 100, 0.016);
    anyhow::ensure!(
        menu(&mut gui, &mut state, 120.0).1 == 1,
        "menu callback must not be replayed"
    );
    let target = OffscreenTarget::new(&ctx, 200, 100, format);
    target
        .target(&ctx)
        .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
    let mut encoder = ctx.create_encoder(Some("menu during measured follow"));
    gui.render(&ctx, target.view(), &mut encoder)?;
    ctx.submit([encoder.finish()]);
    let pixels = target.read_pixels(&ctx);
    let at = (22 * 200 + 5) * 4;
    let expected = gui
        .style()
        .palette
        .popup
        .map(|channel| (channel * 255.0).round() as u8);
    anyhow::ensure!(
        pixels[at..at + 4] == expected,
        "dropdown must remain below its corrected title"
    );
    gui.begin_frame(&click(33.0), 200, 100, 0.016);
    anyhow::ensure!(
        menu(&mut gui, &mut state, 120.0).0,
        "corrected dropdown must accept the next input"
    );
    Ok(())
}
