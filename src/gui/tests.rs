//! Headless checks of the widget layer: input handling, popups, docking, and one rendered
//! frame — all with a GPU when an adapter is available (skipped otherwise).

#![allow(clippy::print_stderr, reason = "tests report when no GPU is available")]

use glam::Vec2;

use crate::context::WgpuContext;
use crate::scene::OffscreenTarget;
use crate::window::event::{Event, Modifiers, MouseButton};

use super::{ButtonKind, Dir, DockNode, DockPanels, DockTree, Gui, Range, Rect, Ui};

fn gpu() -> Option<WgpuContext> {
    match WgpuContext::new_blocking(None) {
        Ok(ctx) => Some(ctx),
        Err(e) => {
            eprintln!("no GPU adapter, skipping: {e}");
            None
        }
    }
}

fn press(x: f32, y: f32) -> Event {
    Event::MousePress {
        button: MouseButton::Left,
        position: (x, y),
        modifiers: Modifiers::default(),
        handled: false,
    }
}

fn release(x: f32, y: f32) -> Event {
    Event::MouseRelease {
        button: MouseButton::Left,
        position: (x, y),
        modifiers: Modifiers::default(),
        handled: false,
    }
}

fn motion(x: f32, y: f32) -> Event {
    Event::MouseMotion {
        delta: (0.0, 0.0),
        position: (x, y),
        modifiers: Modifiers::default(),
        handled: false,
    }
}

#[derive(Default)]
struct Widgets {
    clicks: u32,
    checked: bool,
    value: f32,
    text: String,
    choice: usize,
    button: Rect,
    check: Rect,
    slider: Rect,
    field: Rect,
}

impl Widgets {
    fn build(&mut self, ui: &mut Ui<'_>) {
        let _ = ui.heading("見出し");
        let b = ui.button_kind("押す", ButtonKind::Accent);
        self.button = b.rect;
        if b.clicked {
            self.clicks += 1;
        }
        self.check = ui.checkbox(&mut self.checked, "チェック").rect;
        ui.set_next_width(200.0);
        self.slider = ui.slider(&mut self.value, Range::new(0.0, 10.0)).rect;
        ui.set_next_width(200.0);
        self.field = ui.text_input(&mut self.text).response.rect;
        let _ = ui.combo(&mut self.choice, &["a", "b", "c"]);
        ui.horizontal(|ui| {
            let _ = ui.label("row");
            let _ = ui.small("small");
        });
    }
}

const W: u32 = 320;
const H: u32 = 240;

fn frame(gui: &mut Gui, events: &[Event], build: impl FnOnce(&mut Ui<'_>)) {
    gui.begin_frame(events, W, H, 1.0 / 60.0);
    let mut ui = gui.root();
    build(&mut ui);
}

/// Builds a few frames of UI, clicks a button and a checkbox, drags a slider, types into
/// a field, and checks the state changes and that pixels were painted.
#[test]
fn widgets_react_to_input_and_paint() {
    let Some(ctx) = gpu() else { return };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = OffscreenTarget::new(&ctx, W, H, format);
    let mut gui = Gui::new(&ctx, format);
    let mut s = Widgets::default();

    frame(&mut gui, &[], |ui| s.build(ui));
    assert!(s.button.width() > 10.0 && s.check.width() > 10.0);
    // Widgets stack vertically without overlapping.
    assert!(s.check.min.y >= s.button.max.y);
    assert!(s.slider.min.y >= s.check.max.y);

    // Click the button: press then release inside it.
    let c = s.button.center();
    frame(&mut gui, &[motion(c.x, c.y), press(c.x, c.y)], |ui| {
        s.build(ui);
    });
    frame(&mut gui, &[release(c.x, c.y)], |ui| s.build(ui));
    assert_eq!(s.clicks, 1);

    // Toggle the checkbox.
    let c = s.check.center();
    frame(&mut gui, &[motion(c.x, c.y), press(c.x, c.y)], |ui| {
        s.build(ui);
    });
    frame(&mut gui, &[release(c.x, c.y)], |ui| s.build(ui));
    assert!(s.checked);

    // Drag the slider to its right end.
    let c = s.slider.center();
    let far = s.slider.max.x + 50.0;
    frame(&mut gui, &[motion(c.x, c.y), press(c.x, c.y)], |ui| {
        s.build(ui);
    });
    frame(&mut gui, &[motion(far, c.y)], |ui| s.build(ui));
    frame(&mut gui, &[release(far, c.y)], |ui| s.build(ui));
    assert!((s.value - 10.0).abs() < 1e-3, "{}", s.value);

    // Type into the field.
    let c = s.field.center();
    frame(
        &mut gui,
        &[motion(c.x, c.y), press(c.x, c.y), release(c.x, c.y)],
        |ui| s.build(ui),
    );
    frame(
        &mut gui,
        &[Event::Text {
            text: "ロボット".into(),
        }],
        |ui| s.build(ui),
    );
    assert_eq!(s.text, "ロボット");
    assert!(gui.wants_keyboard());

    // Render the last frame and check something was painted where the button is.
    let mut encoder = ctx.create_encoder(Some("gui test"));
    target
        .target(&ctx)
        .clear(crate::ClearState::color([0.0, 0.0, 0.0, 1.0]));
    gui.render(&ctx, target.view(), &mut encoder).unwrap();
    ctx.submit([encoder.finish()]);
    let px = target.read_pixels(&ctx);
    let c = s.button.center();
    let i = ((c.y as u32) * W + c.x as u32) as usize * 4;
    assert!(
        px[i] > 40 || px[i + 1] > 40 || px[i + 2] > 40,
        "{:?}",
        &px[i..i + 4]
    );
}

#[derive(Default)]
struct MenuState {
    picked: bool,
    base_clicks: u32,
    menu: Rect,
    base: Rect,
}

impl MenuState {
    fn build(&mut self, ui: &mut Ui<'_>) {
        ui.menu_bar(|ui| {
            let before = ui.cursor();
            ui.menu("ファイル", |m| {
                if m.item("開く") {
                    self.picked = true;
                }
            });
            self.menu = Rect::from_min_size(before, Vec2::new(60.0, 26.0));
        });
        // Well below the drop-down so the outside click lands outside the popup.
        ui.add_space(120.0);
        let b = ui.button("base");
        self.base = b.rect;
        if b.clicked {
            self.base_clicks += 1;
        }
    }
}

/// A menu opens on click, blocks the base layer, and closes on an outside click.
#[test]
fn menu_opens_and_blocks() {
    let Some(ctx) = gpu() else { return };
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut s = MenuState::default();
    frame(&mut gui, &[], |ui| s.build(ui));
    let c = s.menu.center();
    frame(&mut gui, &[motion(c.x, c.y), press(c.x, c.y)], |ui| {
        s.build(ui);
    });
    frame(&mut gui, &[release(c.x, c.y)], |ui| s.build(ui));
    frame(&mut gui, &[], |ui| s.build(ui));
    assert!(gui.any_popup_open());
    // The base button under an open menu does not react; the click closes the menu.
    let b = s.base.center();
    frame(&mut gui, &[motion(b.x, b.y), press(b.x, b.y)], |ui| {
        s.build(ui);
    });
    frame(&mut gui, &[release(b.x, b.y)], |ui| s.build(ui));
    assert_eq!(s.base_clicks, 0);
    frame(&mut gui, &[], |ui| s.build(ui));
    assert!(!gui.any_popup_open(), "outside click should close the menu");
    assert!(!s.picked);
}

#[test]
fn disabled_menu_row_does_not_activate_or_close_popup() {
    let Some(ctx) = gpu() else { return };
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut clicks = [0, 0];
    fn build(ui: &mut Ui<'_>, clicks: &mut [u32; 2]) -> [Vec2; 3] {
        let mut positions = [Vec2::ZERO; 3];
        ui.menu_bar(|ui| {
            let before = ui.cursor();
            let row_height = ui.style().row_height;
            positions[0] = before + Vec2::new(20.0, row_height * 0.5);
            // Popup rows begin below the title, inside its 1px border and 2px padding.
            positions[1] = before + Vec2::new(20.0, row_height * 1.5 + 3.0);
            positions[2] = positions[1] + Vec2::new(0.0, row_height);
            ui.menu("File", |menu| {
                menu.set_enabled(false);
                clicks[0] += u32::from(menu.item("Unavailable"));
                menu.set_enabled(true);
                clicks[1] += u32::from(menu.item_shortcut("Open", "Ctrl+O"));
            });
        });
        positions
    }
    let mut positions = [Vec2::ZERO; 3];
    frame(&mut gui, &[], |ui| positions = build(ui, &mut clicks));
    let [title, disabled, enabled] = positions;
    for events in [
        vec![motion(title.x, title.y), press(title.x, title.y)],
        vec![release(title.x, title.y)],
        vec![],
        vec![
            motion(disabled.x, disabled.y),
            press(disabled.x, disabled.y),
        ],
        vec![release(disabled.x, disabled.y)],
    ] {
        frame(&mut gui, &events, |ui| {
            build(ui, &mut clicks);
        });
    }
    assert!(gui.any_popup_open());
    assert_eq!(clicks, [0, 0]);
    frame(
        &mut gui,
        &[motion(enabled.x, enabled.y), press(enabled.x, enabled.y)],
        |ui| {
            build(ui, &mut clicks);
        },
    );
    frame(&mut gui, &[release(enabled.x, enabled.y)], |ui| {
        build(ui, &mut clicks);
    });
    assert_eq!(clicks, [0, 1]);
    // A release closes the menu while its rectangle is still registered for that
    // frame. Two ordinary frame boundaries expire current and previous rectangles.
    frame(&mut gui, &[], |ui| {
        build(ui, &mut clicks);
    });
    frame(&mut gui, &[], |ui| {
        build(ui, &mut clicks);
    });
    assert!(!gui.any_popup_open());
    assert_eq!(clicks, [0, 1]);
}

struct Panels {
    shown: Vec<String>,
}

impl DockPanels for Panels {
    fn title(&self, panel: &str) -> String {
        panel.to_uppercase()
    }

    fn show(&mut self, panel: &str, ui: &mut Ui<'_>) {
        self.shown.push(panel.to_owned());
        let _ = ui.label(panel);
    }
}

fn dock_frame(
    gui: &mut Gui,
    tree: &mut DockTree,
    panels: &mut Panels,
    events: &[Event],
) -> super::DockResponse {
    panels.shown.clear();
    gui.begin_frame(events, 400, 300, 1.0 / 60.0);
    let mut ui = gui.root();
    ui.dock(tree, panels)
}

/// The dock renders every group, activates tabs on click, and a tab drag onto another
/// group moves the panel.
#[test]
fn dock_renders_and_moves_tabs() {
    let Some(ctx) = gpu() else { return };
    let mut gui = Gui::new(&ctx, wgpu::TextureFormat::Rgba8Unorm);
    let mut tree = DockTree::new(DockNode::split(
        Dir::Horizontal,
        vec![
            DockNode::tabs(vec!["a".into(), "b".into()]),
            DockNode::tabs(vec!["c".into()]),
        ],
    ));
    let mut panels = Panels { shown: Vec::new() };
    let r = dock_frame(&mut gui, &mut tree, &mut panels, &[]);
    assert_eq!(panels.shown, vec!["a", "c"]);

    // Click the second tab of the left group.
    let tab_b = r
        .tabs
        .iter()
        .find(|(n, _)| n == "b")
        .map(|(_, r)| r.center())
        .unwrap();
    drop(dock_frame(
        &mut gui,
        &mut tree,
        &mut panels,
        &[motion(tab_b.x, tab_b.y), press(tab_b.x, tab_b.y)],
    ));
    dock_frame(
        &mut gui,
        &mut tree,
        &mut panels,
        &[release(tab_b.x, tab_b.y)],
    );
    drop(dock_frame(&mut gui, &mut tree, &mut panels, &[]));
    assert_eq!(panels.shown, vec!["b", "c"]);

    // Drag tab "b" into the right group's body.
    let right = Vec2::new(300.0, 150.0);
    dock_frame(
        &mut gui,
        &mut tree,
        &mut panels,
        &[motion(tab_b.x, tab_b.y), press(tab_b.x, tab_b.y)],
    );
    dock_frame(&mut gui, &mut tree, &mut panels, &[motion(150.0, 100.0)]);
    dock_frame(
        &mut gui,
        &mut tree,
        &mut panels,
        &[motion(right.x, right.y)],
    );
    dock_frame(
        &mut gui,
        &mut tree,
        &mut panels,
        &[release(right.x, right.y)],
    );
    assert_eq!(tree.tabs_of("b"), tree.tabs_of("c"));
    assert_eq!(tree.panels(), vec!["a", "c", "b"]);
    drop(dock_frame(&mut gui, &mut tree, &mut panels, &[]));
    assert_eq!(panels.shown, vec!["a", "b"]);
}
