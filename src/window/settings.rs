//! Window settings
//!
//! Configuration for window creation.

/// Settings for creating a window.
#[derive(Debug, Clone)]
pub struct WindowSettings {
    /// Window title.
    pub title: String,
    /// Initial window size (width, height) in logical pixels.
    pub size: (u32, u32),
    /// Whether the window is resizable.
    pub resizable: bool,
    /// Whether to enable vsync.
    pub vsync: bool,
    /// Whether to start maximized.
    pub maximized: bool,
    /// Whether to start in fullscreen.
    pub fullscreen: bool,
    /// Whether the OS draws the title bar and borders. `false` for a client-side title
    /// bar: drive moves / resizes / buttons through `FrameOutput::window`.
    pub decorations: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            title: "sard".to_owned(),
            size: (1280, 720),
            resizable: true,
            vsync: true,
            maximized: false,
            fullscreen: false,
            decorations: true,
        }
    }
}

impl WindowSettings {
    /// Create new window settings with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the initial window size.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = (width, height);
        self
    }

    /// Set whether the window is resizable.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Set whether to enable vsync.
    pub fn vsync(mut self, vsync: bool) -> Self {
        self.vsync = vsync;
        self
    }

    /// Set whether to start maximized.
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    /// Set whether to start in fullscreen.
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    /// Set whether the OS draws the window frame.
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }
}
