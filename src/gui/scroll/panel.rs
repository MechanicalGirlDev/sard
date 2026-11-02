use super::{ScrollRegion, ScrollRegions, ScrollState, Ui};
use crate::gui::Dir;

impl Ui<'_> {
    /// Fill the remaining region with a fixed header, scrollable body, and fixed footer.
    /// Heights are clamped to the panel; zero-height fixed regions are not invoked.
    ///
    /// Measured append-follow corrects the body painting after calling `add` once.
    /// Responses inside `add` describe the input layout before that paint correction;
    /// the corrected offset is used for subsequent frames. Application code
    /// must not treat a copied response rectangle as a post-correction paint bound.
    /// Anchor-relative `Ui::popup` painting and hit bounds follow the correction;
    /// mouse-anchored context menus retain their window coordinates.
    pub fn scroll_panel(
        &mut self,
        id_source: impl core::hash::Hash,
        header_height: f32,
        footer_height: f32,
        state: &mut ScrollState,
        mut add: impl FnMut(ScrollRegion, &mut Ui<'_>),
    ) -> ScrollRegions {
        let id = self.make_id(("scroll", id_source));
        let area = self.allocate_row(self.available_height());
        let (header, rest) = area.split_top(header_height);
        let (body, footer) = rest.split_bottom(footer_height);
        for (region, rect) in [
            (ScrollRegion::Header, header),
            (ScrollRegion::Body, body),
            (ScrollRegion::Footer, footer),
        ] {
            if rect.is_empty() {
                continue;
            }
            let suffix = match region {
                ScrollRegion::Header => "header",
                ScrollRegion::Footer => "footer",
                ScrollRegion::Body => {
                    self.scroll_content(id, body, state, |ui| add(ScrollRegion::Body, ui));
                    continue;
                }
            };
            let mut child = self.child(id.with(suffix), rect, Dir::Vertical);
            child.clipped(rect, |ui| add(region, ui));
        }
        if body.is_empty() {
            state.offset = 0.0;
        }
        ScrollRegions {
            header,
            body,
            footer,
        }
    }
}
