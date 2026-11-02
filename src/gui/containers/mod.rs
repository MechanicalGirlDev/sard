//! Container widgets on [`Ui`]: scroll areas, collapsing headers, frames, trees, tables,
//! modals.

use glam::Vec2;

use crate::window::event::Cursor;

use super::builder::Ui;
use super::frame::{Gui, LAYER_POPUP};
use super::style::Color;
use super::text::TextStyle;
use super::types::{Dir, Id, Rect, Response, Sense};

/// Result of a tree node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeResponse {
    /// Interaction on the node's row (click selects).
    pub response: Response,
    /// Whether the node is expanded.
    pub open: bool,
}

mod modal;
mod scroll;
mod sections;
mod table;
mod tree;
pub use modal::modal;

/// A small right- or down-pointing chevron centred at `c` (11 px box, 1.5 px stroke).
pub fn paint_chevron(gui: &mut Gui, c: Vec2, open: bool, color: Color) {
    if open {
        gui.paint_line(
            c + Vec2::new(-3.5, -1.5),
            c + Vec2::new(0.0, 2.0),
            1.5,
            color,
        );
        gui.paint_line(
            c + Vec2::new(0.0, 2.0),
            c + Vec2::new(3.5, -1.5),
            1.5,
            color,
        );
    } else {
        gui.paint_line(
            c + Vec2::new(-1.5, -3.5),
            c + Vec2::new(2.0, 0.0),
            1.5,
            color,
        );
        gui.paint_line(
            c + Vec2::new(2.0, 0.0),
            c + Vec2::new(-1.5, 3.5),
            1.5,
            color,
        );
    }
}
