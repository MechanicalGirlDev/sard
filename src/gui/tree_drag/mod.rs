//! Tree dragging keeps application keys separate from widget capture identities.

use glam::Vec2;

use super::{Gui, Id, Rect, Response, Ui, LAYER_POPUP};
use crate::window::event::{Cursor, Key, MouseButton};

/// Placement relative to a target row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeDropPosition {
    /// Insert before the target.
    Before,
    /// Make the source a child of the target.
    Inside,
    /// Insert after the target.
    After,
}

impl TreeDropPosition {
    /// Representative row fraction for the top quarter, middle half, or bottom quarter.
    pub const fn fraction(self) -> f32 {
        match self {
            Self::Before => 0.0,
            Self::Inside => 0.5,
            Self::After => 1.0,
        }
    }
}

/// An accepted drop, emitted once on release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeDrop<K> {
    /// Application key being moved.
    pub source: K,
    /// Destination application key.
    pub target: K,
    /// Accepted placement.
    pub position: TreeDropPosition,
}

#[derive(Debug)]
struct Source<K> {
    key: K,
    widget: Id,
    origin: Vec2,
    layer: usize,
}

#[derive(Debug)]
struct Row<K> {
    key: K,
    widget: Id,
    rect: Rect,
    visible: Rect,
}

/// Persistent drag state for one tree. Call `begin_frame`, `row` for each visible node,
/// and `finish` after all rows, so target resolution is independent of traversal order.
#[derive(Debug)]
pub struct TreeDrag<K> {
    source: Option<Source<K>>,
    rows: Vec<Row<K>>,
    dragging: bool,
    enabled: bool,
}

impl<K> Default for TreeDrag<K> {
    fn default() -> Self {
        Self {
            source: None,
            rows: Vec::new(),
            dragging: false,
            enabled: false,
        }
    }
}

impl<K: Clone + Eq> TreeDrag<K> {
    /// Begin collecting this frame's rows. Escape, disabled editing, an overlay, or a
    /// lost capture cancels the operation without delivering a drop.
    pub fn begin_frame(&mut self, gui: &mut Gui, enabled: bool) {
        self.rows.clear();
        self.enabled = enabled;
        let cancel = !enabled
            || gui.key_pressed(Key::Escape)
            || self.source.as_ref().is_some_and(|source| {
                !gui.is_active(source.widget)
                    || (source.layer != LAYER_POPUP
                        && (gui.modal().is_some() || gui.any_popup_open()))
            })
            || (!gui.mouse_down(MouseButton::Left) && !gui.mouse_released(MouseButton::Left));
        if cancel {
            if let Some(source) = self.source.take() {
                if gui.is_active(source.widget) {
                    gui.clear_active();
                }
            }
            self.dragging = false;
        }
        if let Some(source) = &self.source {
            self.dragging |= (gui.mouse() - source.origin).length() >= 4.0;
        }
    }

    /// Register a row's existing response. A non-draggable row remains a drop target.
    pub fn row(
        &mut self,
        ui: &mut Ui<'_>,
        key: K,
        response: &Response,
        draggable: bool,
    ) -> Response {
        if !self.enabled || !ui.enabled() {
            return *response;
        }
        if response.pressed && draggable {
            if let Some(origin) = ui.gui().mouse_press_origin(MouseButton::Left) {
                self.source = Some(Source {
                    key: key.clone(),
                    widget: response.id,
                    origin,
                    layer: ui.gui().layer(),
                });
                self.dragging = (ui.gui().mouse() - origin).length() >= 4.0;
            }
        }
        self.rows.push(Row {
            key,
            widget: response.id,
            rect: response.rect,
            visible: response.rect.intersect(&ui.gui().clip_rect()),
        });
        *response
    }

    /// Whether the press has crossed the drag threshold (including its release frame).
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Resolve and paint the drop target. `accept` rejects invalid topology with `None`
    /// or changes the advertised placement. Only an accepted release returns a drop.
    pub fn finish(
        &mut self,
        ui: &mut Ui<'_>,
        mut accept: impl FnMut(&K, &K, TreeDropPosition) -> Option<TreeDropPosition>,
    ) -> Option<TreeDrop<K>> {
        let source = self.source.as_ref()?;
        let released = ui.gui().mouse_released(MouseButton::Left);
        let source_visible = self.rows.iter().any(|row| row.widget == source.widget);
        let target = if self.dragging && source_visible {
            let mouse = ui.gui().mouse();
            ui.gui().set_cursor(Cursor::Grabbing);
            self.rows
                .iter()
                .rev()
                .find(|row| row.key != source.key && row.visible.contains(mouse))
                .and_then(|row| {
                    let fraction = (mouse.y - row.rect.min.y) / row.rect.height();
                    let position = if fraction < 0.25 {
                        TreeDropPosition::Before
                    } else if fraction > 0.75 {
                        TreeDropPosition::After
                    } else {
                        TreeDropPosition::Inside
                    };
                    accept(&source.key, &row.key, position).map(|position| (row, position))
                })
        } else {
            None
        };
        let completed = target.and_then(|(row, position)| {
            let palette = ui.style().palette;
            ui.gui().push_clip(row.visible);
            match position {
                TreeDropPosition::Inside => ui.gui().paint_rect(row.rect, palette.accent_bg),
                TreeDropPosition::Before | TreeDropPosition::After => {
                    let y = row.rect.min.y + (row.rect.height() - 1.0) * position.fraction();
                    ui.gui().paint_line(
                        Vec2::new(row.rect.min.x, y),
                        Vec2::new(row.rect.max.x, y),
                        1.0,
                        palette.text,
                    );
                }
            }
            ui.gui().pop_clip();
            released.then(|| TreeDrop {
                source: source.key.clone(),
                target: row.key.clone(),
                position,
            })
        });
        if released || !source_visible {
            if ui.gui().is_active(source.widget) {
                ui.gui().clear_active();
            }
            self.source = None;
            self.dragging = false;
        }
        completed
    }
}

#[cfg(test)]
mod tests;
