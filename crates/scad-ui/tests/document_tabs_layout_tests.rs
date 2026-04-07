use egui::Stroke;
use scad_ui::document_tabs::{
    active_tab_extension_height, rail_bottom_padding, rail_frame, rail_height,
    rail_inner_content_height, rail_show_separator_line, rail_vertical_padding, tab_height,
    tab_rail_pills_center_y_from_strip_top,
};

#[test]
fn rail_height_fits_tab_height_with_vertical_padding() {
    assert!(
        rail_height() >= tab_height() + f32::from(rail_vertical_padding() + rail_bottom_padding()),
        "tab rail height should fully contain tab height plus top/bottom padding",
    );
}

#[test]
fn rail_height_matches_padding_plus_inner_content_region() {
    assert_eq!(
        rail_height(),
        f32::from(rail_vertical_padding() + rail_bottom_padding()) + rail_inner_content_height(),
    );
}

#[test]
fn tab_rail_pills_center_y_matches_inner_region_midpoint() {
    let expected = f32::from(rail_vertical_padding()) + rail_inner_content_height() * 0.5;
    assert_eq!(tab_rail_pills_center_y_from_strip_top(), expected);
}

#[test]
fn rail_inner_region_leaves_vertical_slack_around_tab_pill() {
    assert!(
        rail_inner_content_height() >= tab_height() + 3.0,
        "inner strip should leave a few pixels above and below the tab for clip and rounding",
    );
}

#[test]
fn rail_frame_has_no_divider_stroke() {
    assert_eq!(rail_frame().stroke, Stroke::NONE);
}

#[test]
fn rail_top_panel_separator_line_is_disabled() {
    assert!(!rail_show_separator_line());
}

#[test]
fn rail_keeps_normal_bottom_padding() {
    assert!(rail_bottom_padding() >= 4);
}

#[test]
fn active_tab_extension_is_disabled_so_tabs_keep_bottom_margin() {
    assert_eq!(active_tab_extension_height(), 0);
}
