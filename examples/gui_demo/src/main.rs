//! GUI Demo — showcases the `sard::gui::UiContext` immediate-mode UI.
//!
//! An interactive HUD is drawn over a small rotating-cube 3D scene. Everything in
//! the overlay goes through a single `UiContext`:
//!
//! * widgets    — `button` / `checkbox` / `radio_button` / `slider` / `text_input`
//! * raw draw   — `rect` / `line` / `polyline` / `circle` (the escape hatch)
//! * containers — `begin_scroll_area` + `push_clip` (scissor-clipped scrolling)
//! * input      — mouse (all buttons + wheel), keyboard and typed text via `UiContext`
//!
//! Run with: cargo run   (do not run headless — `render_loop` blocks until close)

use glam::{Mat4, Vec3};
use sard::gui::UiContext;
use sard::{
    screen_target, Camera, ClearState, ColorMaterial, FrameOutput, Gm, Key, Light, Mesh, Object,
    TextBuilder, Window, WindowSettings,
};

const MAX_CUBES: usize = 8;
const GRAPH_SAMPLES: usize = 96;

const THEMES: [(&str, [f32; 3]); 3] = [
    ("Ocean", [0.20, 0.55, 0.90]),
    ("Forest", [0.25, 0.75, 0.35]),
    ("Ember", [0.90, 0.40, 0.25]),
];

struct State {
    camera: Camera,
    cubes: Vec<Gm<Mesh, ColorMaterial>>,
    ui: Option<UiContext>,

    rotation_speed: f32,
    spread: f32,
    cube_count: f32,
    theme: usize,
    show_grid: bool,
    wobble: bool,
    paused: bool,
    reset_clicks: u32,
    progress: f32,
    frame_ms: Vec<f32>,
    name: String,
    log_scroll: f32,
}

fn main() -> anyhow::Result<()> {
    let window = Window::new(
        WindowSettings::default()
            .title("sard GUI demo")
            .size(1000, 720),
    )?;

    let state = State {
        camera: Camera::new_perspective(
            Vec3::new(0.0, 2.5, 7.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0,
            1.0,
            0.1,
            100.0,
        ),
        cubes: Vec::new(),
        ui: None,
        rotation_speed: 1.0,
        spread: 2.2,
        cube_count: 5.0,
        theme: 0,
        show_grid: true,
        wobble: true,
        paused: false,
        reset_clicks: 0,
        progress: 0.0,
        frame_ms: Vec::with_capacity(GRAPH_SAMPLES),
        name: String::from("robot-01"),
        log_scroll: 0.0,
    };

    window.render_loop(state, move |state, frame| {
        let w = frame.viewport.width as f32;
        let h = frame.viewport.height as f32;
        let dt = frame.delta_time as f32;
        let t = frame.elapsed_time as f32;

        // ---- one-time init -------------------------------------------------
        if state.ui.is_none() {
            state.ui = Some(UiContext::new(frame.ctx, frame.surface_format));
            for _ in 0..MAX_CUBES {
                let mat = ColorMaterial::new(frame.ctx, frame.surface_format)
                    .expect("color material");
                let mesh = Mesh::cube(frame.ctx, 0.8, [0.8, 0.8, 0.8]);
                state.cubes.push(Gm::new(mesh, mat));
            }
        }

        // ---- update scene --------------------------------------------------
        state.camera.set_viewport(frame.viewport);
        if !state.paused {
            state.progress = (state.progress + dt * 0.25).fract();
        }
        let base = THEMES[state.theme].1;
        let count = (state.cube_count.round() as usize).max(1);
        for (i, cube) in state.cubes.iter_mut().enumerate() {
            let offset = (i as f32 - (count as f32 - 1.0) * 0.5) * state.spread;
            let spin = if state.paused { 0.0 } else { t * state.rotation_speed };
            let wob = if state.wobble {
                (t * 1.7 + i as f32).sin() * 0.4
            } else {
                0.0
            };
            cube.transform = Mat4::from_translation(Vec3::new(offset, wob, 0.0))
                * Mat4::from_rotation_y(spin + i as f32 * 0.5)
                * Mat4::from_rotation_x(spin * 0.6);
        }
        if state.frame_ms.len() >= GRAPH_SAMPLES {
            state.frame_ms.remove(0);
        }
        state.frame_ms.push(dt * 1000.0);

        // ---- render 3D backdrop -------------------------------------------
        let mut encoder = frame.ctx.create_encoder(Some("gui_demo"));
        {
            let target = screen_target(&frame);
            let bg = [base[0] * 0.08, base[1] * 0.08, base[2] * 0.10, 1.0];
            let mut pass =
                target.begin_render_pass(&mut encoder, ClearState::color_and_depth(bg, 1.0));
            let no_lights: Vec<&dyn Light> = vec![];
            for cube in state.cubes.iter().take(count) {
                cube.render(frame.ctx, &state.camera, &no_lights, &mut pass);
            }
        }

        // ---- build UI (single UiContext, no second renderer) --------------
        let ui = state.ui.as_mut().expect("ui");
        ui.update(&frame.events, frame.viewport.width, frame.viewport.height);
        let accent = [base[0], base[1], base[2], 1.0];

        // Keyboard reaches UiContext now (B6): Space pauses, R resets — but suppress
        // shortcuts while a text field is focused so typing doesn't trigger them.
        if !ui.has_keyboard_focus() {
            if ui.pressed_keys().contains(&Key::Space) {
                state.paused = !state.paused;
            }
            if ui.pressed_keys().contains(&Key::R) {
                state.reset_clicks = 0;
                state.rotation_speed = 1.0;
                state.cube_count = 5.0;
            }
        }

        // Optional HUD grid drawn with raw lines (B1 + B9).
        if state.show_grid {
            let grid = [accent[0], accent[1], accent[2], 0.10];
            let mut gx = 40.0;
            while gx < w {
                ui.line(gx, 36.0, gx, h, 1.0, grid);
                gx += 40.0;
            }
            let mut gy = 76.0;
            while gy < h {
                ui.line(0.0, gy, w, gy, 1.0, grid);
                gy += 40.0;
            }
        }

        // Header bar.
        ui.rect(0.0, 0.0, w, 36.0, [0.10, 0.10, 0.13, 0.9]);
        ui.label("sard GUI demo", 16.0, 10.0, [1.0, 1.0, 1.0, 1.0]);
        let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        ui.label(
            &format!(
                "{fps:5.1} fps   t={t:6.1}s   {}",
                if state.paused { "PAUSED" } else { "running" }
            ),
            w - 320.0,
            10.0,
            accent,
        );

        // ----- LEFT panel: discrete widgets -----
        ui.rect(16.0, 52.0, 300.0, 408.0, [0.12, 0.12, 0.16, 0.85]);
        ui.label("Widgets", 30.0, 64.0, accent);

        if ui.button("Reset clicks", 30.0, 92.0, 130.0, 30.0) {
            state.reset_clicks = 0;
        }
        if ui.button("+1", 170.0, 92.0, 60.0, 30.0) {
            state.reset_clicks += 1;
        }
        ui.label(&format!("clicks: {}", state.reset_clicks), 240.0, 100.0, [0.9, 0.9, 0.9, 1.0]);

        state.show_grid = ui.checkbox(state.show_grid, "background grid", 30.0, 138.0);
        state.wobble = ui.checkbox(state.wobble, "cube wobble", 30.0, 168.0);
        state.paused = ui.checkbox(state.paused, "pause (Space)", 30.0, 198.0);

        ui.label("Theme:", 30.0, 238.0, [0.9, 0.9, 0.9, 1.0]);
        for (i, (name, _)) in THEMES.iter().enumerate() {
            if ui.radio_button(state.theme == i, name, 30.0, 262.0 + i as f32 * 30.0) {
                state.theme = i;
            }
        }

        // Text input field (B5).
        ui.label("robot name:", 30.0, 360.0, [0.9, 0.9, 0.9, 1.0]);
        ui.text_input(&mut state.name, 30.0, 380.0, 256.0);

        // Info block built with TextBuilder.
        let info = TextBuilder::new()
            .line(format!("name : {}", state.name))
            .line(format!("theme: {}", THEMES[state.theme].0))
            .line(format!("cubes: {count}"))
            .build();
        ui.label(&info, 30.0, 418.0, [0.78, 0.82, 0.9, 1.0]);

        // ----- RIGHT panel: sliders, progress, palette -----
        let rx = w - 360.0;
        ui.rect(rx, 52.0, 344.0, 230.0, [0.12, 0.12, 0.16, 0.85]);
        ui.label("Sliders & values", rx + 14.0, 64.0, accent);

        // Built-in slider widget (B4) — caller draws its own label.
        ui.label(&format!("rotation speed: {:.2}", state.rotation_speed), rx + 14.0, 90.0, [0.9, 0.9, 0.9, 1.0]);
        state.rotation_speed = ui.slider(state.rotation_speed, 0.0, 4.0, rx + 14.0, 104.0, 316.0);

        ui.label(&format!("cube spread: {:.2}", state.spread), rx + 14.0, 128.0, [0.9, 0.9, 0.9, 1.0]);
        state.spread = ui.slider(state.spread, 0.5, 4.0, rx + 14.0, 142.0, 316.0);

        ui.label(&format!("cube count: {count}"), rx + 14.0, 166.0, [0.9, 0.9, 0.9, 1.0]);
        state.cube_count = ui.slider(state.cube_count, 1.0, MAX_CUBES as f32, rx + 14.0, 180.0, 316.0);

        // Progress bar + palette swatches (raw rects).
        ui.rect(rx + 14.0, 214.0, 316.0, 16.0, [0.25, 0.25, 0.30, 1.0]);
        ui.rect(rx + 14.0, 214.0, 316.0 * state.progress, 16.0, accent);
        for (i, (_, c)) in THEMES.iter().enumerate() {
            ui.rect(rx + 14.0 + i as f32 * 40.0, 244.0, 32.0, 22.0, [c[0], c[1], c[2], 1.0]);
        }

        // ----- frame-time line graph (B9 polyline) -----
        ui.rect(rx, 296.0, 344.0, 150.0, [0.12, 0.12, 0.16, 0.85]);
        ui.label("frame time (ms)", rx + 14.0, 306.0, accent);
        line_graph(ui, rx + 14.0, 328.0, 316.0, 104.0, &state.frame_ms, accent);

        // ----- scrollable log list (B8 scroll area + clipping) -----
        ui.rect(rx, 460.0, 344.0, 232.0, [0.12, 0.12, 0.16, 0.85]);
        ui.label("scroll log (wheel over the list)", rx + 14.0, 470.0, accent);
        let rows = 40;
        let row_h = 18.0;
        let content_h = rows as f32 * row_h;
        let top = ui.begin_scroll_area(rx + 14.0, 492.0, 316.0, 190.0, content_h, &mut state.log_scroll);
        for i in 0..rows {
            let y = top + i as f32 * row_h;
            let c = if i % 2 == 0 { [0.85, 0.88, 0.95, 1.0] } else { [0.6, 0.65, 0.75, 1.0] };
            ui.label(&format!("event {i:02}  t={:.1}s  speed={:.2}", t, state.rotation_speed), rx + 22.0, y, c);
        }
        ui.end_scroll_area();

        // Footer hint.
        ui.label(
            "keys: [Space] pause  [R] reset   |  drag sliders, type in the name field, scroll the log",
            16.0,
            h - 22.0,
            [0.7, 0.7, 0.75, 1.0],
        );

        ui.render(frame.ctx, frame.surface_view, &mut encoder)
            .expect("ui render");
        frame.ctx.submit([encoder.finish()]);
        FrameOutput::default()
    })
}

/// A frame-time line graph drawn with a single polyline (auto-scaled).
fn line_graph(
    ui: &mut UiContext,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    samples: &[f32],
    color: [f32; 4],
) {
    ui.rect(x, y, w, h, [0.08, 0.08, 0.10, 0.9]);
    if samples.len() < 2 {
        return;
    }
    let max = samples.iter().copied().fold(1.0_f32, f32::max);
    let step = w / (GRAPH_SAMPLES - 1) as f32;
    let points: Vec<[f32; 2]> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| [x + i as f32 * step, y + h - (s / max) * (h - 4.0)])
        .collect();
    ui.polyline(&points, 1.5, color);
}
