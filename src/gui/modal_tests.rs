use super::{modal, Gui, Id, ModalDismissal, ModalPolicy};
use crate::{
    window::event::{Event, Key, Modifiers, MouseButton},
    WgpuContext,
};
use glam::Vec2;

#[test]
fn modal_policy_controls_escape_and_outside_click_without_base_activation() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let escape = Event::KeyPress {
        key: Key::Escape,
        modifiers: Modifiers::default(),
        handled: false,
    };
    let outside = Event::MousePress {
        button: MouseButton::Left,
        position: (5.0, 5.0),
        modifiers: Modifiers::default(),
        handled: false,
    };
    for (event, reason) in [
        (escape, ModalDismissal::Escape),
        (outside, ModalDismissal::OutsideClick),
    ] {
        for policy in [
            ModalPolicy::Dismiss,
            ModalPolicy::Request,
            ModalPolicy::Locked,
        ] {
            let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
            let id = Id::new("operation");
            gui.begin_frame(&[], 200, 100, 0.016);
            gui.open_modal(id, policy);
            let _ = modal(&mut gui, id, Vec2::new(100.0, 60.0), |ui| {
                ui.button("Cancel")
            });
            gui.begin_frame(std::slice::from_ref(&event), 200, 100, 0.016);
            let base = gui.root().button("Underlying");
            anyhow::ensure!(!base.pressed && !base.clicked);
            match policy {
                ModalPolicy::Dismiss => anyhow::ensure!((gui.modal()).is_none()),
                ModalPolicy::Request | ModalPolicy::Locked => {
                    anyhow::ensure!((gui.modal()) == (Some(id)));
                }
            }
            let expected = match policy {
                ModalPolicy::Dismiss | ModalPolicy::Request => Some(reason),
                ModalPolicy::Locked => None,
            };
            anyhow::ensure!((gui.modal_dismissal(id)) == (expected));
            anyhow::ensure!((gui.modal_dismissal(id)).is_none());
        }
    }
    Ok(())
}
