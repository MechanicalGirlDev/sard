use super::{Dir, Id, Rect, Ui, Vec2};

impl Ui<'_> {
    /// A child `Ui` over `rect` with the given direction. Its used size is **not** fed
    /// back; use it for absolutely placed regions (dock panels, popups).
    pub fn child(&mut self, id: Id, rect: Rect, dir: Dir) -> Ui<'_> {
        let mut c = Ui::new(self.gui, id, rect);
        c.dir = dir;
        c.enabled = self.enabled;
        c
    }

    /// Lay out `add` horizontally starting at the cursor; the row's size is allocated
    /// from this `Ui` afterwards.
    pub fn horizontal<R>(&mut self, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        self.nested(Dir::Horizontal, add)
    }

    /// Lay out `add` vertically in a nested `Ui`; its used size is allocated afterwards.
    pub fn vertical<R>(&mut self, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        self.nested(Dir::Vertical, add)
    }

    fn nested<R>(&mut self, dir: Dir, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let id = self.next_widget_id("layout");
        let rect = self.available();
        let enabled = self.enabled;
        let gap = self.gap;
        let (result, used) = {
            let mut c = Ui::new(self.gui, id, rect);
            c.dir = dir;
            c.enabled = enabled;
            c.gap = gap;
            let r = add(&mut c);
            (r, c.used_size())
        };
        let _ = self.allocate(used);
        result
    }

    /// Run `add` inside a clip rect (content outside `rect` is not drawn).
    pub fn clipped<R>(&mut self, rect: Rect, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        self.gui.push_clip(rect);
        let r = add(self);
        self.gui.pop_clip();
        r
    }

    /// Whether this `Ui` is inside a clip (informational).
    pub fn is_clipped(&self) -> bool {
        self.clipped
    }

    pub(crate) fn set_clipped(&mut self, clipped: bool) {
        self.clipped = clipped;
    }

    /// Split the remaining region into `n` equal columns and run `add` for each.
    pub fn columns(&mut self, n: usize, mut add: impl FnMut(usize, &mut Ui<'_>)) {
        let n = n.max(1);
        let avail = self.available();
        let gap = self.gap;
        let col_w = ((avail.width() - gap * (n as f32 - 1.0)) / n as f32).max(0.0);
        let enabled = self.enabled;
        let mut max_h: f32 = 0.0;
        for i in 0..n {
            let x = avail.min.x + i as f32 * (col_w + gap);
            let rect = Rect::new(x, avail.min.y, col_w, avail.height());
            let id = self.id.with(("col", i));
            let mut c = Ui::new(self.gui, id, rect);
            c.enabled = enabled;
            c.gap = gap;
            add(i, &mut c);
            max_h = max_h.max(c.used_size().y);
        }
        let _ = self.allocate(Vec2::new(avail.width(), max_h));
    }
}
