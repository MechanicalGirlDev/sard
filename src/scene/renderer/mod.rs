//! Draws a [`Scene`] into any [`RenderTarget`] with shared pipelines.
//!
//! One pipeline per (primitive, blend / depth mode); per-object data lives in a single
//! dynamic-offset uniform buffer, so adding a thousand objects costs no pipeline or bind
//! group creation. Lines and points are re-uploaded every frame (they are the things that
//! change every tick in a telemetry viewer).

use core::mem::size_of;

use glam::{Mat4, Vec3, Vec4};

use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::render_states::ClearState;
use crate::core::render_target::RenderTarget;
use crate::renderer::viewer::Viewer;

use super::lines::LineInstance;
use super::scene::{Layer, Object, PointInstance, Scene, ScreenLabel};

mod draw;
mod gpu;
mod setup;
use gpu::{bind_uniform, FrameUniform, GridUniform, GrowBuffer, ObjectUniform};

/// How one [`SceneRenderer::render_with`] call uses the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Clear colour and depth to the scene background first. `false` draws over whatever
    /// is already in the target (a second scene composited on top).
    pub clear: bool,
    /// Restrict drawing to a pixel rectangle `[x, y, width, height]` of the target. The
    /// depth buffer inside the rectangle is reset, so the region behaves like a small
    /// independent viewport (an axis gizmo in a corner, a picture-in-picture view).
    /// Rectangles are intersected with the target; empty regions draw no content.
    pub region: Option<[u32; 4]>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            clear: true,
            region: None,
        }
    }
}

/// A prepared draw of one object.
struct ObjectDraw<'a> {
    object: &'a Object,
    offset: u32,
    /// Distance from the eye (for back-to-front sorting).
    depth: f32,
}

/// Renders [`Scene`]s.
pub struct SceneRenderer {
    format: wgpu::TextureFormat,
    frame_buffer: RawUniformBuffer,
    frame_bind_group: wgpu::BindGroup,
    object_layout: wgpu::BindGroupLayout,
    object_buffer: GrowBuffer,
    object_bind_group: wgpu::BindGroup,
    object_stride: u64,
    grid_buffer: RawUniformBuffer,
    grid_bind_group: wgpu::BindGroup,
    line_buffer: GrowBuffer,
    point_buffer: GrowBuffer,
    mesh_opaque: wgpu::RenderPipeline,
    mesh_blend: wgpu::RenderPipeline,
    mesh_overlay: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    line_overlay: wgpu::RenderPipeline,
    point: wgpu::RenderPipeline,
    point_overlay: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    /// Writes far depth over the current viewport without touching colour.
    depth_reset: wgpu::RenderPipeline,
}

impl SceneRenderer {
    /// The colour format the pipelines were built for.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Draw `scene` from `viewer` into `target`, clearing it to the scene background.
    /// Returns the projected labels for a text renderer to draw on top.
    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        target: &RenderTarget<'_>,
        viewer: &dyn Viewer,
        scene: &Scene,
    ) -> Vec<ScreenLabel> {
        self.render_with(ctx, target, viewer, scene, RenderOptions::default())
    }

    /// [`Self::render`] with explicit clear / region options. Label positions are in
    /// target pixels (the region offset is already applied).
    pub fn render_with(
        &mut self,
        ctx: &WgpuContext,
        target: &RenderTarget<'_>,
        viewer: &dyn Viewer,
        scene: &Scene,
        options: RenderOptions,
    ) -> Vec<ScreenLabel> {
        let [x, y, w, h] = options
            .region
            .unwrap_or_else(|| [0, 0, target.width(), target.height()]);
        let region = [
            x,
            y,
            w.min(target.width().saturating_sub(x)),
            h.min(target.height().saturating_sub(y)),
        ];
        if region[2] == 0 || region[3] == 0 {
            if options.clear {
                target.clear(ClearState::color_and_depth(scene.background, 1.0));
            }
            return Vec::new();
        }
        let options = RenderOptions {
            region: options.region.map(|_| region),
            ..options
        };
        let (width, height) = (region[2] as f32, region[3] as f32);
        let view = viewer.view_matrix();
        let proj = viewer.projection_matrix();
        let view_proj = proj * view;
        let eye = viewer.position();
        let inv_view = view.inverse();
        let cam_right = inv_view.x_axis.truncate().normalize_or(Vec3::X);
        let cam_up = inv_view.y_axis.truncate().normalize_or(Vec3::Y);
        let up = scene.up.normalize_or(Vec3::Y);
        let l = &scene.lighting;
        let sun_dir = (-l.sun_direction).normalize_or(up);
        let v4 = |v: Vec3| [v.x, v.y, v.z, 0.0];
        let c4 = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        self.frame_buffer.write(
            ctx,
            &FrameUniform {
                view_proj: view_proj.to_cols_array_2d(),
                view: view.to_cols_array_2d(),
                eye: [eye.x, eye.y, eye.z, 1.0],
                sun_dir: v4(sun_dir),
                sun_color: c4(l.sun_color),
                sky: c4(l.sky),
                ground: c4(l.ground),
                up: v4(up),
                cam_right: v4(cam_right),
                cam_up: v4(cam_up),
                viewport: [width, height, 0.0, 0.0],
            },
        );

        // ----- objects -------------------------------------------------------------
        let stride = self.object_stride as usize;
        let mut draws: Vec<ObjectDraw<'_>> = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        for (_, o) in scene.objects() {
            if !o.visible {
                continue;
            }
            let offset = bytes.len();
            bytes.resize(offset + stride, 0);
            let u = ObjectUniform {
                model: o.transform.to_cols_array_2d(),
                normal_matrix: o.transform.inverse().transpose().to_cols_array_2d(),
                color: o.surface.color,
                emissive: [
                    o.surface.emissive[0],
                    o.surface.emissive[1],
                    o.surface.emissive[2],
                    if o.surface.unlit { 1.0 } else { 0.0 },
                ],
            };
            bytes[offset..offset + size_of::<ObjectUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&u));
            let centre = o.world_aabb().center();
            draws.push(ObjectDraw {
                object: o,
                offset: offset as u32,
                depth: (centre - eye).length(),
            });
        }
        if self.object_buffer.write(ctx, &bytes) {
            self.object_bind_group = bind_uniform(
                ctx,
                &self.object_layout,
                &self.object_buffer.buffer,
                Some(size_of::<ObjectUniform>() as u64),
            );
        }

        // ----- lines / points --------------------------------------------------------
        let mut line_verts: Vec<LineInstance> = Vec::new();
        let mut line_ranges = [(0_u32, 0_u32); 2]; // [scene, overlay]
        for overlay in [false, true] {
            let start = line_verts.len() as u32;
            for set in scene.lines.iter().filter(|s| s.visible) {
                if (set.layer == Layer::Overlay) == overlay {
                    line_verts.extend(set.world_segments());
                }
            }
            line_ranges[usize::from(overlay)] = (start, line_verts.len() as u32);
        }
        let _ = self
            .line_buffer
            .write(ctx, bytemuck::cast_slice(&line_verts));

        let mut point_insts: Vec<PointInstance> = Vec::new();
        let mut point_ranges = [(0_u32, 0_u32); 2];
        for overlay in [false, true] {
            let start = point_insts.len() as u32;
            for set in scene.points.iter().filter(|s| s.visible) {
                if (set.layer == Layer::Overlay) == overlay {
                    point_insts.extend(set.world_points());
                }
            }
            point_ranges[usize::from(overlay)] = (start, point_insts.len() as u32);
        }
        let _ = self
            .point_buffer
            .write(ctx, bytemuck::cast_slice(&point_insts));

        // ----- grid ------------------------------------------------------------------
        if let Some(g) = &scene.grid {
            let axis_u = up.any_orthonormal_vector();
            let axis_v = up.cross(axis_u).normalize_or(Vec3::Y);
            // Prefer world X as the first axis when it is in-plane (Z-up / Y-up scenes).
            let (axis_u, axis_v) = if up.dot(Vec3::X).abs() < 1e-6 {
                (Vec3::X, up.cross(Vec3::X).normalize_or(axis_v))
            } else {
                (axis_u, axis_v)
            };
            let extent = if g.step > 0.0 {
                (g.half_extent / g.step).ceil() * g.step
            } else {
                g.half_extent
            };
            let origin = up * g.height;
            self.grid_buffer.write(
                ctx,
                &GridUniform {
                    origin: [origin.x, origin.y, origin.z, 1.0],
                    axis_u: v4(axis_u),
                    axis_v: v4(axis_v),
                    params: [g.step, g.major_every.max(1) as f32, extent, 0.0],
                    minor_color: g.minor_color,
                    major_color: g.major_color,
                    axis_u_color: g.axis_u_color,
                    axis_v_color: g.axis_v_color,
                },
            );
        }

        self.draw(
            target,
            draw::PreparedScene {
                scene,
                objects: draws,
                lines: line_ranges,
                points: point_ranges,
            },
            options,
        );

        super::primitives::project_labels(&scene.labels, viewer, region)
    }
}

/// Distance helper shared with tests: view-space depth of a world point.
#[allow(dead_code, reason = "kept for symmetry with the shader; used by tests")]
fn view_depth(view: Mat4, p: Vec3) -> f32 {
    -(view * Vec4::from((p, 1.0))).z
}
