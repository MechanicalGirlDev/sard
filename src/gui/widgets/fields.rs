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
        let r = self.allocate_response(size, Sense::DRAG, id);
        let chars: Vec<char> = text.chars().collect();
        let mut edit = self.gui().text_edit(id, chars.len());
        let pad = 5.0;
        let text_x = r.rect.min.x + pad;
        let enabled = self.enabled();

        if enabled && (r.pressed || r.dragged || r.drag_released) {
            self.gui().set_focus(Some(id));
            let origin = self.gui().mouse_press_position(crate::MouseButton::Left).x;
            let mouse = self.gui().mouse().x;
            for (mx, extend) in [(origin, self.gui().key_down(Key::Shift)), (mouse, true)]
                .into_iter()
                .skip(usize::from(!r.pressed))
            {
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
                edit.move_to(best, extend);
            }
        }
        let focused = self.gui().has_focus(id) && enabled;
        if focused {
            self.gui().use_keyboard();
        }
        let (changed, submitted) = self.gui().edit_text(id, text, &mut edit, focused);
        if focused && self.gui().key_pressed(Key::Escape) {
            self.gui().set_focus(None);
        }
        let chars: Vec<char> = text.chars().collect();

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
        if focused && !edit.selection().is_empty() {
            let selection = edit.selection();
            let before: String = chars[..selection.start].iter().collect();
            let selected: String = chars[selection].iter().collect();
            let start = text_x + self.gui().measure_styled(&before, style.font_size, ts).x;
            let width = self.gui().measure_styled(&selected, style.font_size, ts).x;
            self.gui().paint_rect(
                Rect::new(start, y, width, style.font_size * 1.2),
                p.accent_bg,
            );
        }
        if text.is_empty() && !hint.is_empty() && !focused {
            self.gui()
                .paint_text(hint, Vec2::new(text_x, y), style.font_size, p.text_faint);
        } else {
            self.gui()
                .paint_text_styled(text, Vec2::new(text_x, y), style.font_size, fg, ts);
        }
        if focused && (self.gui().time() * 2.0).fract() < 0.5 {
            let prefix: String = chars[..edit.cursor].iter().collect();
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
