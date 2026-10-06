//! Fog Effect - Post-processing fog demonstration
//!
//! Shows the FogEffect applied to a scene with depth-based fog.
//!
//! Run with: cargo run

use glam::Vec3;
use sard::{
    Camera, ColorMaterial, DepthTexture, FogEffect, FrameOutput, Gm, Light, Mesh, Object,
    OrbitControl, Texture2D, Window, WindowSettings,
};

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("Fog Effect")
            .size(1024, 768),
    )?;

    struct State {
        camera: Camera,
        control: OrbitControl,
        spheres: Vec<Gm<Mesh, ColorMaterial>>,
        fog: Option<FogEffect>,
        color_texture: Option<Texture2D>,
        depth_texture: Option<DepthTexture>,
        initialized: bool,
    }

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(0.0, 3.0, 8.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.33,
            0.1,
            100.0,
        ),
        control: OrbitControl::new(Vec3::ZERO, 2.0, 50.0),
        spheres: Vec::new(),
        fog: None,
        color_texture: None,
        depth_texture: None,
        initialized: false,
    };

    window.render_loop(state, |state, frame| {
        if !state.initialized {
            // Create a grid of spheres extending into the distance
            for z in 0..10 {
                for x in -3..=3 {
                    let material = ColorMaterial::new(frame.ctx, frame.surface_format)
                        .expect("Failed to create material");
                    let mesh = Mesh::sphere(frame.ctx, 0.4, 16, 12, [0.8, 0.3, 0.3]);
                    let sphere = Gm::new(mesh, material).with_position(
                        x as f32 * 2.0,
                        0.0,
                        -(z as f32) * 3.0,
                    );
                    state.spheres.push(sphere);
                }
            }

            // Create fog effect
            let mut fog = FogEffect::new(frame.ctx, frame.surface_format)
                .expect("Failed to create fog effect");
            fog.color = Vec3::new(0.5, 0.6, 0.7);
            fog.start = 5.0;
            fog.end = 30.0;
            state.fog = Some(fog);

            state.initialized = true;
        }

        state.camera.set_viewport(frame.viewport);
        let mut events = frame.events.clone();
        state.control.handle_events(&mut state.camera, &mut events);

        let vp = frame.viewport;

        // Ensure offscreen textures exist
        let needs_resize = state.color_texture.as_ref().is_none_or(|t| {
            let (w, h) = t.size();
            w != vp.width || h != vp.height
        });

        if needs_resize {
            state.color_texture = Some(Texture2D::new(
                frame.ctx,
                vp.width,
                vp.height,
                frame.surface_format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                Some("fog color texture"),
            ));
            state.depth_texture = Some(DepthTexture::new(
                frame.ctx,
                vp.width,
                vp.height,
                Some("fog depth texture"),
            ));
        }

        let color_tex = state.color_texture.as_ref().unwrap();
        let depth_tex = state.depth_texture.as_ref().unwrap();

        let mut encoder = frame.ctx.create_encoder(Some("main encoder"));

        // Render scene to offscreen textures
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_tex.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.5,
                            g: 0.6,
                            b: 0.7,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_tex.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            let lights: Vec<&dyn Light> = vec![];
            for sphere in &state.spheres {
                sphere.render(frame.ctx, &state.camera, &lights, &mut pass);
            }
        }

        // Apply fog effect
        if let Some(fog) = &state.fog {
            fog.update_uniform(frame.ctx, 0.1, 100.0);
            fog.apply_with_depth(
                frame.ctx,
                &mut encoder,
                color_tex.view(),
                depth_tex.view(),
                frame.surface_view,
            );
        }

        frame.ctx.submit([encoder.finish()]);

        FrameOutput::default()
    })
}
