//! GUI module
//!
//! Provides UI rendering capabilities including text and 2D primitives.
//!
//! [`PrimitiveRenderer`] and [`TextRenderer`] are window-independent and available
//! whenever the `gui` feature is on. [`UiContext`] is an immediate-mode widget layer
//! built on the window event types, so it additionally requires the `window` feature.

pub mod primitive;
pub mod style;
pub mod text;

#[cfg(feature = "window")]
pub mod ui;

#[cfg(feature = "window")]
mod builder;
#[cfg(feature = "window")]
mod containers;
#[cfg(feature = "window")]
mod dock;
#[cfg(feature = "window")]
mod frame;
#[cfg(feature = "window")]
mod menu;
#[cfg(feature = "window")]
mod modal;
#[cfg(feature = "window")]
mod plot;
#[cfg(feature = "window")]
mod scroll;
#[cfg(feature = "window")]
mod tree_drag;
#[cfg(feature = "window")]
mod types;
#[cfg(feature = "window")]
mod widgets;

#[cfg(all(test, feature = "window"))]
mod tests;

#[cfg(all(test, feature = "window"))]
mod editing_tests;
#[cfg(all(test, feature = "window"))]
mod interaction_tests;
#[cfg(all(test, feature = "window"))]
mod modal_tests;
#[cfg(all(test, feature = "window"))]
mod render_tests;
#[cfg(all(test, feature = "window"))]
mod visual_tests;

pub use primitive::PrimitiveRenderer;
pub use style::{linear_to_srgb, over, srgb, srgb_to_linear, with_alpha, Color, Palette, Style};
pub use text::{TextBuilder, TextRenderer, TextStyle};

#[cfg(feature = "window")]
pub use builder::Ui;
#[cfg(feature = "window")]
pub use containers::{modal, paint_chevron, TreeResponse};
#[cfg(feature = "window")]
pub use dock::{DockNode, DockPanels, DockResponse, DockTree, DropTarget, Side};
#[cfg(feature = "window")]
pub use frame::{Gui, LAYERS, LAYER_BASE, LAYER_POPUP};
#[cfg(feature = "window")]
pub use menu::Menu;
#[cfg(feature = "window")]
pub use modal::{ModalDismissal, ModalPolicy};
#[cfg(feature = "window")]
pub use plot::{Plot, Series};
#[cfg(feature = "window")]
pub use scroll::{ScrollRegion, ScrollRegions, ScrollState};
#[cfg(feature = "window")]
pub use tree_drag::{TreeDrag, TreeDrop, TreeDropPosition};
#[cfg(feature = "window")]
pub use types::{Align, Dir, Id, Rect, Response, Sense};
#[cfg(feature = "window")]
pub use ui::UiContext;
#[cfg(feature = "window")]
pub use widgets::{ButtonKind, ButtonStyle, Range, TextResponse};
