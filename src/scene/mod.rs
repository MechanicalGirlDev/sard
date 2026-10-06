//! Retained scene layer: a data-only [`Scene`] drawn by a [`SceneRenderer`].
//!
//! Compared with the `renderer` module's `Gm<Geometry, Material>` objects, this layer
//! shares one pipeline per primitive kind across every object, keeps CPU mesh data for
//! exact picking, re-uploads lines / points each frame, and renders into any
//! [`crate::RenderTarget`] (a window, an [`OffscreenTarget`], an XR eye). It exists for
//! viewers that rebuild hundreds of objects from telemetry every tick.
//!
//! ```no_run
//! use sard::scene::{MeshAsset, MeshData, Object, OrbitCamera, Scene, SceneRenderer, Surface};
//! use sard::glam::Vec3;
//! # fn main() -> anyhow::Result<()> {
//! # let ctx = sard::WgpuContext::new_blocking(None)?;
//! let mut scene = Scene::new_z_up();
//! let cube = MeshAsset::shared(&ctx, MeshData::cuboid(Vec3::ONE), Some("cube"));
//! scene.add(Object::new(cube).with_surface(Surface::rgb(0.9, 0.2, 0.2)));
//! let mut renderer = SceneRenderer::new(&ctx, wgpu::TextureFormat::Rgba8UnormSrgb)?;
//! let camera = OrbitCamera::new(Vec3::new(3.0, -3.0, 2.0), Vec3::ZERO, Vec3::Z);
//! # let target = sard::scene::OffscreenTarget::new(&ctx, 64, 64, renderer.format());
//! let labels = renderer.render(&ctx, &target.target(&ctx), &camera, &scene);
//! # Ok(()) }
//! ```

mod camera;
mod mesh;
mod offscreen;
mod pick;
mod renderer;
#[allow(clippy::module_inception, reason = "`scene::Scene` reads naturally")]
mod scene;

pub use camera::OrbitCamera;
pub use mesh::{MeshAsset, MeshData, MeshHandle, ObjError};
pub use offscreen::OffscreenTarget;
pub use pick::Ray;
pub use renderer::{RenderOptions, SceneRenderer};
pub use scene::{
    GridParams, Hit, Label, Layer, Lighting, LineSet, Object, ObjectId, PointInstance, PointSet,
    Scene, ScreenLabel, Surface,
};

/// Draw projected labels with a [`crate::gui::TextRenderer`], centred on their anchors.
#[cfg(feature = "gui")]
pub fn draw_labels(text: &mut crate::gui::TextRenderer, labels: &[ScreenLabel]) {
    for l in labels {
        let (w, h) = text.measure(&l.text, l.size);
        text.draw_text(&l.text, l.x - w * 0.5, l.y - h * 0.5, l.size, l.color);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::print_stderr, reason = "tests report when no GPU is available")]

    use glam::{Mat4, Vec3};

    use super::*;
    use crate::context::WgpuContext;
    use crate::renderer::viewer::Viewer;

    fn gpu() -> Option<WgpuContext> {
        match WgpuContext::new_blocking(None) {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                None
            }
        }
    }

    fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    /// Renders a red cube into an off-screen target and checks the centre pixel is red
    /// while a corner keeps the background; also exercises picking on the same scene.
    #[test]
    fn offscreen_cube_is_visible_and_pickable() {
        let Some(ctx) = gpu() else { return };
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = SceneRenderer::new(&ctx, format).unwrap();
        let (w, h) = (64, 64);
        let target = OffscreenTarget::new(&ctx, w, h, format);

        let mut scene = Scene::new_z_up();
        scene.background = [0.0, 0.0, 1.0, 1.0];
        scene.grid = Some(GridParams::default());
        let cube = MeshAsset::shared(&ctx, MeshData::cuboid(Vec3::ONE), Some("cube"));
        let id = scene.add(
            Object::new(cube)
                .with_surface(Surface {
                    color: [1.0, 0.0, 0.0, 1.0],
                    unlit: true,
                    ..Surface::default()
                })
                .with_pick_id(7),
        );
        scene.lines.push(LineSet::strip(
            &[Vec3::ZERO, Vec3::Z * 2.0],
            [0.0, 1.0, 0.0, 1.0],
        ));
        scene
            .points
            .push(PointSet::uniform(&[Vec3::X * 2.0], 0.2, [1.0; 4]));
        scene
            .labels
            .push(Label::new(Vec3::Z * 0.6, "cube", 0.2, [1.0; 4]));

        let mut camera = OrbitCamera::new(Vec3::new(0.0, -4.0, 0.0), Vec3::ZERO, Vec3::Z);
        camera.set_viewport(0.0, 0.0, w as f32, h as f32);
        let labels = renderer.render(&ctx, &target.target(&ctx), &camera, &scene);
        assert_eq!(labels.len(), 1);
        assert!(labels[0].size > 1.0);

        let px = target.read_pixels(&ctx);
        assert_eq!(px.len(), (w * h * 4) as usize);
        let centre = pixel(&px, w, w / 2, h / 2);
        assert!(centre[0] > 200 && centre[2] < 50, "centre = {centre:?}");
        let corner = pixel(&px, w, 1, 1);
        assert!(corner[2] > 200 && corner[0] < 50, "corner = {corner:?}");

        // Picking through the centre hits the cube's front face (y = -0.5).
        let hit = scene
            .pick(&camera.screen_ray(w as f32 / 2.0, h as f32 / 2.0))
            .unwrap();
        assert_eq!(hit.object, id);
        assert_eq!(hit.pick_id, 7);
        assert!((hit.point.y + 0.5).abs() < 1e-3, "{hit:?}");
        assert!(scene.pick(&camera.screen_ray(1.0, 1.0)).is_none());

        // Hidden objects are neither drawn nor picked; the slot is reusable.
        scene.get_mut(id).unwrap().visible = false;
        assert!(scene
            .pick(&camera.screen_ray(w as f32 / 2.0, h as f32 / 2.0))
            .is_none());
        drop(renderer.render(&ctx, &target.target(&ctx), &camera, &scene));
        let px = target.read_pixels(&ctx);
        let centre = pixel(&px, w, w / 2, h / 2);
        assert!(centre[0] < 50, "hidden cube still drawn: {centre:?}");
        assert!(scene.remove(id).is_some());
        assert!(scene.is_empty());
        let again = scene.add(Object::new(MeshAsset::shared(
            &ctx,
            MeshData::sphere(0.5, 8, 6),
            None,
        )));
        assert_eq!(again, id);
        assert_eq!(scene.len(), 1);

        // A transparent object goes through the blend path without panicking, and an
        // object count beyond the initial uniform capacity grows the buffer.
        for i in 0..300 {
            let m = MeshAsset::shared(&ctx, MeshData::cuboid(Vec3::splat(0.1)), None);
            scene.add(
                Object::new(m)
                    .with_transform(Mat4::from_translation(Vec3::X * (i as f32)))
                    .with_surface(Surface::rgba([0.0, 1.0, 0.0, 0.5])),
            );
        }
        drop(renderer.render(&ctx, &target.target(&ctx), &camera, &scene));
        assert!(camera.view_matrix().is_finite());
    }
}
