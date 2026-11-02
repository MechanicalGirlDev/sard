use super::{Cursor, Dir, Id, Rect, Response, Sense, TextStyle, Ui, Vec2, LAYER_POPUP};

impl Ui<'_> {
    // ----- choice --------------------------------------------------------------------

    /// A drop-down over `options`. Returns `true` when `current` changed.
    pub fn combo(&mut self, current: &mut usize, options: &[&str]) -> bool {
        let style = self.style().clone();
        let id = self.next_widget_id("combo");
        let shown = options.get(*current).copied().unwrap_or("");
        let widest = options
            .iter()
            .map(|o| self.gui().measure(o, style.font_size).x)
            .fold(0.0_f32, f32::max);
        let natural = Vec2::new(widest + 5.0 + 17.0, style.control_height);
        let size = self.resolve_size(natural);
        let r = self.allocate_response(size, Sense::CLICK, id);
        let enabled = self.enabled();
        if r.clicked && enabled {
            let was = self.gui().was_open(id);
            self.gui().set_open(id, !was);
        }
        let p = &style.palette;
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, p.line_hard)
        } else if self.gui().is_open(id) {
            (p.sunken, p.text, p.accent)
        } else if r.hovered {
            (p.sunken, p.text, p.text_faint)
        } else {
            (p.sunken, p.text, p.line_hard)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let measured = self.gui().measure(shown, style.font_size);
        let pos = Vec2::new(r.rect.min.x + 5.0, r.rect.center().y - measured.y * 0.5);
        self.gui().push_clip(r.rect.shrink2(Vec2::new(4.0, 0.0)));
        self.gui().paint_text(shown, pos, style.font_size, fg);
        self.gui().pop_clip();
        // Chevron (▾).
        let c = Vec2::new(r.rect.max.x - 9.0, r.rect.center().y + 1.0);
        let cc = if enabled { p.text_dim } else { p.text_disabled };
        self.gui()
            .paint_line(c + Vec2::new(-3.5, -2.0), c + Vec2::new(0.0, 1.5), 1.2, cc);
        self.gui()
            .paint_line(c + Vec2::new(0.0, 1.5), c + Vec2::new(3.5, -2.0), 1.2, cc);
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }

        let mut changed = false;
        if self.gui().is_open(id) {
            let width = r.rect.width().max(120.0);
            let row = style.row_height;
            let height = row * options.len() as f32 + 6.0;
            self.popup(id, r.rect, Vec2::new(width, height), |ui| {
                ui.set_gap(0.0);
                for (i, o) in options.iter().enumerate() {
                    ui.set_next_width(width - 6.0);
                    if ui.menu_row(o, i == *current, "").clicked {
                        if *current != i {
                            *current = i;
                            changed = true;
                        }
                        ui.gui().set_open(id, false);
                    }
                }
            });
        }
        changed
    }

    /// One row of a menu / drop-down: a check column, the label, an optional right-aligned
    /// hint; hover = accent tint.
    pub fn menu_row(&mut self, text: &str, checked: bool, hint: &str) -> Response {
        let style = self.style().clone();
        let p = style.palette;
        let m = self.gui().measure(text, style.font_size);
        let hint_m = if hint.is_empty() {
            Vec2::ZERO
        } else {
            self.gui()
                .measure_styled(hint, style.font_size_small, TextStyle::MONO)
        };
        let natural = Vec2::new(
            8.0 + 9.0
                + 7.0
                + m.x
                + if hint.is_empty() {
                    0.0
                } else {
                    18.0 + hint_m.x
                }
                + 8.0,
            style.row_height,
        );
        let size = self.resolve_size(natural);
        let id = self.next_widget_id(("item", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let enabled = self.enabled();
        if r.hovered && enabled {
            self.gui().paint_rect(r.rect, p.accent_bg);
            self.gui().set_cursor(Cursor::Pointer);
        }
        let fg = if enabled { p.text } else { p.text_disabled };
        if checked {
            let cm = self.gui().measure("✓", style.font_size);
            self.gui().paint_text(
                "✓",
                Vec2::new(r.rect.min.x + 8.0, r.rect.center().y - cm.y * 0.5),
                style.font_size,
                p.accent,
            );
        }
        self.gui().paint_text(
            text,
            Vec2::new(
                r.rect.min.x + 8.0 + 9.0 + 7.0,
                r.rect.center().y - m.y * 0.5,
            ),
            style.font_size,
            fg,
        );
        if !hint.is_empty() {
            self.gui().paint_text_styled(
                hint,
                Vec2::new(
                    r.rect.max.x - 8.0 - hint_m.x,
                    r.rect.center().y - hint_m.y * 0.5,
                ),
                style.font_size_small,
                p.text_faint,
                TextStyle::MONO,
            );
        }
        r
    }

    /// Draw `add` in a popup below `anchor` while `id` is open. The popup lives on the
    /// popup layer and blocks the base layer until closed.
    pub fn popup(&mut self, id: Id, anchor: Rect, size: Vec2, add: impl FnOnce(&mut Ui<'_>)) {
        if !self.gui().is_open(id) {
            return;
        }
        let style = self.style().clone();
        let win = self.gui().size();
        let rect = crate::gui::frame::popup_rect(anchor, size, win);
        let prev_layer = self.gui().layer();
        self.gui().set_layer(LAYER_POPUP);
        let paint = self.gui().begin_anchored_popup_paint(anchor, rect);
        self.gui().register_popup(rect);
        self.gui()
            .paint_shadow(rect, Vec2::new(0.0, 6.0), 12.0, 0.35);
        self.gui().paint_rect(rect, style.palette.popup);
        self.gui()
            .paint_rect_outline(rect, 1.0, style.palette.line_hard);
        let inner = rect.shrink(3.0);
        let enabled = self.enabled();
        {
            let mut c = self.child(id.with("popup"), inner, Dir::Vertical);
            c.set_enabled(enabled);
            c.gui().push_clip(inner);
            add(&mut c);
            c.gui().pop_clip();
        }
        if let Some(paint) = paint {
            self.gui().finish_anchored_popup_paint(paint);
        }
        self.gui().set_layer(prev_layer);
    }
}
