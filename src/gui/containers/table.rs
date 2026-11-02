use super::{Dir, Id, Rect, Response, Sense, Ui, Vec2};

impl Ui<'_> {
    /// A full-width selectable row. Selection spans every cell and the whitespace
    /// between them; cells keep their own clipped layout and widget identities.
    pub fn selectable_table_row(
        &mut self,
        id: Id,
        weights: &[f32],
        selected: bool,
        mut cell: impl FnMut(usize, &mut Ui<'_>),
    ) -> Response {
        let style = self.style().clone();
        let row = self.allocate_row(style.row_height);
        let background = self.gui().reserve_rect();
        let total = weights
            .iter()
            .map(|w| w.max(0.0))
            .sum::<f32>()
            .max(f32::EPSILON);
        let mut x = row.min.x;
        for (column, weight) in weights.iter().enumerate() {
            let width = row.width() * weight.max(0.0) / total;
            let rect = Rect::new(x, row.min.y, width, row.height()).shrink2(Vec2::new(4.0, 0.0));
            let mut child = self.child(id.with(("cell", column)), rect, Dir::Horizontal);
            child.set_align(crate::gui::Align::Center);
            child.clipped(rect, |ui| cell(column, ui));
            x += width;
        }
        // Children get first refusal so a button inside a cell does not select its row.
        let response = self.interact(id, row, Sense::CLICK);
        let color = if selected {
            style.palette.accent_bg
        } else if response.hovered {
            style.palette.row_hover
        } else {
            [0.0; 4]
        };
        self.gui().fill_reserved(background, row, color);
        self.gui().paint_rect(
            Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0),
            style.palette.line_grid,
        );
        response
    }

    /// A table: `weights` are relative column widths, `header` the column titles,
    /// `rows` the row count, and `cell` draws one cell. Rows are separated by grid rules
    /// and highlight on hover.
    pub fn table(
        &mut self,
        weights: &[f32],
        header: &[&str],
        rows: usize,
        mut cell: impl FnMut(usize, usize, &mut Ui<'_>),
    ) {
        let style = self.style().clone();
        let total = weights.iter().sum::<f32>().max(1e-6);
        let width = self.available_width();
        let widths: Vec<f32> = weights.iter().map(|w| width * w / total).collect();
        let row_h = style.row_height;
        let xs: Vec<f32> = widths
            .iter()
            .scan(self.cursor().x, |x, w| {
                let start = *x;
                *x += w;
                Some(start)
            })
            .collect();
        let p = style.palette;
        let gap = self.gap();
        self.set_gap(0.0);
        if !header.is_empty() {
            let row = self.allocate_row(row_h);
            self.gui().paint_rect(
                Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0),
                p.line,
            );
            for (i, h) in header.iter().enumerate() {
                let x = xs.get(i).copied().unwrap_or(row.min.x);
                let measured = self.gui().measure(h, style.font_size_small);
                self.gui().paint_text(
                    h,
                    Vec2::new(x + 4.0, row.center().y - measured.y * 0.5),
                    style.font_size_small,
                    p.text_faint,
                );
            }
        }
        for r in 0..rows {
            let row = self.allocate_row(row_h);
            let hovered = self
                .interact(self.make_id(("row", r)), row, Sense::HOVER)
                .hovered;
            if hovered {
                self.gui().paint_rect(row, p.row_hover);
            }
            self.gui().paint_rect(
                Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0),
                p.line_grid,
            );
            for (c, w) in widths.iter().enumerate() {
                let rect = Rect::new(xs[c] + 4.0, row.min.y, (w - 8.0).max(0.0), row.height());
                let id = self.make_id(("cell", r, c));
                let mut cu = self.child(id, rect, Dir::Horizontal);
                cu.set_gap(4.0);
                cu.set_align(crate::gui::Align::Center);
                cu.gui().push_clip(rect);
                cell(r, c, &mut cu);
                cu.gui().pop_clip();
            }
        }
        self.set_gap(gap);
    }
}
