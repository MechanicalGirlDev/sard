//! Scene point sprites, annotations, grid and lighting parameters.

use super::Layer;
use glam::{Mat4, Vec3};

/// One camera-facing point sprite.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointInstance {
    /// Centre.
    pub position: [f32; 3],
    /// Diameter in world units.
    pub size: f32,
    /// RGBA.
    pub color: [f32; 4],
}

/// A batch of point sprites.
#[derive(Debug, Clone)]
pub struct PointSet {
    /// Points in local space.
    pub points: Vec<PointInstance>,
    /// Local → world.
    pub transform: Mat4,
    /// Draw layer.
    pub layer: Layer,
    /// Hidden sets are skipped.
    pub visible: bool,
}

impl PointSet {
    /// Points of one size and colour.
    pub fn uniform(positions: &[Vec3], size: f32, color: [f32; 4]) -> Self {
        Self {
            points: positions
                .iter()
                .map(|p| PointInstance {
                    position: p.to_array(),
                    size,
                    color,
                })
                .collect(),
            transform: Mat4::IDENTITY,
            layer: Layer::Scene,
            visible: true,
        }
    }

    /// Builder: transform.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub(crate) fn world_points(&self) -> Vec<PointInstance> {
        let scale = self
            .transform
            .to_scale_rotation_translation()
            .0
            .max_element();
        self.points
            .iter()
            .map(|p| PointInstance {
                position: self
                    .transform
                    .transform_point3(Vec3::from(p.position))
                    .to_array(),
                size: p.size * scale,
                color: p.color,
            })
            .collect()
    }
}

/// A text label anchored to a world position (drawn as a screen-space billboard).
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// Anchor in world space.
    pub position: Vec3,
    /// Text.
    pub text: String,
    /// Height of one text line in world units (perspective-scaled). `0` = fixed pixel size.
    pub height: f32,
    /// Pixel size used when `height == 0`.
    pub pixel_size: f32,
    /// RGBA.
    pub color: [f32; 4],
}

impl Label {
    /// A label `height` world units tall.
    pub fn new(position: Vec3, text: impl Into<String>, height: f32, color: [f32; 4]) -> Self {
        Self {
            position,
            text: text.into(),
            height,
            pixel_size: 14.0,
            color,
        }
    }
}

/// A label after projection to the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenLabel {
    /// Pixel x of the anchor (from the left).
    pub x: f32,
    /// Pixel y of the anchor (from the top).
    pub y: f32,
    /// Text height in pixels.
    pub size: f32,
    /// Text.
    pub text: String,
    /// RGBA.
    pub color: [f32; 4],
    /// Normalized depth (0 near … 1 far), for sorting.
    pub depth: f32,
}

/// Scene lighting: one directional "sun" plus a hemisphere ambient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// Direction the light travels **towards** (need not be normalized).
    pub sun_direction: Vec3,
    /// Sun colour × intensity.
    pub sun_color: [f32; 3],
    /// Ambient from the `up` hemisphere.
    pub sky: [f32; 3],
    /// Ambient from the opposite hemisphere.
    pub ground: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(-0.5, -0.3, -1.0),
            sun_color: [0.8; 3],
            sky: [0.55; 3],
            ground: [0.25; 3],
        }
    }
}

/// Ground grid on the plane perpendicular to the scene's `up` axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridParams {
    /// Offset of the plane along `up`.
    pub height: f32,
    /// The grid covers a square of ±`half_extent` (rounded up to a multiple of `step`).
    pub half_extent: f32,
    /// Cell size.
    pub step: f32,
    /// Every n-th line is a major line (`0` / `1` = all major).
    pub major_every: u32,
    /// Minor line colour.
    pub minor_color: [f32; 4],
    /// Major line colour.
    pub major_color: [f32; 4],
    /// Colour of the line along the first in-plane axis (world X for a Z-up scene).
    pub axis_u_color: [f32; 4],
    /// Colour of the line along the second in-plane axis (world Y for a Z-up scene).
    pub axis_v_color: [f32; 4],
}

impl Default for GridParams {
    fn default() -> Self {
        Self {
            height: 0.0,
            half_extent: 50.0,
            step: 1.0,
            major_every: 10,
            minor_color: [0.35, 0.35, 0.35, 0.5],
            major_color: [0.55, 0.55, 0.55, 0.8],
            axis_u_color: [0.9, 0.25, 0.25, 0.9],
            axis_v_color: [0.3, 0.85, 0.3, 0.9],
        }
    }
}

/// Project scene annotations into the same physical-pixel rectangle used for drawing.
pub(super) fn project_labels(
    labels: &[Label],
    viewer: &dyn crate::renderer::viewer::Viewer,
    region: [u32; 4],
) -> Vec<ScreenLabel> {
    let proj = viewer.projection_matrix();
    let view_proj = proj * viewer.view_matrix();
    let (ox, oy) = (region[0] as f32, region[1] as f32);
    let (width, height) = (region[2] as f32, region[3] as f32);
    let py = proj.y_axis.y.abs();
    labels
        .iter()
        .filter_map(|l| {
            let clip = view_proj * l.position.extend(1.0);
            if clip.w <= 0.0 {
                return None;
            }
            let ndc = clip.truncate() / clip.w;
            if !(-1.0..=1.0).contains(&ndc.z) {
                return None;
            }
            let size = if l.height > 0.0 {
                l.height * py * height / (2.0 * clip.w)
            } else {
                l.pixel_size
            };
            Some(ScreenLabel {
                x: ox + f32::midpoint(ndc.x, 1.0) * width,
                y: oy + (1.0 - ndc.y) * 0.5 * height,
                size,
                text: l.text.clone(),
                color: l.color,
                depth: ndc.z,
            })
        })
        .collect()
}
