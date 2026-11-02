//! Leaf widgets on [`Ui`].
//!
//! The look follows an instrument-panel convention: 20 px text rows, 22 px controls,
//! square corners, no bevels. Depth is surface brightness plus a 1 px line; the only hues
//! are the accent (selection / ON), danger and ok.

use glam::Vec2;

use crate::window::event::{Cursor, Key};

use super::builder::Ui;
use super::frame::LAYER_POPUP;
use super::style::{with_alpha, Color};
use super::text::TextStyle;
use super::types::{Dir, Id, Rect, Response, Sense};

mod model;

pub use model::{ButtonKind, ButtonStyle, Range, TextResponse};

mod buttons;
mod choices;
mod fields;
mod labels;
mod misc;
mod numbers;
mod toggles;
