pub fn allocate_viewport_ui<R>(
    ui: &mut egui::Ui,
    desired_size: egui::Vec2,
    layout: egui::Layout,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Rect, R) {
    let (_, rect) = ui.allocate_space(desired_size);
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
    let result = add_contents(&mut child_ui);
    (rect, result)
}

pub fn allocate_filled_strip_ui<R>(
    ui: &mut egui::Ui,
    desired_size: egui::Vec2,
    margin: egui::Margin,
    fill: egui::Color32,
    layout: egui::Layout,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Rect, egui::Rect, R) {
    let (_, outer_rect) = ui.allocate_space(desired_size);
    ui.painter()
        .rect_filled(outer_rect, egui::CornerRadius::ZERO, fill);
    let inner_rect = egui::Rect::from_min_max(
        egui::pos2(
            outer_rect.min.x + f32::from(margin.left),
            outer_rect.min.y + f32::from(margin.top),
        ),
        egui::pos2(
            outer_rect.max.x - f32::from(margin.right),
            outer_rect.max.y - f32::from(margin.bottom),
        ),
    );
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner_rect).layout(layout));
    let result = add_contents(&mut child_ui);
    (outer_rect, inner_rect, result)
}
