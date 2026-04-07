use crate::app::{ProjectionMode, StudioApp};
use scad_ui::theme::{self, palette};

const WRAP_THRESHOLD: f32 = 520.0;

pub fn show(ctx: &egui::Context, studio: &StudioApp) {
    egui::TopBottomPanel::bottom("status_bar")
        .frame(theme::panel_bar_frame(10, 3))
        .show(ctx, |ui| {
            paint_status_row(ui, studio);
        });
}

/// 在已有 `Ui` 内绘制状态栏一行（用于 SCAD Studio 标签页内嵌）。
pub fn paint_status_row(ui: &mut egui::Ui, studio: &StudioApp) {
    if wraps_for_width(ui.available_width()) {
        paint_status_row_wrapped(ui, studio);
    } else {
        paint_status_row_wide(ui, studio);
    }
}

/// 供 SCAD Studio 内嵌标签页按可用宽度预留行高；独立 `scad-viewer` 二进制使用底部面板固定高度。
#[allow(dead_code)]
pub fn embedded_height(available_width: f32) -> f32 {
    if wraps_for_width(available_width) {
        46.0
    } else {
        28.0
    }
}

fn paint_status_row_wide(ui: &mut egui::Ui, studio: &StudioApp) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(studio.current_file_label())
                .color(palette::TEXT_PRIMARY)
                .size(11.0),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let proj = match studio.viewer_state().projection_mode {
                ProjectionMode::Perspective => "透视",
                ProjectionMode::Orthographic => "正交",
            };
            ui.label(
                egui::RichText::new(proj)
                    .color(palette::TEXT_SECONDARY)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new("\u{2022}")
                    .color(palette::STROKE_MED)
                    .size(8.0),
            );
            ui.label(
                egui::RichText::new(studio.status_message())
                    .color(palette::TEXT_SECONDARY)
                    .size(11.0),
            );
        });
    });
}

fn paint_status_row_wrapped(ui: &mut egui::Ui, studio: &StudioApp) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(studio.current_file_label())
                .color(palette::TEXT_PRIMARY)
                .size(11.0),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(studio.status_message())
                    .color(palette::TEXT_SECONDARY)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new("\u{2022}")
                    .color(palette::STROKE_MED)
                    .size(8.0),
            );
            let proj = match studio.viewer_state().projection_mode {
                ProjectionMode::Perspective => "透视",
                ProjectionMode::Orthographic => "正交",
            };
            ui.label(
                egui::RichText::new(proj)
                    .color(palette::TEXT_SECONDARY)
                    .size(11.0),
            );
        });
    });
}

fn wraps_for_width(available_width: f32) -> bool {
    available_width < WRAP_THRESHOLD
}
