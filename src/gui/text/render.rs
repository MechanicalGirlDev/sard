use super::{
    GlyphonTextRenderer, Resolution, TextArea, TextAtlas, TextBounds, TextLayer, TextRenderer,
    WgpuContext,
};

impl TextRenderer {
    /// Render every layer of text (in layer order) on top of `view`.
    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        for layer in 0..=self.max_layer() {
            self.render_layer(ctx, encoder, view, width, height, layer)?;
        }
        Ok(())
    }

    /// Render only the entries tagged with `layer`.
    pub fn render_layer(
        &mut self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        layer: u8,
    ) -> anyhow::Result<()> {
        // Queue writes precede the entire command buffer. Re-preparing one glyphon
        // renderer for another layer would overwrite vertices used by earlier passes.
        // Each layer therefore retains its vertices, atlas, and viewport independently.
        let gpu = self.layers.entry(layer).or_insert_with(|| {
            let mut atlas = TextAtlas::new(&ctx.device, &ctx.queue, &self.cache, self.format);
            let renderer = GlyphonTextRenderer::new(
                &mut atlas,
                &ctx.device,
                wgpu::MultisampleState::default(),
                None,
            );
            let viewport = glyphon::Viewport::new(&ctx.device, &self.cache);
            TextLayer {
                atlas,
                renderer,
                viewport,
            }
        });
        gpu.viewport
            .update(&ctx.queue, Resolution { width, height });

        let mut text_areas = Vec::with_capacity(self.entries.len());

        for entry in self.entries.iter().filter(|e| e.layer == layer) {
            let bounds = entry.clip.map_or(
                TextBounds {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                },
                |[left, top, right, bottom]| TextBounds {
                    left: left as i32,
                    top: top as i32,
                    right: right as i32,
                    bottom: bottom as i32,
                },
            );
            text_areas.push(TextArea {
                buffer: &entry.buffer,
                left: entry.left,
                top: entry.top,
                scale: 1.0,
                bounds,
                default_color: entry.color,
                custom_glyphs: &[],
            });
        }

        gpu.renderer.prepare(
            &ctx.device,
            &ctx.queue,
            &mut self.font_system,
            &mut gpu.atlas,
            &gpu.viewport,
            text_areas,
            &mut self.swash_cache,
        )?;

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            gpu.renderer.render(&gpu.atlas, &gpu.viewport, &mut pass)?;
        }

        Ok(())
    }

    /// Trim the atlas to free unused space.
    pub fn trim(&mut self) {
        for layer in self.layers.values_mut() {
            layer.atlas.trim();
        }
    }
}
