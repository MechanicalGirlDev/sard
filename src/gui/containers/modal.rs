use super::{Gui, Id, Rect, Ui, Vec2, LAYER_POPUP};

/// Draw `add` in a centred modal box when `gui.modal() == Some(id)`. The base layer is
/// blocked while it is open; dismissal follows the policy set by `Gui::open_modal`.
pub fn modal<R>(
    gui: &mut Gui,
    id: Id,
    size: Vec2,
    add: impl FnOnce(&mut Ui<'_>) -> R,
) -> Option<R> {
    if gui.modal() != Some(id) {
        return None;
    }
    let style = gui.style().clone();
    let win = gui.size();
    let prev = gui.layer();
    gui.set_layer(LAYER_POPUP);
    let paint = gui.begin_window_popup_paint();
    gui.paint_rect(Rect::from_min_size(Vec2::ZERO, win), style.palette.dim);
    let size = size
        .max(Vec2::ZERO)
        .min((win - Vec2::splat(16.0)).max(Vec2::ZERO));
    let rect = Rect::from_min_size(((win - size) * 0.5).round(), size);
    gui.register_modal_rect(rect);
    gui.paint_shadow(rect, Vec2::new(0.0, 10.0), 28.0, 0.45);
    gui.paint_rect(rect, style.palette.popup);
    gui.paint_rect_outline(rect, 1.0, style.palette.line_hard);
    let inner = rect.shrink2(Vec2::new(12.0, 10.0));
    let result = {
        let mut ui = Ui::new(gui, id.with("modal"), inner);
        gui_clip(&mut ui, inner, add)
    };
    if let Some(paint) = paint {
        gui.finish_anchored_popup_paint(paint);
    }
    gui.set_layer(prev);
    Some(result)
}

fn gui_clip<R>(ui: &mut Ui<'_>, rect: Rect, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
    ui.gui().push_clip(rect);
    let r = add(ui);
    ui.gui().pop_clip();
    r
}
