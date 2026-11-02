use super::{Gui, Id, Rect, Response, Sense, LAYER_POPUP};

impl Gui {
    // ----- interaction ---------------------------------------------------------------

    /// Register a widget rect and compute what happened to it this frame.
    ///
    /// Hover respects the current clip and the popup / modal blocking rules: while a
    /// popup is open, only widgets on the popup layer under the pointer are hoverable.
    pub fn interact(&mut self, id: Id, rect: Rect, sense: Sense) -> Response {
        let mut r = Response::none(id, rect);
        let visible = self.clip_rect().intersect(&rect);
        let over = visible.contains(self.mouse);
        let blocked =
            if self.modal.is_some() || self.modal_blocked_frame || !self.popups_prev.is_empty() {
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
        let pressed_here = self.press_origin[0].is_some_and(|p| {
            let allowed = if self.modal.is_some() {
                self.layer == LAYER_POPUP
            } else if self.modal_blocked_frame {
                false
            } else if self.popups_prev.is_empty() {
                true
            } else {
                self.layer == LAYER_POPUP && self.popups_prev.iter().any(|rect| rect.contains(p))
            };
            visible.contains(p) && allowed
        });
        if pressed_here
            && !self.press_claimed[0]
            && self.pressed[0]
            && (sense.click || sense.drag)
            && (self.active.is_none() || self.active == Some(id))
        {
            r.pressed = true;
            self.press_claimed[0] = true;
            self.active = Some(id);
            self.active_layer = self.layer;
            r.double_clicked = self.double_click;
        }
        if hovered
            && self.released[2]
            && sense.click
            && self.press_origin[2].is_some_and(|p| visible.contains(p))
        {
            r.secondary_clicked = true;
        }
        let capture_allowed = self.layer == LAYER_POPUP
            || (self.modal.is_none() && !self.modal_blocked_frame && self.popups_prev.is_empty());
        if self.active == Some(id) && self.active_layer == self.layer && capture_allowed {
            self.wants_pointer = true;
            if self.down[0] {
                r.dragged = sense.drag;
                r.drag_delta = if r.pressed {
                    self.mouse - self.press_origin[0].unwrap_or(self.mouse)
                } else {
                    self.mouse_delta
                };
            }
            if self.released[0] {
                r.drag_released = true;
                r.clicked = over && !blocked && sense.click;
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
}
