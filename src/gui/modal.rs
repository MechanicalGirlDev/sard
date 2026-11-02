//! Application-controlled modal dismissal.

/// What Escape and a press outside the modal do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ModalPolicy {
    /// Close immediately, preserving the traditional `set_modal` behavior.
    #[default]
    Dismiss,
    /// Report a request and keep the modal open until the application closes it.
    Request,
    /// Ignore dismissal gestures while an operation cannot be interrupted.
    Locked,
}

/// A user gesture requesting dismissal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalDismissal {
    /// Escape was pressed.
    Escape,
    /// The primary pointer was pressed outside the modal.
    OutsideClick,
}
