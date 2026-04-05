use crate::app::StudioApp;
use scad_data::LogLevel;
use scad_ui::theme::palette;
use scad_ui::widgets::{small_button, toolbar_label};

pub fn show(ctx: &egui::Context, app: &mut StudioApp) {
    if !app.log_panel_open() {
        return;
    }
    egui::TopBottomPanel::bottom("studio_log_panel")
        .resizable(true)
        .default_height(160.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("日志");
                toolbar_label(ui, &format!("{} 条", app.log_entries().len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_button(ui, "折叠").clicked() {
                        app.toggle_log_panel();
                    }
                    if small_button(ui, "清空").clicked() {
                        app.clear_logs();
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in app.log_entries() {
                        ui.colored_label(color_for(entry.level), &entry.message);
                    }
                });
        });
}

fn color_for(level: LogLevel) -> egui::Color32 {
    match level {
        LogLevel::Info => palette::LOG_INFO,
        LogLevel::Warning => palette::LOG_WARN,
        LogLevel::Error => palette::LOG_ERROR,
    }
}
