use super::*;

#[test]
fn steep_taper_outline_covers_sloped_edges_and_padded_caps() {
    for vertical in [false, true] {
        for (widths, samples) in [
            (
                [0.0, 100.0],
                [[133, 183], [135, 198], [136, 198], [135, 204]],
            ),
            (
                [100.0, 0.0],
                [[123, 183], [121, 198], [120, 198], [121, 204]],
            ),
        ] {
            let mut f = Fixture::new(256);
            // Pixel-centred endpoints at (123.5, 128.5) and (133.5, 128.5):
            // a 10px segment with half-width slope +/-5 and a 2px outline.
            let points = [Vec3::new(-4.5, -0.5, 0.0), Vec3::new(5.5, -0.5, 0.0)].map(|p| {
                if vertical {
                    Vec3::new(-p.y, -p.x, p.z) / 64.0
                } else {
                    p / 64.0
                }
            });
            f.scene.lines.push(
                LineSet::strip(&points, [1.0, 0.0, 0.0, 1.0])
                    .with_widths(widths[0], widths[1])
                    .with_outline(2.0),
            );

            let image = f.render(RenderOptions::default());
            f.capture(&image, &format!("steep-{vertical}-{}", widths[0]));
            let samples = samples.map(|p| if vertical { [p[1], p[0]] } else { p });
            // At the wide endpoint, 55px off-axis lies fully inside the outline.
            assert_eq!(f.pixel(&image, samples[0]), [0, 0, 0, 255]);
            // Two pixels beyond that endpoint, 70px off-axis has 50% coverage.
            // This needs BOTH the perpendicular slope factor and cap extrapolation.
            let [red, green, blue, alpha] = f.pixel(&image, samples[1]);
            assert_eq!([red, green, alpha], [0, 0, 255]);
            // Only the fractional blue channel permits one UNORM unit of rounding.
            assert!(blue.abs_diff(128) <= 1, "blue = {blue}");
            assert_eq!(f.pixel(&image, samples[2]), [0, 0, 255, 255]);
            assert_eq!(f.pixel(&image, samples[3]), [0, 0, 255, 255]);
        }
    }
}

#[test]
fn perspective_segments_clip_before_dividing_by_w() {
    for reverse in [false, true] {
        let mut f = Fixture::new(128);
        f.camera.ortho = false;
        let mut points = [Vec3::new(-0.05, 0.0, 2.5), Vec3::X];
        if reverse {
            points.reverse();
        }
        f.scene.lines.push(
            LineSet::strip(&points, [1.0, 0.0, 0.0, 1.0])
                .with_widths(8.0, 8.0)
                .with_outline(1.0),
        );
        let image = f.render(RenderOptions::default());
        f.capture(&image, &format!("near-{reverse}"));
        assert_eq!(f.pixel(&image, [110, 64]), [255, 0, 0, 255]);
        assert_eq!(f.pixel(&image, [80, 64]), [0, 0, 255, 255]);
        assert_eq!(f.pixel(&image, [110, 72]), [0, 0, 255, 255]);
    }
}

#[test]
fn near_clipping_preserves_the_projected_taper() {
    let mut f = Fixture::new(128);
    f.camera.ortho = false;
    f.scene.lines.push(
        LineSet::strip(
            &[Vec3::new(-0.01, 0.0, 1.95), Vec3::new(0.5, 0.0, 1.0)],
            [1.0, 0.0, 0.0, 1.0],
        )
        .with_widths(20.0, 0.0)
        .with_layer(Layer::Overlay),
    );
    let image = f.render(RenderOptions::default());
    f.capture(&image, "near-taper");
    // Unclipped endpoints project to x=51.2 and 96. At x=80.5 the
    // screen-linear full width is about 6.92, independent of near clipping.
    assert_eq!(f.pixel(&image, [80, 66]), [255, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [80, 68]), [0, 0, 255, 255]);
}

#[test]
fn invisible_and_zero_length_segments_do_not_cover_the_target() {
    for points in [
        [-Vec3::X + Vec3::Z * 3.0, Vec3::X + Vec3::Z * 3.0],
        [-Vec3::X - Vec3::Z * 9.0, Vec3::X - Vec3::Z * 9.0],
        [Vec3::ZERO, Vec3::ZERO],
        [Vec3::ZERO, Vec3::Z],
    ] {
        let mut f = Fixture::new(128);
        f.camera.ortho = false;
        f.scene.lines.push(
            LineSet::strip(&points, [1.0, 0.0, 0.0, 1.0])
                .with_widths(30.0, 2.0)
                .with_outline(3.0)
                .with_layer(Layer::Overlay),
        );
        let image = f.render(RenderOptions::default());
        assert!(image
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0, 0, 255, 255]));
    }
}

#[test]
fn region_clamping_preserves_physical_width_and_scissors() {
    let mut f = Fixture::new(128);
    f.scene.lines.push(
        LineSet::strip(&[-Vec3::X * 4.0, Vec3::X * 4.0], [1.0, 0.0, 0.0, 1.0])
            .with_widths(8.0, 8.0)
            .with_outline(2.0),
    );
    let image = f.render(RenderOptions {
        clear: true,
        region: Some([64, 64, 128, 128]),
    });
    f.capture(&image, "region");
    let rows: Vec<_> = (0..128)
        .filter(|y| f.pixel(&image, [96, *y])[0] > 200)
        .collect();
    assert_eq!(rows, (92..100).collect::<Vec<_>>());
    assert_eq!(f.pixel(&image, [96, 101]), [0, 0, 0, 255]);
    assert_eq!(f.pixel(&image, [63, 96]), [0, 0, 255, 255]);
}

#[test]
fn empty_regions_clear_without_issuing_invalid_gpu_viewports() {
    for region in [[128, 0, 64, 64], [0, 128, 64, 64], [32, 32, 0, 64]] {
        let mut f = Fixture::new(128);
        f.scene.lines.push(line([8.0; 2], 0.0));
        let image = f.render(RenderOptions {
            clear: true,
            region: Some(region),
        });
        assert!(image
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0, 0, 255, 255]));
    }
}
