//! Interactive `Gui` / `Ui` HUD over a rotating-cube 3D scene.
//! Run with `cargo run -p gui_demo`.

use glam::{Mat4, Vec2, Vec3};
use sard::gui::{Dir, Gui, Range, Rect};
use sard::{
    screen_target, Camera, ClearState, ColorMaterial, FrameOutput, Gm, Key, Light, Mesh,
    MouseButton, Object, TextBuilder, Window, WindowSettings,
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
    gui: Option<Gui>,

    rotation_speed: f32,
    spread: f32,
    cube_count: f32,
    theme: usize,
    show_grid: bool,
    wobble: bool,
    paused: bool,
    reset_clicks: u32,
    progress: f32,
    animation_time: f32,
    frame_ms: Vec<f32>,
    name: String,
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
        gui: None,
        rotation_speed: 1.0,
        spread: 2.2,
        cube_count: 5.0,
        theme: 0,
        show_grid: true,
        wobble: true,
        paused: false,
        reset_clicks: 0,
        progress: 0.0,
        animation_time: 0.0,
        frame_ms: Vec::with_capacity(GRAPH_SAMPLES),
        name: String::from("robot-01"),
    };

    window.render_loop(state, move |state, frame| {
        let w = frame.viewport.width as f32;
        let h = frame.viewport.height as f32;
        let dt = frame.delta_time as f32;
        let t = frame.elapsed_time as f32;

        // ---- one-time init -------------------------------------------------
        if state.gui.is_none() {
            state.gui = Some(Gui::new(frame.ctx, frame.surface_format));
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
            state.animation_time += dt;
        }
        let base = THEMES[state.theme].1;
        let count = (state.cube_count.round() as usize).max(1);
        for (i, cube) in state.cubes.iter_mut().enumerate() {
            let offset = (i as f32 - (count as f32 - 1.0) * 0.5) * state.spread;
            let spin = state.animation_time * state.rotation_speed;
            let wob = if state.wobble {
                (state.animation_time * 1.7 + i as f32).sin() * 0.4
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

        // ---- build the overlay through one GUI ----------------------------
        let gui = state.gui.as_mut().expect("gui");
        gui.begin_frame(&frame.events, frame.viewport.width, frame.viewport.height, dt);
        let accent = [base[0], base[1], base[2], 1.0];

        // Raw painting shares the widget renderers and layers.
        if state.show_grid {
            let grid = [accent[0], accent[1], accent[2], 0.10];
            let mut gx = 40.0;
            while gx < w {
                gui.paint_line(Vec2::new(gx, 36.0), Vec2::new(gx, h), 1.0, grid);
                gx += 40.0;
            }
            let mut gy = 76.0;
            while gy < h {
                gui.paint_line(Vec2::new(0.0, gy), Vec2::new(w, gy), 1.0, grid);
                gy += 40.0;
            }
        }

        // Header bar.
        gui.paint_rect(Rect::new(0.0, 0.0, w, 36.0), [0.10, 0.10, 0.13, 0.9]);
        gui.paint_text("sard GUI demo", Vec2::new(16.0, 10.0), 14.0, [1.0; 4]);
        let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        gui.paint_text(
            &format!(
                "{fps:5.1} fps   t={t:6.1}s   {}",
                if state.paused { "PAUSED" } else { "running" }
            ),
            Vec2::new(w - 320.0, 10.0),
            14.0,
            accent,
        );

        // ----- LEFT panel: discrete widgets -----
        gui.paint_rect(Rect::new(16.0, 52.0, 300.0, 408.0), [0.12, 0.12, 0.16, 0.85]);
        {
            let mut root = gui.root();
            let id = root.make_id("widgets");
            let mut ui = root.child(id, Rect::new(30.0, 64.0, 272.0, 382.0), Dir::Vertical);
            ui.label_colored("Widgets", accent);
            ui.horizontal(|ui| {
                if ui.button("Reset clicks").clicked {
                    state.reset_clicks = 0;
                }
                if ui.button("+1").clicked {
                    state.reset_clicks += 1;
                }
                ui.label(&format!("clicks: {}", state.reset_clicks));
            });
            ui.checkbox(&mut state.show_grid, "background grid");
            ui.checkbox(&mut state.wobble, "cube wobble");
            ui.checkbox(&mut state.paused, "pause (Space)");
            ui.label("Theme:");
            for (i, (name, _)) in THEMES.iter().enumerate() {
                if ui.radio(state.theme == i, name).clicked {
                    state.theme = i;
                }
            }
            ui.label("robot name:");
            ui.text_input(&mut state.name);
            let info = TextBuilder::new()
                .line(format!("name : {}", state.name))
                .line(format!("theme: {}", THEMES[state.theme].0))
                .line(format!("cubes: {count}"))
                .build();
            ui.label_colored(&info, [0.78, 0.82, 0.9, 1.0]);
        }

        // ----- RIGHT panel: sliders, progress, palette -----
        let rx = w - 360.0;
        gui.paint_rect(Rect::new(rx, 52.0, 344.0, 230.0), [0.12, 0.12, 0.16, 0.85]);
        {
            let mut root = gui.root();
            let id = root.make_id("sliders");
            let mut ui = root.child(id, Rect::new(rx + 14.0, 64.0, 316.0, 164.0), Dir::Vertical);
            ui.label_colored("Sliders & values", accent);
            ui.label(&format!("rotation speed: {:.2}", state.rotation_speed));
            ui.slider(&mut state.rotation_speed, Range::new(0.0, 4.0));
            ui.label(&format!("cube spread: {:.2}", state.spread));
            ui.slider(&mut state.spread, Range::new(0.5, 4.0));
            ui.label(&format!("cube count: {count}"));
            ui.slider(&mut state.cube_count, Range::new(1.0, MAX_CUBES as f32));
        }

        // Progress bar + palette swatches (raw rects).
        gui.paint_rect(Rect::new(rx + 14.0, 234.0, 316.0, 16.0), [0.25, 0.25, 0.30, 1.0]);
        gui.paint_rect(Rect::new(rx + 14.0, 234.0, 316.0 * state.progress, 16.0), accent);
        for (i, (_, c)) in THEMES.iter().enumerate() {
            gui.paint_rect(Rect::new(rx + 14.0 + i as f32 * 40.0, 256.0, 32.0, 22.0), [c[0], c[1], c[2], 1.0]);
        }

        // ----- frame-time line graph -----
        gui.paint_rect(Rect::new(rx, 296.0, 344.0, 150.0), [0.12, 0.12, 0.16, 0.85]);
        gui.paint_text("frame time (ms)", Vec2::new(rx + 14.0, 306.0), 14.0, accent);
        line_graph(gui, rx + 14.0, 328.0, 316.0, 104.0, &state.frame_ms, accent);

        // The measured scroll region owns its persistent offset and clipping.
        gui.paint_rect(Rect::new(rx, 460.0, 344.0, 232.0), [0.12, 0.12, 0.16, 0.85]);
        gui.paint_text("scroll log (wheel over the list)", Vec2::new(rx + 14.0, 470.0), 14.0, accent);
        {
            let mut root = gui.root();
            let id = root.make_id("log");
            let mut ui = root.child(id, Rect::new(rx + 14.0, 492.0, 316.0, 190.0), Dir::Vertical);
            ui.scroll_area("events", Some(190.0), |ui| {
                ui.set_gap(0.0);
                for i in 0..40 {
                    let c = if i % 2 == 0 { [0.85, 0.88, 0.95, 1.0] } else { [0.6, 0.65, 0.75, 1.0] };
                    ui.set_next_height(18.0);
                    ui.label_colored(&format!("event {i:02}  t={t:.1}s  speed={:.2}", state.rotation_speed), c);
                }
            });
        }

        // Check shortcuts after fields claim focus, including on their press frame.
        if !gui.wants_keyboard() {
            if gui.key_pressed(Key::Space) {
                state.paused = !state.paused;
            }
            if gui.key_pressed(Key::R) {
                state.reset_clicks = 0;
                state.rotation_speed = 1.0;
                state.cube_count = 5.0;
            }
        }
        let mouse = gui.mouse();
        gui.paint_circle(mouse, 3.0, accent);
        gui.paint_text(
            &format!("mouse: {:.0}, {:.0}  L:{} M:{} R:{}  wheel: {:.1}", mouse.x, mouse.y,
                gui.mouse_down(MouseButton::Left), gui.mouse_down(MouseButton::Middle),
                gui.mouse_down(MouseButton::Right), gui.scroll_delta().y),
            Vec2::new(30.0, 476.0), 12.0, [0.78, 0.82, 0.9, 1.0],
        );
        // Footer hint.
        gui.paint_text(
            "keys: [Space] pause  [R] reset   |  drag sliders, type in the name field, scroll the log",
            Vec2::new(16.0, h - 22.0),
            12.0,
            [0.7, 0.7, 0.75, 1.0],
        );

        gui.render(frame.ctx, frame.surface_view, &mut encoder)
            .expect("gui render");
        frame.ctx.submit([encoder.finish()]);
        FrameOutput::default().with_cursor(gui.cursor())
    })
}

/// A frame-time line graph drawn with a single polyline (auto-scaled).
fn line_graph(gui: &mut Gui, x: f32, y: f32, w: f32, h: f32, samples: &[f32], color: [f32; 4]) {
    gui.paint_rect(Rect::new(x, y, w, h), [0.08, 0.08, 0.10, 0.9]);
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
    gui.paint_polyline(&points, 1.5, color);
}
