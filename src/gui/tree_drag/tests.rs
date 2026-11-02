use super::*;
use crate::{
    window::event::{Event, Modifiers},
    WgpuContext,
};

fn pointer(y: f32, pressed: bool) -> Event {
    let position = (40.0, y);
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

fn rows(gui: &mut Gui, drag: &mut TreeDrag<u32>, accepted: bool) -> Option<TreeDrop<u32>> {
    drag.begin_frame(gui, true);
    let mut ui = gui.root();
    ui.set_gap(0.0);
    for key in 0..2 {
        let node = ui.tree_node(key, "Object", false, true, |_| {});
        let _ = drag.row(&mut ui, key, &node.response, true);
    }
    drag.finish(&mut ui, |_, _, position| accepted.then_some(position))
}

#[test]
fn tree_drop_resolves_all_positions_and_rejects_invalid_targets() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    for (y, position) in [
        (2.0, TreeDropPosition::Before),
        (10.0, TreeDropPosition::Inside),
        (18.0, TreeDropPosition::After),
    ] {
        for accepted in [false, true] {
            let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
            let mut drag = TreeDrag::default();
            // The destination is visited before the source; both events are batched.
            gui.begin_frame(&[pointer(30.0, true), pointer(y, false)], 200, 100, 0.016);
            let result = rows(&mut gui, &mut drag, accepted);
            anyhow::ensure!(
                (result)
                    == (accepted.then_some(TreeDrop {
                        source: 1,
                        target: 0,
                        position
                    }))
            );
            gui.begin_frame(&[], 200, 100, 0.016);
            anyhow::ensure!(rows(&mut gui, &mut drag, accepted).is_none());
        }
    }
    Ok(())
}

#[test]
fn escape_cancels_tree_drag_before_release() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut drag = TreeDrag::default();
    gui.begin_frame(&[pointer(30.0, true)], 200, 100, 0.016);
    anyhow::ensure!(rows(&mut gui, &mut drag, true).is_none());
    gui.begin_frame(
        &[Event::KeyPress {
            key: Key::Escape,
            modifiers: Modifiers::default(),
            handled: false,
        }],
        200,
        100,
        0.016,
    );
    anyhow::ensure!(rows(&mut gui, &mut drag, true).is_none());
    gui.begin_frame(&[pointer(10.0, false)], 200, 100, 0.016);
    anyhow::ensure!(rows(&mut gui, &mut drag, true).is_none());
    anyhow::ensure!(!drag.is_dragging());
    Ok(())
}
