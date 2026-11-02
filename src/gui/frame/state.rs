use super::{Gui, Id, ModalDismissal, ModalPolicy, Rect, Vec2};

impl Gui {
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
        if let Some(id) = id {
            self.open_modal(id, ModalPolicy::Dismiss);
        } else {
            if self.modal.take().is_some() {
                self.modal_blocked_frame = true;
                self.focus = None;
                self.focus_next = None;
                self.active = None;
            }
            self.modal_rect = None;
        }
    }

    /// Open a modal or update its policy without reopening it. A different modal
    /// releases the old pointer and keyboard capture.
    pub fn open_modal(&mut self, id: Id, policy: ModalPolicy) {
        if self.modal != Some(id) {
            self.focus = None;
            self.focus_next = None;
            self.active = None;
            self.open.clear();
            self.modal_rect = None;
            self.modal_dismissal = None;
        }
        self.modal = Some(id);
        self.modal_policy = policy;
    }

    /// Consume this frame's dismissal request for `id`, if any.
    pub fn modal_dismissal(&mut self, id: Id) -> Option<ModalDismissal> {
        match self.modal_dismissal {
            Some((owner, reason)) if owner == id => {
                self.modal_dismissal = None;
                Some(reason)
            }
            Some(_) | None => None,
        }
    }

    pub(crate) fn register_modal_rect(&mut self, rect: Rect) {
        self.modal_rect = Some(rect);
        self.register_popup(rect);
    }

    pub(super) fn dismiss_modal(&mut self, reason: ModalDismissal) {
        let Some(id) = self.modal else { return };
        match self.modal_policy {
            ModalPolicy::Dismiss => {
                self.set_modal(None);
                self.modal_dismissal = Some((id, reason));
            }
            ModalPolicy::Request => self.modal_dismissal = Some((id, reason)),
            ModalPolicy::Locked => {}
        }
        self.modal_blocked_frame = true;
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
}
