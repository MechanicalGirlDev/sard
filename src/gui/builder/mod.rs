//! [`Ui`]: a cursor-based layout builder over a [`Gui`] context.
//!
//! A `Ui` owns a rectangle and lays widgets out along one direction; nested `Ui`s
//! (`horizontal`, `vertical`, `child`) allocate their used size from the parent when
//! they finish. Widgets live in `widgets.rs` / `containers.rs` as further `impl Ui`.

use glam::Vec2;
use std::collections::HashMap;

use super::frame::Gui;
use super::style::Style;
use super::types::{Align, Dir, Id, Rect, Response, Sense};

/// Layout builder for one region.
pub struct Ui<'g> {
    gui: &'g mut Gui,
    id: Id,
    /// The region this `Ui` may use.
    rect: Rect,
    /// Where the next widget goes.
    cursor: Vec2,
    dir: Dir,
    /// Cross-axis extent of the current row (horizontal) / column (vertical).
    line_cross: f32,
    /// Bounds of everything allocated so far.
    used: Option<Rect>,
    gap: f32,
    align: Align,
    enabled: bool,
    /// Fixed cross-axis size for the next widget (`None` = natural / fill).
    next_width: Option<f32>,
    next_height: Option<f32>,
    clipped: bool,
    next_id: Option<Id>,
    id_counts: HashMap<Id, u64>,
}

impl core::fmt::Debug for Ui<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ui")
            .field("id", &self.id)
            .field("rect", &self.rect)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl<'g> Ui<'g> {
    /// A vertical `Ui` over `rect`.
    pub fn new(gui: &'g mut Gui, id: Id, rect: Rect) -> Self {
        let gap = gui.style().spacing;
        Self {
            gui,
            id,
            rect,
            cursor: rect.min,
            dir: Dir::Vertical,
            line_cross: 0.0,
            used: None,
            gap,
            align: Align::Start,
            enabled: true,
            next_width: None,
            next_height: None,
            clipped: false,
            next_id: None,
            id_counts: HashMap::new(),
        }
    }

    /// The context.
    pub fn gui(&mut self) -> &mut Gui {
        self.gui
    }

    /// The style.
    pub fn style(&self) -> &Style {
        self.gui.style()
    }

    /// This `Ui`'s id (children derive from it).
    pub fn id(&self) -> Id {
        self.id
    }

    /// An id for a child widget.
    pub fn make_id(&self, source: impl core::hash::Hash) -> Id {
        self.id.with(source)
    }

    /// Give the next widget a stable identity independent of its label and call order.
    /// The identity is scoped to this `Ui` and consumed exactly once.
    pub fn set_next_id(&mut self, source: impl core::hash::Hash) {
        self.next_id = Some(self.make_id(source));
    }

    pub(crate) fn next_widget_id(&mut self, source: impl core::hash::Hash) -> Id {
        if let Some(id) = self.next_id.take() {
            return id;
        }
        let base = self.make_id(source);
        let count = self.id_counts.entry(base).or_default();
        let id = base.with(*count);
        *count = count.wrapping_add(1);
        id
    }

    /// The region.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Layout direction.
    pub fn dir(&self) -> Dir {
        self.dir
    }

    /// Where the next widget will be placed.
    pub fn cursor(&self) -> Vec2 {
        self.cursor
    }

    /// Move the cursor (e.g. to skip space).
    pub fn set_cursor(&mut self, cursor: Vec2) {
        self.cursor = cursor;
    }

    /// Gap between widgets.
    pub fn gap(&self) -> f32 {
        self.gap
    }

    /// Set the gap between widgets.
    pub fn set_gap(&mut self, gap: f32) {
        self.gap = gap;
    }

    /// Cross-axis alignment of widgets narrower than the line.
    pub fn set_align(&mut self, align: Align) {
        self.align = align;
    }

    /// Whether widgets react to input.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Enable / disable widgets built after this call.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Space left after the cursor along the layout direction, and the full cross size.
    pub fn available(&self) -> Rect {
        Rect::from_min_max(self.cursor, self.rect.max)
    }

    /// Available width (horizontal room from the cursor).
    pub fn available_width(&self) -> f32 {
        (self.rect.max.x - self.cursor.x).max(0.0)
    }

    /// Available height (vertical room from the cursor).
    pub fn available_height(&self) -> f32 {
        (self.rect.max.y - self.cursor.y).max(0.0)
    }

    /// Bounds of everything allocated so far.
    pub fn used_rect(&self) -> Rect {
        self.used
            .unwrap_or_else(|| Rect::from_min_size(self.rect.min, Vec2::ZERO))
    }

    /// Size of everything allocated so far.
    pub fn used_size(&self) -> Vec2 {
        self.used_rect().size()
    }

    /// Force the next widget's width.
    pub fn set_next_width(&mut self, width: f32) {
        self.next_width = Some(width);
    }

    /// Force the next widget's height.
    pub fn set_next_height(&mut self, height: f32) {
        self.next_height = Some(height);
    }

    /// Resolve a widget's natural size against `set_next_*` overrides.
    pub fn resolve_size(&mut self, natural: Vec2) -> Vec2 {
        let w = self.next_width.take().unwrap_or(natural.x);
        let h = self.next_height.take().unwrap_or(natural.y);
        Vec2::new(w, h)
    }

    /// Allocate `size` at the cursor and advance. Returns the rect (cross-axis aligned).
    pub fn allocate(&mut self, size: Vec2) -> Rect {
        let size = size.max(Vec2::ZERO);
        let rect = match self.dir {
            Dir::Vertical => {
                let x = match self.align {
                    Align::Start => self.cursor.x,
                    Align::Center => self.cursor.x + (self.available_width() - size.x) * 0.5,
                    Align::End => self.rect.max.x - size.x,
                };
                let r = Rect::from_min_size(Vec2::new(x, self.cursor.y), size);
                self.cursor.y += size.y + self.gap;
                self.line_cross = self.line_cross.max(size.x);
                r
            }
            Dir::Horizontal => {
                let y = match self.align {
                    Align::Start => self.cursor.y,
                    Align::Center => self.cursor.y + (self.line_height_hint() - size.y) * 0.5,
                    Align::End => self.rect.max.y - size.y,
                };
                let r = Rect::from_min_size(Vec2::new(self.cursor.x, y), size);
                self.cursor.x += size.x + self.gap;
                self.line_cross = self.line_cross.max(size.y);
                r
            }
        };
        self.used = Some(match self.used {
            Some(u) => Rect::from_min_max(u.min.min(rect.min), u.max.max(rect.max)),
            None => rect,
        });
        rect
    }

    /// Cross size used for vertical centring inside a horizontal row: the row's height
    /// so far, or the style row height when nothing has been placed yet.
    fn line_height_hint(&self) -> f32 {
        if self.line_cross > 0.0 {
            self.line_cross
        } else {
            self.gui.style().row_height
        }
    }

    /// Allocate the full available width by `height` (vertical layouts).
    pub fn allocate_row(&mut self, height: f32) -> Rect {
        let w = self
            .next_width
            .take()
            .unwrap_or_else(|| self.available_width());
        let h = self.next_height.take().unwrap_or(height);
        self.allocate(Vec2::new(w, h))
    }

    /// Allocate a rect and register it for interaction.
    pub fn allocate_response(&mut self, size: Vec2, sense: Sense, id: Id) -> Response {
        let rect = self.allocate(size);
        self.interact(id, rect, sense)
    }

    /// Register interaction on an arbitrary rect (respects `enabled`).
    pub fn interact(&mut self, id: Id, rect: Rect, sense: Sense) -> Response {
        if self.enabled {
            self.gui.interact(id, rect, sense)
        } else {
            Response::none(id, rect)
        }
    }

    /// Add empty space along the layout direction.
    pub fn add_space(&mut self, amount: f32) {
        match self.dir {
            Dir::Vertical => self.cursor.y += amount,
            Dir::Horizontal => self.cursor.x += amount,
        }
    }

    /// Start a new line (horizontal layouts): move below the tallest widget of the row.
    pub fn end_row(&mut self) {
        if self.dir == Dir::Horizontal {
            self.cursor.x = self.rect.min.x;
            self.cursor.y += self.line_cross + self.gap;
            self.line_cross = 0.0;
        }
    }
}

mod layout;
