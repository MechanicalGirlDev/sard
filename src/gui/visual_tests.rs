//! Native widget capture matrix. Set SARD_GUI_CAPTURE_DIR to retain BMP evidence.

use super::{Dir, Gui, Id, Palette, Range, Rect, ScrollRegion, ScrollState, Style};
use crate::{scene::OffscreenTarget, ClearState, WgpuContext};
use glam::Vec2;

#[test]
fn native_scroll_panel_renders_in_both_themes_and_viewport_sizes() -> anyhow::Result<()> {
    let ctx = WgpuContext::new_blocking(None)?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    for width in [320, 390, 1440] {
        for (theme, palette) in [("dark", Palette::DARK), ("light", Palette::LIGHT)] {
            for offset in [0.0, 220.0] {
                let mut gui = Gui::new(&ctx, format);
                let mut style = Style::default().scaled(1.5);
                style.palette = palette;
                gui.set_style(style);
                let mut scroll = ScrollState::default();
                let mut header_button = Rect::default();
                let mut footer_button = Rect::default();
                // The first frame measures body height; the next renders the chosen offset.
                for frame in 0..2 {
                    gui.begin_frame(&[], width, 320, 0.016);
                    if frame == 1 {
                        scroll.set_offset(offset);
                    }
                    let mut root = gui.root();
                    let bounds = root.rect().shrink(8.0);
                    let mut ui = root.child(Id::new("panel"), bounds, Dir::Vertical);
                    let regions =
                        ui.scroll_panel("content", 76.0, 38.0, &mut scroll, |region, ui| {
                            match region {
                                ScrollRegion::Header => {
                                    let _ = ui.heading("Scene objects and component properties");
                                    header_button = ui.button("Inspect selected object").rect;
                                }
                                ScrollRegion::Body => {
                                    let _ = ui.tree_node("robot", "Robot", true, true, |_| {});
                                    ui.horizontal(|ui| {
                                        let _ = ui.label("Position");
                                        let mut value = 1.25;
                                        let _ = ui.drag_value_edit(
                                            &mut value,
                                            0.1,
                                            Range::UNBOUNDED,
                                            2,
                                        );
                                    });
                                    for index in 0..20 {
                                        let _ = ui.selectable_table_row(
                                            Id::new(index),
                                            &[2.0, 1.0],
                                            index == 2,
                                            |column, ui| {
                                                let _ = ui.label(if column == 0 {
                                                    "Component"
                                                } else {
                                                    "Ready"
                                                });
                                            },
                                        );
                                    }
                                }
                                ScrollRegion::Footer => {
                                    footer_button = ui.button("Close inspector").rect;
                                }
                            }
                        });
                    anyhow::ensure!(header_button.max.y <= regions.body.min.y);
                    anyhow::ensure!(footer_button.min.y >= regions.body.max.y);
                    anyhow::ensure!(footer_button.max.y <= bounds.max.y);
                    anyhow::ensure!(regions.body.width() <= width as f32);
                }
                let target = OffscreenTarget::new(&ctx, width, 320, format);
                target
                    .target(&ctx)
                    .clear(ClearState::color(palette.background));
                let mut encoder = ctx.create_encoder(Some("native GUI visual evidence"));
                gui.render(&ctx, target.view(), &mut encoder)?;
                ctx.submit([encoder.finish()]);
                let pixels = target.read_pixels(&ctx);
                // Both fixed controls must remain painted after the body is scrolled.
                for rect in [header_button, footer_button] {
                    let point = rect.min + Vec2::splat(2.0);
                    let at = (point.y as usize * width as usize + point.x as usize) * 4;
                    anyhow::ensure!((&pixels[at..at + 3]) != (&pixels[..3]));
                }
                if let Some(directory) = std::env::var_os("SARD_GUI_CAPTURE_DIR") {
                    let directory = std::path::PathBuf::from(directory);
                    std::fs::create_dir_all(&directory)?;
                    let name = format!("{theme}-{width}-{offset}.bmp");
                    let mut bmp = vec![0_u8; 54];
                    bmp[..2].copy_from_slice(b"BM");
                    for (at, value) in [
                        (2, u32::try_from(pixels.len())? + 54),
                        (10, 54),
                        (14, 40),
                        (18, width),
                    ] {
                        bmp[at..at + 4].copy_from_slice(&value.to_le_bytes());
                    }
                    bmp[22..26].copy_from_slice(&(-320_i32).to_le_bytes());
                    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
                    bmp[28..30].copy_from_slice(&32_u16.to_le_bytes());
                    for pixel in pixels.chunks_exact(4) {
                        bmp.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                    }
                    std::fs::write(directory.join(name), bmp)?;
                }
            }
        }
    }
    Ok(())
}
