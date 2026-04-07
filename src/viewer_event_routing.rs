#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerEventKind {
    CursorMoved,
    MouseWheel,
    MousePressed,
    MouseReleased,
    KeyboardInput,
    ModifiersChanged,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewerEventDispatch {
    pub update_modifiers: bool,
    pub evaluate_shortcuts: bool,
}

pub fn dispatch_effects(
    event_kind: ViewerEventKind,
    egui_consumed: bool,
) -> ViewerEventDispatch {
    match event_kind {
        ViewerEventKind::KeyboardInput => ViewerEventDispatch {
            update_modifiers: false,
            evaluate_shortcuts: !egui_consumed,
        },
        ViewerEventKind::ModifiersChanged => ViewerEventDispatch {
            update_modifiers: true,
            evaluate_shortcuts: false,
        },
        _ => ViewerEventDispatch::default(),
    }
}

pub fn should_route_event(
    event_kind: ViewerEventKind,
    pointer_in_viewport: bool,
    pointer_layer_order: Option<egui::Order>,
    captures_pointer: bool,
) -> bool {
    match event_kind {
        ViewerEventKind::KeyboardInput | ViewerEventKind::ModifiersChanged => true,
        ViewerEventKind::MouseReleased => captures_pointer,
        ViewerEventKind::CursorMoved => captures_pointer || pointer_in_background_view(pointer_in_viewport, pointer_layer_order),
        ViewerEventKind::MouseWheel | ViewerEventKind::MousePressed => {
            pointer_in_background_view(pointer_in_viewport, pointer_layer_order)
        }
        ViewerEventKind::Other => false,
    }
}

fn pointer_in_background_view(
    pointer_in_viewport: bool,
    pointer_layer_order: Option<egui::Order>,
) -> bool {
    pointer_in_viewport
        && matches!(
            pointer_layer_order,
            None | Some(egui::Order::Background)
        )
}
