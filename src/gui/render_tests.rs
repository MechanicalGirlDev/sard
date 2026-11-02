//! GPU readback regressions for GUI layer ordering.

use super::TextRenderer;
use crate::{scene::OffscreenTarget, ClearState, WgpuContext};

#[test]
fn text_layers_match_separately_submitted_passes() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let combined = OffscreenTarget::new(&ctx, 256, 128, format);
    let reference = OffscreenTarget::new(&ctx, 256, 128, format);
    let mut text = TextRenderer::new(&ctx, format);
    text.draw_text("Base layer", 8.0, 8.0, 22.0, [1.0, 0.0, 0.0, 1.0]);
    text.set_layer(1);
    text.draw_text("Popup", 120.0, 70.0, 16.0, [0.0, 1.0, 0.0, 1.0]);
    for target in [&reference, &combined] {
        target
            .target(&ctx)
            .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
    }
    // Reference submissions complete in queue order before the next prepare can
    // overwrite a renderer's vertex buffer.
    for layer in 0..2 {
        let mut encoder = ctx.create_encoder(Some("text layer reference"));
        text.render_layer(&ctx, &mut encoder, reference.view(), 256, 128, layer)?;
        ctx.submit([encoder.finish()]);
    }
    let mut encoder = ctx.create_encoder(Some("combined text layers"));
    text.render(&ctx, &mut encoder, combined.view(), 256, 128)?;
    ctx.submit([encoder.finish()]);
    let actual = combined.read_pixels(&ctx);
    let expected = reference.read_pixels(&ctx);
    anyhow::ensure!(expected
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[0] > 0 && p[1] == 0));
    anyhow::ensure!(expected
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[1] > 0 && p[0] == 0));
    let mismatch = actual
        .iter()
        .zip(&expected)
        .enumerate()
        .find(|(_, (a, b))| a != b);
    anyhow::ensure!(
        (mismatch).is_none(),
        "combined passes must preserve both layers"
    );
    Ok(())
}

#[test]
fn long_heading_ellipsizes_without_painting_outside_its_row() -> anyhow::Result<()> {
    use super::{Dir, Gui, Id, Rect, TextStyle};
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = OffscreenTarget::new(&ctx, 256, 64, format);
    let reference = OffscreenTarget::new(&ctx, 256, 64, format);
    let mut gui = Gui::new(&ctx, format);
    let font = gui.style().font_size_heading;
    let width = gui
        .measure_styled("A…", font, TextStyle::SEMIBOLD.spaced(0.06))
        .x
        + 0.1;
    for (surface, title) in [
        (&reference, "A…"),
        (&target, "A heading much longer than its available region"),
    ] {
        gui.begin_frame(&[], 256, 64, 0.016);
        let mut root = gui.root();
        let mut child = root.child(
            Id::new("heading"),
            Rect::new(8.0, 8.0, width, 24.0),
            Dir::Vertical,
        );
        let _ = child.heading(title);
        surface
            .target(&ctx)
            .clear(ClearState::color([0.0, 0.0, 0.0, 1.0]));
        let mut encoder = ctx.create_encoder(Some("heading clipping"));
        gui.render(&ctx, surface.view(), &mut encoder)?;
        ctx.submit([encoder.finish()]);
    }
    let pixels = target.read_pixels(&ctx);
    let expected = reference.read_pixels(&ctx);
    let mismatch = pixels
        .iter()
        .zip(&expected)
        .enumerate()
        .find(|(_, (a, b))| a != b);
    anyhow::ensure!(
        (mismatch).is_none(),
        "overflow must end in an ellipsis, not a cut glyph"
    );
    for y in 8..32 {
        for x in 80..256 {
            let at = (y * 256 + x) * 4;
            anyhow::ensure!(
                pixels[at..at + 3] == [0, 0, 0],
                "paint outside heading at {x},{y}"
            );
        }
    }
    Ok(())
}
