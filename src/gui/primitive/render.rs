use super::{PrimitiveRenderer, Vertex, WgpuContext};

impl PrimitiveRenderer {
    /// Upload vertices to the GPU.
    pub fn prepare(&mut self, ctx: &WgpuContext, width: u32, height: u32) {
        if self.vertices.is_empty() {
            return;
        }

        let new_size = [width as f32, height as f32];
        // Sizes originate from integer pixel counts, so exact comparison is intentional
        // and never sees NaN; we only want to skip the upload when nothing changed.
        #[allow(
            clippy::float_cmp,
            reason = "screen size derived from integer pixel counts"
        )]
        let size_changed = self.screen_size != new_size;
        if size_changed {
            self.screen_size = new_size;
            let data = [width as f32, height as f32, 0.0, 0.0];
            ctx.queue
                .write_buffer(&self.screen_buffer, 0, bytemuck::cast_slice(&data));
        }

        let needed_size = (self.vertices.len() * size_of::<Vertex>()) as u64;
        if needed_size > self.vertex_buffer.size() {
            self.vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GUI Vertex Buffer"),
                size: needed_size * 2,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        ctx.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
    }

    /// Clear the draw list (vertices, batches and any unbalanced clips).
    pub fn finish(&mut self) {
        self.vertices.clear();
        self.batches.clear();
        self.clip_stack.clear();
    }

    /// Render the primitives, applying each batch's scissor rectangle.
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.vertices.is_empty() {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .slice(0..((self.vertices.len() * size_of::<Vertex>()) as u64)),
        );

        let total = self.vertices.len() as u32;
        let sw = self.screen_size[0] as u32;
        let sh = self.screen_size[1] as u32;

        for (i, batch) in self.batches.iter().enumerate() {
            let end = self.batches.get(i + 1).map_or(total, |next| next.start);
            if end <= batch.start {
                continue;
            }
            match batch.scissor {
                Some([left, top, right, bottom]) => {
                    // Clamp to the surface; wgpu rejects scissors past the edge or with
                    // zero area, so fully-offscreen / empty clips skip the draw entirely.
                    let x = (left.max(0.0) as u32).min(sw);
                    let y = (top.max(0.0) as u32).min(sh);
                    let w = (right.max(0.0) as u32).min(sw).saturating_sub(x);
                    let h = (bottom.max(0.0) as u32).min(sh).saturating_sub(y);
                    if w == 0 || h == 0 {
                        continue;
                    }
                    pass.set_scissor_rect(x, y, w, h);
                }
                None => pass.set_scissor_rect(0, 0, sw, sh),
            }
            pass.draw(batch.start..end, 0..1);
        }
    }
}
