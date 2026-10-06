# gui_demo

A complete showcase of the `sard::gui::UiContext` immediate-mode UI: an interactive
HUD drawn over a small rotating-cube 3D scene.

```bash
cd examples/gui_demo && cargo run
```

Everything in the overlay goes through a **single** `UiContext` — no second renderer,
no manual overlay pass:

| Capability | API |
|---|---|
| Buttons, checkboxes, radio buttons, labels | `button` / `checkbox` / `radio_button` / `label` |
| Sliders (click-and-drag) | `slider` |
| Editable text field | `text_input` |
| Raw drawing (panels, grid, graph) | `rect` / `line` / `polyline` / `circle` / `primitives()` |
| Scrollable, scissor-clipped list | `begin_scroll_area` / `end_scroll_area` / `push_clip` |
| Mouse (all buttons + wheel), keyboard, typed text | `update` + `pressed_keys()` / `scroll_delta()` / … |

---

## What changed in this iteration

The first version of this demo had to carry a second `PrimitiveRenderer` and composite
it through a separate depth-less overlay `RenderTarget`, and hand-roll its own slider,
mouse tracking and bar-graph — because `UiContext` exposed none of that. Those gaps were
reported, then fixed in the library. The demo was rewritten to consume the new API, and
the entire workaround scaffold is gone — which is the real test that the gaps are closed.

### Resolved

- **A — `gui` no longer requires `window`** *(was a verified build bug).* `src/gui/ui.rs`
  (`UiContext`) is now gated behind `#[cfg(feature = "window")]`; `text.rs`/`primitive.rs`
  are window-free, so `cargo build --no-default-features --features gui` compiles.
  `cargo check --no-default-features --features gui` was added to CI's `check-features`
  job so it can't regress.
- **B1 — raw-draw escape hatch.** `UiContext` now exposes `rect` / `rect_outline` /
  `line` / `polyline` / `circle` / `text` / `measure`, plus `primitives()` / `texts()`
  for full access to the inner renderers. Panels, the HUD grid and the graph all draw
  through the one `UiContext`.
- **B4 — `slider` widget** with proper press-capture (grab persists until release, even
  if the cursor leaves the handle).
- **B5 — `text_input` widget**: click-to-focus, typed-text entry, Backspace, blinking
  caret. Enabled by a new `Event::Text` (composed, shift/layout-correct characters from
  winit's `key_event.text`).
- **B6 — full input in `UiContext::update`**: all mouse buttons, the wheel, key presses
  and `Event::Text` are ingested.
- **B7 — input accessors**: `mouse_pos`, `mouse_down(button)`, `mouse_pressed(button)`,
  `mouse_clicked`, `scroll_delta`, `pressed_keys`, `typed_text`, `is_hovered`, and
  `has_keyboard_focus` (so apps can suppress global shortcuts while a field is focused).
- **B8 — clipping + scrolling.** `PrimitiveRenderer` got a clip stack with scissor
  batching; `TextRenderer` clips via per-entry `TextBounds`. `UiContext` exposes
  `push_clip` / `pop_clip` and a `begin_scroll_area` / `end_scroll_area` helper.
- **B9 — `draw_line` / `draw_polyline`** on `PrimitiveRenderer` (used for the grid and
  the frame-time line graph; also backs `rect_outline`).

## Still open (not in this change)

- **B2 — layout system.** Still all absolute pixel coordinates; no row/column/stack,
  spacing, padding or auto-sizing (see the literal coordinates throughout `main.rs`).
- **B3 — container / window widget** (movable/resizable panel with title bar). The new
  `push_clip` + `begin_scroll_area` are the building blocks, but there is no packaged
  panel widget yet.
- **C10 — theming / `Style` struct.** Widget colors are still hardcoded inside
  `UiContext`.
- **C11 — text is monospace-only.** `text.rs` hardcodes `Family::Monospace`; no font
  loading, per-call weight/italic, alignment, or wrap controls (glyphon supports them).
- **C12 — richer click semantics.** No double-click; `mouse_clicked` is still a plain
  left-release (per-widget press-capture now exists for sliders/fields, but there is no
  general click-vs-drag classification).
- **C13 — focus polish.** Text fields focus on click and the widget-id scheme exists,
  but there is no Tab navigation / focus ring across widgets.
- **C14 — re-export inconsistency.** `TextRenderer`/`TextBuilder` are at the crate root;
  `UiContext`/`PrimitiveRenderer` are still only under `sard::gui::`.
