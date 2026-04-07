use egui::{Align2, RichText, Sense, TextStyle, TextWrapMode, WidgetText};

use crate::rail_style;

#[derive(Debug, Clone, Copy)]
pub struct PanelSwitchItem<'a> {
    pub label: &'a str,
    pub active: bool,
}

pub fn item_height() -> f32 {
    rail_style::metrics().item_height
}

pub fn show_panel_switcher(ui: &mut egui::Ui, items: &[PanelSwitchItem<'_>]) -> Option<usize> {
    let mut activate = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (index, item) in items.iter().enumerate() {
            if show_switch_item(ui, item) {
                activate = Some(index);
            }
        }
    });

    activate
}

fn show_switch_item(ui: &mut egui::Ui, item: &PanelSwitchItem<'_>) -> bool {
    let metrics = rail_style::metrics();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(metrics.switch_item_width, metrics.item_height),
        Sense::click(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let state =
        rail_style::resolve_item_state(item.active, response.hovered(), response.has_focus());
    let visuals = rail_style::item_visuals(state);
    rail_style::paint_item_surface(ui, rect, visuals);

    let inner_rect = rect.shrink2(egui::vec2(
        f32::from(metrics.item_padding_x),
        f32::from(metrics.item_padding_y),
    ));
    let galley = WidgetText::from(RichText::new(item.label).size(12.0).color(visuals.text))
        .into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            inner_rect.width(),
            TextStyle::Button,
        );
    let text_pos = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), inner_rect)
        .min
        - galley.rect.min.to_vec2();
    ui.painter().galley(text_pos, galley, visuals.text);

    response.clicked()
}
