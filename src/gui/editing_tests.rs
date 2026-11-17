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
fn text_selection_replaces_unicode_text_with_batched_shortcut() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let id = Id::new("clipboard-field");
    let mut text = String::from("足立レイ");
    gui.set_focus(Some(id));
    let events = [
        Event::KeyPress {
            key: Key::A,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            handled: false,
        },
        Event::KeyRelease {
            key: Key::Control,
            modifiers: Modifiers::default(),
            handled: false,
        },
        Event::Text {
            text: "robot".into(),
        },
    ];
    gui.begin_frame(&events, 200, 100, 0.016);
    let response = gui
        .root()
        .text_input_sized(id, &mut text, glam::Vec2::new(180.0, 26.0));
    anyhow::ensure!(text == "robot", "selected text was not replaced: {text:?}");
    anyhow::ensure!(response.changed);
    Ok(())
}

#[test]
fn shifted_navigation_deletes_selected_unicode_without_splitting_characters() -> anyhow::Result<()>
{
    let ctx = WgpuContext::new_blocking(None)?;
    for key in [Key::Backspace, Key::Delete] {
        let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
        let id = Id::new("field");
        let mut text = String::from("a足立");
        gui.set_focus(Some(id));
        gui.begin_frame(
            &[
                Event::KeyPress {
                    key: Key::Left,
                    modifiers: Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    },
                    handled: false,
                },
                Event::KeyPress {
                    key,
                    modifiers: Modifiers::default(),
                    handled: false,
                },
            ],
            200,
            100,
            0.016,
        );
        let result = gui
            .root()
            .text_input_sized(id, &mut text, glam::Vec2::new(180.0, 26.0));
        anyhow::ensure!(text == "a足" && result.changed);
    }
    Ok(())
}

#[test]
fn text_drag_selects_through_a_batched_release_outside_the_field() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let drag = [pointer(true, (5.0, 10.0)), pointer(false, (250.0, 10.0))];
    let delete = [Event::KeyPress {
        key: Key::Delete,
        modifiers: Modifiers::default(),
        handled: false,
    }];
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let id = Id::new("field");
    let mut text = String::from("robot");
    gui.begin_frame(&drag, 300, 100, 0.016);
    let _ = gui
        .root()
        .text_input_sized(id, &mut text, glam::Vec2::new(180.0, 26.0));
    gui.begin_frame(&delete, 300, 100, 0.016);
    let _ = gui
        .root()
        .text_input_sized(id, &mut text, glam::Vec2::new(180.0, 26.0));
    anyhow::ensure!(text.is_empty(), "drag did not select the field: {text:?}");
    Ok(())
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
