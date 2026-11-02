//! Public widget configuration and response types.

use super::super::builder::Ui;
use super::super::style::{over, Color};
use super::super::text::TextStyle;
use super::super::types::Response;

/// Button emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    /// Raised surface with a border.
    #[default]
    Normal,
    /// Accent-filled (the primary action).
    Accent,
    /// Danger outline (red text and border on the raised surface).
    Danger,
    /// Danger-filled (a dangerous state is *on*: E-STOP engaged).
    DangerFilled,
    /// Borderless (toolbar / menu / row buttons); a border appears on hover.
    Flat,
}

/// Explicit button colours for looks the [`ButtonKind`]s do not cover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonStyle {
    /// Surface.
    pub bg: Color,
    /// Surface while hovered.
    pub bg_hover: Color,
    /// Surface while pressed.
    pub bg_active: Color,
    /// Text.
    pub fg: Color,
    /// Text while pressed (`None` = `fg`).
    pub fg_active: Option<Color>,
    /// 1 px border (`None` = none).
    pub border: Option<Color>,
    /// Border while hovered (`None` = `border`).
    pub border_hover: Option<Color>,
    /// Font size (`None` = the style's base size).
    pub font_size: Option<f32>,
    /// Text attributes.
    pub text: TextStyle,
}

impl ButtonStyle {
    /// The colours for `kind` from the current palette.
    pub fn for_kind(ui: &Ui<'_>, kind: ButtonKind) -> Self {
        let p = &ui.style().palette;
        let base = Self {
            bg: p.raised,
            bg_hover: p.raised_hover,
            bg_active: p.raised_active,
            fg: p.text,
            fg_active: None,
            border: Some(p.line),
            border_hover: None,
            font_size: None,
            text: TextStyle::NORMAL,
        };
        match kind {
            ButtonKind::Normal => base,
            ButtonKind::Accent => Self {
                bg: p.accent,
                bg_hover: over([1.0, 1.0, 1.0, 0.08], p.accent),
                bg_active: over([0.0, 0.0, 0.0, 0.1], p.accent),
                fg: p.text_on_accent,
                border: None,
                ..base
            },
            ButtonKind::Danger => Self {
                fg: p.danger,
                border: Some(p.danger),
                ..base
            },
            ButtonKind::DangerFilled => Self {
                bg: p.danger,
                bg_hover: over([0.0, 0.0, 0.0, 0.12], p.danger),
                bg_active: over([0.0, 0.0, 0.0, 0.2], p.danger),
                fg: [1.0, 1.0, 1.0, 1.0],
                border: None,
                ..base
            },
            ButtonKind::Flat => Self {
                bg: [0.0; 4],
                bg_hover: p.raised,
                bg_active: p.raised_active,
                border: None,
                border_hover: Some(p.line),
                ..base
            },
        }
    }
}

/// Result of a text field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextResponse {
    /// Interaction on the field.
    pub response: Response,
    /// The text changed this frame.
    pub changed: bool,
    /// Enter was pressed while focused.
    pub submitted: bool,
}

/// Inclusive numeric range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    /// Minimum.
    pub min: f32,
    /// Maximum.
    pub max: f32,
}

impl Range {
    /// `min..=max`.
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    /// Clamp `v` into the range.
    pub fn clamp(&self, v: f32) -> f32 {
        v.clamp(self.min.min(self.max), self.max.max(self.min))
    }

    /// Unbounded.
    pub const UNBOUNDED: Self = Self {
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
    };

    pub(super) fn fraction(self, v: f32) -> f32 {
        let span = self.max - self.min;
        if span.abs() > 0.0 {
            ((v - self.min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}
