//! Event types for input handling
//!
//! Provides platform-independent event types for mouse and keyboard input.

/// Mouse button type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Special keys
    Escape,
    Tab,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,

    // Arrow keys
    Left,
    Right,
    Up,
    Down,

    // Modifier keys
    Shift,
    Control,
    Alt,
}

impl Key {
    /// Convert from winit key.
    pub fn from_winit(key: &winit::keyboard::Key) -> Option<Self> {
        use winit::keyboard::{Key as WKey, NamedKey};

        match key {
            WKey::Character(c) => {
                let c = c.chars().next()?;
                match c.to_ascii_lowercase() {
                    'a' => Some(Self::A),
                    'b' => Some(Self::B),
                    'c' => Some(Self::C),
                    'd' => Some(Self::D),
                    'e' => Some(Self::E),
                    'f' => Some(Self::F),
                    'g' => Some(Self::G),
                    'h' => Some(Self::H),
                    'i' => Some(Self::I),
                    'j' => Some(Self::J),
                    'k' => Some(Self::K),
                    'l' => Some(Self::L),
                    'm' => Some(Self::M),
                    'n' => Some(Self::N),
                    'o' => Some(Self::O),
                    'p' => Some(Self::P),
                    'q' => Some(Self::Q),
                    'r' => Some(Self::R),
                    's' => Some(Self::S),
                    't' => Some(Self::T),
                    'u' => Some(Self::U),
                    'v' => Some(Self::V),
                    'w' => Some(Self::W),
                    'x' => Some(Self::X),
                    'y' => Some(Self::Y),
                    'z' => Some(Self::Z),
                    '0' => Some(Self::Key0),
                    '1' => Some(Self::Key1),
                    '2' => Some(Self::Key2),
                    '3' => Some(Self::Key3),
                    '4' => Some(Self::Key4),
                    '5' => Some(Self::Key5),
                    '6' => Some(Self::Key6),
                    '7' => Some(Self::Key7),
                    '8' => Some(Self::Key8),
                    '9' => Some(Self::Key9),
                    _ => None,
                }
            }
            WKey::Named(named) => match named {
                NamedKey::Escape => Some(Self::Escape),
                NamedKey::Tab => Some(Self::Tab),
                NamedKey::Space => Some(Self::Space),
                NamedKey::Enter => Some(Self::Enter),
                NamedKey::Backspace => Some(Self::Backspace),
                NamedKey::Delete => Some(Self::Delete),
                NamedKey::Insert => Some(Self::Insert),
                NamedKey::Home => Some(Self::Home),
                NamedKey::End => Some(Self::End),
                NamedKey::PageUp => Some(Self::PageUp),
                NamedKey::PageDown => Some(Self::PageDown),
                NamedKey::ArrowLeft => Some(Self::Left),
                NamedKey::ArrowRight => Some(Self::Right),
                NamedKey::ArrowUp => Some(Self::Up),
                NamedKey::ArrowDown => Some(Self::Down),
                NamedKey::Shift => Some(Self::Shift),
                NamedKey::Control => Some(Self::Control),
                NamedKey::Alt => Some(Self::Alt),
                NamedKey::F1 => Some(Self::F1),
                NamedKey::F2 => Some(Self::F2),
                NamedKey::F3 => Some(Self::F3),
                NamedKey::F4 => Some(Self::F4),
                NamedKey::F5 => Some(Self::F5),
                NamedKey::F6 => Some(Self::F6),
                NamedKey::F7 => Some(Self::F7),
                NamedKey::F8 => Some(Self::F8),
                NamedKey::F9 => Some(Self::F9),
                NamedKey::F10 => Some(Self::F10),
                NamedKey::F11 => Some(Self::F11),
                NamedKey::F12 => Some(Self::F12),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Mouse cursor shape (a subset of the CSS cursor names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cursor {
    /// Arrow.
    #[default]
    Default,
    /// Hand (clickable).
    Pointer,
    /// I-beam.
    Text,
    /// Crosshair.
    Crosshair,
    /// Four-way move.
    Move,
    /// Open hand.
    Grab,
    /// Closed hand.
    Grabbing,
    /// Not allowed.
    NotAllowed,
    /// Horizontal resize.
    EwResize,
    /// Vertical resize.
    NsResize,
    /// Diagonal resize (north-east / south-west).
    NeswResize,
    /// Diagonal resize (north-west / south-east).
    NwseResize,
}

impl Cursor {
    /// The winit cursor icon.
    pub fn to_winit(self) -> winit::window::CursorIcon {
        use winit::window::CursorIcon as C;
        match self {
            Self::Default => C::Default,
            Self::Pointer => C::Pointer,
            Self::Text => C::Text,
            Self::Crosshair => C::Crosshair,
            Self::Move => C::Move,
            Self::Grab => C::Grab,
            Self::Grabbing => C::Grabbing,
            Self::NotAllowed => C::NotAllowed,
            Self::EwResize => C::EwResize,
            Self::NsResize => C::NsResize,
            Self::NeswResize => C::NeswResize,
            Self::NwseResize => C::NwseResize,
        }
    }
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Modifiers {
    /// Check if any modifier is pressed.
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt
    }

    /// Check if no modifier is pressed.
    pub fn none(&self) -> bool {
        !self.any()
    }
}

/// Input event.
#[derive(Debug, Clone)]
pub enum Event {
    /// Mouse button pressed.
    MousePress {
        button: MouseButton,
        position: (f32, f32),
        modifiers: Modifiers,
        handled: bool,
    },

    /// Mouse button released.
    MouseRelease {
        button: MouseButton,
        position: (f32, f32),
        modifiers: Modifiers,
        handled: bool,
    },

    /// Mouse moved.
    MouseMotion {
        delta: (f32, f32),
        position: (f32, f32),
        modifiers: Modifiers,
        handled: bool,
    },

    /// Mouse wheel scrolled.
    MouseWheel {
        delta: (f32, f32),
        position: (f32, f32),
        modifiers: Modifiers,
        handled: bool,
    },

    /// Key pressed.
    KeyPress {
        key: Key,
        modifiers: Modifiers,
        handled: bool,
    },

    /// Key released.
    KeyRelease {
        key: Key,
        modifiers: Modifiers,
        handled: bool,
    },

    /// Text input — composed, layout/shift-correct characters produced by a key
    /// press. Use this (not `KeyPress`) to drive text fields; control characters
    /// such as Backspace and Enter are filtered out and arrive as `KeyPress`.
    Text { text: String },

    /// Window resized.
    Resize { width: u32, height: u32 },

    /// The OS requested closure while [`super::WindowSettings::defer_close`] is enabled.
    /// The app remains alive until it returns `FrameOutput::exit` or `WindowCommand::Close`.
    CloseRequested,
}

impl Event {
    /// Check if the event has been handled.
    pub fn is_handled(&self) -> bool {
        match self {
            Self::MousePress { handled, .. }
            | Self::MouseRelease { handled, .. }
            | Self::MouseMotion { handled, .. }
            | Self::MouseWheel { handled, .. }
            | Self::KeyPress { handled, .. }
            | Self::KeyRelease { handled, .. } => *handled,
            Self::Text { .. } | Self::Resize { .. } | Self::CloseRequested => false,
        }
    }

    /// Mark the event as handled.
    pub fn set_handled(&mut self) {
        match self {
            Self::MousePress { handled, .. }
            | Self::MouseRelease { handled, .. }
            | Self::MouseMotion { handled, .. }
            | Self::MouseWheel { handled, .. }
            | Self::KeyPress { handled, .. }
            | Self::KeyRelease { handled, .. } => *handled = true,
            Self::Text { .. } | Self::Resize { .. } | Self::CloseRequested => {}
        }
    }
}
