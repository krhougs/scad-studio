use crate::app::{ProjectionMode, StudioApp};
use scad_ui::theme::{self, palette};

pub fn show(ctx: &egui::Context, studio: &StudioApp) {
    egui::TopBottomPanel::bottom("status_bar")
        .frame(theme::panel_bar_frame(10, 3))
        .show(ctx, |ui| {
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
        });
}
