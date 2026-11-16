//! UI System
//!
//! Provides a simple immediate mode UI system on top of [`PrimitiveRenderer`] and
//! [`TextRenderer`].
//!
//! # Widget identity
//!
//! Interactive widgets that need to persist state across frames (drag-capture for
//! [`slider`](UiContext::slider), focus for [`text_input`](UiContext::text_input))
//! are identified by call order: the Nth interactive widget each frame gets id `N`.
//! This is the usual immediate-mode trade-off — keep the per-frame widget call order
//! stable and ids stay stable.

use crate::context::WgpuContext;
use crate::gui::primitive::PrimitiveRenderer;
use crate::gui::text::TextRenderer;
use crate::gui::text_edit::{TextEdit, TextInput};
use crate::window::event::{Event, Key, MouseButton};
use glam::Vec2;
use std::collections::HashMap;

mod text_field;

const fn button_index(button: MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
    }
}

fn point_in(p: Vec2, x: f32, y: f32, w: f32, h: f32) -> bool {
    p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h
}

/// UI Context managing draw calls and input state.
pub struct UiContext {
    primitive: PrimitiveRenderer,
    text: TextRenderer,
    viewport_width: u32,
    viewport_height: u32,

    // Input state, refreshed every `update`.
    mouse_pos: Vec2,
    mouse_down: [bool; 3],
    mouse_pressed: [bool; 3],
    mouse_released: [bool; 3],
    scroll_delta: Vec2,
    text_input: String,
    pressed_keys: Vec<Key>,
    text_editor: TextInput,
    mouse_press_shift: bool,
    mouse_press_position: Vec2,

    // Cross-frame widget state.
    active_widget: Option<u64>,
    focused_widget: Option<u64>,
    widget_counter: u64,
    blink: u32,
    text_edits: HashMap<u64, TextEdit>,
}

impl UiContext {
    /// Create a new UI context.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> Self {
        Self {
            primitive: PrimitiveRenderer::new(ctx, format),
            text: TextRenderer::new(ctx, format),
            viewport_width: 0,
            viewport_height: 0,
            mouse_pos: Vec2::ZERO,
            mouse_down: [false; 3],
            mouse_pressed: [false; 3],
            mouse_released: [false; 3],
            scroll_delta: Vec2::ZERO,
            text_input: String::new(),
            pressed_keys: Vec::new(),
            text_editor: TextInput::default(),
            mouse_press_shift: false,
            mouse_press_position: Vec2::ZERO,
            active_widget: None,
            focused_widget: None,
            widget_counter: 0,
            blink: 0,
            text_edits: HashMap::new(),
        }
    }

    /// Update input state and prepare for a new frame.
    ///
    /// Ingests the full event set — every mouse button, the wheel, key presses and
    /// composed text — so widgets can react to more than just the left mouse button.
    pub fn update(&mut self, events: &[Event], width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        self.mouse_pressed = [false; 3];
        self.mouse_released = [false; 3];
        self.scroll_delta = Vec2::ZERO;
        self.text_input.clear();
        self.pressed_keys.clear();
        self.text_editor.update(events);
        self.mouse_press_shift = false;
        self.widget_counter = 0;
        self.blink = self.blink.wrapping_add(1);

        for event in events {
            match event {
                Event::MouseMotion { position, .. } => {
                    self.mouse_pos = Vec2::new(position.0, position.1);
                }
                Event::MousePress {
                    button,
                    position,
                    modifiers,
                    ..
                } => {
                    self.mouse_pos = Vec2::new(position.0, position.1);
                    let i = button_index(*button);
                    self.mouse_down[i] = true;
                    self.mouse_pressed[i] = true;
                    if *button == MouseButton::Left {
                        self.mouse_press_shift = modifiers.shift;
                        self.mouse_press_position = self.mouse_pos;
                    }
                }
                Event::MouseRelease {
                    button, position, ..
                } => {
                    self.mouse_pos = Vec2::new(position.0, position.1);
                    let i = button_index(*button);
                    self.mouse_down[i] = false;
                    self.mouse_released[i] = true;
                }
                Event::MouseWheel { delta, .. } => {
                    self.scroll_delta += Vec2::new(delta.0, delta.1);
                }
                Event::KeyPress { key, .. } => self.pressed_keys.push(*key),
                Event::Text { text } => self.text_input.push_str(text),
                _ => {}
            }
        }

        // A left press anywhere defocuses; a `text_input` re-claims focus the same
        // frame if the press landed inside it.
        if self.mouse_pressed[0] {
            self.focused_widget = None;
        }
        if self.pressed_keys.contains(&Key::Escape) {
            self.focused_widget = None;
            self.active_widget = None;
        }
        // Keep capture through the release frame so fields see its final position.
        if !self.mouse_down[0] && !self.mouse_released[0] {
            self.active_widget = None;
        }

        self.primitive.finish();
        self.text.begin_frame();
    }

    /// Render the UI to the surface (primitives first, text on top).
    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        self.primitive
            .prepare(ctx, self.viewport_width, self.viewport_height);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Primitives Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.primitive.render(&mut pass);
        }

        self.text.render(
            ctx,
            encoder,
            view,
            self.viewport_width,
            self.viewport_height,
        )?;

        Ok(())
    }

    // ----- input accessors -------------------------------------------------

    /// Current cursor position in physical pixels.
    pub fn mouse_pos(&self) -> Vec2 {
        self.mouse_pos
    }

    /// Whether the given mouse button is currently held.
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        self.mouse_down[button_index(button)]
    }

    /// Whether the given mouse button went down this frame.
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed[button_index(button)]
    }

    /// Whether the left mouse button was released this frame (a "click").
    pub fn mouse_clicked(&self) -> bool {
        self.mouse_released[0]
    }

    /// Accumulated scroll-wheel delta for this frame (`y` is the vertical wheel).
    pub fn scroll_delta(&self) -> Vec2 {
        self.scroll_delta
    }

    /// Keys pressed this frame.
    pub fn pressed_keys(&self) -> &[Key] {
        &self.pressed_keys
    }

    /// Text typed this frame (shift/layout-correct, control characters excluded).
    pub fn typed_text(&self) -> &str {
        &self.text_input
    }

    /// Whether a widget currently holds keyboard focus (e.g. a focused
    /// [`text_input`](Self::text_input)). Use this to suppress global keyboard
    /// shortcuts while the user is typing into a field.
    pub fn has_keyboard_focus(&self) -> bool {
        self.focused_widget.is_some()
    }

    /// Whether the cursor is inside the given rectangle.
    pub fn is_hovered(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        point_in(self.mouse_pos, x, y, w, h)
    }

    fn next_id(&mut self) -> u64 {
        self.widget_counter += 1;
        self.widget_counter
    }

    // ----- raw drawing escape hatch ---------------------------------------

    /// Draw a filled rectangle directly (behind/around widgets).
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.primitive.draw_rect(x, y, w, h, color);
    }

    /// Draw a rectangle outline of the given thickness.
    pub fn rect_outline(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        self.primitive.draw_line(x, y, x + w, y, thickness, color);
        self.primitive
            .draw_line(x + w, y, x + w, y + h, thickness, color);
        self.primitive
            .draw_line(x + w, y + h, x, y + h, thickness, color);
        self.primitive.draw_line(x, y + h, x, y, thickness, color);
    }

    /// Draw a line segment.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, color: [f32; 4]) {
        self.primitive.draw_line(x0, y0, x1, y1, thickness, color);
    }

    /// Draw a connected polyline.
    pub fn polyline(&mut self, points: &[[f32; 2]], thickness: f32, color: [f32; 4]) {
        self.primitive.draw_polyline(points, thickness, color);
    }

    /// Draw a filled circle (`x`, `y` is the centre).
    pub fn circle(&mut self, x: f32, y: f32, radius: f32, color: [f32; 4]) {
        self.primitive
            .draw_circle(x - radius, y - radius, radius, color);
    }

    /// Draw text at a custom size.
    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
        self.text.draw_text(text, x, y, size, color);
    }

    /// Measure text at the given size.
    pub fn measure(&mut self, text: &str, size: f32) -> (f32, f32) {
        self.text.measure(text, size)
    }

    /// Direct access to the underlying primitive renderer (full escape hatch).
    pub fn primitives(&mut self) -> &mut PrimitiveRenderer {
        &mut self.primitive
    }

    /// Direct access to the underlying text renderer (full escape hatch).
    pub fn texts(&mut self) -> &mut TextRenderer {
        &mut self.text
    }

    // ----- clipping / scroll ----------------------------------------------

    /// Push a clip rectangle applied to both primitives and text. Balance with
    /// [`pop_clip`](Self::pop_clip).
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.primitive.push_clip(x, y, w, h);
        self.text.push_clip(x, y, w, h);
    }

    /// Pop the most recently pushed clip rectangle.
    pub fn pop_clip(&mut self) {
        self.primitive.pop_clip();
        self.text.pop_clip();
    }

    /// Begin a vertically scrollable region.
    ///
    /// Draws the background and scrollbar, applies wheel input when hovered, clamps
    /// `scroll`, clips to the region and returns the `y` coordinate at which content
    /// should start (i.e. `y - scroll`). Draw content using that origin, then call
    /// [`end_scroll_area`](Self::end_scroll_area).
    pub fn begin_scroll_area(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        content_height: f32,
        scroll: &mut f32,
    ) -> f32 {
        self.primitive
            .draw_rect(x, y, w, h, [0.10, 0.10, 0.13, 0.9]);

        let max_scroll = (content_height - h).max(0.0);
        if point_in(self.mouse_pos, x, y, w, h) {
            *scroll -= self.scroll_delta.y;
        }
        *scroll = scroll.clamp(0.0, max_scroll);

        if max_scroll > 0.0 {
            let bar_w = 6.0;
            let bx = x + w - bar_w - 2.0;
            self.primitive
                .draw_rect(bx, y, bar_w, h, [0.20, 0.20, 0.25, 0.8]);
            let thumb_h = (h * (h / content_height)).clamp(20.0, h);
            let thumb_y = y + (*scroll / max_scroll) * (h - thumb_h);
            self.primitive
                .draw_rect(bx, thumb_y, bar_w, thumb_h, [0.50, 0.50, 0.60, 1.0]);
        }

        self.push_clip(x, y, w, h);
        y - *scroll
    }

    /// End the scrollable region opened by [`begin_scroll_area`](Self::begin_scroll_area).
    pub fn end_scroll_area(&mut self) {
        self.pop_clip();
    }

    // ----- widgets ---------------------------------------------------------

    /// Draw a text label.
    pub fn label(&mut self, text: &str, x: f32, y: f32, color: [f32; 4]) {
        self.text.draw_text(text, x, y, 14.0, color);
    }

    /// Draw a button. Returns `true` on the frame it is clicked.
    pub fn button(&mut self, text: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
        let hovered = point_in(self.mouse_pos, x, y, w, h);
        let clicked = hovered && self.mouse_released[0];

        let color = if hovered {
            if self.mouse_down[0] {
                [0.3, 0.3, 0.3, 1.0]
            } else {
                [0.4, 0.4, 0.4, 1.0]
            }
        } else {
            [0.2, 0.2, 0.2, 1.0]
        };

        self.primitive.draw_rect(x, y, w, h, color);

        let (text_w, text_h) = self.text.measure(text, 14.0);
        let text_x = x + (w - text_w) / 2.0;
        let text_y = y + (h - text_h) / 2.0;

        self.text
            .draw_text(text, text_x, text_y, 14.0, [1.0, 1.0, 1.0, 1.0]);

        clicked
    }

    /// Draw a checkbox. Returns the new checked state.
    pub fn checkbox(&mut self, checked: bool, text: &str, x: f32, y: f32) -> bool {
        let size = 20.0;
        let hovered = point_in(self.mouse_pos, x, y, size, size);
        let clicked = hovered && self.mouse_released[0];

        let new_checked = if clicked { !checked } else { checked };

        let color = if hovered {
            [0.4, 0.4, 0.4, 1.0]
        } else {
            [0.2, 0.2, 0.2, 1.0]
        };
        self.primitive.draw_rect(x, y, size, size, color);

        if new_checked {
            let inner_size = size * 0.6;
            let offset = (size - inner_size) / 2.0;
            self.primitive.draw_rect(
                x + offset,
                y + offset,
                inner_size,
                inner_size,
                [0.8, 0.8, 0.8, 1.0],
            );
        }

        self.text
            .draw_text(text, x + size + 8.0, y + 2.0, 14.0, [1.0, 1.0, 1.0, 1.0]);

        new_checked
    }

    /// Draw a radio button. Returns `true` on the frame it is clicked.
    pub fn radio_button(&mut self, selected: bool, text: &str, x: f32, y: f32) -> bool {
        let size = 20.0;
        let hovered = point_in(self.mouse_pos, x, y, size, size);
        let clicked = hovered && self.mouse_released[0];

        let color = if hovered {
            [0.4, 0.4, 0.4, 1.0]
        } else {
            [0.2, 0.2, 0.2, 1.0]
        };
        self.primitive.draw_circle(x, y, size / 2.0, color);

        if selected {
            let inner_size = size * 0.6;
            let offset = (size - inner_size) / 2.0;
            self.primitive.draw_circle(
                x + offset,
                y + offset,
                inner_size / 2.0,
                [0.8, 0.8, 0.8, 1.0],
            );
        }

        self.text
            .draw_text(text, x + size + 8.0, y + 2.0, 14.0, [1.0, 1.0, 1.0, 1.0]);

        clicked
    }

    /// Draw a horizontal slider. Returns the (possibly updated) value.
    ///
    /// Supports click-and-drag with proper press-capture: a press near the track
    /// grabs the handle until the mouse is released, even if the cursor leaves it.
    pub fn slider(&mut self, value: f32, min: f32, max: f32, x: f32, y: f32, w: f32) -> f32 {
        let id = self.next_id();
        let track_y = y + 8.0;
        let track_h = 6.0;
        let radius = 9.0;

        let grab = point_in(
            self.mouse_pos,
            x - radius,
            track_y - 12.0,
            w + 2.0 * radius,
            30.0,
        );
        if self.mouse_pressed[0] && grab {
            self.active_widget = Some(id);
        }

        let value = if self.active_widget == Some(id) {
            let f = ((self.mouse_pos.x - x) / w).clamp(0.0, 1.0);
            min + f * (max - min)
        } else {
            value
        };

        let frac = ((value - min) / (max - min)).clamp(0.0, 1.0);
        let cx = x + frac * w;

        self.primitive
            .draw_rect(x, track_y, w, track_h, [0.25, 0.25, 0.30, 1.0]);
        self.primitive
            .draw_rect(x, track_y, frac * w, track_h, [0.45, 0.65, 0.95, 1.0]);
        let hcol = if self.active_widget == Some(id) {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            [0.85, 0.85, 0.9, 1.0]
        };
        self.primitive
            .draw_circle(cx - radius, track_y + track_h * 0.5 - radius, radius, hcol);

        value
    }
}
