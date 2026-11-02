use super::{Attrs, Weight};

/// Per-call text attributes on top of the renderer's family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Use the monospace family (see [`super::TextRenderer::set_mono_family`]).
    pub mono: bool,
    /// Font weight (CSS scale: 400 normal, 600 semi-bold, 700 bold).
    pub weight: u16,
    /// Extra advance per glyph, as a fraction of the font size (CSS `letter-spacing` in em).
    pub letter_spacing: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl TextStyle {
    /// Proportional, normal weight.
    pub const NORMAL: Self = Self {
        mono: false,
        weight: 400,
        letter_spacing: 0.0,
    };
    /// Monospace, normal weight.
    pub const MONO: Self = Self {
        mono: true,
        weight: 400,
        letter_spacing: 0.0,
    };
    /// Proportional, semi-bold.
    pub const SEMIBOLD: Self = Self {
        mono: false,
        weight: 600,
        letter_spacing: 0.0,
    };

    /// With `letter_spacing` (em).
    #[must_use]
    pub const fn spaced(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }
}

/// glyphon attributes for `style` (fields are borrowed separately so buffers and the font
/// system stay mutably available).
pub(super) fn attrs_for<'a>(
    family: &'a glyphon::FamilyOwned,
    mono: &'a glyphon::FamilyOwned,
    style: TextStyle,
) -> Attrs<'a> {
    let family = if style.mono { mono } else { family };
    Attrs::new()
        .family(family.as_family())
        .weight(Weight(style.weight))
}

/// Hashable form of a [`TextStyle`] for the measure cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MeasureKey {
    mono: bool,
    weight: u16,
    spacing: u32,
}

impl From<TextStyle> for MeasureKey {
    fn from(s: TextStyle) -> Self {
        Self {
            mono: s.mono,
            weight: s.weight,
            spacing: s.letter_spacing.to_bits(),
        }
    }
}

/// Letter spacing is emulated by interleaving hair spaces (U+200A) — cosmic-text has no
/// per-glyph advance, and the headings that use it are a few characters long.
pub(super) fn spaced(text: &str, letter_spacing: f32) -> std::borrow::Cow<'_, str> {
    if letter_spacing <= 0.0 || text.chars().count() < 2 {
        return std::borrow::Cow::Borrowed(text);
    }
    // One hair space is about 1/8 em; round the requested spacing to that unit.
    let n = ((letter_spacing * 8.0).round() as usize).max(1);
    let filler = "\u{200a}".repeat(n);
    let mut out = String::with_capacity(text.len() * (n + 1));
    let mut first = true;
    for c in text.chars() {
        if !first {
            out.push_str(&filler);
        }
        first = false;
        out.push(c);
    }
    std::borrow::Cow::Owned(out)
}
