//! Per-frame GUI context: input, persistent widget state, layered painting.
//!
//! [`Gui`] owns the renderers and the state that survives between frames (which popup is
//! open, where a scroll area is, which field has focus). Widgets are built through
//! [`Ui`](super::Ui), obtained from [`Gui::root`] after [`Gui::begin_frame`].
//!
//! Painting goes to **layers**: 0 is the base UI, 1 is popups / tooltips / modals. Each
//! layer is a primitive pass followed by a text pass, so a popup's background covers the
//! text underneath it.

use std::collections::{HashMap, HashSet};

use glam::Vec2;

use crate::context::WgpuContext;
use crate::window::event::{Cursor, Event, Key, MouseButton};

use super::modal::{ModalDismissal, ModalPolicy};
use super::primitive::PrimitiveRenderer;
use super::style::{Color, Style};
use super::text::{TextRenderer, TextStyle};
use super::types::{Id, Rect, Response, Sense};

/// Number of paint layers.
pub const LAYERS: usize = 2;
/// Base layer.
pub const LAYER_BASE: usize = 0;
/// Popup / tooltip / modal layer.
pub const LAYER_POPUP: usize = 1;

/// Seconds within which two clicks count as a double click.
const DOUBLE_CLICK: f32 = 0.35;

/// The GUI context (one per window).
pub struct Gui {
    style: Style,
    layers: Vec<PrimitiveRenderer>,
    text: TextRenderer,
    size: Vec2,
    time: f32,
    dt: f32,

    // ----- input (refreshed each frame) -----
    mouse: Vec2,
    mouse_delta: Vec2,
    down: [bool; 3],
    pressed: [bool; 3],
    released: [bool; 3],
    press_origin: [Option<Vec2>; 3],
    press_claimed: [bool; 3],
    scroll: Vec2,
    keys: Vec<Key>,
    keys_down: HashSet<Key>,
    typed: String,
    last_click: Option<(f32, Vec2)>,
    double_click: bool,

    // ----- interaction -----
    layer: usize,
    clip: Vec<Rect>,
    paint_scopes: Vec<paint_scope::PaintScopeFrame>,
    anchored_popups: Vec<paint_scope::AnchoredPopup>,
    hover: Option<Id>,
    active: Option<Id>,
    active_layer: usize,
    focus: Option<Id>,
    focus_next: Option<Id>,
    hover_since: Option<(Id, f32)>,
    tooltip: Option<(String, Vec2)>,
    wants_pointer: bool,
    wants_keyboard: bool,
    cursor: Cursor,

    // ----- popups / modals -----
    popups: Vec<Rect>,
    popups_prev: Vec<Rect>,
    pointer_over_popup_prev: bool,
    open: HashMap<Id, bool>,
    open_prev: HashMap<Id, bool>,
    modal: Option<Id>,
    modal_policy: ModalPolicy,
    modal_rect: Option<Rect>,
    modal_dismissal: Option<(Id, ModalDismissal)>,
    modal_blocked_frame: bool,

    // ----- persistent widget state -----
    scalars: HashMap<Id, f32>,
    cursors: HashMap<Id, usize>,
    strings: HashMap<Id, String>,
    vec2s: HashMap<Id, Vec2>,
}

impl Gui {
    /// Create a context for a colour target of `format`.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> Self {
        let mut text = TextRenderer::new(ctx, format);
        text.set_family(glyphon::FamilyOwned::SansSerif);
        Self {
            style: Style::default(),
            layers: (0..LAYERS)
                .map(|_| PrimitiveRenderer::new(ctx, format))
                .collect(),
            text,
            size: Vec2::ONE,
            time: 0.0,
            dt: 0.0,
            mouse: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            down: [false; 3],
            pressed: [false; 3],
            released: [false; 3],
            press_origin: [None; 3],
            press_claimed: [false; 3],
            scroll: Vec2::ZERO,
            keys: Vec::new(),
            keys_down: HashSet::new(),
            typed: String::new(),
            last_click: None,
            double_click: false,
            layer: LAYER_BASE,
            clip: Vec::new(),
            paint_scopes: Vec::new(),
            anchored_popups: Vec::new(),
            hover: None,
            active: None,
            active_layer: LAYER_BASE,
            focus: None,
            focus_next: None,
            hover_since: None,
            tooltip: None,
            wants_pointer: false,
            wants_keyboard: false,
            cursor: Cursor::Default,
            popups: Vec::new(),
            popups_prev: Vec::new(),
            pointer_over_popup_prev: false,
            open: HashMap::new(),
            open_prev: HashMap::new(),
            modal: None,
            modal_policy: ModalPolicy::default(),
            modal_rect: None,
            modal_dismissal: None,
            modal_blocked_frame: false,
            scalars: HashMap::new(),
            cursors: HashMap::new(),
            strings: HashMap::new(),
            vec2s: HashMap::new(),
        }
    }

    /// The style.
    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Replace the style.
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    /// The text renderer (fonts, families).
    pub fn text_renderer(&mut self) -> &mut TextRenderer {
        &mut self.text
    }

    /// Window size in pixels.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Seconds since the previous frame.
    pub fn dt(&self) -> f32 {
        self.dt
    }

    /// Seconds since the context was created.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// The top-level [`Ui`](super::Ui) covering the whole window.
    pub fn root(&mut self) -> super::Ui<'_> {
        let rect = Rect::from_min_size(Vec2::ZERO, self.size);
        super::Ui::new(self, Id::ROOT, rect)
    }

    /// Whether the pointer is over (or captured by) any widget this frame. A 3D viewport
    /// underneath the UI should ignore pointer input when this is set.
    pub fn wants_pointer(&self) -> bool {
        self.modal.is_some()
            || self.modal_blocked_frame
            || self.wants_pointer
            || self.hover.is_some()
            || self.active.is_some()
    }

    /// Whether a text field has focus (suppress global shortcuts).
    pub fn wants_keyboard(&self) -> bool {
        self.modal.is_some()
            || self.modal_blocked_frame
            || self.wants_keyboard
            || self.focus.is_some()
    }

    /// The widget under the pointer so far this frame (the last one registered wins, so
    /// a region registered *before* its children reads as hovered only when none of the
    /// children is).
    pub fn hovered(&self) -> Option<Id> {
        self.hover
    }

    /// The cursor shape requested by widgets this frame (hand over buttons, I-beam over
    /// fields, resize arrows over splitters). Hand it to `FrameOutput::cursor`.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Request a cursor shape for this frame (widgets call it while hovered; the last
    /// call wins, and a drag capture keeps its shape).
    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }

    /// Finish the frame (tooltips) and draw every layer onto `view`.
    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        self.draw_tooltip();
        let (w, h) = (self.size.x as u32, self.size.y as u32);
        for (i, layer) in self.layers.iter_mut().enumerate() {
            layer.prepare(ctx, w, h);
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gui primitives"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                layer.render(&mut pass);
            }
            self.text.render_layer(ctx, encoder, view, w, h, i as u8)?;
        }
        Ok(())
    }
}

impl core::fmt::Debug for Gui {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Gui")
            .field("size", &self.size)
            .field("layer", &self.layer)
            .finish_non_exhaustive()
    }
}

fn button_index(button: MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

mod input;
mod interaction;
mod paint_scope;
pub(crate) use paint_scope::popup_rect;
mod painting;
mod state;
