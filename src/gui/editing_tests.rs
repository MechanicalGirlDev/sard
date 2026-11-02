//! Numeric edits and row selection through native input.

use super::{Gui, Id, Range, TextResponse, Ui};
use crate::{
    window::event::{Event, Key, Modifiers, MouseButton},
    WgpuContext,
};

fn pointer(pressed: bool, position: (f32, f32)) -> Event {
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

fn number(ui: &mut Ui<'_>, value: &mut f32) -> TextResponse {
    ui.set_next_id("value");
    ui.drag_value_edit(value, 0.1, Range::new(0.0, 10.0), 0)
}

#[test]
fn numeric_text_commits_clamped_value_or_cancels_on_escape() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    for (key, expected, submitted) in [(Key::Enter, 10.0_f32, true), (Key::Escape, 1.0, false)] {
        let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
        let mut value = 1.0;
        for _ in 0..2 {
            gui.begin_frame(
                &[pointer(true, (20.0, 10.0)), pointer(false, (20.0, 10.0))],
                200,
                100,
                0.016,
            );
            let _ = number(&mut gui.root(), &mut value);
        }
        gui.begin_frame(&[Event::Text { text: "2".into() }], 200, 100, 0.016);
        let editing = number(&mut gui.root(), &mut value);
        anyhow::ensure!(!editing.changed);
        gui.begin_frame(
            &[Event::KeyPress {
                key,
                modifiers: Modifiers::default(),
                handled: false,
            }],
            200,
            100,
            0.016,
        );
        let result = number(&mut gui.root(), &mut value);
        anyhow::ensure!((value - expected).abs() < f32::EPSILON);
        anyhow::ensure!((result.submitted) == (submitted));
        anyhow::ensure!((result.changed) == (submitted));
        anyhow::ensure!((result.response.rect.size()) == (editing.response.rect.size()));
    }
    Ok(())
}

#[test]
fn numeric_drag_uses_press_origin_when_motion_and_release_are_batched() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut value = 1.0;
    gui.begin_frame(
        &[pointer(true, (20.0, 10.0)), pointer(false, (70.0, 10.0))],
        200,
        100,
        0.016,
    );
    let result = number(&mut gui.root(), &mut value);
    anyhow::ensure!((value - 6.0).abs() < f32::EPSILON);
    anyhow::ensure!(result.changed && result.response.drag_released);
    Ok(())
}

#[test]
fn selectable_table_row_accepts_whitespace_without_stealing_cell_button() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    for (x, row_clicked, button_clicked) in [(180.0, true, false), (20.0, false, true)] {
        let mut child_clicked = false;
        gui.begin_frame(
            &[pointer(true, (x, 10.0)), pointer(false, (x, 10.0))],
            200,
            100,
            0.5,
        );
        let response =
            gui.root()
                .selectable_table_row(Id::new("row"), &[1.0, 1.0], false, |column, ui| {
                    if column == 0 {
                        child_clicked = ui.button("Open").clicked;
                    } else {
                        let _ = ui.label("Value");
                    }
                });
        anyhow::ensure!((response.clicked) == (row_clicked));
        anyhow::ensure!((child_clicked) == (button_clicked));
    }
    Ok(())
}
