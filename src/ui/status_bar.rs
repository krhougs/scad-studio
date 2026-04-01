use crate::app::{ProjectionMode, StudioApp};

pub fn show(ctx: &egui::Context, studio: &StudioApp) {
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("文件: {}", studio.current_file_label()));
            ui.separator();
            ui.label(studio.status_message());
            ui.separator();
            ui.label(format!(
                "投影: {}",
                projection_label(studio.viewer_state().projection_mode)
            ));
        });
    });
}

fn projection_label(mode: ProjectionMode) -> &'static str {
    match mode {
        ProjectionMode::Perspective => "透视",
        ProjectionMode::Orthographic => "正交",
    }
}
