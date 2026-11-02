use super::{Rect, ScrollState, Ui};
use crate::gui::Dir;

impl Ui<'_> {
    /// Draw only rows intersecting the viewport. Each row receives a stable namespace
    /// from `key`, not its current visible index or pixel position. `row_height` is
    /// the complete stride; no inter-row gap is added.
    ///
    /// While an ancestor awaits measured scroll correction, callbacks cover this
    /// list's own viewport once per row; stale ancestor clipping cannot safely cull
    /// paint that may become visible. Input retains inherited clipping. Supply a
    /// bounded height when nesting inside measured content: `None` inherits the
    /// available layout height, which can be unbounded in a measured body.
    pub fn virtual_rows<K: core::hash::Hash>(
        &mut self,
        id_source: impl core::hash::Hash,
        height: Option<f32>,
        state: &mut ScrollState,
        row_height: f32,
        rows: usize,
        mut key: impl FnMut(usize) -> K,
        mut add: impl FnMut(usize, &mut Ui<'_>),
    ) {
        let id = self.make_id(("scroll", id_source));
        let h = height.unwrap_or_else(|| self.available_height()).max(0.0);
        let area = self.allocate_row(h);
        // Invalid geometry has no rows and must not divide by zero or create a huge range.
        let stride = if row_height.is_finite() {
            row_height.max(0.0)
        } else {
            0.0
        };
        state.content_height = stride * rows as f32;
        let content = state.begin(self, id, area);
        if stride > 0.0 && !content.is_empty() {
            let visible = if self.gui().paint_is_deferred() {
                content
            } else {
                content.intersect(&self.gui().clip_rect())
            };
            let top = (state.offset + visible.min.y - content.min.y).max(0.0);
            let bottom = (state.offset + visible.max.y - content.min.y).max(0.0);
            let start = (top / stride).floor() as usize;
            let end = ((bottom / stride).ceil() as usize).min(rows);
            if !visible.is_empty() {
                for index in start.min(rows)..end {
                    let row = Rect::new(
                        content.min.x,
                        content.min.y + index as f32 * stride - state.offset,
                        content.width(),
                        stride,
                    );
                    let mut child = self.child(id.with(("row", key(index))), row, Dir::Vertical);
                    child.set_gap(0.0);
                    child.set_clipped(true);
                    child.clipped(row.intersect(&visible), |ui| add(index, ui));
                }
            }
        }
        state.paint_scrollbar(self, area);
    }
}
