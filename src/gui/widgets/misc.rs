use super::{with_alpha, Color, Dir, Rect, Response, Sense, Ui, Vec2};

impl Ui<'_> {
    // ----- misc ----------------------------------------------------------------------

    /// A thin separator: a full-width rule in a vertical layout, a short vertical rule
    /// with side margins in a horizontal one.
    pub fn separator(&mut self) {
        let style = self.style().clone();
        match self.dir() {
            Dir::Vertical => {
                let rect = self.allocate(Vec2::new(self.available_width(), 1.0));
                self.gui().paint_rect(rect, style.palette.line);
            }
            Dir::Horizontal => {
                let h = style.control_height;
                let rect = self.allocate(Vec2::new(9.0, h));
                let line = Rect::new(rect.min.x + 4.0, rect.center().y - 8.0, 1.0, 16.0);
                self.gui().paint_rect(line, style.palette.line);
            }
        }
    }

    /// A status-bar cell: small text (dim by default) with an optional 7 px light in
    /// front and a hard rule on its right. Fills the row's height.
    pub fn cell(&mut self, text: &str, color: Option<Color>, led: Option<Color>) -> Response {
        let style = self.style().clone();
        let p = style.palette;
        let font = style.font_size_small;
        let m = self.gui().measure(text, font);
        let led_w = if led.is_some() { 7.0 + 5.0 } else { 0.0 };
        let h = self.rect().height().max(style.row_height);
        let size = self.resolve_size(Vec2::new(m.x + led_w + 16.0 + 1.0, h));
        let id = self.next_widget_id(("cell", text));
        let r = self.allocate_response(size, Sense::HOVER, id);
        let mut x = r.rect.min.x + 8.0;
        if let Some(c) = led {
            let bx = Rect::from_min_size(
                Vec2::new(x, (r.rect.center().y - 3.5).round()),
                Vec2::splat(7.0),
            );
            self.gui().paint_rect(bx, c);
            self.gui()
                .paint_rect_outline(bx, 1.0, with_alpha([0.0, 0.0, 0.0, 1.0], 0.35));
            x += led_w;
        }
        self.gui().paint_text(
            text,
            Vec2::new(x, r.rect.center().y - m.y * 0.5),
            font,
            color.unwrap_or(p.text_dim),
        );
        self.gui().paint_rect(
            Rect::new(r.rect.max.x - 1.0, r.rect.min.y, 1.0, r.rect.height()),
            p.line_hard,
        );
        r
    }

    /// A small square status light (`.led`), 7 px, in `color`.
    pub fn led(&mut self, color: Color) -> Response {
        let style = self.style().clone();
        let size = self.resolve_size(Vec2::new(7.0, style.row_height));
        let id = self.next_widget_id("led");
        let r = self.allocate_response(size, Sense::HOVER, id);
        let bx = Rect::from_min_size(
            Vec2::new(r.rect.min.x, (r.rect.center().y - 3.5).round()),
            Vec2::splat(7.0),
        );
        self.gui().paint_rect(bx, color);
        self.gui()
            .paint_rect_outline(bx, 1.0, with_alpha([0.0, 0.0, 0.0, 1.0], 0.35));
        r
    }

    /// Show `text` as a tooltip when `response`'s widget is hovered long enough.
    pub fn tooltip(&mut self, response: &Response, text: &str) {
        self.gui().tooltip(response.id, text);
    }
}
