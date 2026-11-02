use super::Gui;
use crate::{
    scene::OffscreenTarget,
    window::event::{Event, Modifiers, MouseButton},
    ClearState, WgpuContext,
};

fn click(y: f32) -> [Event; 2] {
    [
        Event::MousePress {
            button: MouseButton::Left,
            position: (30.0, y),
            modifiers: Modifiers::default(),
            handled: false,
        },
        Event::MouseRelease {
            button: MouseButton::Left,
            position: (30.0, y),
            modifiers: Modifiers::default(),
            handled: false,
        },
    ]
}

fn build(gui: &mut Gui, selected: &mut bool) {
    gui.root().menu_bar(|ui| {
        ui.menu("View", |menu| {
            menu.heading("Workspace and robot configuration panels");
            let _ = menu.toggle("Visible", selected);
        });
    });
}

#[test]
fn heading_paints_inside_menu_without_activating_or_disabling_items() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gui = Gui::new(&ctx, format);
    let mut selected = false;
    gui.begin_frame(&click(10.0), 260, 140, 0.016);
    build(&mut gui, &mut selected);
    gui.begin_frame(&click(33.0), 260, 140, 0.016);
    build(&mut gui, &mut selected);
    anyhow::ensure!(!selected, "a category heading is not an actionable item");
    gui.begin_frame(&[], 260, 140, 0.016);
    build(&mut gui, &mut selected);
    anyhow::ensure!(
        gui.any_popup_open(),
        "clicking a heading must leave the menu open"
    );
    let target = OffscreenTarget::new(&ctx, 260, 140, format);
    target
        .target(&ctx)
        .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
    let mut encoder = ctx.create_encoder(Some("menu heading"));
    gui.render(&ctx, target.view(), &mut encoder)?;
    ctx.submit([encoder.finish()]);
    let pixels = target.read_pixels(&ctx);
    let mut ink = false;
    for y in 26..40 {
        for x in 10..180 {
            let at = (y * 260 + x) * 4;
            ink |= pixels[at] > 50;
        }
        for x in 220..260 {
            let at = (y * 260 + x) * 4;
            anyhow::ensure!(
                pixels[at..at + 3] == [0, 0, 0],
                "heading must respect menu width"
            );
        }
    }
    anyhow::ensure!(
        ink,
        "the heading must draw readable text, not just reserve space"
    );
    gui.begin_frame(&click(56.0), 260, 140, 0.016);
    build(&mut gui, &mut selected);
    anyhow::ensure!(
        selected,
        "heading must preserve the following item's enabled state"
    );
    Ok(())
}
