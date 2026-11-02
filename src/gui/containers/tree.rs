use super::{Cursor, Rect, Sense, TreeResponse, Ui, Vec2};

impl Ui<'_> {
    /// A tree node: a caret column, a selectable label row (accent tint + 1 px accent
    /// outline when selected), children indented below when open. Open by default.
    pub fn tree_node(
        &mut self,
        id_source: impl core::hash::Hash,
        text: &str,
        selected: bool,
        leaf: bool,
        add: impl FnOnce(&mut Ui<'_>),
    ) -> TreeResponse {
        self.tree_node_with(id_source, text, selected, leaf, true, add)
    }

    /// [`Self::tree_node`] with an explicit initial state (a deep tree usually opens only
    /// its root).
    pub fn tree_node_with(
        &mut self,
        id_source: impl core::hash::Hash,
        text: &str,
        selected: bool,
        leaf: bool,
        default_open: bool,
        add: impl FnOnce(&mut Ui<'_>),
    ) -> TreeResponse {
        let style = self.style().clone();
        let id = self.make_id(("tree", id_source));
        let stored = self.gui().scalar(id);
        let mut open = !leaf
            && if stored == 0.0 {
                default_open
            } else {
                stored > 0.0
            };
        let row = self.allocate_row(style.row_height);
        let caret_w = 14.0;
        let chevron = Rect::new(row.min.x, row.min.y, caret_w, row.height());
        let label_rect = Rect::from_min_max(Vec2::new(row.min.x + caret_w, row.min.y), row.max);
        let c = self.interact(id.with("chevron"), chevron, Sense::CLICK);
        let r = self.interact(id, label_rect, Sense::CLICK);
        if !leaf && (c.clicked || r.double_clicked) {
            open = !open;
        }
        self.gui().set_scalar(id, if open { 1.0 } else { -1.0 });
        let p = style.palette;
        if selected {
            self.gui().paint_rect(row, p.accent_bg);
            self.gui().paint_rect_outline(row, 1.0, p.accent);
        } else if r.hovered || c.hovered {
            self.gui().paint_rect(row, p.row_hover);
        }
        if !leaf {
            let glyph = if open { "▾" } else { "▸" };
            let m = self.gui().measure(glyph, 9.0);
            self.gui().paint_text(
                glyph,
                Vec2::new(
                    chevron.min.x + (caret_w - m.x) * 0.5,
                    chevron.center().y - m.y * 0.5,
                ),
                9.0,
                p.text_faint,
            );
            if c.hovered {
                self.gui().set_cursor(Cursor::Pointer);
            }
        }
        let measured = self.gui().measure(text, style.font_size);
        let fg = if self.enabled() {
            p.text
        } else {
            p.text_disabled
        };
        self.gui().push_clip(label_rect);
        self.gui().paint_text(
            text,
            Vec2::new(label_rect.min.x, row.center().y - measured.y * 0.5),
            style.font_size,
            fg,
        );
        self.gui().pop_clip();
        if open {
            self.set_next_id(id.with("children"));
            self.indented(style.indent, add);
        }
        TreeResponse { response: r, open }
    }
}
