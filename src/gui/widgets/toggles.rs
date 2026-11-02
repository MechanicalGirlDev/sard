use super::{Cursor, Rect, Response, Sense, Ui, Vec2};

impl Ui<'_> {
    // ----- toggles -----------------------------------------------------------------

    /// A checkbox: an 11 px sunken box, accent-filled with a dark check when on.
    pub fn checkbox(&mut self, value: &mut bool, text: &str) -> Response {
        let style = self.style().clone();
        let box_size = (style.font_size * 0.96).round();
        let measured = self.gui().measure(text, style.font_size);
        let gap = if text.is_empty() { 0.0 } else { 4.0 };
        let natural = Vec2::new(box_size + gap + measured.x, style.row_height);
        let size = self.resolve_size(natural);
        let id = self.next_widget_id(("check", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let enabled = self.enabled();
        if r.clicked {
            *value = !*value;
        }
        let p = &style.palette;
        let bx = Rect::from_min_size(
            Vec2::new(r.rect.min.x, (r.rect.center().y - box_size * 0.5).round()),
            Vec2::splat(box_size),
        );
        if !enabled {
            self.gui().paint_rect(bx, p.background);
            self.gui().paint_rect_outline(bx, 1.0, p.line);
        } else if *value {
            self.gui().paint_rect(bx, p.accent);
            // Check mark: two strokes.
            let c = bx.center();
            let k = box_size / 11.0;
            self.gui().paint_line(
                c + Vec2::new(-3.2 * k, 0.2 * k),
                c + Vec2::new(-k, 2.4 * k),
                1.6 * k,
                p.text_on_accent,
            );
            self.gui().paint_line(
                c + Vec2::new(-k, 2.4 * k),
                c + Vec2::new(3.4 * k, -2.4 * k),
                1.6 * k,
                p.text_on_accent,
            );
        } else {
            self.gui().paint_rect(bx, p.sunken);
            self.gui().paint_rect_outline(bx, 1.0, p.line_hard);
        }
        if !text.is_empty() {
            let pos = Vec2::new(bx.max.x + gap, r.rect.center().y - measured.y * 0.5);
            let fg = if enabled { p.text_dim } else { p.text_disabled };
            self.gui().paint_text(text, pos, style.font_size, fg);
        }
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    /// A radio button; `clicked` means "select me".
    pub fn radio(&mut self, selected: bool, text: &str) -> Response {
        let style = self.style().clone();
        let d = (style.font_size * 0.96).round();
        let measured = self.gui().measure(text, style.font_size);
        let natural = Vec2::new(d + 4.0 + measured.x, style.row_height);
        let size = self.resolve_size(natural);
        let id = self.next_widget_id(("radio", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let c = Vec2::new(r.rect.min.x + d * 0.5, r.rect.center().y);
        self.gui().paint_circle(c, d * 0.5, p.line_hard);
        self.gui().paint_circle(c, d * 0.5 - 1.0, p.sunken);
        if selected {
            self.gui().paint_circle(c, d * 0.28, p.accent);
        }
        let pos = Vec2::new(r.rect.min.x + d + 4.0, c.y - measured.y * 0.5);
        let fg = if self.enabled() {
            p.text_dim
        } else {
            p.text_disabled
        };
        self.gui().paint_text(text, pos, style.font_size, fg);
        if r.hovered && self.enabled() {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }
}
