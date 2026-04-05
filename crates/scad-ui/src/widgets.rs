use crate::theme::palette;

pub fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .color(palette::TEXT_SECONDARY)
            .size(10.0),
    );
    ui.add_space(4.0);
}

pub fn section_header(ui: &mut egui::Ui, title: &str) {
    section_label(ui, title);
    ui.add_space(2.0);
    ui.separator();
}

pub fn toolbar_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(palette::TEXT_SECONDARY)
            .size(11.0),
    );
}

pub fn selectable_button(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let text = if selected {
        egui::RichText::new(label)
            .color(palette::TEXT_BRIGHT)
            .size(12.0)
    } else {
        egui::RichText::new(label)
            .color(palette::TEXT_SECONDARY)
            .size(12.0)
    };
    let fill = if selected {
        palette::BG_WIDGET_ACTIVE
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.add(
        egui::Button::new(text)
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(4)),
    )
}

pub fn toggle_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let text = if enabled {
        egui::RichText::new(label)
            .color(palette::TEXT_ACCENT)
            .size(12.0)
    } else {
        egui::RichText::new(label)
            .color(palette::TEXT_SECONDARY)
            .size(12.0)
    };
    let fill = if enabled {
        palette::BG_SELECTION
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.add(
        egui::Button::new(text)
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(4)),
    )
}

pub fn small_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .color(palette::TEXT_SECONDARY)
                .size(11.0),
        )
        .fill(egui::Color32::TRANSPARENT)
        .corner_radius(egui::CornerRadius::same(3)),
    )
}

pub fn filled_small_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .color(palette::TEXT_PRIMARY)
                .size(11.0),
        )
        .fill(palette::BG_WIDGET)
        .corner_radius(egui::CornerRadius::same(3))
        .min_size(egui::vec2(28.0, 0.0)),
    )
}

pub fn icon_button(ui: &mut egui::Ui, label: &str, tooltip: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .color(palette::TEXT_SECONDARY)
                .size(11.0),
        )
        .fill(egui::Color32::TRANSPARENT)
        .corner_radius(egui::CornerRadius::same(3)),
    )
    .on_hover_text(tooltip)
}
