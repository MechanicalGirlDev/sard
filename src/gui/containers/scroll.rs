use super::{Dir, Id, Rect, Ui, Vec2};
use crate::gui::ScrollState;

impl Ui<'_> {
    /// A measured scroll region whose offset persists in the GUI context.
    /// Returned widget rectangles use the input layout, as described by [`Self::scroll_panel`].
    pub fn scroll_area<R>(
        &mut self,
        id_source: impl core::hash::Hash,
        height: Option<f32>,
        add: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> R {
        let id = self.make_id(("scroll", id_source));
        let mut state = ScrollState::default();
        state.offset = self.gui().scalar(id);
        state.content_height = self.gui().scalar(id.with("content_h"));
        let h = height.unwrap_or_else(|| self.available_height()).max(0.0);
        let area = self.allocate_row(h);
        let result = self.scroll_content(id, area, &mut state, add);
        self.gui().set_scalar(id, state.offset);
        self.gui()
            .set_scalar(id.with("content_h"), state.content_height);
        result
    }

    pub(crate) fn scroll_content<R>(
        &mut self,
        id: Id,
        area: Rect,
        state: &mut ScrollState,
        add: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> R {
        let content = state.begin(self, id, area);
        let inner = Rect::new(
            content.min.x,
            content.min.y - state.offset,
            content.width(),
            f32::MAX / 4.0,
        );
        let result = {
            let mut child = self.child(id.with("content"), inner, Dir::Vertical);
            child.set_clipped(true);
            child.gui().push_clip(content);
            let paint = state
                .is_following_end()
                .then(|| child.gui().begin_paint_scope());
            let result = add(&mut child);
            let height = child.used_rect().max.y - inner.min.y;
            let correction = state.measure(height, area.height());
            if let Some(paint) = paint {
                child
                    .gui()
                    .end_paint_scope(paint, Vec2::new(0.0, correction));
            }
            child.gui().pop_clip();
            result
        };
        state.paint_scrollbar(self, area);
        result
    }
}
