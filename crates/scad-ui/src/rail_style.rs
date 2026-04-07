use egui::{Color32, CornerRadius, Rect, Stroke, StrokeKind, Ui};

use crate::theme::palette;

const RAIL_ACCENT: Color32 = palette::TEXT_ACCENT;
const FOCUS_RING: Color32 = palette::ACCENT;
const CHIP_FILL_IDLE: Color32 = palette::BG_PANEL;
const TOP_HIGHLIGHT_IDLE: Color32 = Color32::TRANSPARENT;
const TAB_IDLE_FILL: Color32 = Color32::from_rgb(24, 24, 24);
const TAB_CHIP_FILL_IDLE: Color32 = Color32::from_rgb(20, 20, 20);
const TAB_CHIP_FILL_ON_ACTIVE_TAB: Color32 = Color32::from_rgb(32, 32, 32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailItemState {
    Idle,
    Hovered,
    Active,
    Focused,
}

#[derive(Debug, Clone, Copy)]
pub struct RailMetrics {
    pub item_height: f32,
    pub switch_item_width: f32,
    pub kind_chip_width: f32,
    pub tab_min_width: f32,
    pub tab_max_width: f32,
    pub corner_radius: u8,
    pub item_padding_x: i8,
    pub item_padding_y: i8,
    pub content_gap: f32,
    pub close_button_size: f32,
    pub close_button_slot_width: f32,
    pub status_dot_slot_width: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RailItemVisuals {
    pub fill: Color32,
    pub stroke: Stroke,
    pub text: Color32,
    pub subtle_text: Color32,
    pub chip_fill: Color32,
    pub chip_text: Color32,
    pub top_highlight: Color32,
    pub focus_stroke: Stroke,
}

pub fn metrics() -> RailMetrics {
    RailMetrics {
        item_height: 30.0,
        switch_item_width: 72.0,
        kind_chip_width: 28.0,
        tab_min_width: 120.0,
        tab_max_width: 280.0,
        corner_radius: crate::theme::palette::SEGMENT_CORNER_RADIUS,
        item_padding_x: 9,
        item_padding_y: 4,
        content_gap: 6.0,
        close_button_size: 18.0,
        close_button_slot_width: 20.0,
        status_dot_slot_width: 10.0,
    }
}

pub fn content_height() -> f32 {
    let metrics = metrics();
    metrics.item_height - 2.0 * f32::from(metrics.item_padding_y)
}

pub fn resolve_item_state(active: bool, hovered: bool, focused: bool) -> RailItemState {
    if focused {
        RailItemState::Focused
    } else if active {
        RailItemState::Active
    } else if hovered {
        RailItemState::Hovered
    } else {
        RailItemState::Idle
    }
}

pub fn item_visuals(state: RailItemState) -> RailItemVisuals {
    match state {
        RailItemState::Idle => RailItemVisuals {
            fill: TAB_IDLE_FILL,
            stroke: Stroke::NONE,
            text: palette::TEXT_SECONDARY,
            subtle_text: palette::TEXT_SECONDARY,
            chip_fill: CHIP_FILL_IDLE,
            chip_text: palette::TEXT_SECONDARY,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::NONE,
        },
        RailItemState::Hovered => RailItemVisuals {
            fill: palette::BG_WIDGET,
            stroke: Stroke::NONE,
            text: palette::TEXT_PRIMARY,
            subtle_text: palette::TEXT_SECONDARY,
            chip_fill: CHIP_FILL_IDLE,
            chip_text: palette::TEXT_PRIMARY,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::NONE,
        },
        RailItemState::Active => RailItemVisuals {
            fill: palette::BG_WIDGET_HOVER,
            stroke: Stroke::NONE,
            text: palette::TEXT_BRIGHT,
            subtle_text: palette::TEXT_PRIMARY,
            chip_fill: Color32::from_rgba_premultiplied(
                RAIL_ACCENT.r(),
                RAIL_ACCENT.g(),
                RAIL_ACCENT.b(),
                14,
            ),
            chip_text: palette::TEXT_ACCENT,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::NONE,
        },
        RailItemState::Focused => RailItemVisuals {
            fill: palette::BG_WIDGET_HOVER,
            stroke: Stroke::NONE,
            text: palette::TEXT_BRIGHT,
            subtle_text: palette::TEXT_PRIMARY,
            chip_fill: Color32::from_rgba_premultiplied(
                RAIL_ACCENT.r(),
                RAIL_ACCENT.g(),
                RAIL_ACCENT.b(),
                18,
            ),
            chip_text: palette::TEXT_ACCENT,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::new(1.0, FOCUS_RING),
        },
    }
}

pub fn document_tab_corner_radius() -> CornerRadius {
    CornerRadius::same(metrics().corner_radius)
}

pub fn document_tab_visuals(state: RailItemState) -> RailItemVisuals {
    match state {
        RailItemState::Idle => RailItemVisuals {
            fill: TAB_IDLE_FILL,
            stroke: Stroke::NONE,
            text: palette::TEXT_SECONDARY,
            subtle_text: palette::TEXT_SECONDARY,
            chip_fill: TAB_CHIP_FILL_IDLE,
            chip_text: palette::TEXT_SECONDARY,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::NONE,
        },
        RailItemState::Hovered => RailItemVisuals {
            fill: palette::BG_WIDGET,
            stroke: Stroke::NONE,
            text: palette::TEXT_PRIMARY,
            subtle_text: palette::TEXT_SECONDARY,
            chip_fill: TAB_CHIP_FILL_IDLE,
            chip_text: palette::TEXT_PRIMARY,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::NONE,
        },
        RailItemState::Active => RailItemVisuals {
            fill: palette::BG_WIDGET_HOVER,
            stroke: Stroke::NONE,
            text: palette::TEXT_BRIGHT,
            subtle_text: palette::TEXT_PRIMARY,
            chip_fill: TAB_CHIP_FILL_ON_ACTIVE_TAB,
            chip_text: palette::TEXT_BRIGHT,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::NONE,
        },
        RailItemState::Focused => RailItemVisuals {
            fill: palette::BG_WIDGET_HOVER,
            stroke: Stroke::NONE,
            text: palette::TEXT_BRIGHT,
            subtle_text: palette::TEXT_PRIMARY,
            chip_fill: TAB_CHIP_FILL_ON_ACTIVE_TAB,
            chip_text: palette::TEXT_BRIGHT,
            top_highlight: TOP_HIGHLIGHT_IDLE,
            focus_stroke: Stroke::new(1.0, FOCUS_RING),
        },
    }
}

pub fn close_button_color(emphasized: bool) -> Color32 {
    let base = if emphasized {
        palette::TEXT_PRIMARY
    } else {
        palette::TEXT_SECONDARY
    };
    let alpha = if emphasized { 192 } else { 56 };
    Color32::from_rgba_premultiplied(base.r(), base.g(), base.b(), alpha)
}

pub fn accent_color() -> Color32 {
    RAIL_ACCENT
}

pub fn paint_item_surface(ui: &Ui, rect: Rect, visuals: RailItemVisuals) {
    paint_rail_item_surface(
        ui,
        rect,
        visuals,
        CornerRadius::same(metrics().corner_radius),
    );
}

pub fn paint_rail_item_surface(
    ui: &Ui,
    rect: Rect,
    visuals: RailItemVisuals,
    corner_radius: CornerRadius,
) {
    if visuals.stroke == Stroke::NONE {
        ui.painter()
            .rect_filled(rect, corner_radius, visuals.fill);
    } else {
        ui.painter().rect(
            rect,
            corner_radius,
            visuals.fill,
            visuals.stroke,
            StrokeKind::Outside,
        );
    }
    if visuals.top_highlight != Color32::TRANSPARENT {
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 1.5, rect.top() + 1.0),
                egui::pos2(rect.right() - 1.5, rect.top() + 1.0),
            ],
            Stroke::new(1.0, visuals.top_highlight),
        );
    }
    if visuals.focus_stroke != Stroke::NONE {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            corner_radius,
            visuals.focus_stroke,
            StrokeKind::Outside,
        );
    }
}
