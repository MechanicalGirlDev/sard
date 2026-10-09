use sard::core::pipeline::Vertex;
use sard::glam::{Mat4, Quat, Vec3};
use sard::renderer::{
    Mesh, SensorCamera, SensorCameraConfig, SensorChannels, SensorError, SensorObject,
};
use sard::{wgpu, WgpuContext};

fn required_context() -> WgpuContext {
    pollster::block_on(async {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .expect("sensor tests REQUIRE a GPU adapter; no skip path");
        tracing::info!(adapter = ?adapter.get_info(), "sensor compute adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("required sensor device");
        WgpuContext::new(device, queue)
    })
}

#[cfg(feature = "gpu-mpm")]
#[test]
fn resident_mpm_snapshot_stays_frozen_in_sensor_rendering() {
    use sard::physics::mpm::{
        GpuMpmResidentSession, GpuMpmTransfers, MaterialModel, MpmParams, MpmParticle, MpmWorld,
        WorldBounds,
    };
    use sard::physics::nalgebra::Vector3;
    use sard::physics::{mpm_particle_data, GpuContactDevice};
    use sard::renderer::ParticleSystem;

    // Given: native GPU MPM on its compute device and a separate render device.
    let ctx = required_context();
    let device = GpuContactDevice::new().expect("required MPM compute adapter");
    let mut particle = MpmParticle::new(
        Vector3::repeat(0.5),
        0.04,
        1_000.0,
        MaterialModel::neo_hookean(2_000.0, 0.2),
    );
    particle.velocity.x = 1.0;
    let native = MpmWorld::new(
        vec![particle],
        MpmParams {
            bounds: Some(WorldBounds {
                min: Vector3::zeros(),
                max: Vector3::repeat(1.0),
            }),
            ..MpmParams::default()
        },
    )
    .expect("native material world");
    let transfers = GpuMpmTransfers::new(device.device());
    let mut session =
        GpuMpmResidentSession::new(&transfers, device.device(), device.queue(), native, 0.001)
            .expect("resident MPM");
    session.submit_steps(4).expect("queued material steps");
    session.synchronize().expect("explicit particle readback");
    let solved = session.world().particles[0].position;
    assert!(solved.x > 0.503);
    assert!(solved.z < 0.49995);

    // When: solved particles become frozen geometry, even as render time advances.
    let data = mpm_particle_data(&session.world().particles, [0.0, 1.0, 0.0])
        .expect("authoritative particle data");
    let mut geometry = ParticleSystem::new(&ctx, data, Vec3::ZERO, 0.1);
    let mut camera = SensorCamera::new(
        &ctx,
        SensorCameraConfig {
            width: 65,
            height: 65,
            ..SensorCameraConfig::default()
        },
    )
    .expect("MPM camera");
    camera
        .set_pose(Mat4::from_translation(Vec3::new(
            solved.x as f32,
            solved.y as f32,
            2.0,
        )))
        .expect("MPM camera pose");
    let before = camera
        .render(
            &[SensorObject {
                geometry: &geometry,
                transform: Mat4::IDENTITY,
                color: [0.0, 1.0, 0.0],
                segmentation_id: 77,
                texture: None,
            }],
            all(),
        )
        .expect("native MPM capture");
    geometry.update(&ctx, 1.0);
    let after = camera
        .render(
            &[SensorObject {
                geometry: &geometry,
                transform: Mat4::IDENTITY,
                color: [0.0, 1.0, 0.0],
                segmentation_id: 77,
                texture: None,
            }],
            all(),
        )
        .expect("frozen MPM capture");

    // Then: depth is native solved depth, not a cosmetic second integration.
    let center = 32 * 65 + 32;
    assert_eq!(before.segmentation.as_ref().expect("MPM IDs")[center], 77);
    assert!(
        (f64::from(before.depth.as_ref().expect("MPM depth")[center]) - (2.0 - solved.z)).abs()
            < 1e-5
    );
    if let Ok(path) = std::env::var("SARD_SENSOR_CAPTURE") {
        write_capture(
            &format!("{path}.mpm.bmp"),
            before.width,
            before.height,
            before.rgb.as_ref().expect("MPM RGB"),
        );
    }
    assert_eq!(before.rgb, after.rgb);
    assert_eq!(before.depth, after.depth);
    assert_eq!(before.segmentation, after.segmentation);

    // Billboards must also face side cameras, including the world-up pole.
    let target = Vec3::new(solved.x as f32, solved.y as f32, solved.z as f32);
    for (view, offset) in [Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 2.0, 0.0)]
        .into_iter()
        .enumerate()
    {
        camera
            .set_pose(
                glam::camera::rh::view::look_at_mat4(target + offset, target, Vec3::Z).inverse(),
            )
            .expect("billboard side camera");
        let frame = camera
            .render(
                &[SensorObject {
                    geometry: &geometry,
                    transform: Mat4::IDENTITY,
                    color: [0.0, 1.0, 0.0],
                    segmentation_id: 77,
                    texture: None,
                }],
                all(),
            )
            .expect("angled MPM billboard");
        assert_eq!(frame.segmentation.expect("angled MPM IDs")[center], 77);
        assert!((frame.depth.expect("angled MPM depth")[center] - 2.0).abs() < 1e-5);
        if let Ok(path) = std::env::var("SARD_SENSOR_CAPTURE") {
            write_capture(
                &format!("{path}.mpm-side-{view}.bmp"),
                frame.width,
                frame.height,
                &frame.rgb.expect("angled MPM RGB"),
            );
        }
    }
}

fn xy_plane(ctx: &WgpuContext) -> Mesh {
    let vertices = [
        Vertex {
            position: [-1.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [1.0; 3],
        },
        Vertex {
            position: [1.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [1.0; 3],
        },
        Vertex {
            position: [1.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [1.0; 3],
        },
        Vertex {
            position: [-1.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [1.0; 3],
        },
    ];
    Mesh::new(
        ctx,
        &vertices,
        Some(&[0, 1, 2, 0, 2, 3]),
        Some("sensor test plane"),
    )
}

fn at(scale: f32, x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(Vec3::splat(scale), Quat::IDENTITY, Vec3::new(x, y, z))
}

fn all() -> SensorChannels {
    SensorChannels {
        rgb: true,
        depth: true,
        segmentation: true,
    }
}

fn write_capture(path: &str, width: u32, height: u32, rgb: &[u8]) {
    let mut bmp = vec![0_u8; 54];
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&(54 + width * height * 4).to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bmp[22..26].copy_from_slice(&(height as i32).to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&32_u16.to_le_bytes());
    for row in rgb.chunks_exact(width as usize * 3).rev() {
        for pixel in row.chunks_exact(3) {
            bmp.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
    }
    std::fs::write(path, bmp).expect("write sensor fixture capture");
}

// One GPU fixture deliberately serializes adapter/device initialization and all
// captures. SensorCamera::render subscribes to map_async before its bounded GPU
// wait; assertions never depend on sleep or delayed polling.
#[test]
fn sensor_camera_gpu_contract() {
    let ctx = required_context();
    let config = SensorCameraConfig {
        width: 65,
        height: 65,
        fov_y_deg: 90.0,
        znear: 0.1,
        zfar: 20.0,
    };
    let mut camera = SensorCamera::new(&ctx, config).expect("camera");
    let plane = xy_plane(&ctx);
    let near_id = 0xf123_4567;
    let far_id = 0x0100_0001;
    let scene = [
        SensorObject {
            geometry: &plane,
            transform: at(3.0, 0.0, 0.0, -4.0),
            color: [0.0, 0.0, 1.0],
            segmentation_id: far_id,
            texture: None,
        },
        SensorObject {
            geometry: &plane,
            transform: at(0.6, 0.0, 0.0, -2.0),
            color: [1.0, 0.0, 0.0],
            segmentation_id: near_id,
            texture: None,
        },
    ];
    let frame = camera.render(&scene, all()).expect("first capture");
    let center = 32 * 65 + 32;
    let off_axis = 32 * 65 + 10;
    let depth = frame.depth.as_ref().expect("depth");
    let ids = frame.segmentation.as_ref().expect("IDs");
    let rgb = frame.rgb.as_ref().expect("RGB");
    assert_eq!(rgb.len(), 65 * 65 * 3);
    assert_eq!(ids.len(), 65 * 65);
    assert_eq!(ids[center], near_id, "occlusion and full 32-bit ID");
    assert_eq!(
        ids[off_axis], far_id,
        "distinct draws retain their transforms and IDs"
    );
    assert!((depth[center] - 2.0).abs() < 1.0e-5);
    assert!(
        (depth[off_axis] - 4.0).abs() < 1.0e-5,
        "off-axis depth is axial, not longer Euclidean ray distance"
    );
    assert_eq!(&rgb[center * 3..center * 3 + 3], &[255, 0, 0]);
    assert_eq!(&rgb[off_axis * 3..off_axis * 3 + 3], &[0, 0, 255]);
    assert_eq!(ids[0], 0);
    assert_eq!(depth[0].to_bits(), 0_f32.to_bits());
    assert_eq!(&rgb[..3], &[0, 0, 0]);

    // Writes a real RGB fixture on demand for parent/manual image inspection.
    if let Ok(path) = std::env::var("SARD_SENSOR_CAPTURE") {
        write_capture(&path, 65, 65, rgb);
        tracing::info!(path, "sensor fixture capture");
    }

    // Reverse draw order: all three outputs agree on the nearest surface.
    let reversed = [
        SensorObject {
            geometry: &plane,
            transform: scene[1].transform,
            color: scene[1].color,
            segmentation_id: near_id,
            texture: None,
        },
        SensorObject {
            geometry: &plane,
            transform: scene[0].transform,
            color: scene[0].color,
            segmentation_id: far_id,
            texture: None,
        },
    ];
    let reversed_frame = camera.render(&reversed, all()).expect("reversed scene");
    assert_eq!(reversed_frame.segmentation.as_ref(), Some(ids));
    assert_eq!(reversed_frame.rgb.as_ref(), Some(rgb));
    assert_eq!(reversed_frame.depth.as_ref(), Some(depth));

    for mask in 0..8 {
        let channels = SensorChannels {
            rgb: mask & 1 != 0,
            depth: mask & 2 != 0,
            segmentation: mask & 4 != 0,
        };
        let subset = camera.render(&scene, channels).expect("optional channels");
        assert_eq!(subset.rgb.as_ref(), channels.rgb.then_some(rgb));
        assert_eq!(subset.depth.as_ref(), channels.depth.then_some(depth));
        assert_eq!(
            subset.segmentation.as_ref(),
            channels.segmentation.then_some(ids)
        );
    }

    // Different scene and camera selection cannot retain any previous body.
    let mut other_camera = SensorCamera::new(&ctx, config).expect("other camera");
    other_camera
        .set_pose(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))
        .expect("other pose");
    let unseen = other_camera
        .render(&scene, all())
        .expect("select second camera");
    assert!(unseen.segmentation.expect("IDs").iter().all(|id| *id == 0));
    let empty = camera.render(&[], all()).expect("different empty scene");
    assert!(empty
        .depth
        .expect("depth")
        .iter()
        .all(|value| *value == 0.0));
    assert!(empty.segmentation.expect("IDs").iter().all(|id| *id == 0));
    assert_eq!(
        camera
            .render(&scene, all())
            .expect("first camera again")
            .segmentation
            .as_ref(),
        Some(ids)
    );

    // Top-left row order and a non-256-aligned resized row.
    camera.resize(37, 29).expect("resize");
    let upper = [SensorObject {
        geometry: &plane,
        transform: at(0.2, 0.0, 0.7, -2.0),
        color: [0.0, 1.0, 0.0],
        segmentation_id: near_id,
        texture: None,
    }];
    let resized = camera.render(&upper, all()).expect("resized frame");
    assert_eq!((resized.width, resized.height), (37, 29));
    let resized_ids = resized.segmentation.expect("resized IDs");
    assert_eq!(resized_ids.len(), 37 * 29);
    assert_eq!(
        resized_ids[9 * 37 + 18],
        near_id,
        "+Y appears in upper rows"
    );
    assert_eq!(resized_ids[19 * 37 + 18], 0);
    assert_eq!(other_camera.config().width, 65, "resize is per-camera");
    assert!(matches!(
        camera.resize(0, 29),
        Err(SensorError::InvalidCamera)
    ));
    assert_eq!(
        camera.config().width,
        37,
        "invalid resize does not change targets"
    );

    // Attachment composes a rotated body pose with a translated mount. Geometry
    // is placed in camera space, then transformed to world with that same pose.
    camera.resize(65, 65).expect("restore size");
    let body = Mat4::from_rotation_translation(
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        Vec3::new(3.0, 4.0, 5.0),
    );
    let mount = Mat4::from_translation(Vec3::new(0.2, 0.3, 0.4));
    camera.attach_to_body(body, mount).expect("attach");
    assert!(camera.pose().abs_diff_eq(body * mount, 1.0e-6));
    let attached_scene = [SensorObject {
        geometry: &plane,
        transform: body * mount * at(0.6, 0.0, 0.0, -2.0),
        color: [1.0, 0.0, 0.0],
        segmentation_id: near_id,
        texture: None,
    }];
    let attached = camera
        .render(&attached_scene, all())
        .expect("attached render");
    assert_eq!(attached.segmentation.expect("IDs")[center], near_id);
    assert!((attached.depth.expect("depth")[center] - 2.0).abs() < 1.0e-5);
    camera
        .attach_to_body(
            Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)) * body,
            mount,
        )
        .expect("authoritative body movement");
    assert!(camera
        .render(&attached_scene, all())
        .expect("moved attachment")
        .segmentation
        .expect("IDs")
        .iter()
        .all(|id| *id == 0));
    assert!(matches!(
        camera.set_pose(Mat4::from_scale(Vec3::splat(2.0))),
        Err(SensorError::InvalidPose)
    ));

    camera.set_pose(Mat4::IDENTITY).expect("reset pose");
    let clipped = [
        SensorObject {
            geometry: &plane,
            transform: at(3.0, 0.0, 0.0, -21.0),
            color: [1.0; 3],
            segmentation_id: 1,
            texture: None,
        },
        SensorObject {
            geometry: &plane,
            transform: at(3.0, 0.0, 0.0, -0.05),
            color: [1.0; 3],
            segmentation_id: 2,
            texture: None,
        },
    ];
    assert!(camera
        .render(&clipped, all())
        .expect("clipped surfaces")
        .segmentation
        .expect("IDs")
        .iter()
        .all(|id| *id == 0));
    camera.set_background([0.0, 1.0, 0.0]).expect("background");
    let background = camera.render(&[], all()).expect("background capture");
    assert_eq!(&background.rgb.expect("RGB")[..3], &[0, 255, 0]);
    assert_eq!(
        background.depth.expect("depth")[0].to_bits(),
        0_f32.to_bits()
    );
    assert_eq!(background.segmentation.expect("IDs")[0], 0);
    let invalid = [SensorObject {
        geometry: &plane,
        transform: Mat4::IDENTITY,
        color: [1.0; 3],
        segmentation_id: 0,
        texture: None,
    }];
    assert!(matches!(
        camera.render(&invalid, all()),
        Err(SensorError::InvalidObject)
    ));
    assert!(matches!(
        SensorCamera::new(
            &ctx,
            SensorCameraConfig {
                fov_y_deg: f32::NAN,
                ..config
            }
        ),
        Err(SensorError::InvalidCamera)
    ));

    // Given: a UV-mapped plane with four distinct image texels.
    let vertices = [
        sard::VertexPNUC::new([-1.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0], [1.0; 4]),
        sard::VertexPNUC::new([1.0, -1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0], [1.0; 4]),
        sard::VertexPNUC::new([1.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0], [1.0; 4]),
        sard::VertexPNUC::new([-1.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0], [1.0; 4]),
    ];
    let uv_plane = Mesh::new_textured(&ctx, &vertices, Some(&[0, 1, 2, 0, 2, 3]), Some("UV plane"));
    let image = sard::Texture2D::from_rgba8(
        &ctx,
        2,
        2,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
        Some("sensor checker"),
    );
    camera.resize(65, 65).expect("texture capture size");
    let textured_scene = [SensorObject {
        geometry: &uv_plane,
        transform: at(1.0, 0.0, 0.0, -2.0),
        color: [1.0; 3],
        segmentation_id: near_id,
        texture: Some(&image),
    }];

    // When: all channels capture the textured body.
    let textured = camera
        .render(&textured_scene, all())
        .expect("textured capture");

    // Then: UV orientation affects RGB, not exact IDs or axial metric depth.
    let pixels = textured.rgb.expect("textured RGB");
    let textured_ids = textured.segmentation.expect("textured IDs");
    let textured_depth = textured.depth.expect("textured depth");
    for (x, y, dominant) in [(20, 20, 0), (44, 20, 1), (20, 44, 2)] {
        let pixel = (y * 65 + x) * 3;
        assert!(pixels[pixel + dominant] > 240);
        assert!(pixels[pixel + (dominant + 1) % 3] < 16);
        assert!(pixels[pixel + (dominant + 2) % 3] < 16);
        let index = y * 65 + x;
        assert_eq!(textured_ids[index], near_id);
        assert!((textured_depth[index] - 2.0).abs() < 1e-5);
    }
}

#[cfg(feature = "physics")]
#[test]
fn native_body_motion_reaches_sensor_depth_and_segmentation() {
    use sard::ecs::components::{GlobalTransform, RigidBody, Transform};
    use sard::physics::nalgebra::{Isometry3, Matrix3, Vector3};
    use sard::physics::{PhysicsConfig, PhysicsWorld, SceneCollider};

    // Given: an ECS body whose world pose is owned by native CPU physics.
    let ctx = required_context();
    let cube = Mesh::cube(&ctx, 1.0, [1.0; 3]);
    let mut world = hecs::World::new();
    let mut physics = PhysicsWorld::new(PhysicsConfig::default()).expect("native world");
    let body = RigidBody::new(
        Isometry3::translation(0.0, 0.0, 2.0),
        1.0,
        Matrix3::identity(),
        vec![SceneCollider::Box {
            origin: Isometry3::identity(),
            half_extents: Vector3::repeat(0.5),
        }],
    )
    .expect("native box");
    let entity = world.spawn((body, Transform::identity(), GlobalTransform::default()));
    let mut camera = SensorCamera::new(
        &ctx,
        SensorCameraConfig {
            width: 129,
            height: 129,
            fov_y_deg: 60.0,
            ..SensorCameraConfig::default()
        },
    )
    .expect("physics camera");
    camera
        .set_pose(Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0)))
        .expect("downward camera");
    let center = 64 * 129 + 64;

    // When: rendering consumes solved transforms before and after native motion.
    let mut depths = Vec::new();
    for steps in [1, 60] {
        for _ in 0..steps {
            physics.step(&mut world, 0.005).expect("native step");
        }
        let transform = world
            .get::<&GlobalTransform>(entity)
            .expect("solved pose")
            .0;
        let frame = camera
            .render(
                &[SensorObject {
                    geometry: &cube,
                    transform,
                    color: [1.0, 0.2, 0.0],
                    segmentation_id: 42,
                    texture: None,
                }],
                all(),
            )
            .expect("solved-body capture");
        assert_eq!(frame.segmentation.expect("body IDs")[center], 42);
        depths.push(frame.depth.expect("body depth")[center]);
    }

    // Then: falling increases camera depth without changing the body's ID.
    assert!(depths[1] > depths[0] + 0.4);
    let transform = world
        .get::<&GlobalTransform>(entity)
        .expect("solved pose")
        .0;
    let target = transform.transform_point3(Vec3::ZERO);
    assert!((depths[1] - (5.0 - target.z - 0.5)).abs() < 1e-5);

    // Inspect the same authoritative body from three independent camera angles.
    for (view, offset) in [
        Vec3::new(0.0, -4.0, 0.0),
        Vec3::new(4.0, 0.0, 0.0),
        Vec3::new(3.0, -3.0, 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        camera
            .set_pose(
                glam::camera::rh::view::look_at_mat4(target + offset, target, Vec3::Z).inverse(),
            )
            .expect("independent camera angle");
        let frame = camera
            .render(
                &[SensorObject {
                    geometry: &cube,
                    transform,
                    color: [1.0, 0.2, 0.0],
                    segmentation_id: 42,
                    texture: None,
                }],
                all(),
            )
            .expect("angled native-body capture");
        let ids = frame.segmentation.expect("angled IDs");
        assert_eq!(ids[center], 42);
        assert_eq!(ids[0], 0);
        assert!(frame.depth.expect("angled depth")[center] > 0.0);
        if let Ok(path) = std::env::var("SARD_SENSOR_CAPTURE") {
            write_capture(
                &format!("{path}.native-{view}.bmp"),
                frame.width,
                frame.height,
                &frame.rgb.expect("angled RGB"),
            );
        }
    }
}
