use super::{ButtonKind, ButtonStyle, Color, Cursor, Rect, Response, Sense, Ui, Vec2};

impl Ui<'_> {
    // ----- buttons -----------------------------------------------------------------

    /// A button. Natural width fits the text; `set_next_width` overrides.
    pub fn button(&mut self, text: &str) -> Response {
        self.button_kind(text, ButtonKind::Normal)
    }

    /// A button with emphasis.
    pub fn button_kind(&mut self, text: &str, kind: ButtonKind) -> Response {
        let bs = ButtonStyle::for_kind(self, kind);
        self.button_styled(text, bs)
    }

    /// A button with explicit colours.
    pub fn button_styled(&mut self, text: &str, bs: ButtonStyle) -> Response {
        let style = self.style().clone();
        let font = bs.font_size.unwrap_or(style.font_size);
        let measured = self.gui().measure_styled(text, font, bs.text);
        let natural = Vec2::new(
            measured.x + style.button_padding * 2.0,
            style.control_height,
        );
        let size = self.resolve_size(natural);
        // Two buttons with the same label in one `Ui` (two "リセット" in a toolbar) must not
        // share an id: the first would swallow the release meant for the second.
        let id = self.next_widget_id(("button", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let enabled = self.enabled();
        let pressed = self.gui().is_active(id) && self.gui().mouse_down(crate::MouseButton::Left);
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, Some(p.line))
        } else if pressed {
            (bs.bg_active, bs.fg_active.unwrap_or(bs.fg), bs.border)
        } else if r.hovered {
            (bs.bg_hover, bs.fg, bs.border_hover.or(bs.border))
        } else {
            (bs.bg, bs.fg, bs.border)
        };
        self.gui().paint_rect(r.rect, bg);
        if let Some(b) = border {
            self.gui().paint_rect_outline(r.rect, 1.0, b);
        }
        let pos = r.rect.min + (r.rect.size() - measured) * 0.5;
        self.gui().paint_text_styled(text, pos, font, fg, bs.text);
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    /// An icon button: a square control whose glyph is painted by `paint` into the
    /// given rect (colour = the text colour for the state).
    pub fn icon_button(
        &mut self,
        id_source: impl core::hash::Hash,
        selected: bool,
        paint: impl FnOnce(&mut Ui<'_>, Rect, Color),
    ) -> Response {
        let style = self.style().clone();
        let side = style.control_height;
        let size = self.resolve_size(Vec2::splat(side));
        let id = self.next_widget_id(("icon", id_source));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let enabled = self.enabled();
        let pressed = self.gui().is_active(id) && self.gui().mouse_down(crate::MouseButton::Left);
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, p.line)
        } else if selected {
            (
                if r.hovered {
                    p.accent_bg_hover
                } else {
                    p.accent_bg
                },
                p.accent,
                p.accent,
            )
        } else if pressed {
            (p.raised_active, p.text, p.line)
        } else if r.hovered {
            (p.raised_hover, p.text, p.line)
        } else {
            (p.raised, p.text, p.line)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let inner = r.rect.shrink((side - 16.0).max(0.0) * 0.5);
        paint(self, inner, fg);
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    /// A toggle-style button that shows `selected` (`.btn.on`): the accent-tinted
    /// surface with accent text and border.
    pub fn selectable(&mut self, text: &str, selected: bool) -> Response {
        let p = self.style().palette;
        let mut bs = ButtonStyle::for_kind(self, ButtonKind::Normal);
        if selected {
            bs.bg = p.accent_bg;
            bs.bg_hover = p.accent_bg_hover;
            bs.bg_active = p.accent_bg;
            bs.fg = p.accent;
            bs.border = Some(p.accent);
        }
        self.button_styled(text, bs)
    }

    /// An exclusive choice strip (`.seg`). Returns `true` when `current` changed.
    pub fn segment(&mut self, current: &mut usize, options: &[&str]) -> bool {
        let on: Vec<bool> = (0..options.len()).map(|i| i == *current).collect();
        match self.segment_strip(options, &on) {
            Some(i) if i != *current => {
                *current = i;
                true
            }
            _ => false,
        }
    }

    /// A strip of independent toggles in one frame (log levels). Returns the index that
    /// was clicked (its flag is flipped).
    pub fn segment_toggles(&mut self, options: &[&str], on: &mut [bool]) -> Option<usize> {
        let snapshot: Vec<bool> = on.to_vec();
        let hit = self.segment_strip(options, &snapshot);
        if let Some(i) = hit {
            if let Some(v) = on.get_mut(i) {
                *v = !*v;
            }
        }
        hit
    }

    /// One outer border, 1 px dividers, ON = accent tint + 2 px accent underline.
    fn segment_strip(&mut self, options: &[&str], on: &[bool]) -> Option<usize> {
        let style = self.style().clone();
        let p = style.palette;
        let font = style.font_size;
        let widths: Vec<f32> = options
            .iter()
            .map(|o| self.gui().measure(o, font).x + style.button_padding * 2.0)
            .collect();
        let natural = Vec2::new(
            widths.iter().sum::<f32>() + (options.len() as f32 - 1.0).max(0.0) + 2.0,
            style.control_height,
        );
        let size = self.resolve_size(natural);
        let id = self.next_widget_id("segment");
        let outer = self.allocate(size);
        let enabled = self.enabled();
        // Distribute a forced width evenly.
        let scale = if natural.x > 2.0 {
            (size.x - 2.0 - (options.len() as f32 - 1.0).max(0.0))
                / (natural.x - 2.0 - (options.len() as f32 - 1.0).max(0.0))
        } else {
            1.0
        };
        let mut hit = None;
        let mut x = outer.min.x + 1.0;
        for (i, o) in options.iter().enumerate() {
            let w = widths[i] * scale;
            let cell = Rect::new(x, outer.min.y + 1.0, w, outer.height() - 2.0);
            let r = self.interact(id.with(i), cell, Sense::CLICK);
            let pressed =
                self.gui().is_active(id.with(i)) && self.gui().mouse_down(crate::MouseButton::Left);
            let selected = on.get(i).copied().unwrap_or(false);
            let (bg, fg) = if !enabled {
                ([0.0; 4], p.text_disabled)
            } else if selected {
                (
                    if r.hovered {
                        p.accent_bg_hover
                    } else {
                        p.accent_bg
                    },
                    p.accent,
                )
            } else if pressed {
                (p.raised_active, p.text)
            } else if r.hovered {
                (p.raised_hover, p.text)
            } else {
                ([0.0; 4], p.text)
            };
            self.gui().paint_rect(cell, bg);
            if selected && enabled {
                self.gui().paint_rect(
                    Rect::new(cell.min.x, cell.max.y - 2.0, cell.width(), 2.0),
                    p.accent,
                );
            }
            let m = self.gui().measure(o, font);
            let pos = cell.min + (cell.size() - m) * 0.5;
            self.gui().paint_text(o, pos, font, fg);
            if r.hovered && enabled {
                self.gui().set_cursor(Cursor::Pointer);
            }
            if r.clicked {
                hit = Some(i);
            }
            x += w;
            if i + 1 < options.len() {
                self.gui()
                    .paint_rect(Rect::new(x, cell.min.y, 1.0, cell.height()), p.line);
                x += 1.0;
            }
        }
        self.gui().paint_rect_outline(outer, 1.0, p.line);
        hit
    }
}
