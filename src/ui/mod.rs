pub mod log_panel;
pub mod param_editor;
pub mod side_panel;
pub mod settings_dialog;
pub mod status_bar;
pub mod toolbar;

use crate::{
    app::{StudioApp, UiActions},
    camera::CameraMatrices,
};

pub fn show_app(
    studio: &mut StudioApp,
    ctx: &egui::Context,
    show_embedded_menu: bool,
    camera_matrices: CameraMatrices,
    frame: crate::app::UiFrame<'_>,
) -> UiActions {
    let previous_viewer_state = studio.viewer_state().clone();
    let log_entries = studio.log_entries().to_vec();
    let has_current_file = studio.has_current_file();
    let mut actions = UiActions::default();
    if show_embedded_menu {
        show_menu(studio, ctx, &mut actions, frame.settings_open);
    }
    toolbar::show(ctx, studio, &mut actions);
    status_bar::show(ctx, studio);
    if log_panel::show(ctx, studio.viewer_state_mut(), &log_entries) {
        studio.clear_logs();
    }
    side_panel::show(
        ctx,
        studio.viewer_state(),
        has_current_file,
        frame.document,
        frame.slicers,
        &mut actions,
    );
    if settings_dialog::show(ctx, frame.settings_open, frame.config) {
        actions.commands.push(crate::app::UiCommand::SaveSettings);
    }
    crate::gizmo::paint_overlay(
        ctx,
        studio.viewer_state().show_axis_gizmo,
        camera_matrices.view,
    );
    actions.viewer_state_changed = previous_viewer_state != *studio.viewer_state();
    actions
}

fn show_menu(
    studio: &mut StudioApp,
    ctx: &egui::Context,
    actions: &mut UiActions,
    settings_open: &mut bool,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    actions.open_file = true;
                    ui.close();
                }
                if ui.button("设置").clicked() {
                    *settings_open = true;
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                let has_file = studio.has_current_file();
                let side_label = if studio.viewer_state().side_panel_open {
                    "隐藏右侧面板"
                } else {
                    "显示右侧面板"
                };
                if ui
                    .add_enabled(has_file, egui::Button::new(side_label))
                    .clicked()
                {
                    studio.viewer_state_mut().toggle_side_panel();
                    ui.close();
                }
                let log_label = if studio.viewer_state().log_panel_open {
                    "折叠日志面板"
                } else {
                    "展开日志面板"
                };
                if ui.button(log_label).clicked() {
                    studio.viewer_state_mut().toggle_log_panel();
                    ui.close();
                }
            });
        });
    });
}
