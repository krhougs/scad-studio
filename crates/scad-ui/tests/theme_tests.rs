use scad_ui::theme::palette;

#[test]
fn theme_palette_keeps_expected_accent_color() {
    assert_eq!(palette::ACCENT, egui::Color32::from_rgb(55, 100, 160));
}

#[test]
fn theme_segment_corner_radius_matches_rail_metrics() {
    assert_eq!(palette::SEGMENT_CORNER_RADIUS, 10);
}

#[test]
fn floating_panel_margins_are_asymmetric_for_tighter_top_tab_strip() {
    assert_eq!(palette::FLOATING_PANEL_MARGIN_TOP, 0);
    assert_eq!(palette::FLOATING_PANEL_MARGIN_H, 6);
    assert_eq!(palette::FLOATING_PANEL_MARGIN_BOTTOM, 10);
    assert_eq!(palette::TAB_STRIP_GAP_BELOW, 8.0);
}
