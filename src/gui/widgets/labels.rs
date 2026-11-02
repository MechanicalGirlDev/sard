use super::{Color, Dir, Rect, Response, Sense, TextStyle, Ui, Vec2};

impl Ui<'_> {
    // ----- text --------------------------------------------------------------------

    /// A line of primary text.
    pub fn label(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size, self.style().palette.text);
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// A line of text in `color`.
    pub fn label_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size;
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// Secondary text: a row label (`.label`), base size in the dim colour.
    pub fn dim(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size, self.style().palette.text_dim);
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// Tertiary small text (hints, captions).
    pub fn small(&mut self, text: &str) -> Response {
        let (size, color) = (
            self.style().font_size_small,
            self.style().palette.text_faint,
        );
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// Small text in `color`.
    pub fn small_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size_small;
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// A monospace value (`.value`): small, tabular, primary colour.
    pub fn mono(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size_small, self.style().palette.text);
        self.text_widget(text, size, color, TextStyle::MONO)
    }

    /// A monospace value in `color`.
    pub fn mono_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size_small;
        self.text_widget(text, size, color, TextStyle::MONO)
    }

    /// A section heading: a full-width row with small-caps style text and a rule under
    /// it. Overflow is ellipsized, with the full title available as a hover tooltip.
    /// In a horizontal layout it takes only its natural width.
    pub fn heading(&mut self, text: &str) -> Response {
        let style = self.style().clone();
        let ts = TextStyle::SEMIBOLD.spaced(0.06);
        let measured = self.gui().measure_styled(text, style.font_size_heading, ts);
        let natural = match self.dir() {
            Dir::Vertical => Vec2::new(self.available_width(), style.row_height),
            Dir::Horizontal => Vec2::new(
                (measured.x + 2.0).min(self.available_width()),
                style.row_height,
            ),
        };
        let size = self.resolve_size(natural);
        let id = self.next_widget_id(("heading", text));
        let r = self.allocate_response(size, Sense::HOVER, id);
        let p = &style.palette;
        let rule = Rect::new(r.rect.min.x, r.rect.max.y - 1.0, r.rect.width(), 1.0);
        self.gui().paint_rect(rule, p.line);
        let y = r.rect.min.y + (r.rect.height() - 1.0 - measured.y) * 0.5;
        let mut shown = text.to_owned();
        if measured.x > r.rect.width() {
            while !shown.is_empty() {
                let candidate = format!("{shown}…");
                if self
                    .gui()
                    .measure_styled(&candidate, style.font_size_heading, ts)
                    .x
                    <= r.rect.width()
                {
                    break;
                }
                let _ = shown.pop();
            }
            if self
                .gui()
                .measure_styled("…", style.font_size_heading, ts)
                .x
                <= r.rect.width()
            {
                shown.push('…');
            }
            self.gui().tooltip(id, text);
        }
        self.gui().push_clip(r.rect);
        self.gui().paint_text_styled(
            &shown,
            Vec2::new(r.rect.min.x, y),
            style.font_size_heading,
            p.text_dim,
            ts,
        );
        self.gui().pop_clip();
        // Room under the rule so the first row does not touch it.
        self.add_space(3.0);
        r
    }

    /// Text in a custom size / colour / attributes.
    pub fn text_styled(&mut self, text: &str, size: f32, color: Color, ts: TextStyle) -> Response {
        self.text_widget(text, size, color, ts)
    }

    fn text_widget(&mut self, text: &str, size: f32, color: Color, ts: TextStyle) -> Response {
        let row = self.style().row_height;
        let measured = self.gui().measure_styled(text, size, ts);
        let natural = Vec2::new(
            measured.x,
            row.min(measured.y.max(size * 1.2)).max(measured.y),
        );
        let alloc = self.resolve_size(natural);
        let id = self.next_widget_id(("label", text));
        let r = self.allocate_response(alloc, Sense::HOVER, id);
        let y = r.rect.min.y + (r.rect.height() - measured.y) * 0.5;
        let color = if self.enabled() {
            color
        } else {
            self.style().palette.text_disabled
        };
        self.gui()
            .paint_text_styled(text, Vec2::new(r.rect.min.x, y), size, color, ts);
        r
    }

    /// Text wrapped to the available width (splits on whitespace, and between CJK
    /// characters).
    pub fn wrapped(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size, self.style().palette.text);
        self.wrapped_styled(text, size, color, 1.4)
    }

    /// A wrapped hint (`.hint`): small, faint, generous line height.
    pub fn hint(&mut self, text: &str) -> Response {
        let (size, color) = (
            self.style().font_size_small,
            self.style().palette.text_faint,
        );
        self.wrapped_styled(text, size, color, 1.4)
    }

    /// Wrapped text in a custom size / colour.
    pub fn wrapped_styled(
        &mut self,
        text: &str,
        size: f32,
        color: Color,
        leading: f32,
    ) -> Response {
        let width = self.available_width().max(1.0);
        let mut lines: Vec<String> = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in wrap_units(paragraph) {
                let candidate = format!("{line}{word}");
                if !line.is_empty() && self.gui().measure(candidate.trim_end(), size).x > width {
                    lines.push(line.trim_end().to_owned());
                    word.trim_start().clone_into(&mut line);
                } else {
                    line = candidate;
                }
            }
            lines.push(line.trim_end().to_owned());
        }
        let line_h = size * leading;
        let rect = self.allocate(Vec2::new(width, line_h * lines.len() as f32));
        let color = if self.enabled() {
            color
        } else {
            self.style().palette.text_disabled
        };
        for (i, l) in lines.iter().enumerate() {
            let pos = Vec2::new(
                rect.min.x,
                rect.min.y + i as f32 * line_h + (line_h - size * 1.2) * 0.5,
            );
            self.gui().paint_text(l, pos, size, color);
        }
        let id = self.next_widget_id(("wrapped", text));
        self.interact(id, rect, Sense::HOVER)
    }
}
/// Split a paragraph into wrap units: words with their trailing space, and single CJK
/// characters (which may break anywhere).
fn wrap_units(paragraph: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in paragraph.chars() {
        let cjk = matches!(c as u32, 0x3000..=0x30ff | 0x3400..=0x9fff | 0xf900..=0xfaff | 0xff00..=0xffef);
        if c.is_whitespace() {
            cur.push(c);
            out.push(core::mem::take(&mut cur));
        } else if cjk {
            if !cur.is_empty() {
                out.push(core::mem::take(&mut cur));
            }
            out.push(c.to_string());
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_units_split_words_and_cjk() {
        let u = wrap_units("ab cd 漢字x");
        assert_eq!(u, vec!["ab ", "cd ", "漢", "字", "x"]);
    }
}
