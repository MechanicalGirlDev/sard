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
    scroll: Vec2,
    keys: Vec<Key>,
    keys_down: HashSet<Key>,
    typed: String,
    last_click: Option<(f32, Vec2)>,
    double_click: bool,

    // ----- interaction -----
    layer: usize,
    clip: Vec<Rect>,
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
            scroll: Vec2::ZERO,
            keys: Vec::new(),
            keys_down: HashSet::new(),
            typed: String::new(),
            last_click: None,
            double_click: false,
            layer: LAYER_BASE,
            clip: Vec::new(),
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

    // ----- frame lifecycle ------------------------------------------------------------

    /// Ingest input and reset per-frame state. Call once per frame before building UI.
    pub fn begin_frame(&mut self, events: &[Event], width: u32, height: u32, dt: f32) {
        self.size = Vec2::new(width.max(1) as f32, height.max(1) as f32);
        self.dt = dt;
        self.time += dt;
        self.pressed = [false; 3];
        self.released = [false; 3];
        self.scroll = Vec2::ZERO;
        self.keys.clear();
        self.typed.clear();
        self.mouse_delta = Vec2::ZERO;
        self.double_click = false;

        for event in events {
            match event {
                Event::MouseMotion {
                    position, delta, ..
                } => {
                    self.mouse = Vec2::new(position.0, position.1);
                    self.mouse_delta += Vec2::new(delta.0, delta.1);
                }
                Event::MousePress {
                    button, position, ..
                } => {
                    self.mouse = Vec2::new(position.0, position.1);
                    let i = button_index(*button);
                    self.down[i] = true;
                    self.pressed[i] = true;
                    if i == 0 {
                        let now = self.time;
                        let is_double = self.last_click.is_some_and(|(t, p)| {
                            now - t < DOUBLE_CLICK && (p - self.mouse).length() < 4.0
                        });
                        if is_double {
                            self.double_click = true;
                            self.last_click = None;
                        } else {
                            self.last_click = Some((now, self.mouse));
                        }
                    }
                }
                Event::MouseRelease {
                    button, position, ..
                } => {
                    self.mouse = Vec2::new(position.0, position.1);
                    let i = button_index(*button);
                    self.down[i] = false;
                    self.released[i] = true;
                }
                Event::MouseWheel { delta, .. } => {
                    self.scroll += Vec2::new(delta.0, delta.1);
                }
                Event::KeyPress { key, .. } => {
                    self.keys.push(*key);
                    let _ = self.keys_down.insert(*key);
                }
                Event::KeyRelease { key, .. } => {
                    let _ = self.keys_down.remove(key);
                }
                Event::Text { text } => self.typed.push_str(text),
                Event::Resize { .. } | Event::CloseRequested => {}
            }
        }

        // Popups: a press outside every popup closes them all (a widget that toggles its
        // own popup looks at `open_prev` so the same press does not re-open it).
        self.popups_prev = core::mem::take(&mut self.popups);
        self.pointer_over_popup_prev = self.popups_prev.iter().any(|r| r.contains(self.mouse));
        self.open_prev.clone_from(&self.open);
        if self.pressed[0] && !self.pointer_over_popup_prev && self.modal.is_none() {
            self.open.clear();
        }
        // Escape closes popups and modals.
        if self.keys.contains(&Key::Escape) {
            self.open.clear();
            self.modal = None;
        }

        if let Some(id) = self.focus_next.take() {
            self.focus = Some(id);
        }
        if self.pressed[0] {
            // A press anywhere defocuses; the field under the pointer re-claims focus.
            self.focus = None;
        }
        // A capture ends on the release frame *after* widgets have seen it (so a click can
        // complete); anything still active with the button up is stale.
        if !self.down[0] && !self.released[0] {
            self.active = None;
        }
        self.hover = None;
        self.wants_pointer = self.active.is_some();
        self.wants_keyboard = false;
        self.tooltip = None;
        self.cursor = Cursor::Default;
        self.layer = LAYER_BASE;
        self.clip.clear();

        for l in &mut self.layers {
            l.finish();
        }
        self.text.begin_frame();
    }

    /// The top-level [`Ui`](super::Ui) covering the whole window.
    pub fn root(&mut self) -> super::Ui<'_> {
        let rect = Rect::from_min_size(Vec2::ZERO, self.size);
        super::Ui::new(self, Id::ROOT, rect)
    }

    /// Whether the pointer is over (or captured by) any widget this frame. A 3D viewport
    /// underneath the UI should ignore pointer input when this is set.
    pub fn wants_pointer(&self) -> bool {
        self.wants_pointer || self.hover.is_some() || self.active.is_some()
    }

    /// Whether a text field has focus (suppress global shortcuts).
    pub fn wants_keyboard(&self) -> bool {
        self.wants_keyboard || self.focus.is_some()
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

    // ----- input accessors ------------------------------------------------------------

    /// Pointer position in pixels.
    pub fn mouse(&self) -> Vec2 {
        self.mouse
    }

    /// Pointer motion this frame.
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    /// Whether `button` is held.
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        self.down[button_index(button)]
    }

    /// Whether `button` went down this frame.
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.pressed[button_index(button)]
    }

    /// Whether `button` went up this frame.
    pub fn mouse_released(&self, button: MouseButton) -> bool {
        self.released[button_index(button)]
    }

    /// Wheel delta this frame (pixels; `y` is the vertical wheel).
    pub fn scroll_delta(&self) -> Vec2 {
        self.scroll
    }

    /// Keys pressed this frame.
    pub fn keys_pressed(&self) -> &[Key] {
        &self.keys
    }

    /// Whether `key` went down this frame.
    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }

    /// Whether `key` is held.
    pub fn key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    /// Text typed this frame.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    // ----- layers / clipping ---------------------------------------------------------

    /// Current paint layer.
    pub fn layer(&self) -> usize {
        self.layer
    }

    /// Switch the paint layer for subsequent painting and interaction.
    pub fn set_layer(&mut self, layer: usize) {
        self.layer = layer.min(LAYERS - 1);
        self.text.set_layer(self.layer as u8);
    }

    /// Push a clip rect (intersected with the current one).
    pub fn push_clip(&mut self, rect: Rect) {
        let rect = match self.clip.last() {
            Some(c) => c.intersect(&rect),
            None => rect,
        };
        self.clip.push(rect);
        self.layers[self.layer].push_clip(rect.x(), rect.y(), rect.width(), rect.height());
        self.text
            .push_clip(rect.x(), rect.y(), rect.width(), rect.height());
    }

    /// Pop the clip rect.
    pub fn pop_clip(&mut self) {
        let _ = self.clip.pop();
        self.layers[self.layer].pop_clip();
        self.text.pop_clip();
    }

    /// Current clip rect (the window when none is pushed).
    pub fn clip_rect(&self) -> Rect {
        self.clip
            .last()
            .copied()
            .unwrap_or_else(|| Rect::from_min_size(Vec2::ZERO, self.size))
    }

    // ----- interaction ---------------------------------------------------------------

    /// Register a widget rect and compute what happened to it this frame.
    ///
    /// Hover respects the current clip and the popup / modal blocking rules: while a
    /// popup is open, only widgets on the popup layer under the pointer are hoverable.
    pub fn interact(&mut self, id: Id, rect: Rect, sense: Sense) -> Response {
        let mut r = Response::none(id, rect);
        let visible = self.clip_rect().intersect(&rect);
        let over = visible.contains(self.mouse);
        let blocked = if self.modal.is_some() || !self.popups_prev.is_empty() {
            let on_top = self.layer == LAYER_POPUP;
            !(on_top && (self.pointer_over_popup_prev || self.modal.is_some()))
        } else {
            false
        };
        let hovered = over && !blocked && (self.active.is_none() || self.active == Some(id));
        r.hovered = hovered;
        if hovered {
            self.hover = Some(id);
            self.wants_pointer = true;
            match self.hover_since {
                Some((h, _)) if h == id => {}
                _ => self.hover_since = Some((id, self.time)),
            }
        }
        if hovered && self.pressed[0] && (sense.click || sense.drag) {
            r.pressed = true;
            self.active = Some(id);
            self.active_layer = self.layer;
            r.double_clicked = self.double_click;
        }
        if hovered && self.released[2] && sense.click {
            r.secondary_clicked = true;
        }
        if self.active == Some(id) {
            self.wants_pointer = true;
            if self.down[0] {
                r.dragged = sense.drag;
                r.drag_delta = self.mouse_delta;
            }
            if self.released[0] {
                r.drag_released = true;
                r.clicked = over && sense.click;
                self.active = None;
            }
        }
        r.focused = self.focus == Some(id);
        r
    }

    /// Give keyboard focus to `id` from the next frame.
    pub fn request_focus(&mut self, id: Id) {
        self.focus_next = Some(id);
    }

    /// Focus `id` now.
    pub fn set_focus(&mut self, id: Option<Id>) {
        self.focus = id;
    }

    /// Whether `id` has keyboard focus.
    pub fn has_focus(&self, id: Id) -> bool {
        self.focus == Some(id)
    }

    /// Mark that a widget consumed keyboard input this frame.
    pub fn use_keyboard(&mut self) {
        self.wants_keyboard = true;
    }

    /// Whether `id` currently holds the pointer.
    pub fn is_active(&self, id: Id) -> bool {
        self.active == Some(id)
    }

    /// Take the pointer capture for `id` (e.g. a splitter starting a drag).
    pub fn set_active(&mut self, id: Id) {
        self.active = Some(id);
        self.active_layer = self.layer;
    }

    /// Release the pointer capture.
    pub fn clear_active(&mut self) {
        self.active = None;
    }

    /// Show `text` as a tooltip when `id` has been hovered for the style's delay.
    pub fn tooltip(&mut self, id: Id, text: &str) {
        let due = self
            .hover_since
            .is_some_and(|(h, since)| h == id && self.time - since >= self.style.tooltip_delay);
        if due && self.hover == Some(id) {
            self.tooltip = Some((text.to_owned(), self.mouse));
        }
    }

    // ----- popups / modals -----------------------------------------------------------

    /// Whether the popup `id` is open.
    pub fn is_open(&self, id: Id) -> bool {
        self.open.get(&id).copied().unwrap_or(false)
    }

    /// Whether the popup `id` was open when this frame began (before outside clicks
    /// closed it). Toggle logic uses this so one press does not close-then-reopen.
    pub fn was_open(&self, id: Id) -> bool {
        self.open_prev.get(&id).copied().unwrap_or(false)
    }

    /// Open or close the popup `id`.
    pub fn set_open(&mut self, id: Id, open: bool) {
        if open {
            let _ = self.open.insert(id, true);
        } else {
            let _ = self.open.remove(&id);
        }
    }

    /// Close every popup.
    pub fn close_popups(&mut self) {
        self.open.clear();
    }

    /// Register the rect of an open popup this frame (clicks outside close it; widgets
    /// inside stay interactive while others are blocked).
    pub fn register_popup(&mut self, rect: Rect) {
        self.popups.push(rect);
    }

    /// Whether any popup is open (blocking the base layer).
    pub fn any_popup_open(&self) -> bool {
        !self.popups_prev.is_empty() || !self.open.is_empty()
    }

    /// The open modal, if any.
    pub fn modal(&self) -> Option<Id> {
        self.modal
    }

    /// Open (`Some`) or close (`None`) the modal.
    pub fn set_modal(&mut self, id: Option<Id>) {
        self.modal = id;
    }

    // ----- persistent state -----------------------------------------------------------

    /// Scalar state (scroll offsets, drag origins).
    pub fn scalar(&self, id: Id) -> f32 {
        self.scalars.get(&id).copied().unwrap_or(0.0)
    }

    /// Set scalar state.
    pub fn set_scalar(&mut self, id: Id, value: f32) {
        let _ = self.scalars.insert(id, value);
    }

    /// Caret state (text fields).
    pub fn text_cursor(&self, id: Id) -> Option<usize> {
        self.cursors.get(&id).copied()
    }

    /// Set caret state.
    pub fn set_text_cursor(&mut self, id: Id, value: usize) {
        let _ = self.cursors.insert(id, value);
    }

    /// String state (a field's edit buffer).
    pub fn string(&self, id: Id) -> Option<&String> {
        self.strings.get(&id)
    }

    /// Set string state.
    pub fn set_string(&mut self, id: Id, value: Option<String>) {
        match value {
            Some(v) => drop(self.strings.insert(id, v)),
            None => drop(self.strings.remove(&id)),
        }
    }

    /// Vector state (drag origins, panel positions).
    pub fn vec2(&self, id: Id) -> Option<Vec2> {
        self.vec2s.get(&id).copied()
    }

    /// Set vector state.
    pub fn set_vec2(&mut self, id: Id, value: Option<Vec2>) {
        match value {
            Some(v) => {
                let _ = self.vec2s.insert(id, v);
            }
            None => {
                let _ = self.vec2s.remove(&id);
            }
        }
    }

    // ----- painting -------------------------------------------------------------------

    /// Filled rectangle on the current layer.
    pub fn paint_rect(&mut self, rect: Rect, color: Color) {
        if color[3] <= 0.0 || rect.is_empty() {
            return;
        }
        self.layers[self.layer].draw_rect(rect.x(), rect.y(), rect.width(), rect.height(), color);
    }

    /// Rectangle outline.
    pub fn paint_rect_outline(&mut self, rect: Rect, thickness: f32, color: Color) {
        let l = &mut self.layers[self.layer];
        let (x, y, w, h) = (rect.x(), rect.y(), rect.width(), rect.height());
        l.draw_rect(x, y, w, thickness, color);
        l.draw_rect(x, y + h - thickness, w, thickness, color);
        l.draw_rect(x, y, thickness, h, color);
        l.draw_rect(x + w - thickness, y, thickness, h, color);
    }

    /// Line segment.
    pub fn paint_line(&mut self, a: Vec2, b: Vec2, thickness: f32, color: Color) {
        self.layers[self.layer].draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }

    /// Polyline.
    pub fn paint_polyline(&mut self, points: &[[f32; 2]], thickness: f32, color: Color) {
        self.layers[self.layer].draw_polyline(points, thickness, color);
    }

    /// Filled circle.
    pub fn paint_circle(&mut self, center: Vec2, radius: f32, color: Color) {
        self.layers[self.layer].draw_circle(center.x - radius, center.y - radius, radius, color);
    }

    /// Text at `pos` (top-left) in `size` pixels.
    pub fn paint_text(&mut self, text: &str, pos: Vec2, size: f32, color: Color) {
        self.text.draw_text(text, pos.x, pos.y, size, color);
    }

    /// Text at `pos` with explicit family / weight / letter spacing.
    pub fn paint_text_styled(
        &mut self,
        text: &str,
        pos: Vec2,
        size: f32,
        color: Color,
        style: TextStyle,
    ) {
        self.text
            .draw_text_styled(text, pos.x, pos.y, size, color, style);
    }

    /// Text size in pixels.
    pub fn measure(&mut self, text: &str, size: f32) -> Vec2 {
        let (w, h) = self.text.measure(text, size);
        Vec2::new(w, h)
    }

    /// Text size in pixels with explicit family / weight / letter spacing.
    pub fn measure_styled(&mut self, text: &str, size: f32, style: TextStyle) -> Vec2 {
        let (w, h) = self.text.measure_styled(text, size, style);
        Vec2::new(w, h)
    }

    /// A soft drop shadow under `rect` (a few translucent rings, no blur pass).
    pub fn paint_shadow(&mut self, rect: Rect, offset: Vec2, spread: f32, alpha: f32) {
        let steps = 4;
        for i in 0..steps {
            let t = (i + 1) as f32 / steps as f32;
            let r = rect.translate(offset).expand(spread * (1.0 - t));
            self.paint_rect(r, [0.0, 0.0, 0.0, alpha * t / steps as f32]);
        }
    }

    /// Reserve a background rect on the current layer, to be filled with
    /// [`Self::fill_reserved`] once the content size is known.
    pub fn reserve_rect(&mut self) -> (usize, usize) {
        (self.layer, self.layers[self.layer].reserve_rect())
    }

    /// Fill a rect reserved with [`Self::reserve_rect`].
    pub fn fill_reserved(&mut self, handle: (usize, usize), rect: Rect, color: Color) {
        self.layers[handle.0].set_reserved_rect(
            handle.1,
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            color,
        );
    }

    fn draw_tooltip(&mut self) {
        let Some((text, at)) = self.tooltip.take() else {
            return;
        };
        let style = self.style.clone();
        let prev = self.layer;
        self.set_layer(LAYER_POPUP);
        let size = self.measure(&text, style.font_size_small) + Vec2::splat(style.padding * 2.0);
        let mut pos = at + Vec2::new(12.0, 18.0);
        if pos.x + size.x > self.size.x {
            pos.x = (self.size.x - size.x).max(0.0);
        }
        if pos.y + size.y > self.size.y {
            pos.y = (at.y - size.y - 4.0).max(0.0);
        }
        let rect = Rect::from_min_size(pos, size);
        self.paint_rect(rect, style.palette.header);
        self.paint_rect_outline(rect, 1.0, style.palette.line_hard);
        self.paint_text(
            &text,
            pos + Vec2::splat(style.padding),
            style.font_size_small,
            style.palette.text,
        );
        self.set_layer(prev);
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
