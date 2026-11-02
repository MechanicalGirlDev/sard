//! Pointer and identity regressions driven through the public widget boundary.

use super::{Gui, Id, Rect, Sense};
use crate::window::event::{Event, Modifiers, MouseButton};
use crate::WgpuContext;
use glam::Vec2;

fn pointer(position: (f32, f32), pressed: bool) -> Event {
    if pressed {
        Event::MousePress {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            handled: false,
        }
    } else {
        Event::MouseRelease {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            handled: false,
        }
    }
}

#[test]
fn press_origin_does_not_follow_later_pointer_events() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let source = Id::new("source");
    let target = Id::new("target");
    gui.begin_frame(
        &[pointer((10.0, 10.0), true), pointer((110.0, 10.0), false)],
        200,
        100,
        0.016,
    );
    let a = gui.interact(source, Rect::new(0.0, 0.0, 50.0, 50.0), Sense::DRAG);
    let b = gui.interact(target, Rect::new(100.0, 0.0, 50.0, 50.0), Sense::CLICK);
    anyhow::ensure!(a.pressed && a.drag_released && !a.clicked);
    anyhow::ensure!(!b.pressed && !b.clicked);
    Ok(())
}

#[test]
fn nested_button_capture_survives_layout_translation() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let draw = |gui: &mut Gui, y| {
        let mut ui = gui.root();
        ui.add_space(y);
        ui.horizontal(|ui| ui.button("Move"))
    };
    gui.begin_frame(&[], 200, 100, 0.016);
    let initial = draw(&mut gui, 0.0);
    let at = initial.rect.center();
    gui.begin_frame(&[pointer(at.into(), true)], 200, 100, 0.016);
    let _ = draw(&mut gui, 0.0);
    let moved = at + Vec2::new(0.0, 30.0);
    gui.begin_frame(&[pointer(moved.into(), false)], 200, 100, 0.016);
    let released = draw(&mut gui, 30.0);
    anyhow::ensure!(
        released.clicked,
        "capture must follow widget identity, not its old position"
    );
    Ok(())
}

#[test]
fn explicit_identity_keeps_focus_when_same_kind_widgets_reorder() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut text = String::new();
    gui.begin_frame(
        &[pointer((10.0, 10.0), true), pointer((10.0, 10.0), false)],
        200,
        100,
        0.016,
    );
    {
        let mut ui = gui.root();
        ui.set_next_id("name");
        let _ = ui.text_input(&mut text);
    }
    gui.begin_frame(
        &[Event::Text {
            text: "Robot".into(),
        }],
        200,
        100,
        0.016,
    );
    let mut ui = gui.root();
    let mut inserted = String::new();
    ui.set_next_id("inserted");
    let _ = ui.text_input(&mut inserted);
    ui.set_next_id("name");
    let _ = ui.text_input(&mut text);
    let first = ui.button("Repeated");
    let second = ui.button("Repeated");
    anyhow::ensure!((text) == ("Robot"));
    anyhow::ensure!((first.id) != (second.id));
    Ok(())
}
