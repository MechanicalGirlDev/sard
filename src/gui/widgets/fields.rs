use super::{Cursor, Id, Key, Rect, Sense, TextResponse, TextStyle, Ui, Vec2};

impl Ui<'_> {
    // ----- text input --------------------------------------------------------------

    /// A single-line text field over the available width.
    pub fn text_input(&mut self, text: &mut String) -> TextResponse {
        self.text_input_hint(text, "")
    }

    /// A text field with a placeholder shown while empty.
    pub fn text_input_hint(&mut self, text: &mut String, hint: &str) -> TextResponse {
        let h = self.style().control_height;
        let natural = Vec2::new(self.available_width().max(40.0), h);
        let size = self.resolve_size(natural);
        let id = self.next_widget_id("text");
        self.text_field(id, text, size, hint)
    }

    /// A text field with an explicit id and size.
    pub fn text_input_sized(&mut self, id: Id, text: &mut String, size: Vec2) -> TextResponse {
        self.text_field(id, text, size, "")
    }

    pub(super) fn text_field(
        &mut self,
        id: Id,
        text: &mut String,
        size: Vec2,
        hint: &str,
    ) -> TextResponse {
        let style = self.style().clone();
        let ts = TextStyle::MONO;
        let r = self.allocate_response(size, Sense::CLICK, id);
        let chars: Vec<char> = text.chars().collect();
        let mut cursor = self
            .gui()
            .text_cursor(id)
            .unwrap_or(chars.len())
            .min(chars.len());
        let pad = 5.0;
        let text_x = r.rect.min.x + pad;
        let enabled = self.enabled();

        if r.pressed && enabled {
            self.gui().set_focus(Some(id));
            // Place the caret at the nearest character boundary.
            let mx = self.gui().mouse().x;
            let mut best = chars.len();
            let mut best_d = f32::INFINITY;
            for i in 0..=chars.len() {
                let prefix: String = chars[..i].iter().collect();
                let w = self.gui().measure_styled(&prefix, style.font_size, ts).x;
                let d = (text_x + w - mx).abs();
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            cursor = best;
        }
        let focused = self.gui().has_focus(id) && enabled;
        let mut changed = false;
        let mut submitted = false;
        let mut chars = chars;
        if focused {
            self.gui().use_keyboard();
            let typed = self.gui().typed().to_owned();
            if !typed.is_empty() {
                for c in typed.chars() {
                    chars.insert(cursor, c);
                    cursor += 1;
                }
                changed = true;
            }
            let keys: Vec<Key> = self.gui().keys_pressed().to_vec();
            for k in keys {
                match k {
                    Key::Backspace if cursor > 0 => {
                        let _ = chars.remove(cursor - 1);
                        cursor -= 1;
                        changed = true;
                    }
                    Key::Delete if cursor < chars.len() => {
                        let _ = chars.remove(cursor);
                        changed = true;
                    }
                    Key::Left => cursor = cursor.saturating_sub(1),
                    Key::Right => cursor = (cursor + 1).min(chars.len()),
                    Key::Home => cursor = 0,
                    Key::End => cursor = chars.len(),
                    Key::Enter => submitted = true,
                    Key::Escape => self.gui().set_focus(None),
                    _ => {}
                }
            }
            if changed {
                *text = chars.iter().collect();
            }
        }
        self.gui().set_text_cursor(id, cursor);

        let p = &style.palette;
        let (bg, border) = if !enabled {
            (p.background, p.line_hard)
        } else if focused {
            (p.sunken, p.accent)
        } else if r.hovered {
            (p.sunken, p.text_faint)
        } else {
            (p.sunken, p.line_hard)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let inner = r.rect.shrink2(Vec2::new(pad, 0.0));
        let measured = self.gui().measure_styled(text, style.font_size, ts);
        let y = r.rect.center().y - measured.y.max(style.font_size * 1.2) * 0.5;
        let fg = if enabled { p.text } else { p.text_disabled };
        self.gui().push_clip(inner);
        if text.is_empty() && !hint.is_empty() && !focused {
            self.gui()
                .paint_text(hint, Vec2::new(text_x, y), style.font_size, p.text_faint);
        } else {
            self.gui()
                .paint_text_styled(text, Vec2::new(text_x, y), style.font_size, fg, ts);
        }
        if focused && (self.gui().time() * 2.0).fract() < 0.5 {
            let prefix: String = chars[..cursor.min(chars.len())].iter().collect();
            let cx = text_x + self.gui().measure_styled(&prefix, style.font_size, ts).x;
            let caret = Rect::new(cx, y + 1.0, 1.0, style.font_size * 1.2 - 2.0);
            self.gui().paint_rect(caret, p.text);
        }
        self.gui().pop_clip();
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Text);
        }
        TextResponse {
            response: r,
            changed,
            submitted,
        }
    }
}
