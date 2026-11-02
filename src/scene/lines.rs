//! Screen-space line batches.

use super::Layer;
use crate::core::vertex::VertexPC;
use glam::{Mat4, Vec3};

/// One independently expanded screen-space segment; layout matches `scene_line.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LineInstance {
    pub start: [f32; 3],
    pub start_width: f32,
    pub end: [f32; 3],
    pub end_width: f32,
    pub start_color: [f32; 4],
    pub end_color: [f32; 4],
    pub outline: f32,
}

impl LineInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32,
        4 => Float32x4, 5 => Float32x4, 6 => Float32
    ];

    pub(super) const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: 68,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

const _: () = assert!(size_of::<LineInstance>() == 68);

/// A batch of line segments (or a polyline) with per-vertex colour.
#[derive(Debug, Clone)]
pub struct LineSet {
    /// Vertices in local space. Pairs for `strip == false`, consecutive for a strip.
    pub vertices: Vec<VertexPC>,
    /// Local → world.
    pub transform: Mat4,
    /// Draw layer.
    pub layer: Layer,
    /// Hidden sets are skipped.
    pub visible: bool,
    /// Treat `vertices` as a polyline instead of independent segments.
    pub strip: bool,
    /// Full start/end widths of each segment in physical target pixels, excluding
    /// the outline. Defaults to one pixel. Apply DPI scaling here, not in `transform`.
    /// Widths interpolate in screen space; zero produces a pointed endpoint.
    pub widths: [f32; 2],
    /// Black border thickness in physical pixels, outside the fill, on every edge.
    /// The border inherits vertex alpha. Defaults to zero.
    pub outline: f32,
}

impl LineSet {
    /// Independent segments (`vertices` in pairs).
    pub fn segments(vertices: Vec<VertexPC>) -> Self {
        Self {
            vertices,
            transform: Mat4::IDENTITY,
            layer: Layer::Scene,
            visible: true,
            strip: false,
            widths: [1.0; 2],
            outline: 0.0,
        }
    }

    /// A polyline through `points` in one colour.
    pub fn strip(points: &[Vec3], color: [f32; 4]) -> Self {
        Self {
            vertices: points
                .iter()
                .map(|p| VertexPC::new(p.to_array(), color))
                .collect(),
            transform: Mat4::IDENTITY,
            layer: Layer::Scene,
            visible: true,
            strip: true,
            widths: [1.0; 2],
            outline: 0.0,
        }
    }

    /// Builder: transform.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    /// Builder: layer.
    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }

    /// Builder: full endpoint widths in physical pixels. Negative widths become zero.
    /// Widths must be finite. A strip applies these widths to each constituent segment;
    /// its segments have independent butt caps (no joined miter geometry).
    pub fn with_widths(mut self, start: f32, end: f32) -> Self {
        self.widths = [start.max(0.0), end.max(0.0)];
        self
    }

    /// Builder: black outline thickness in physical pixels. Must be finite.
    pub fn with_outline(mut self, width: f32) -> Self {
        self.outline = width.max(0.0);
        self
    }

    /// Expand into world-space segments without scaling screen-space widths.
    pub(super) fn world_segments(&self) -> impl Iterator<Item = LineInstance> + '_ {
        let map = |v: &VertexPC| {
            self.transform
                .transform_point3(Vec3::from(v.position))
                .to_array()
        };
        self.vertices
            .windows(2)
            .step_by(if self.strip { 1 } else { 2 })
            .map(move |pair| LineInstance {
                start: map(&pair[0]),
                start_width: self.widths[0].max(0.0),
                end: map(&pair[1]),
                end_width: self.widths[1].max(0.0),
                start_color: pair[0].color,
                end_color: pair[1].color,
                outline: self.outline.max(0.0),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_expansion() {
        let strip = LineSet::strip(&[Vec3::ZERO, Vec3::X, Vec3::Y], [1.0; 4])
            .with_transform(Mat4::from_translation(Vec3::Z));
        let seg: Vec<_> = strip.world_segments().collect();
        assert_eq!(seg.len(), 2);
        assert_eq!(seg[0].end, [1.0, 0.0, 1.0]);
        assert_eq!(seg[1].start, [1.0, 0.0, 1.0]);

        let odd = LineSet::segments(vec![VertexPC::new([0.0; 3], [1.0; 4]); 3]);
        assert_eq!(odd.world_segments().count(), 1);
    }
}
