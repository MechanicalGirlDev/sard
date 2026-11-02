//! Encode the prepared scene in layer and depth order.

use core::mem::size_of;

use super::{ObjectDraw, RenderOptions, SceneRenderer};
use crate::core::render_states::ClearState;
use crate::core::render_target::RenderTarget;
use crate::scene::lines::LineInstance;
use crate::scene::{Layer, PointInstance, Scene};

pub(super) struct PreparedScene<'a> {
    pub scene: &'a Scene,
    pub objects: Vec<ObjectDraw<'a>>,
    pub lines: [(u32, u32); 2],
    pub points: [(u32, u32); 2],
}

impl SceneRenderer {
    pub(super) fn draw(
        &self,
        target: &RenderTarget<'_>,
        prepared: PreparedScene<'_>,
        options: RenderOptions,
    ) {
        let ctx = target.context();
        let PreparedScene {
            scene,
            objects: draws,
            lines: line_ranges,
            points: point_ranges,
        } = prepared;
        // ----- draw ------------------------------------------------------------------
        let (opaque, rest): (Vec<_>, Vec<_>) = draws
            .iter()
            .partition(|d| d.object.layer != Layer::Overlay && !d.object.surface.is_transparent());
        let (mut blend, overlay): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|d| d.object.layer != Layer::Overlay);
        blend.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        let mut ground: Vec<&ObjectDraw<'_>> = opaque
            .iter()
            .copied()
            .filter(|d| d.object.layer == Layer::Ground)
            .collect();
        ground.extend(
            opaque
                .iter()
                .copied()
                .filter(|d| d.object.layer != Layer::Ground),
        );

        let mut encoder = ctx.create_encoder(Some("scene"));
        {
            let clear = if options.clear {
                ClearState::color_and_depth(scene.background, 1.0)
            } else {
                ClearState::none()
            };
            let mut pass = target.begin_render_pass(&mut encoder, clear);
            if let Some([x, y, w, h]) = options.region {
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w, h);
                pass.set_pipeline(&self.depth_reset);
                pass.draw(0..3, 0..1);
            }
            pass.set_bind_group(0, &self.frame_bind_group, &[]);

            let draw_objects = |pass: &mut wgpu::RenderPass<'_>, list: &[&ObjectDraw<'_>]| {
                for d in list {
                    pass.set_bind_group(1, &self.object_bind_group, &[d.offset]);
                    let mesh = &d.object.mesh;
                    pass.set_vertex_buffer(0, mesh.vertex_buffer().slice());
                    let ib = mesh.index_buffer();
                    pass.set_index_buffer(ib.slice(), ib.format());
                    pass.draw_indexed(0..ib.count(), 0, 0..1);
                }
            };

            pass.set_pipeline(&self.mesh_opaque);
            draw_objects(&mut pass, &ground);

            // The grid goes over opaque geometry: its pipeline compares `LessEqual` with a
            // depth bias, so a ground plate lying exactly on the grid plane does not hide it.
            if scene.grid.is_some() {
                pass.set_pipeline(&self.grid);
                pass.set_bind_group(1, &self.grid_bind_group, &[]);
                pass.draw(0..6, 0..1);
            }

            let stride_line = LineInstance::layout().array_stride;
            let stride_point = size_of::<PointInstance>() as u64;
            let (ls, le) = line_ranges[0];
            if le > ls {
                pass.set_pipeline(&self.line);
                pass.set_vertex_buffer(
                    0,
                    self.line_buffer
                        .buffer
                        .slice(u64::from(ls) * stride_line..u64::from(le) * stride_line),
                );
                pass.draw(0..6, 0..(le - ls));
            }
            let (ps, pe) = point_ranges[0];
            if pe > ps {
                pass.set_pipeline(&self.point);
                pass.set_vertex_buffer(
                    0,
                    self.point_buffer
                        .buffer
                        .slice(u64::from(ps) * stride_point..u64::from(pe) * stride_point),
                );
                pass.draw(0..6, 0..(pe - ps));
            }

            if !blend.is_empty() {
                pass.set_pipeline(&self.mesh_blend);
                draw_objects(&mut pass, &blend);
            }

            if !overlay.is_empty() {
                pass.set_pipeline(&self.mesh_overlay);
                draw_objects(&mut pass, &overlay);
            }
            let (ls, le) = line_ranges[1];
            if le > ls {
                pass.set_pipeline(&self.line_overlay);
                pass.set_vertex_buffer(
                    0,
                    self.line_buffer
                        .buffer
                        .slice(u64::from(ls) * stride_line..u64::from(le) * stride_line),
                );
                pass.draw(0..6, 0..(le - ls));
            }
            let (ps, pe) = point_ranges[1];
            if pe > ps {
                pass.set_pipeline(&self.point_overlay);
                pass.set_vertex_buffer(
                    0,
                    self.point_buffer
                        .buffer
                        .slice(u64::from(ps) * stride_point..u64::from(pe) * stride_point),
                );
                pass.draw(0..6, 0..(pe - ps));
            }
        }
        ctx.submit(Some(encoder.finish()));
    }
}
