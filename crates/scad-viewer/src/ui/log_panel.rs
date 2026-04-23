use crate::app::{LogEntry, LogLevel, ViewerState};
use scad_ui::theme::{self, palette};
use scad_ui::widgets::small_button;
use studio_common::AppConfig;

const PANEL_WIDTH: f32 = 400.0;

pub struct LogPanelOutcome {
    pub clear_requested: bool,
    pub save_settings: bool,
}

pub fn show(
    ctx: &egui::Context,
    viewer_state: &mut ViewerState,
    logs: &[LogEntry],
    config: &mut AppConfig,
    viewport_rect: egui::Rect,
) -> LogPanelOutcome {
    let mut clear_requested = false;
    let mut save_settings = false;

    if !viewer_state.log_panel_open {
        return LogPanelOutcome {
            clear_requested,
            save_settings,
        };
    }

    let opacity = config.floating_panel_opacity.clamp(0.1, 1.0);

    let default_size = config
        .log_panel_size
        .map(|s| egui::vec2(s[0], s[1]))
        .unwrap_or(egui::vec2(PANEL_WIDTH, 250.0));
    let pos = stored_panel_pos(
        config.log_panel_pos,
        viewport_rect,
        default_size,
        egui::vec2(12.0, viewport_rect.height() - default_size.y - 12.0),
    );

    let response = egui::Window::new("log_panel")
        .title_bar(false)
        .collapsible(false)
        .resizable(true)
        .movable(true)
        .constrain(true)
        .constrain_to(viewport_rect)
        .default_size(default_size)
        .current_pos(pos)
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
                    if small_button(ui, "清空").clicked() {
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
            config.log_panel_pos = Some(panel_offset(rect, viewport_rect));
        }
        if inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.log_panel_size = Some([rect.width(), rect.height()]);
            save_settings = true;
        }
    }

    LogPanelOutcome {
        clear_requested,
        save_settings,
    }
}

fn stored_panel_pos(
    stored_offset: Option<[f32; 2]>,
    viewport_rect: egui::Rect,
    panel_size: egui::Vec2,
    default_offset: egui::Vec2,
) -> egui::Pos2 {
    let offset = stored_offset
        .map(|offset| egui::vec2(offset[0], offset[1]))
        .unwrap_or(default_offset);
    let x = (viewport_rect.min.x + offset.x).clamp(
        viewport_rect.min.x,
        (viewport_rect.max.x - panel_size.x).max(viewport_rect.min.x),
    );
    let y = (viewport_rect.min.y + offset.y).clamp(
        viewport_rect.min.y,
        (viewport_rect.max.y - panel_size.y).max(viewport_rect.min.y),
    );
    egui::pos2(x, y)
}

fn panel_offset(rect: egui::Rect, viewport_rect: egui::Rect) -> [f32; 2] {
    [
        rect.min.x - viewport_rect.min.x,
        rect.min.y - viewport_rect.min.y,
    ]
}

fn color_for(level: LogLevel) -> egui::Color32 {
    match level {
        LogLevel::Info => palette::LOG_INFO,
        LogLevel::Warning => palette::LOG_WARN,
        LogLevel::Error => palette::LOG_ERROR,
    }
}
