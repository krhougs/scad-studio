#[test]
fn viewer_active_uses_transparent_central_panel() {
    let frame = scad_ui::work_area_frame::central_panel_frame(true);
    assert_eq!(frame.fill, egui::Color32::TRANSPARENT);
}

#[test]
fn non_viewer_keeps_default_central_panel_fill() {
    let frame = scad_ui::work_area_frame::central_panel_frame(false);
    assert_eq!(frame.fill, scad_ui::theme::palette::BG_WINDOW);
}
