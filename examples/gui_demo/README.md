# gui_demo

An interactive `sard::gui::Gui` / `Ui` HUD over a rotating-cube 3D scene.

Run from the workspace root with a graphical display:

```bash
cargo run -p gui_demo
```

One persistent `Gui` ingests events with `begin_frame`, supplies `Ui` regions for
widgets, and renders the overlay after the 3D pass. The demo includes buttons,
checkboxes, theme radio buttons, an editable robot name, sliders for speed, spacing
and cube count, a frame-time graph, and a clipped scrolling log. Panels, the grid,
progress bar, palette and mouse marker use `Gui::paint_*` directly.

Press Space to pause animation or R to reset the click count, rotation speed and
cube count. These shortcuts are suppressed while the name field has focus.
Drag the sliders, scroll over the log, and watch the mouse position, button states
and wheel delta below the widget panel.
