use scad_ui::viewer_event_routing::dispatch_effects;
use scad_ui::viewer_event_routing::{ViewerEventKind, should_route_event};

#[test]
fn mouse_press_inside_viewport_background_routes_to_viewer() {
    assert!(should_route_event(
        ViewerEventKind::MousePressed,
        true,
        Some(egui::Order::Background),
        false,
    ));
}

#[test]
fn mouse_press_over_floating_panel_does_not_route_to_viewer() {
    assert!(!should_route_event(
        ViewerEventKind::MousePressed,
        true,
        Some(egui::Order::Foreground),
        false,
    ));
}

#[test]
fn mouse_release_routes_when_viewer_has_pointer_capture() {
    assert!(should_route_event(
        ViewerEventKind::MouseReleased,
        false,
        Some(egui::Order::Foreground),
        true,
    ));
}

#[test]
fn cursor_move_inside_background_routes_without_capture() {
    assert!(should_route_event(
        ViewerEventKind::CursorMoved,
        true,
        None,
        false,
    ));
}

#[test]
fn mouse_wheel_over_floating_panel_does_not_route_to_viewer() {
    assert!(!should_route_event(
        ViewerEventKind::MouseWheel,
        true,
        Some(egui::Order::Middle),
        false,
    ));
}

#[test]
fn keyboard_and_modifiers_always_route_to_viewer_handler_layer() {
    assert!(should_route_event(
        ViewerEventKind::KeyboardInput,
        false,
        Some(egui::Order::Foreground),
        false,
    ));
    assert!(should_route_event(
        ViewerEventKind::ModifiersChanged,
        false,
        None,
        false
    ));
}

#[test]
fn keyboard_and_modifiers_keep_side_effects_when_they_enter_viewer_path() {
    let keyboard = dispatch_effects(ViewerEventKind::KeyboardInput, false);
    assert!(keyboard.evaluate_shortcuts);
    assert!(!keyboard.update_modifiers);

    let consumed_keyboard = dispatch_effects(ViewerEventKind::KeyboardInput, true);
    assert!(!consumed_keyboard.evaluate_shortcuts);

    let modifiers = dispatch_effects(ViewerEventKind::ModifiersChanged, false);
    assert!(modifiers.update_modifiers);
    assert!(!modifiers.evaluate_shortcuts);
}

#[test]
fn unrelated_events_do_not_route() {
    assert!(!should_route_event(
        ViewerEventKind::Other,
        true,
        Some(egui::Order::Background),
        false,
    ));
}
