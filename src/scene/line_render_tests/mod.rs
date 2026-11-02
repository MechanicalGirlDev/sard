//! Pixel-level contracts for screen-space line rendering on a real GPU.

use super::*;
use crate::context::WgpuContext;
use glam::{Mat4, Vec3};

mod clipping;

struct Fixture {
    ctx: WgpuContext,
    renderer: SceneRenderer,
    target: OffscreenTarget,
    camera: OrbitCamera,
    scene: Scene,
}

impl Fixture {
    fn new(size: u16) -> Self {
        let ctx = WgpuContext::new_blocking(None).unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let renderer = SceneRenderer::new(&ctx, format).unwrap();
        let target = OffscreenTarget::new(&ctx, u32::from(size), u32::from(size), format);
        let mut camera = OrbitCamera::new(Vec3::Z * 2.0, Vec3::ZERO, Vec3::Y);
        camera.fov_y = 90.0;
        camera.near = 0.1;
        camera.far = 10.0;
        camera.ortho = true;
        camera.set_viewport(0.0, 0.0, f32::from(size), f32::from(size));
        let mut scene = Scene::new();
        scene.background = [0.0, 0.0, 1.0, 1.0];
        Self {
            ctx,
            renderer,
            target,
            camera,
            scene,
        }
    }

    fn render(&mut self, options: RenderOptions) -> Vec<u8> {
        drop(self.renderer.render_with(
            &self.ctx,
            &self.target.target(&self.ctx),
            &self.camera,
            &self.scene,
            options,
        ));
        self.target.read_pixels(&self.ctx)
    }

    fn pixel(&self, image: &[u8], position: [u32; 2]) -> [u8; 4] {
        let offset =
            usize::try_from((position[1] * self.target.width() + position[0]) * 4).unwrap();
        image[offset..offset + 4].try_into().unwrap()
    }

    fn capture(&self, image: &[u8], name: &str) {
        let Some(directory) = std::env::var_os("SARD_SCENE_CAPTURE") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        // Top-down, uncompressed 32-bit BMP; the bytes come directly from GPU readback.
        let mut bmp = b"BM".to_vec();
        bmp.extend(u32::try_from(54 + image.len()).unwrap().to_le_bytes());
        bmp.extend([0; 4]);
        bmp.extend(54_u32.to_le_bytes());
        bmp.extend(40_u32.to_le_bytes());
        bmp.extend(self.target.width().to_le_bytes());
        bmp.extend((-i32::try_from(self.target.height()).unwrap()).to_le_bytes());
        bmp.extend(1_u16.to_le_bytes());
        bmp.extend(32_u16.to_le_bytes());
        bmp.extend([0; 24]);
        for pixel in image.as_chunks::<4>().0 {
            bmp.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        std::fs::write(directory.join(format!("{name}.bmp")), bmp).unwrap();
    }
}

fn line(widths: [f32; 2], outline: f32) -> LineSet {
    LineSet::strip(&[-Vec3::X, Vec3::X], [1.0, 0.0, 0.0, 1.0])
        .with_widths(widths[0], widths[1])
        .with_outline(outline)
}

#[test]
fn physical_width_survives_zoom_transform_resolution_and_dpi() {
    for (size, dpi, ortho, yaw) in [
        (128, 1_u16, true, 0.0),
        (256, 2, true, 0.0),
        (128, 1, false, 0.0),
        (128, 1, false, -0.6),
        (128, 1, false, 0.6),
    ] {
        let mut f = Fixture::new(size);
        f.camera.ortho = ortho;
        f.camera.eye *= 1.5;
        f.camera.orbit(yaw, 0.0);
        f.scene.lines.push(
            line([8.0 * f32::from(dpi); 2], 0.0).with_transform(Mat4::from_scale(Vec3::splat(1.5))),
        );
        let image = f.render(RenderOptions::default());
        f.capture(&image, &format!("width-{size}-{ortho}-{yaw}"));
        let red_rows: Vec<_> = (0..u32::from(size))
            .filter(|y| f.pixel(&image, [u32::from(size / 2), *y])[0] > 200)
            .collect();
        let center = u32::from(size / 2);
        let half = u32::from(4 * dpi);
        assert_eq!(red_rows, (center - half..center + half).collect::<Vec<_>>());
    }
}

#[test]
fn tapered_fill_has_black_border_and_a_pointed_tip() {
    let mut f = Fixture::new(128);
    f.scene.lines.push(line([24.0, 0.0], 2.0));
    let image = f.render(RenderOptions::default());
    f.capture(&image, "taper");
    assert_eq!(f.pixel(&image, [48, 64]), [255, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [48, 73]), [0, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [48, 78]), [0, 0, 255, 255]);
    assert_eq!(f.pixel(&image, [88, 70]), [0, 0, 255, 255]);
    assert_eq!(f.pixel(&image, [30, 64]), [0, 0, 0, 255]);
}

#[test]
fn half_pixel_gui_outline_scales_with_dpi() {
    for (scale, expected_border, tolerance) in [(1_u16, 128, 1), (2, 0, 0)] {
        let mut f = Fixture::new(128 * scale);
        f.scene
            .lines
            .push(line([2.0 * f32::from(scale); 2], 0.5 * f32::from(scale)));
        let image = f.render(RenderOptions::default());
        f.capture(&image, &format!("gui-outline-{scale}"));
        let center = u32::from(64 * scale);
        let edge = center + u32::from(scale);
        assert_eq!(f.pixel(&image, [center, edge - 1]), [255, 0, 0, 255]);
        let [red, green, blue, alpha] = f.pixel(&image, [center, edge]);
        assert_eq!([red, green, alpha], [0, 0, 255]);
        // Half coverage can round by one UNORM unit; opaque black stays exact.
        assert!(blue.abs_diff(expected_border) <= tolerance, "blue = {blue}");
    }
}

#[test]
fn transparent_outline_inherits_the_segment_alpha() {
    let mut f = Fixture::new(128);
    let mut shape = line([12.0; 2], 3.0);
    for vertex in &mut shape.vertices {
        vertex.color[3] = 0.25;
    }
    f.scene.lines.push(shape);
    let image = f.render(RenderOptions::default());
    f.capture(&image, "alpha");
    assert_eq!(f.pixel(&image, [64, 64]), [64, 0, 191, 255]);
    assert_eq!(f.pixel(&image, [64, 71]), [0, 0, 191, 255]);
    assert_eq!(f.pixel(&image, [64, 76]), [0, 0, 255, 255]);
}

#[test]
fn endpoint_colors_interpolate_perspectively_without_outline_padding() {
    let mut f = Fixture::new(128);
    f.camera.ortho = false;
    f.scene.lines.push(
        LineSet::segments(vec![
            crate::core::vertex::VertexPC::new([-0.5, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]),
            crate::core::vertex::VertexPC::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0, 1.0]),
        ])
        .with_widths(8.0, 8.0)
        .with_outline(10.0),
    );
    let image = f.render(RenderOptions::default());
    f.capture(&image, "gradient");
    // Endpoints project to x=32 and 96, with homogeneous w=1 and 2.
    // At pixel centre 40.5 the perspective-correct fraction is 17/239.
    let pixel = f.pixel(&image, [40, 64]);
    assert!(
        (235..=239).contains(&pixel[0]) && (16..=20).contains(&pixel[1]),
        "{pixel:?}"
    );
}

#[test]
fn empty_quad_pixels_leave_depth_for_later_geometry() {
    let mut f = Fixture::new(128);
    f.scene
        .lines
        .push(line([24.0, 0.0], 2.0).with_transform(Mat4::from_translation(Vec3::Z * 0.5)));
    let mut behind = line([2.0; 2], 0.0).with_transform(Mat4::from_translation(Vec3::Y * -0.25));
    for vertex in &mut behind.vertices {
        vertex.color = [0.0, 1.0, 0.0, 1.0];
    }
    f.scene.lines.push(behind);
    let image = f.render(RenderOptions::default());
    f.capture(&image, "depth");
    assert_eq!(f.pixel(&image, [40, 72]), [255, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [80, 72]), [0, 255, 0, 255]);
}

#[test]
fn outline_padding_does_not_shift_segment_depth() {
    let mut f = Fixture::new(128);
    f.scene.lines.push(
        LineSet::strip(
            &[Vec3::new(-1.0, 0.0, 0.5), Vec3::new(1.0, 0.0, -0.5)],
            [1.0, 0.0, 0.0, 1.0],
        )
        .with_widths(8.0, 8.0)
        .with_outline(10.0),
    );
    f.scene.lines.push(
        LineSet::strip(
            &[Vec3::new(-1.0, 0.0, 0.3), Vec3::new(1.0, 0.0, 0.3)],
            [0.0, 1.0, 0.0, 1.0],
        )
        .with_widths(4.0, 4.0),
    );
    let image = f.render(RenderOptions::default());
    assert_eq!(f.pixel(&image, [40, 64]), [255, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [88, 64]), [0, 255, 0, 255]);
}

#[test]
fn grown_buffers_keep_layer_ranges_and_drop_stale_instances() {
    let mut f = Fixture::new(128);
    f.scene.lines =
        vec![line([4.0; 2], 0.0).with_transform(Mat4::from_translation(Vec3::Y * 10.0)); 1025];
    let mut foreground = line([16.0; 2], 0.0).with_transform(Mat4::from_translation(Vec3::Z));
    for vertex in &mut foreground.vertices {
        vertex.color = [0.0, 1.0, 0.0, 1.0];
    }
    f.scene.lines.push(foreground);
    f.scene
        .lines
        .push(line([8.0; 2], 0.0).with_layer(Layer::Overlay));
    let mut hidden = line([24.0; 2], 0.0).with_layer(Layer::Overlay);
    hidden.visible = false;
    f.scene.lines.push(hidden);
    let image = f.render(RenderOptions::default());
    assert_eq!(f.pixel(&image, [64, 64]), [255, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [64, 70]), [0, 255, 0, 255]);
    assert_eq!(f.pixel(&image, [64, 74]), [0, 0, 255, 255]);

    f.scene.lines.clear();
    let empty = f.render(RenderOptions::default());
    assert!(empty
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| *pixel == [0, 0, 255, 255]));
}
