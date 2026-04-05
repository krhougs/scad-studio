use scad_ui::theme::palette;

#[test]
fn theme_palette_keeps_expected_accent_color() {
    assert_eq!(palette::ACCENT, egui::Color32::from_rgb(55, 100, 160));
}
