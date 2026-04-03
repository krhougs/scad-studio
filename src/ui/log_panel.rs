use crate::app::{LogEntry, LogLevel, ViewerState};
use crate::config::AppConfig;
use crate::ui::theme::{self, palette};

const PANEL_WIDTH: f32 = 400.0;

pub fn show(
    ctx: &egui::Context,
    viewer_state: &mut ViewerState,
    logs: &[LogEntry],
    config: &mut AppConfig,
) -> bool {
    let mut clear_requested = false;

    if !viewer_state.log_panel_open {
        return false;
    }

    let opacity = config.floating_panel_opacity.clamp(0.1, 1.0);

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(screen.min.x + 12.0, screen.max.y - 200.0);
    let pos = config
        .log_panel_pos
        .map(|p| egui::pos2(p[0], p[1]))
        .unwrap_or(default_pos);

    let default_size = config
        .log_panel_size
        .map(|s| egui::vec2(s[0], s[1]))
        .unwrap_or(egui::vec2(PANEL_WIDTH, 250.0));

    let response = egui::Window::new("log_panel")
        .title_bar(false)
        .collapsible(false)
        .resizable(true)
        .movable(true)
        .constrain(true)
        .default_size(default_size)
        .default_pos(pos)
        .frame(theme::floating_frame(opacity))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("日志")
                        .color(palette::TEXT_PRIMARY)
                        .strong()
                        .size(13.0),
                );
                ui.label(
                    egui::RichText::new(format!("{} 条", logs.len()))
                        .color(palette::TEXT_SECONDARY)
                        .size(11.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::close_button(ui, "关闭日志面板").clicked() {
                        viewer_state.log_panel_open = false;
                    }
                    if small_btn(ui, "清空").clicked() {
                        clear_requested = true;
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if logs.is_empty() {
                        ui.label(
                            egui::RichText::new("暂无日志")
                                .color(palette::TEXT_SECONDARY)
                                .italics()
                                .size(12.0),
                        );
                        return;
                    }
                    for entry in logs {
                        ui.label(
                            egui::RichText::new(&entry.message)
                                .color(color_for(entry.level))
                                .size(12.0),
                        );
                    }
                });
        });

    // 持久化拖动后的位置和尺寸
    if let Some(inner) = response {
        if inner.response.dragged() || inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.log_panel_pos = Some([rect.min.x, rect.min.y]);
        }
        if inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.log_panel_size = Some([rect.width(), rect.height()]);
        }
    }

    clear_requested
}

fn color_for(level: LogLevel) -> egui::Color32 {
    match level {
        LogLevel::Info => palette::LOG_INFO,
        LogLevel::Warning => palette::LOG_WARN,
        LogLevel::Error => palette::LOG_ERROR,
    }
}

fn small_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
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
