//! Text rendering
//!
//! Provides text rendering using glyphon.

use crate::context::WgpuContext;
use std::collections::HashMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer, Weight,
};

struct TextEntry {
    buffer: Buffer,
    left: f32,
    top: f32,
    color: Color,
    /// Clip bounds `[left, top, right, bottom]` in pixels, or `None` for unclipped.
    clip: Option<[f32; 4]>,
    /// Draw layer (see `TextRenderer::set_layer`).
    layer: u8,
}

struct TextLayer {
    atlas: TextAtlas,
    renderer: GlyphonTextRenderer,
    viewport: glyphon::Viewport,
}

/// Text renderer using glyphon.
pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    cache: Cache,
    format: wgpu::TextureFormat,
    layers: HashMap<u8, TextLayer>,

    // Immediate mode text entries
    entries: Vec<TextEntry>,
    available_buffers: Vec<Buffer>,
    scratch_buffer: Buffer,
    clip_stack: Vec<[f32; 4]>,

    /// Font family for every draw / measure call.
    family: glyphon::FamilyOwned,
    /// Family used when [`TextStyle::mono`] is set.
    mono_family: glyphon::FamilyOwned,
    /// Layer new entries are tagged with (see [`Self::set_layer`]).
    layer: u8,
    /// `(text, size bits, style)` → `(width, height)`; shaping is the cost, and labels
    /// repeat.
    measure_cache: HashMap<(String, u32, MeasureKey), (f32, f32)>,
}

impl TextRenderer {
    /// Create a new text renderer.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&ctx.device);

        let scratch_buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 18.0));

        Self {
            font_system,
            swash_cache,
            cache,
            format,
            layers: HashMap::new(),
            entries: Vec::new(),
            available_buffers: Vec::new(),
            scratch_buffer,
            clip_stack: Vec::new(),
            family: glyphon::FamilyOwned::Monospace,
            mono_family: glyphon::FamilyOwned::Monospace,
            layer: 0,
            measure_cache: HashMap::new(),
        }
    }

    /// Use this font family for all subsequent text (`FamilyOwned::SansSerif`,
    /// `FamilyOwned::Name("Noto Sans JP".into())`, …). Clears the measure cache.
    pub fn set_family(&mut self, family: glyphon::FamilyOwned) {
        self.family = family;
        self.measure_cache.clear();
    }

    /// Pick the proportional and monospace families by name (`"Segoe UI"`, `"Consolas"`);
    /// unknown names fall back to the system defaults. Clears the measure cache.
    pub fn set_family_names(&mut self, sans: &str, mono: &str) {
        self.family = glyphon::FamilyOwned::Name(sans.into());
        self.mono_family = glyphon::FamilyOwned::Name(mono.into());
        self.measure_cache.clear();
    }

    /// Use this family for text drawn with [`TextStyle::mono`]. Clears the measure cache.
    pub fn set_mono_family(&mut self, family: glyphon::FamilyOwned) {
        self.mono_family = family;
        self.measure_cache.clear();
    }

    /// Register font data (TTF / OTF bytes) with the font database, so it can be selected
    /// by name with [`Self::set_family`] and used as a fallback.
    pub fn load_font_data(&mut self, data: Vec<u8>) {
        self.font_system.db_mut().load_font_data(data);
        self.measure_cache.clear();
    }

    /// Tag subsequent entries with `layer`. [`Self::render_layer`] draws one layer at a
    /// time so a caller can interleave text with other passes (popups above base UI).
    pub fn set_layer(&mut self, layer: u8) {
        self.layer = layer;
    }

    /// Highest layer index used since [`Self::begin_frame`].
    pub fn max_layer(&self) -> u8 {
        self.entries.iter().map(|e| e.layer).max().unwrap_or(0)
    }

    /// Update the viewport size.
    pub fn resize(&mut self, ctx: &WgpuContext, width: u32, height: u32) {
        for layer in self.layers.values_mut() {
            layer
                .viewport
                .update(&ctx.queue, Resolution { width, height });
        }
    }

    /// Begin a new frame for immediate mode text rendering.
    pub fn begin_frame(&mut self) {
        while let Some(entry) = self.entries.pop() {
            self.available_buffers.push(entry.buffer);
        }
        self.clip_stack.clear();
        self.layer = 0;
        if self.measure_cache.len() > 8192 {
            self.measure_cache.clear();
        }
    }

    /// Push a clip rectangle; subsequent text is bounded to the intersection of
    /// this rectangle with any clip already on the stack. Balance with [`pop_clip`].
    ///
    /// [`pop_clip`]: Self::pop_clip
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut left = x;
        let mut top = y;
        let mut right = x + w;
        let mut bottom = y + h;
        if let Some(&[pl, pt, pr, pb]) = self.clip_stack.last() {
            left = left.max(pl);
            top = top.max(pt);
            right = right.min(pr);
            bottom = bottom.min(pb);
        }
        // Keep the rect non-inverted so glyphon never sees right < left.
        let right = right.max(left);
        let bottom = bottom.max(top);
        self.clip_stack.push([left, top, right, bottom]);
    }

    /// Pop the most recently pushed clip rectangle.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Draw text immediately (queues for render).
    /// Queue `text` at (`x`, `y`) (top-left) in **linear** `color`.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        self.draw_text_styled(text, x, y, font_size, color, TextStyle::NORMAL);
    }

    /// [`Self::draw_text`] with explicit family / weight / letter spacing.
    pub fn draw_text_styled(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
        style: TextStyle,
    ) {
        let text = spaced(text, style.letter_spacing);
        let text = text.as_ref();
        let mut buffer = self.available_buffers.pop().unwrap_or_else(|| {
            Buffer::new(
                &mut self.font_system,
                Metrics::new(font_size, font_size * 1.2),
            )
        });

        buffer.set_metrics(Metrics::new(font_size, font_size * 1.2));
        let attrs = attrs_for(&self.family, &self.mono_family, style);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // glyphon takes sRGB and converts to linear itself (`ColorMode::Accurate`).
        let ch = |c: f32| (super::style::linear_to_srgb(c.clamp(0.0, 1.0)) * 255.0).round() as u8;
        self.entries.push(TextEntry {
            buffer,
            left: x,
            top: y,
            color: Color::rgba(
                ch(color[0]),
                ch(color[1]),
                ch(color[2]),
                (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
            ),
            clip: self.clip_stack.last().copied(),
            layer: self.layer,
        });
    }

    /// Measure text dimensions.
    pub fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        self.measure_styled(text, font_size, TextStyle::NORMAL)
    }

    /// [`Self::measure`] with explicit family / weight / letter spacing.
    pub fn measure_styled(&mut self, text: &str, font_size: f32, style: TextStyle) -> (f32, f32) {
        let key = (
            text.to_owned(),
            font_size.to_bits(),
            MeasureKey::from(style),
        );
        if let Some(&m) = self.measure_cache.get(&key) {
            return m;
        }
        let m = self.measure_uncached(text, font_size, style);
        let _ = self.measure_cache.insert(key, m);
        m
    }

    fn measure_uncached(&mut self, text: &str, font_size: f32, style: TextStyle) -> (f32, f32) {
        let text = spaced(text, style.letter_spacing);
        let text = text.as_ref();
        self.scratch_buffer
            .set_metrics(Metrics::new(font_size, font_size * 1.2));
        let attrs = attrs_for(&self.family, &self.mono_family, style);
        self.scratch_buffer
            .set_text(text, &attrs, Shaping::Advanced, None);
        self.scratch_buffer
            .shape_until_scroll(&mut self.font_system, false);

        let mut width = 0.0_f32;
        let mut height = 0.0_f32;

        for run in self.scratch_buffer.layout_runs() {
            width = width.max(run.line_w);
            height += run.line_height;
        }

        if height == 0.0 && !text.is_empty() {
            height = font_size * 1.2;
        }

        (width, height)
    }
}

mod attributes;
mod builder;
mod render;
#[cfg(feature = "window")]
mod scope;
pub use attributes::TextStyle;
use attributes::{attrs_for, spaced, MeasureKey};
pub use builder::TextBuilder;
#[cfg(feature = "window")]
pub(crate) use scope::{TextPaintRange, TextPaintScope};
