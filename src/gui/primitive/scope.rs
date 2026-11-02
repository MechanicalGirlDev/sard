use super::{DrawBatch, PrimitiveRenderer};

/// A CPU draw-list range with its enclosing fixed clip temporarily detached.
pub(crate) struct PrimitivePaintScope {
    vertices: usize,
    batches: usize,
    clips: Vec<[f32; 4]>,
}

pub(crate) struct PrimitivePaintRange {
    vertices: core::ops::Range<usize>,
    batches: core::ops::Range<usize>,
}

impl PrimitiveRenderer {
    pub(crate) fn begin_paint_scope(&mut self) -> PrimitivePaintScope {
        let scope = PrimitivePaintScope {
            vertices: self.vertices.len(),
            batches: self.batches.len(),
            clips: core::mem::take(&mut self.clip_stack),
        };
        // The scope must not merge into geometry queued before its start.
        self.batches.push(DrawBatch {
            start: self.vertices.len() as u32,
            scissor: None,
        });
        scope
    }

    pub(crate) fn end_paint_scope(
        &mut self,
        scope: PrimitivePaintScope,
        offset: [f32; 2],
        excluded: &[&PrimitivePaintRange],
    ) {
        let enclosing = scope.clips.last().copied();
        let range = self.finish_paint_scope(scope);
        let mut excluded = excluded.to_vec();
        excluded.sort_unstable_by_key(|part| part.vertices.start);
        let mut start = (range.vertices.start, range.batches.start);
        for part in excluded {
            self.translate_paint_range(
                &PrimitivePaintRange {
                    vertices: start.0..part.vertices.start,
                    batches: start.1..part.batches.start,
                },
                offset,
                enclosing,
            );
            start = (part.vertices.end, part.batches.end);
        }
        self.translate_paint_range(
            &PrimitivePaintRange {
                vertices: start.0..range.vertices.end,
                batches: start.1..range.batches.end,
            },
            offset,
            enclosing,
        );
    }

    pub(crate) fn finish_paint_scope(&mut self, scope: PrimitivePaintScope) -> PrimitivePaintRange {
        self.clip_stack = scope.clips;
        let range = PrimitivePaintRange {
            vertices: scope.vertices..self.vertices.len(),
            batches: scope.batches..self.batches.len(),
        };
        self.batches.push(DrawBatch {
            start: self.vertices.len() as u32,
            scissor: self.clip_stack.last().copied(),
        });
        range
    }

    pub(crate) fn translate_paint_range(
        &mut self,
        range: &PrimitivePaintRange,
        offset: [f32; 2],
        enclosing: Option<[f32; 4]>,
    ) {
        for vertex in &mut self.vertices[range.vertices.clone()] {
            vertex.position[0] += offset[0];
            vertex.position[1] += offset[1];
        }
        for batch in &mut self.batches[range.batches.clone()] {
            let shifted = batch.scissor.map(|[left, top, right, bottom]| {
                [
                    left + offset[0],
                    top + offset[1],
                    right + offset[0],
                    bottom + offset[1],
                ]
            });
            batch.scissor = match (shifted, enclosing) {
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
