use egui::{Color32, Stroke};
use scad_ui::{
    document_tabs, panel_switcher,
    rail_style::{self, RailItemState},
    theme::palette,
};

#[test]
fn shared_rail_metrics_match_document_tabs_and_panel_switcher() {
    let metrics = rail_style::metrics();

    assert_eq!(metrics.item_height, document_tabs::tab_height());
    assert_eq!(metrics.item_height, panel_switcher::item_height());
    assert_eq!(metrics.corner_radius, palette::SEGMENT_CORNER_RADIUS);
}

#[test]
fn rail_item_visuals_increase_emphasis_from_idle_to_hover_to_active() {
    let idle = rail_style::item_visuals(RailItemState::Idle);
    let hover = rail_style::item_visuals(RailItemState::Hovered);
    let active = rail_style::item_visuals(RailItemState::Active);
    let focus = rail_style::item_visuals(RailItemState::Focused);

    assert!(brightness(idle.fill) < brightness(hover.fill));
    assert!(brightness(hover.fill) < brightness(active.fill));
    assert!(brightness(active.text) > brightness(idle.text));
    assert_eq!(idle.stroke, Stroke::NONE);
    assert_eq!(active.stroke, Stroke::NONE);
    assert_ne!(focus.focus_stroke, Stroke::NONE);
}

#[test]
fn trailing_close_slot_keeps_hit_area_stable() {
    let metrics = rail_style::metrics();

    assert!(metrics.close_button_slot_width > metrics.close_button_size);
    assert!(metrics.close_button_size >= 18.0);
    assert!(rail_style::content_height() < metrics.item_height);
    assert!(metrics.close_button_size <= rail_style::content_height());
    assert!(metrics.tab_min_width <= metrics.tab_max_width);
    assert!(metrics.kind_chip_width > 0.0);
}

#[test]
fn document_tab_idle_is_flat_segment_without_outline() {
    let idle = rail_style::document_tab_visuals(RailItemState::Idle);
    assert_eq!(idle.fill, Color32::from_rgb(24, 24, 24));
    assert_eq!(idle.stroke, Stroke::NONE);
}

#[test]
fn document_tab_active_uses_emphasis_surface_like_toolbar_segments() {
    let active = rail_style::document_tab_visuals(RailItemState::Active);
    assert_eq!(active.fill, palette::BG_WIDGET_HOVER);
}

fn brightness(color: Color32) -> u16 {
    u16::from(color.r()) + u16::from(color.g()) + u16::from(color.b())
}
