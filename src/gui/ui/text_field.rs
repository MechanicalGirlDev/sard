use super::{point_in, TextEdit, TextRenderer, UiContext};

impl UiContext {
    /// Draw an editable single-line text field. Returns `true` if `text` changed.
    ///
    /// Click to place the caret, drag or Shift-click to select, and use the
    /// keyboard to edit or copy, cut, and paste. Escape or clicking elsewhere
    /// releases focus.
    pub fn text_input(&mut self, text: &mut String, x: f32, y: f32, w: f32) -> bool {
        let id = self.next_id();
        let h = 26.0;
        let pad = 6.0;
        let len = text.chars().count();
        let mut edit = *self
            .text_edits
            .entry(id)
            .or_insert_with(|| TextEdit::at(len));
        edit.cursor = edit.cursor.min(len);
        edit.anchor = edit.anchor.min(len);

        let pressed = self.mouse_pressed[0]
            && !self.pressed_keys.contains(&super::Key::Escape)
            && point_in(self.mouse_press_position, x, y, w, h);
        if pressed {
            self.focused_widget = Some(id);
            self.active_widget = Some(id);
        }
        let focused = self.focused_widget == Some(id);
        if focused
            && (pressed
                || self.active_widget == Some(id) && (self.mouse_down[0] || self.mouse_released[0]))
        {
            for (at, extend) in [
                (self.mouse_press_position.x, self.mouse_press_shift),
                (self.mouse_pos.x, true),
            ]
            .into_iter()
            .skip(usize::from(!pressed))
            {
                let mut nearest = 0;
                let mut distance = f32::INFINITY;
                for position in 0..=len {
                    let width = prefix_width(&mut self.text, text, position);
                    let candidate = (x + pad + width - at).abs();
                    if candidate < distance {
                        nearest = position;
                        distance = candidate;
                    }
                }
                edit.move_to(nearest, extend);
            }
            self.blink = 0;
        }

        let changed = if focused {
            let before = edit;
            let (changed, _) = self.text_editor.edit(text, &mut edit);
            if changed || before.cursor != edit.cursor || before.anchor != edit.anchor {
                self.blink = 0;
            }
            changed
        } else {
            false
        };
        self.text_edits.insert(id, edit);

        let bg = if focused {
            [0.18, 0.18, 0.24, 1.0]
        } else {
            [0.14, 0.14, 0.18, 1.0]
        };
        self.primitive.draw_rect(x, y, w, h, bg);
        let border = if focused {
            [0.45, 0.65, 0.95, 1.0]
        } else {
            [0.30, 0.30, 0.35, 1.0]
        };
        self.rect_outline(x, y, w, h, 1.5, border);

        self.push_clip(x + 1.0, y, w - 2.0, h);
        if focused {
            let selection = edit.selection();
            if !selection.is_empty() {
                let start = prefix_width(&mut self.text, text, selection.start);
                let end = prefix_width(&mut self.text, text, selection.end);
                self.primitive.draw_rect(
                    x + pad + start,
                    y + 5.0,
                    end - start,
                    16.0,
                    [0.30, 0.45, 0.70, 1.0],
                );
            }
        }
        self.text
            .draw_text(text, x + pad, y + 5.0, 14.0, [0.95, 0.95, 0.95, 1.0]);
        if focused && (self.blink / 30).is_multiple_of(2) {
            let caret = prefix_width(&mut self.text, text, edit.cursor);
            self.primitive
                .draw_rect(x + pad + caret, y + 5.0, 1.5, 16.0, [0.95, 0.95, 0.95, 1.0]);
        }
        self.pop_clip();

        changed
    }
}

fn prefix_width(renderer: &mut TextRenderer, text: &str, position: usize) -> f32 {
    let prefix: String = text.chars().take(position).collect();
    renderer.measure(&prefix, 14.0).0
}
