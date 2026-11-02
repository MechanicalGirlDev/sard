use super::{Color, Gui, Rect, TextStyle, Vec2, LAYERS, LAYER_POPUP};

impl Gui {
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
        let requested = rect;
        let rect = match self.clip.last() {
            Some(c) => c.intersect(&rect),
            None => rect,
        };
        self.clip.push(rect);
        // Scoped painting needs unclipped child bounds so a later scroll correction
        // can reveal newly appended content. Input still uses the fixed viewport above.
        let paint = if self.paint_is_deferred() {
            requested
        } else {
            rect
        };
        self.layers[self.layer].push_clip(paint.x(), paint.y(), paint.width(), paint.height());
        self.text
            .push_clip(paint.x(), paint.y(), paint.width(), paint.height());
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

    pub(super) fn draw_tooltip(&mut self) {
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
