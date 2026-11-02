//! Scroll state and viewport mechanics shared by measured and virtual content.

use glam::Vec2;

use super::{Id, Rect, Sense, Ui};
use crate::window::event::MouseButton;

/// Persistent vertical scroll position. Following the end pauses when the user
/// scrolls upward and resumes when they return to the bottom.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub(super) offset: f32,
    pub(super) content_height: f32,
    follow_end: bool,
}

impl ScrollState {
    /// Start pinned to the end, suitable for append-only logs.
    pub fn following_end() -> Self {
        Self {
            follow_end: true,
            ..Self::default()
        }
    }

    /// Current scroll offset in logical pixels.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Set an explicit position and stop following appended content.
    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset.max(0.0);
        self.follow_end = false;
    }

    /// Whether appended content will remain visible.
    pub fn is_following_end(&self) -> bool {
        self.follow_end
    }

    pub(super) fn begin(&mut self, ui: &mut Ui<'_>, id: Id, area: Rect) -> Rect {
        let max = (self.content_height - area.height()).max(0.0);
        self.offset = if self.follow_end {
            max
        } else {
            self.offset.clamp(0.0, max)
        };
        let hovered = ui.interact(id, area, Sense::HOVER).hovered;
        if hovered {
            let wheel = ui.gui().scroll_delta().y;
            if wheel.abs() > 0.0 {
                self.offset = (self.offset - wheel).clamp(0.0, max);
                self.follow_end = wheel < 0.0 && self.offset >= max;
            }
        }
        let width = ui.style().scrollbar.min(area.width()).max(0.0);
        let (content, track) = area.split_right(width);
        if max > 0.0 && !track.is_empty() {
            let thumb = self.thumb(track);
            let response = ui.interact(id.with("thumb"), thumb, Sense::DRAG);
            if response.pressed {
                ui.gui().set_scalar(id.with("drag_offset"), self.offset);
            }
            if response.dragged || response.drag_released {
                if let Some(origin) = ui.gui().mouse_press_origin(MouseButton::Left) {
                    let travel = (track.height() - thumb.height()).max(1.0);
                    let start = ui.gui().scalar(id.with("drag_offset"));
                    self.offset =
                        (start + (ui.gui().mouse().y - origin.y) * max / travel).clamp(0.0, max);
                    self.follow_end = self.offset >= max;
                }
            }
            let response = ui.interact(id.with("track"), track, Sense::CLICK);
            if response.pressed {
                if let Some(origin) = ui.gui().mouse_press_origin(MouseButton::Left) {
                    let page = if origin.y < thumb.min.y {
                        -area.height()
                    } else {
                        area.height()
                    };
                    self.offset = (self.offset + page).clamp(0.0, max);
                    self.follow_end = self.offset >= max;
                }
            }
        }
        content
    }

    pub(super) fn measure(&mut self, content_height: f32, viewport_height: f32) -> f32 {
        let previous = self.offset;
        self.content_height = content_height.max(0.0);
        let max = (self.content_height - viewport_height).max(0.0);
        self.offset = if self.follow_end {
            max
        } else {
            self.offset.clamp(0.0, max)
        };
        previous - self.offset
    }

    pub(super) fn paint_scrollbar(&self, ui: &mut Ui<'_>, area: Rect) {
        let max = (self.content_height - area.height()).max(0.0);
        let width = ui.style().scrollbar.min(area.width()).max(0.0);
        let (_, track) = area.split_right(width);
        if max > 0.0 && !track.is_empty() {
            let color = ui.style().palette.text_faint;
            ui.gui()
                .paint_rect(self.thumb(track).shrink2(Vec2::new(2.0, 0.0)), color);
        }
    }

    fn thumb(&self, track: Rect) -> Rect {
        let h = (track.height() * track.height() / self.content_height)
            .max(16.0)
            .min(track.height());
        let max = (self.content_height - track.height()).max(0.0);
        let y = track.min.y + (self.offset / max) * (track.height() - h);
        Rect::new(track.min.x, y, track.width(), h)
    }
}

/// Region supplied to a fixed-header/footer scroll panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollRegion {
    /// Fixed content above the scroller.
    Header,
    /// Scrollable content.
    Body,
    /// Fixed controls below the scroller.
    Footer,
}

/// Actual bounds of a scroll panel, after clamping to available space.
#[derive(Debug, Clone, Copy)]
pub struct ScrollRegions {
    /// Fixed header.
    pub header: Rect,
    /// Scroll viewport.
    pub body: Rect,
    /// Fixed footer.
    pub footer: Rect,
}

mod panel;
mod virtual_rows;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod measured_tests;

#[cfg(test)]
mod input_tests;

#[cfg(test)]
mod popup_tests;

#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod nested_tests;
