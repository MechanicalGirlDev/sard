use super::{paint_chevron, Color, Cursor, Dir, Rect, Sense, TextStyle, Ui, Vec2};

impl Ui<'_> {
    /// A collapsible section with a heading row (chevron + small-caps title over a
    /// rule). Returns whether it is open.
    pub fn collapsing(
        &mut self,
        text: &str,
        default_open: bool,
        add: impl FnOnce(&mut Ui<'_>),
    ) -> bool {
        let style = self.style().clone();
        let id = self.next_widget_id(("collapsing", text));
        let stored = self.gui().scalar(id);
        let mut open = if stored == 0.0 {
            default_open
        } else {
            stored > 0.0
        };
        let row = self.allocate_row(style.row_height);
        let r = self.interact(id, row, Sense::CLICK);
        if r.clicked {
            open = !open;
        }
        self.gui().set_scalar(id, if open { 1.0 } else { -1.0 });
        let p = style.palette;
        if r.hovered {
            self.gui().paint_rect(row, p.row_hover);
            self.gui().set_cursor(Cursor::Pointer);
        }
        let rule = Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0);
        self.gui().paint_rect(rule, p.line);
        paint_chevron(
            self.gui(),
            Vec2::new(row.min.x + 6.0, row.center().y - 0.5),
            open,
            p.text_faint,
        );
        let ts = TextStyle::SEMIBOLD.spaced(0.06);
        let measured = self.gui().measure_styled(text, style.font_size_heading, ts);
        self.gui().paint_text_styled(
            text,
            Vec2::new(row.min.x + 15.0, row.center().y - 0.5 - measured.y * 0.5),
            style.font_size_heading,
            p.text_faint,
            ts,
        );
        self.add_space(3.0);
        if open {
            add(self);
        }
        open
    }

    /// Lay out `add` shifted right by `indent`.
    pub fn indented(&mut self, indent: f32, add: impl FnOnce(&mut Ui<'_>)) {
        let avail = self.available();
        let rect = Rect::from_min_max(avail.min + Vec2::new(indent, 0.0), avail.max);
        let id = self.next_widget_id("indent");
        let used = {
            let mut c = self.child(id, rect, Dir::Vertical);
            add(&mut c);
            c.used_size()
        };
        let _ = self.allocate(Vec2::new(used.x + indent, used.y));
    }

    /// A sunken panel with padding around `add`.
    pub fn frame<R>(&mut self, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let bg = self.style().palette.sunken;
        self.frame_colored(bg, add)
    }

    /// A background of `color` with padding around `add`.
    pub fn frame_colored<R>(&mut self, color: Color, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let pad = self.style().padding;
        let handle = self.gui().reserve_rect();
        let avail = self.available();
        let inner = avail.shrink(pad);
        let id = self.next_widget_id("frame");
        let dir = self.dir();
        let (result, used) = {
            let mut c = self.child(id, inner, dir);
            let r = add(&mut c);
            (r, c.used_rect())
        };
        let outer = Rect::from_min_max(used.min - Vec2::splat(pad), used.max + Vec2::splat(pad));
        let outer = if dir == Dir::Vertical {
            Rect::new(avail.min.x, outer.min.y, avail.width(), outer.height())
        } else {
            outer
        };
        self.gui().fill_reserved(handle, outer, color);
        let _ = self.allocate(outer.size());
        result
    }
}
