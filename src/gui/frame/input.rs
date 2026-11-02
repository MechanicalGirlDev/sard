use super::{
    button_index, Cursor, Event, Gui, Key, ModalDismissal, MouseButton, Vec2, DOUBLE_CLICK,
    LAYER_BASE,
};

impl Gui {
    // ----- frame lifecycle ------------------------------------------------------------

    /// Ingest input and reset per-frame state. Call once per frame before building UI.
    pub fn begin_frame(&mut self, events: &[Event], width: u32, height: u32, dt: f32) {
        self.size = Vec2::new(width.max(1) as f32, height.max(1) as f32);
        self.dt = dt;
        self.time += dt;
        self.pressed = [false; 3];
        self.released = [false; 3];
        self.press_claimed = [false; 3];
        for (origin, down) in self.press_origin.iter_mut().zip(self.down) {
            if !down {
                *origin = None;
            }
        }
        self.scroll = Vec2::ZERO;
        self.keys.clear();
        self.typed.clear();
        self.mouse_delta = Vec2::ZERO;
        self.double_click = false;
        self.modal_dismissal = None;
        self.modal_blocked_frame = false;

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
                    self.press_origin[i] = Some(self.mouse);
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
                Event::MouseWheel {
                    delta, position, ..
                } => {
                    self.mouse = Vec2::new(position.0, position.1);
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
        let press_over_popup = self.press_origin[0]
            .is_some_and(|at| self.popups_prev.iter().any(|rect| rect.contains(at)));
        if self.pressed[0] && !press_over_popup && self.modal.is_none() {
            self.open.clear();
        }
        if self.pressed[0]
            && self.press_origin[0]
                .is_some_and(|at| self.modal_rect.is_some_and(|rect| !rect.contains(at)))
        {
            self.dismiss_modal(ModalDismissal::OutsideClick);
        }
        // A popup inside a modal gets the first Escape; a policy owns modal dismissal.
        if self.keys.contains(&Key::Escape) {
            if self.open.is_empty() {
                self.dismiss_modal(ModalDismissal::Escape);
            } else {
                self.open.clear();
            }
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
        self.paint_scopes.clear();
        self.anchored_popups.clear();

        for l in &mut self.layers {
            l.finish();
        }
        self.text.begin_frame();
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

    /// Position where the current press began, retained through its release frame.
    pub fn mouse_press_origin(&self, button: MouseButton) -> Option<Vec2> {
        self.press_origin[button_index(button)]
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
}
