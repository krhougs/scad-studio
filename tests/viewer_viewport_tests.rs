#[path = "../src/viewer_viewport.rs"]
mod viewer_viewport;

#[test]
fn viewport_rect_keeps_requested_size_when_contents_are_empty() {
    egui::__run_test_ui(|ui| {
        let desired = egui::vec2(480.0, 320.0);
        let (rect, ()) = viewer_viewport::allocate_viewport_ui(
            ui,
            desired,
            egui::Layout::top_down(egui::Align::LEFT),
            |_| {},
        );
        assert_eq!(rect.size(), desired);
    });
}

#[test]
fn viewport_rect_keeps_requested_size_when_contents_exist() {
    egui::__run_test_ui(|ui| {
        let desired = egui::vec2(320.0, 200.0);
        let (rect, ()) = viewer_viewport::allocate_viewport_ui(
            ui,
            desired,
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.label("loading");
            },
        );
        assert_eq!(rect.size(), desired);
    });
}

#[test]
fn filled_strip_keeps_requested_size_and_insets_content_rect() {
    egui::__run_test_ui(|ui| {
        let desired = egui::vec2(400.0, 40.0);
        let margin = egui::Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 4,
        };
        let (outer, inner, ()) = viewer_viewport::allocate_filled_strip_ui(
            ui,
            desired,
            margin,
            egui::Color32::BLACK,
            egui::Layout::left_to_right(egui::Align::Center),
            |_| {},
        );
        assert_eq!(outer.size(), desired);
        assert_eq!(inner.width(), desired.x - 16.0);
        assert_eq!(inner.height(), desired.y - 8.0);
    });
}
