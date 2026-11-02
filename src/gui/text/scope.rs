use super::TextRenderer;

/// Text queued in one paint layer, with its fixed enclosing clips detached.
pub(crate) struct TextPaintScope {
    start: usize,
    layer: u8,
    clips: Vec<[f32; 4]>,
}

pub(crate) struct TextPaintRange {
    entries: core::ops::Range<usize>,
    layer: u8,
}

impl TextRenderer {
    pub(crate) fn begin_paint_scope(&mut self) -> TextPaintScope {
        TextPaintScope {
            start: self.entries.len(),
            layer: self.layer,
            clips: core::mem::take(&mut self.clip_stack),
        }
    }

    pub(crate) fn end_paint_scope(
        &mut self,
        scope: TextPaintScope,
        offset: [f32; 2],
        excluded: &[&TextPaintRange],
    ) {
        let enclosing = scope.clips.last().copied();
        let range = self.finish_paint_scope(scope);
        let mut excluded = excluded.to_vec();
        excluded.sort_unstable_by_key(|part| part.entries.start);
        let mut start = range.entries.start;
        for part in excluded {
            self.translate_paint_range(
                &TextPaintRange {
                    entries: start..part.entries.start,
                    layer: range.layer,
                },
                offset,
                enclosing,
            );
            start = part.entries.end;
        }
        self.translate_paint_range(
            &TextPaintRange {
                entries: start..range.entries.end,
                layer: range.layer,
            },
            offset,
            enclosing,
        );
    }

    pub(crate) fn finish_paint_scope(&mut self, scope: TextPaintScope) -> TextPaintRange {
        self.clip_stack = scope.clips;
        TextPaintRange {
            entries: scope.start..self.entries.len(),
            layer: scope.layer,
        }
    }

    pub(crate) fn translate_paint_range(
        &mut self,
        range: &TextPaintRange,
        offset: [f32; 2],
        enclosing: Option<[f32; 4]>,
    ) {
        for entry in self.entries[range.entries.clone()]
            .iter_mut()
            .filter(|entry| entry.layer == range.layer)
        {
            entry.left += offset[0];
            entry.top += offset[1];
            let shifted = entry.clip.map(|[left, top, right, bottom]| {
                [
                    left + offset[0],
                    top + offset[1],
                    right + offset[0],
                    bottom + offset[1],
                ]
            });
            entry.clip = match (shifted, enclosing) {
                (Some([left, top, right, bottom]), Some([x0, y0, x1, y1])) => {
                    let left = left.max(x0);
                    let top = top.max(y0);
                    Some([left, top, right.min(x1).max(left), bottom.min(y1).max(top)])
                }
                (Some(clip), None) | (None, Some(clip)) => Some(clip),
                (None, None) => None,
            };
        }
    }
}
