pub mod camera_overlay;
pub mod log_panel;
pub mod param_editor;
pub mod settings_dialog;
pub mod side_panel;
pub mod status_bar;
pub mod theme;
pub mod toolbar;

use crate::{
    app::{StudioApp, UiActions},
    camera::{CameraMatrices, OrbitalCamera},
};

pub fn show_app(
    studio: &mut StudioApp,
    ctx: &egui::Context,
    _show_embedded_menu: bool,
    camera_matrices: CameraMatrices,
    camera: &OrbitalCamera,
    frame: crate::app::UiFrame<'_>,
) -> UiActions {
    theme::apply(ctx);
    let previous_viewer_state = studio.viewer_state().clone();
    let log_entries = studio.log_entries().to_vec();
    let has_current_file = studio.has_current_file();
    let is_rendering = studio.is_rendering();
    let mut actions = UiActions::default();

    // 统一顶部工具栏（合并原菜单栏功能）
    toolbar::show(ctx, studio, &mut actions, frame.settings_open);

    status_bar::show(ctx, studio);
    if log_panel::show(ctx, studio.viewer_state_mut(), &log_entries, frame.config) {
        studio.clear_logs();
        actions.commands.push(crate::app::UiCommand::SaveSettings);
    }
    side_panel::show(
        ctx,
        studio.viewer_state_mut(),
        has_current_file,
        is_rendering,
        frame.document,
        frame.slicers,
        &mut actions,
        frame.config,
    );
    if settings_dialog::show(ctx, frame.settings_open, frame.config) {
        actions.commands.push(crate::app::UiCommand::SaveSettings);
    }
    let viewport_rect = ctx.available_rect();
    crate::gizmo::paint_overlay(
        ctx,
        studio.viewer_state().show_axis_gizmo,
        camera_matrices.view,
        viewport_rect,
    );

    // 相机控制浮层
    camera_overlay::show(
        ctx,
        camera,
        &mut actions,
        frame.config,
        studio.viewer_state().camera_overlay_open,
    );

    actions.viewer_state_changed =
        previous_viewer_state != *studio.viewer_state() || actions.camera_action.is_some();
    actions
}
