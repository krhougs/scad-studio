use crate::app::{LogEntry, LogLevel, ViewerState};

pub fn show(ctx: &egui::Context, viewer_state: &mut ViewerState, logs: &[LogEntry]) -> bool {
    let mut clear_requested = false;
    egui::TopBottomPanel::bottom("log_panel")
        .resizable(true)
        .default_height(160.0)
        .show_animated(ctx, viewer_state.log_panel_open, |ui| {
            ui.horizontal(|ui| {
                ui.heading("日志");
                ui.separator();
                ui.label(format!("{} 条", logs.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("清空").clicked() {
                        clear_requested = true;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if logs.is_empty() {
                        ui.label("暂无日志");
                        return;
                    }
                    for entry in logs {
                        ui.colored_label(color_for(entry.level), &entry.message);
                    }
                });
        });
    clear_requested
}

fn color_for(level: LogLevel) -> egui::Color32 {
    match level {
        LogLevel::Info => egui::Color32::from_rgb(120, 180, 255),
        LogLevel::Warning => egui::Color32::from_rgb(255, 196, 93),
        LogLevel::Error => egui::Color32::from_rgb(255, 110, 110),
    }
}
