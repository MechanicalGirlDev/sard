use sard::glam::{Mat4, Vec3};
use sard::renderer::geometry::MeshData;
use sard::scene::OffscreenTarget;
use sard::{
    wgpu, Camera, RenderConfig, RenderError, RenderLighting, RenderMode, RenderObject, Renderer,
    SurfaceMaterial, WgpuContext,
};
use std::time::Duration;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const MODES: [RenderMode; 3] = [
    RenderMode::Rasterization,
    RenderMode::RayTracing,
    RenderMode::PathTracing,
];

fn context() -> WgpuContext {
    WgpuContext::new_blocking(None).expect("render-mode tests require a real GPU adapter")
}

fn renderer(ctx: &WgpuContext) -> Renderer {
    Renderer::new(
        ctx,
        FORMAT,
        RenderConfig {
            shadow_resolution: 512,
            ..RenderConfig::default()
        },
    )
    .expect("render backends")
}

fn camera() -> Camera {
    Camera::new_orthographic(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
        4.0,
        4.0,
        0.1,
        10.0,
    )
}

fn plane(half: f32) -> MeshData {
    MeshData {
        positions: vec![
            Vec3::new(-half, -half, 0.0),
            Vec3::new(half, -half, 0.0),
            Vec3::new(half, half, 0.0),
            Vec3::new(-half, half, 0.0),
        ],
        normals: vec![Vec3::Z; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn object(mesh: &MeshData, transform: Mat4, material: SurfaceMaterial) -> RenderObject<'_> {
    RenderObject {
        mesh,
        transform,
        material,
    }
}

fn read_texture(
    ctx: &WgpuContext,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    aspect: wgpu::TextureAspect,
) -> Vec<u8> {
    let row_bytes = width * 4;
    let padded =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bounded mode-test readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.create_encoder(Some("mode-test readback"));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    ctx.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).expect("readback receiver");
    });
    ctx.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(30)),
        })
        .expect("bounded GPU completion");
    rx.recv_timeout(Duration::from_secs(30))
        .expect("map event")
        .expect("mapped buffer");
    let data = slice.get_mapped_range().expect("mapped bytes");
    let bytes = data
        .chunks_exact(padded as usize)
        .flat_map(|row| row[..row_bytes as usize].iter().copied())
        .collect();
    drop(data);
    staging.unmap();
    bytes
}

fn pixels(ctx: &WgpuContext, target: &OffscreenTarget) -> Vec<u8> {
    read_texture(
        ctx,
        target.texture(),
        target.width(),
        target.height(),
        wgpu::TextureAspect::All,
    )
}

fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    bytes[index..index + 4].try_into().expect("RGBA")
}

#[test]
fn all_modes_render_the_same_scene_and_switch_back_without_stale_pixels() {
    // Given: one uploaded red emissive mesh against a blue environment.
    let ctx = context();
    let target = OffscreenTarget::new(&ctx, 33, 33, FORMAT);
    let mut renderer = renderer(&ctx);
    let mesh = plane(0.7);
    renderer
        .set_scene(&[object(
            &mesh,
            Mat4::IDENTITY,
            SurfaceMaterial {
                base_color: [0.0; 3],
                emissive: [2.0, 0.0, 0.0],
                ..SurfaceMaterial::default()
            },
        )])
        .expect("scene");
    renderer
        .set_lighting(RenderLighting {
            sun_color: [0.0; 3],
            environment: [0.0, 0.0, 0.2],
            ..RenderLighting::default()
        })
        .expect("lighting");

    // When: the same renderer switches among all algorithms and back.
    let mut captures = Vec::new();
    for mode in MODES
        .into_iter()
        .chain([RenderMode::Rasterization, RenderMode::RayTracing])
    {
        renderer.set_mode(mode);
        renderer
            .render(&camera(), &target.target(&ctx))
            .expect("render mode");
        captures.push(pixels(&ctx, &target));
    }

    // Then: geometry/background remain present; deterministic modes replay exactly.
    for frame in &captures {
        let center = pixel(frame, 33, 16, 16);
        assert!(center[0] > 160 && center[1] < 10, "{center:?}");
        let corner = pixel(frame, 33, 0, 0);
        assert!(corner[2] > 35 && corner[0] < 10, "{corner:?}");
    }
    assert_eq!(captures[0], captures[3]);
    assert_eq!(captures[1], captures[4]);
    assert_eq!(renderer.accumulated_samples(), 0);
}

#[test]
fn raster_shadow_maps_and_ray_visibility_occlude_the_directional_light() {
    // Given: a ground plane and a blocker outside the center camera ray.
    let ctx = context();
    let target = OffscreenTarget::new(&ctx, 65, 65, FORMAT);
    let mut renderer = renderer(&ctx);
    let ground = plane(4.0);
    let blocker = MeshData::cuboid(Vec3::splat(0.6));
    let light = RenderLighting {
        sun_direction: Vec3::new(1.0, 0.0, -1.0),
        sun_color: [3.0; 3],
        environment: [0.0; 3],
    };
    renderer.set_lighting(light).expect("sun");
    renderer.set_max_bounces(1).expect("direct only");
    let surface = SurfaceMaterial::default();
    for mode in MODES {
        renderer.set_mode(mode);
        renderer
            .set_scene(&[object(&ground, Mat4::IDENTITY, surface)])
            .expect("unoccluded");
        renderer
            .render(&camera(), &target.target(&ctx))
            .expect("lit frame");
        let unoccluded = pixels(&ctx, &target);
        let lit = pixel(&unoccluded, 65, 32, 32)[0];
        for x in 8..57 {
            let value = pixel(&unoccluded, 65, x, 32)[0];
            assert!(
                value.abs_diff(lit) <= 2,
                "{mode:?}: self-shadow stripe at {x}: {value} vs {lit}"
            );
        }

        // When: photons to the central receiver are blocked.
        renderer
            .set_scene(&[
                object(&ground, Mat4::IDENTITY, surface),
                object(
                    &blocker,
                    Mat4::from_translation(Vec3::new(-1.0, 0.0, 1.0)),
                    surface,
                ),
            ])
            .expect("occluder");
        renderer
            .render(&camera(), &target.target(&ctx))
            .expect("shadow frame");

        // Then: every backend actually evaluates shadow visibility.
        let shadow = pixel(&pixels(&ctx, &target), 65, 32, 32)[0];
        assert!(
            lit > 60 && u16::from(shadow) + 40 < u16::from(lit),
            "{mode:?}: lit={lit}, shadow={shadow}"
        );
    }
}

#[test]
fn ray_reflections_reach_an_emitter_behind_the_camera() {
    // Given: a white mirror and an emitter invisible to primary rays.
    let ctx = context();
    let target = OffscreenTarget::new(&ctx, 33, 33, FORMAT);
    let mut renderer = renderer(&ctx);
    let mesh = plane(4.0);
    renderer.set_mode(RenderMode::RayTracing);
    renderer
        .set_lighting(RenderLighting {
            sun_color: [0.0; 3],
            environment: [0.0; 3],
            ..RenderLighting::default()
        })
        .expect("dark environment");
    renderer
        .set_scene(&[
            object(
                &mesh,
                Mat4::IDENTITY,
                SurfaceMaterial {
                    metallic: 1.0,
                    roughness: 0.0,
                    ..SurfaceMaterial::default()
                },
            ),
            object(
                &mesh,
                Mat4::from_translation(Vec3::Z * 6.0),
                SurfaceMaterial {
                    emissive: [2.0, 0.0, 0.0],
                    ..SurfaceMaterial::default()
                },
            ),
        ])
        .expect("mirror scene");
    renderer.set_max_bounces(1).expect("one bounce");
    renderer
        .render(&camera(), &target.target(&ctx))
        .expect("direct only");
    let direct = pixels(&ctx, &target);

    // When: a secondary reflection bounce is enabled.
    renderer.set_max_bounces(2).expect("reflection");
    renderer
        .render(&camera(), &target.target(&ctx))
        .expect("reflected frame");

    // Then: red comes from a real secondary intersection, not screen-space data.
    assert!(pixel(&direct, 33, 16, 16)[0] < 5);
    assert!(pixel(&pixels(&ctx, &target), 33, 16, 16)[0] > 160);
}

#[test]
fn path_tracing_transports_diffuse_emission_and_replays_its_seed() {
    // Given: a diffuse receiver and a large overhead emissive surface.
    let ctx = context();
    let target = OffscreenTarget::new(&ctx, 17, 17, FORMAT);
    let mut renderer = renderer(&ctx);
    let ground = plane(3.0);
    let emitter = plane(20.0);
    renderer.set_mode(RenderMode::PathTracing);
    renderer
        .set_lighting(RenderLighting {
            sun_color: [0.0; 3],
            environment: [0.0; 3],
            ..RenderLighting::default()
        })
        .expect("emission only");
    renderer
        .set_scene(&[
            object(&ground, Mat4::IDENTITY, SurfaceMaterial::default()),
            object(
                &emitter,
                Mat4::from_translation(Vec3::Z * 6.0),
                SurfaceMaterial {
                    emissive: [1.0, 0.0, 0.0],
                    ..SurfaceMaterial::default()
                },
            ),
        ])
        .expect("diffuse scene");
    renderer.set_max_bounces(1).expect("no secondary transport");
    renderer
        .render(&camera(), &target.target(&ctx))
        .expect("direct frame");
    assert!(pixel(&pixels(&ctx, &target), 17, 8, 8)[0] < 5);
    renderer.set_max_bounces(3).expect("indirect transport");

    // When: stochastic paths accumulate and the same seed sequence is restarted.
    for _ in 0..64 {
        renderer
            .render(&camera(), &target.target(&ctx))
            .expect("path sample");
    }
    let indirect = pixels(&ctx, &target);
    assert_eq!(renderer.accumulated_samples(), 64);
    renderer.reset_accumulation();
    for _ in 0..64 {
        renderer
            .render(&camera(), &target.target(&ctx))
            .expect("replayed sample");
    }

    // Then: diffuse GI reaches the receiver and scheduling does not change the image.
    assert!(pixel(&indirect, 17, 8, 8)[0] > 70);
    assert_eq!(indirect, pixels(&ctx, &target));
}

#[test]
fn accumulation_resets_on_camera_scene_lighting_size_and_mode_changes() {
    // Given: two accumulated samples.
    let ctx = context();
    let target = OffscreenTarget::new(&ctx, 17, 17, FORMAT);
    let mut renderer = renderer(&ctx);
    renderer.set_mode(RenderMode::PathTracing);
    let mut camera = camera();
    for _ in 0..2 {
        renderer
            .render(&camera, &target.target(&ctx))
            .expect("sample");
    }
    assert_eq!(renderer.accumulated_samples(), 2);

    // When: each input that changes the transport problem changes.
    camera.position.x += 0.1;
    renderer
        .render(&camera, &target.target(&ctx))
        .expect("moved camera");
    assert_eq!(renderer.accumulated_samples(), 1);
    renderer
        .set_lighting(RenderLighting {
            environment: [0.1; 3],
            ..RenderLighting::default()
        })
        .expect("changed light");
    assert_eq!(renderer.accumulated_samples(), 0);
    renderer
        .render(&camera, &target.target(&ctx))
        .expect("sample");
    renderer.set_max_bounces(3).expect("changed bounces");
    assert_eq!(renderer.accumulated_samples(), 0);
    renderer
        .render(&camera, &target.target(&ctx))
        .expect("sample");
    renderer.set_scene(&[]).expect("new scene");
    assert_eq!(renderer.accumulated_samples(), 0);
    renderer
        .render(&camera, &target.target(&ctx))
        .expect("sample");
    let resized = OffscreenTarget::new(&ctx, 23, 19, FORMAT);
    renderer
        .render(&camera, &resized.target(&ctx))
        .expect("resized target");
    assert_eq!(renderer.accumulated_samples(), 1);
    renderer.set_mode(RenderMode::RayTracing);
    renderer
        .render(&camera, &resized.target(&ctx))
        .expect("ray frame");
    assert_eq!(renderer.accumulated_samples(), 0);
    renderer.set_mode(RenderMode::PathTracing);
    renderer
        .render(&camera, &resized.target(&ctx))
        .expect("fresh path frame");

    // Then: no samples from the former camera/scene/algorithm remain.
    assert_eq!(renderer.accumulated_samples(), 1);
    let frame = pixels(&ctx, &resized);
    assert_eq!(pixel(&frame, 23, 0, 0), pixel(&frame, 23, 22, 18));
}

#[test]
fn invalid_scene_preserves_the_previous_snapshot() {
    // Given: a valid red emissive snapshot.
    let ctx = context();
    let target = OffscreenTarget::new(&ctx, 17, 17, FORMAT);
    let mut renderer = renderer(&ctx);
    let mut mesh = plane(2.0);
    let material = SurfaceMaterial {
        emissive: [1.0, 0.0, 0.0],
        ..SurfaceMaterial::default()
    };
    renderer
        .set_scene(&[object(&mesh, Mat4::IDENTITY, material)])
        .expect("scene");
    renderer.set_mode(RenderMode::RayTracing);
    renderer
        .render(&camera(), &target.target(&ctx))
        .expect("original");
    let before = pixels(&ctx, &target);

    // When: an upload contains an invalid index, normal or transform.
    mesh.indices[0] = 99;
    assert!(matches!(
        renderer.set_scene(&[object(&mesh, Mat4::IDENTITY, material)]),
        Err(RenderError::Scene { .. })
    ));
    mesh.indices[0] = 0;
    mesh.normals[0] = Vec3::ZERO;
    assert!(renderer
        .set_scene(&[object(&mesh, Mat4::IDENTITY, material)])
        .is_err());
    assert!(renderer
        .set_scene(&[object(&mesh, Mat4::ZERO, material)])
        .is_err());
    assert!(renderer.set_max_bounces(0).is_err());
    assert!(renderer
        .set_lighting(RenderLighting {
            sun_direction: Vec3::ZERO,
            ..RenderLighting::default()
        })
        .is_err());

    // Then: failed validation does not partially replace the uploaded scene.
    renderer
        .render(&camera(), &target.target(&ctx))
        .expect("preserved scene");
    assert_eq!(before, pixels(&ctx, &target));
}

#[test]
fn all_modes_honor_clipping_and_write_primary_depth() {
    // Given: explicit depth attachment readable by the test.
    let ctx = context();
    let color = OffscreenTarget::new(&ctx, 17, 17, FORMAT);
    let depth = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mode test depth"),
        size: wgpu::Extent3d {
            width: 17,
            height: 17,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: sard::DepthTexture::FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let target = sard::RenderTarget::new(&ctx, color.view(), Some(&view), 17, 17, FORMAT);
    let mut renderer = renderer(&ctx);
    let mesh = plane(10.0);
    let cameras = [
        camera(),
        Camera::new_perspective(Vec3::Z * 5.0, Vec3::ZERO, Vec3::Y, 45.0, 1.0, 0.1, 10.0),
    ];

    // When: visible and clipped surfaces are uploaded in every mode.
    for (camera, mode) in cameras
        .iter()
        .flat_map(|camera| MODES.map(|mode| (camera, mode)))
    {
        renderer.set_mode(mode);
        for (z, visible) in [(0.0, true), (5.5, false), (-6.0, false)] {
            renderer
                .set_scene(&[object(
                    &mesh,
                    Mat4::from_translation(Vec3::Z * z),
                    SurfaceMaterial {
                        emissive: [1.0, 0.0, 0.0],
                        ..SurfaceMaterial::default()
                    },
                )])
                .expect("depth scene");
            renderer.render(camera, &target).expect("depth frame");

            // Then: near/far clipped geometry stays background, visible geometry writes NDC depth.
            let bytes = read_texture(&ctx, &depth, 17, 17, wgpu::TextureAspect::DepthOnly);
            let center = (8 * 17 + 8) * 4;
            let actual = f32::from_le_bytes(bytes[center..center + 4].try_into().expect("depth"));
            let expected = if visible {
                let projected =
                    sard::renderer::Viewer::view_projection_matrix(camera) * Vec3::ZERO.extend(1.0);
                projected.z / projected.w
            } else {
                1.0
            };
            assert!(
                (actual - expected).abs() < 1e-4,
                "{mode:?} z={z}: {actual}, {camera:?}"
            );
        }
    }
}

fn capture(name: &str, width: u32, height: u32, rgba: &[u8]) {
    if let Ok(directory) = std::env::var("SARD_RENDER_CAPTURE") {
        std::fs::create_dir_all(&directory).expect("capture directory");
        let size = width * height * 4;
        let mut bmp = Vec::with_capacity((54 + size) as usize);
        bmp.extend(b"BM");
        bmp.extend((54 + size).to_le_bytes());
        bmp.extend([0; 4]);
        bmp.extend(54_u32.to_le_bytes());
        bmp.extend(40_u32.to_le_bytes());
        bmp.extend((width as i32).to_le_bytes());
        bmp.extend((-(height as i32)).to_le_bytes());
        bmp.extend(1_u16.to_le_bytes());
        bmp.extend(32_u16.to_le_bytes());
        bmp.extend([0; 24]);
        for p in rgba.as_chunks::<4>().0 {
            bmp.extend([p[2], p[1], p[0], 255]);
        }
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{name}.bmp")),
            bmp,
        )
        .expect("capture");
    }
}

#[test]
fn visual_scene_renders_every_mode_from_three_angles_and_two_sizes() {
    // Given: shared diffuse/metallic geometry and a floor.
    let ctx = context();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = Renderer::new(
        &ctx,
        format,
        RenderConfig {
            shadow_resolution: 512,
            ..RenderConfig::default()
        },
    )
    .expect("visual renderer");
    let sphere = MeshData::sphere(0.65, 24, 16);
    let floor = plane(5.0);
    renderer
        .set_scene(&[
            object(
                &sphere,
                Mat4::from_translation(Vec3::new(-0.9, 0.0, 0.65)),
                SurfaceMaterial {
                    base_color: [0.8, 0.07, 0.03],
                    roughness: 0.65,
                    ..SurfaceMaterial::default()
                },
            ),
            object(
                &sphere,
                Mat4::from_translation(Vec3::new(0.9, 0.0, 0.65)),
                SurfaceMaterial {
                    base_color: [0.9, 0.7, 0.3],
                    metallic: 1.0,
                    roughness: 0.12,
                    ..SurfaceMaterial::default()
                },
            ),
            object(
                &floor,
                Mat4::IDENTITY,
                SurfaceMaterial {
                    base_color: [0.65; 3],
                    roughness: 0.8,
                    ..SurfaceMaterial::default()
                },
            ),
        ])
        .expect("visual scene");

    // When: every mode is rendered from several independent viewpoints and sizes.
    for (width, height) in [(256, 192), (160, 240)] {
        let target = OffscreenTarget::new(&ctx, width, height, format);
        for (angle, position) in [
            Vec3::new(3.0, -4.5, 3.0),
            Vec3::new(-3.0, -4.5, 2.5),
            Vec3::new(0.0, 1.0, 6.0),
        ]
        .into_iter()
        .enumerate()
        {
            let camera = Camera::new_perspective(
                position,
                Vec3::Z * 0.6,
                Vec3::Z,
                if width < height { 60.0 } else { 45.0 },
                width as f32 / height as f32,
                0.1,
                100.0,
            );
            for mode in MODES {
                renderer.set_mode(mode);
                for _ in 0..if mode == RenderMode::PathTracing {
                    32
                } else {
                    1
                } {
                    renderer
                        .render(&camera, &target.target(&ctx))
                        .expect("visual frame");
                }
                let frame = pixels(&ctx, &target);

                // Then: captures contain visible geometry, not an empty or uniform frame.
                assert!(frame
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[0] > 50 && p[0] > p[2]));
                assert!(frame.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
                capture(
                    &format!("{mode:?}-{width}x{height}-{angle}"),
                    width,
                    height,
                    &frame,
                );
            }
        }
    }
}
