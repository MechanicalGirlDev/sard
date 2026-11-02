use super::{Cursor, Key, Range, Rect, Response, Sense, TextResponse, TextStyle, Ui, Vec2};

impl Ui<'_> {
    // ----- numbers -----------------------------------------------------------------

    /// A horizontal slider over `range`. Fills the available width by default.
    pub fn slider(&mut self, value: &mut f32, range: Range) -> Response {
        self.slider_with_marker(value, range, None)
    }

    /// A slider with an optional measured-value marker (a tall thin line) on the track.
    pub fn slider_with_marker(
        &mut self,
        value: &mut f32,
        range: Range,
        marker: Option<f32>,
    ) -> Response {
        let style = self.style().clone();
        let natural = Vec2::new(self.available_width().max(40.0), style.row_height);
        let size = self.resolve_size(natural);
        let id = self.next_widget_id("slider");
        let r = self.allocate_response(size, Sense::DRAG, id);
        let enabled = self.enabled();
        let radius = 5.0;
        let track = Rect::from_min_size(
            Vec2::new(r.rect.min.x + radius, r.rect.center().y - 1.5),
            Vec2::new((r.rect.width() - radius * 2.0).max(1.0), 3.0),
        );
        if enabled && (r.dragged || r.pressed) {
            let f = ((self.gui().mouse().x - track.min.x) / track.width()).clamp(0.0, 1.0);
            *value = range.min + f * (range.max - range.min);
        }
        *value = range.clamp(*value);
        let frac = range.fraction(*value);
        let p = &style.palette;
        self.gui().paint_rect(track, p.line);
        let (filled, _) = track.split_left(track.width() * frac);
        self.gui()
            .paint_rect(filled, if enabled { p.accent } else { p.text_disabled });
        let knob = Vec2::new(track.min.x + track.width() * frac, track.center().y);
        self.gui().paint_circle(knob, radius, p.panel);
        self.gui().paint_circle(
            knob,
            radius - 1.0,
            if enabled { p.text } else { p.text_disabled },
        );
        if let Some(m) = marker {
            let x = track.min.x + track.width() * range.fraction(m);
            self.gui().paint_rect(
                Rect::new(x - 1.0, track.center().y - 8.5, 2.0, 17.0),
                if enabled { p.text } else { p.text_disabled },
            );
        }
        if (r.hovered || r.dragged) && enabled {
            self.gui().set_cursor(Cursor::EwResize);
        }
        r
    }

    /// A number that changes by dragging horizontally (`speed` units per pixel) and can
    /// be typed into after a double click. Shown with `decimals` digits.
    pub fn drag_value(
        &mut self,
        value: &mut f32,
        speed: f32,
        range: Range,
        decimals: usize,
    ) -> Response {
        self.drag_value_edit(value, speed, range, decimals).response
    }

    /// Drag a number or double-click to edit it. Valid finite text commits on Enter or
    /// focus loss; Escape cancels. `changed` reports changes to the numeric value,
    /// and `submitted` reports a successful text commit.
    pub fn drag_value_edit(
        &mut self,
        value: &mut f32,
        speed: f32,
        range: Range,
        decimals: usize,
    ) -> TextResponse {
        let style = self.style().clone();
        let id = self.next_widget_id("drag");
        let edit_id = id.with("edit");
        let before = *value;
        let size = self.resolve_size(Vec2::new(80.0, style.control_height));
        let editing = self.gui().string(edit_id).is_some();
        if editing {
            let mut buf = self.gui().string(edit_id).cloned().unwrap_or_default();
            let t = self.text_field(edit_id, &mut buf, size, "");
            let focused = self.gui().has_focus(edit_id);
            let cancelled = !self.enabled() || self.gui().key_pressed(Key::Escape);
            let parsed = buf.trim().parse::<f32>().ok().filter(|v| v.is_finite());
            let commit = !cancelled && (t.submitted || !focused);
            let submitted = commit && parsed.is_some();
            if cancelled || submitted || !focused {
                if submitted {
                    if let Some(v) = parsed {
                        *value = range.clamp(v);
                    }
                }
                self.gui().set_string(edit_id, None);
                if focused {
                    self.gui().set_focus(None);
                }
            } else {
                self.gui().set_string(edit_id, Some(buf));
            }
            return TextResponse {
                response: t.response,
                changed: value.to_bits() != before.to_bits(),
                submitted,
            };
        }
        let text = format!("{value:.decimals$}");
        let measured = self
            .gui()
            .measure_styled(&text, style.font_size, TextStyle::MONO);
        let r = self.allocate_response(size, Sense::DRAG, id);
        let enabled = self.enabled();
        if r.pressed {
            self.gui().set_scalar(id.with("start_value"), *value);
        }
        if enabled && (r.dragged || r.drag_released) {
            if let Some(origin) = self.gui().mouse_press_origin(crate::MouseButton::Left) {
                let start = self.gui().scalar(id.with("start_value"));
                *value = range.clamp(start + (self.gui().mouse().x - origin.x) * speed);
            }
        }
        if enabled && r.double_clicked {
            self.gui().set_string(edit_id, Some(text.clone()));
            self.gui().request_focus(edit_id);
            self.gui().clear_active();
        }
        let p = &style.palette;
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, p.line_hard)
        } else if r.hovered || r.dragged {
            (p.sunken, p.text, p.text_faint)
        } else {
            (p.sunken, p.text, p.line_hard)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let pos = Vec2::new(r.rect.min.x + 5.0, r.rect.center().y - measured.y * 0.5);
        self.gui().push_clip(r.rect.shrink2(Vec2::new(5.0, 0.0)));
        self.gui()
            .paint_text_styled(&text, pos, style.font_size, fg, TextStyle::MONO);
        self.gui().pop_clip();
        if (r.hovered || r.dragged) && enabled {
            self.gui().set_cursor(Cursor::EwResize);
        }
        TextResponse {
            response: r,
            changed: value.to_bits() != before.to_bits(),
            submitted: false,
        }
    }

    /// A progress bar (`0..=1`): a sunken groove with an accent fill.
    pub fn progress(&mut self, fraction: f32) -> Response {
        let style = self.style().clone();
        let natural = Vec2::new(self.available_width().max(20.0), 5.0);
        let size = self.resolve_size(natural);
        let id = self.next_widget_id("progress");
        let r = self.allocate_response(size, Sense::HOVER, id);
        self.gui().paint_rect(r.rect, style.palette.sunken);
        let (filled, _) = r.rect.split_left(r.rect.width() * fraction.clamp(0.0, 1.0));
        self.gui().paint_rect(filled, style.palette.accent);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_fraction() {
        let r = Range::new(-1.0, 1.0);
        assert!((r.fraction(0.0) - 0.5).abs() < 1e-6);
        assert_eq!(Range::new(2.0, 2.0).fraction(5.0), 0.0);
    }
}
