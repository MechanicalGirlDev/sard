use super::{Gui, Rect, LAYER_POPUP};
use crate::gui::{
    primitive::{PrimitivePaintRange, PrimitivePaintScope},
    text::{TextPaintRange, TextPaintScope},
};
use glam::Vec2;

#[derive(Clone, Copy)]
enum ScopeKind {
    Body,
    Popup,
}

pub(super) struct PaintScopeFrame {
    pub(super) layer: usize,
    kind: ScopeKind,
}

pub(crate) struct PaintScope {
    layer: usize,
    first_popup: usize,
    primitives: PrimitivePaintScope,
    text: TextPaintScope,
}

pub(crate) struct PopupPaintScope {
    first_rect: usize,
    first_popup: usize,
    placement: PopupPlacement,
    input_clips: Vec<Rect>,
    paint: (PrimitivePaintScope, TextPaintScope),
}

pub(super) struct AnchoredPopup {
    rects: core::ops::Range<usize>,
    placement: PopupPlacement,
    paint: (PrimitivePaintRange, TextPaintRange),
}

enum PopupPlacement {
    Anchored { anchor: Rect, rect: Rect },
    Menu,
    Window,
}

impl Gui {
    pub(crate) fn paint_is_deferred(&self) -> bool {
        self.paint_scopes
            .iter()
            .any(|scope| scope.layer == self.layer)
    }

    pub(crate) fn begin_paint_scope(&mut self) -> PaintScope {
        self.paint_scopes.push(PaintScopeFrame {
            layer: self.layer,
            kind: ScopeKind::Body,
        });
        PaintScope {
            layer: self.layer,
            first_popup: self.anchored_popups.len(),
            primitives: self.layers[self.layer].begin_paint_scope(),
            text: self.text.begin_paint_scope(),
        }
    }

    pub(crate) fn end_paint_scope(&mut self, scope: PaintScope, offset: Vec2) {
        let pending = &self.anchored_popups[scope.first_popup..];
        let excluded_primitives: Vec<_> = pending
            .iter()
            .filter(|_| scope.layer == LAYER_POPUP)
            .map(|popup| &popup.paint.0)
            .collect();
        let excluded_text: Vec<_> = pending
            .iter()
            .filter(|_| scope.layer == LAYER_POPUP)
            .map(|popup| &popup.paint.1)
            .collect();
        self.layers[scope.layer].end_paint_scope(
            scope.primitives,
            offset.to_array(),
            &excluded_primitives,
        );
        self.text
            .end_paint_scope(scope.text, offset.to_array(), &excluded_text);
        let window = self.size;
        let clip = Some([0.0, 0.0, window.x, window.y]);
        for popup in &mut self.anchored_popups[scope.first_popup..] {
            let shift = match &mut popup.placement {
                PopupPlacement::Anchored { anchor, rect } => {
                    *anchor = anchor.translate(offset);
                    let corrected = popup_rect(*anchor, rect.size(), window);
                    let shift = corrected.min - rect.min;
                    *rect = corrected;
                    shift
                }
                PopupPlacement::Menu => offset,
                PopupPlacement::Window => Vec2::ZERO,
            };
            self.layers[LAYER_POPUP].translate_paint_range(&popup.paint.0, shift.to_array(), clip);
            self.text
                .translate_paint_range(&popup.paint.1, shift.to_array(), clip);
            for index in popup.rects.clone() {
                self.popups[index] = self.popups[index].translate(shift);
            }
        }
        let _ = self.paint_scopes.pop();
        if self.paint_scopes.is_empty() {
            self.anchored_popups.clear();
        }
    }

    /// Ui::popup is anchor-relative; context menus and modals do not use this path.
    pub(crate) fn begin_anchored_popup_paint(
        &mut self,
        anchor: Rect,
        rect: Rect,
    ) -> Option<PopupPaintScope> {
        self.begin_popup_paint(PopupPlacement::Anchored { anchor, rect })
    }

    pub(crate) fn begin_window_popup_paint(&mut self) -> Option<PopupPaintScope> {
        self.begin_popup_paint(PopupPlacement::Window)
    }

    pub(crate) fn begin_menu_popup_paint(&mut self) -> Option<PopupPaintScope> {
        self.begin_popup_paint(PopupPlacement::Menu)
    }

    fn begin_popup_paint(&mut self, placement: PopupPlacement) -> Option<PopupPaintScope> {
        let parent = self.paint_scopes.last()?;
        match parent.kind {
            ScopeKind::Popup => return None,
            ScopeKind::Body => {}
        }
        let paint = (
            self.layers[self.layer].begin_paint_scope(),
            self.text.begin_paint_scope(),
        );
        let input_clips = core::mem::take(&mut self.clip);
        self.paint_scopes.push(PaintScopeFrame {
            layer: self.layer,
            kind: ScopeKind::Popup,
        });
        Some(PopupPaintScope {
            first_rect: self.popups.len(),
            first_popup: self.anchored_popups.len(),
            placement,
            input_clips,
            paint,
        })
    }

    pub(crate) fn finish_anchored_popup_paint(&mut self, scope: PopupPaintScope) {
        let paint = (
            self.layers[self.layer].finish_paint_scope(scope.paint.0),
            self.text.finish_paint_scope(scope.paint.1),
        );
        // A completed outer popup now owns the nested geometry and registration range.
        self.anchored_popups.truncate(scope.first_popup);
        self.anchored_popups.push(AnchoredPopup {
            rects: scope.first_rect..self.popups.len(),
            placement: scope.placement,
            paint,
        });
        self.clip = scope.input_clips;
        let _ = self.paint_scopes.pop();
    }
}

pub(crate) fn popup_rect(anchor: Rect, size: Vec2, window: Vec2) -> Rect {
    let mut pos = Vec2::new(anchor.min.x, anchor.max.y + 1.0);
    if pos.y + size.y > window.y {
        pos.y = (anchor.min.y - size.y - 1.0).max(0.0);
    }
    if pos.x + size.x > window.x {
        pos.x = (window.x - size.x).max(0.0);
    }
    Rect::from_min_size(pos, size)
}
