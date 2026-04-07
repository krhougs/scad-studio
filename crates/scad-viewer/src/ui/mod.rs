pub mod camera_overlay;
pub mod log_panel;
pub mod param_editor;
pub mod settings_dialog;
pub mod side_panel;
pub mod status_bar;
pub mod toolbar;

use crate::app::{StudioApp, UiActions};
use scad_scene::{CameraMatrices, OrbitalCamera};
use scad_ui::theme;

pub fn show_app(
    studio: &mut StudioApp,
    ctx: &egui::Context,
    show_embedded_menu: bool,
    camera_matrices: CameraMatrices,
    camera: &OrbitalCamera,
    frame: crate::app::UiFrame<'_>,
) -> UiActions {
    show_app_with_mode(
        studio,
        ctx,
        show_embedded_menu,
        camera_matrices,
        camera,
        frame,
    )
}

#[allow(dead_code)]
pub fn show_embedded_app(
    studio: &mut StudioApp,
    ctx: &egui::Context,
    camera_matrices: CameraMatrices,
    camera: &OrbitalCamera,
    frame: crate::app::UiFrame<'_>,
) -> UiActions {
    show_app_with_mode(studio, ctx, true, camera_matrices, camera, frame)
}

fn show_app_with_mode(
    studio: &mut StudioApp,
    ctx: &egui::Context,
    embedded_mode: bool,
    camera_matrices: CameraMatrices,
    camera: &OrbitalCamera,
    frame: crate::app::UiFrame<'_>,
) -> UiActions {
    theme::apply(ctx);
    let previous_viewer_state = studio.viewer_state().clone();
    let mut actions = UiActions::default();

    if embedded_mode {
        toolbar::show_embedded(ctx, studio, &mut actions);
    } else {
        toolbar::show(ctx, studio, &mut actions, frame.settings_open);
    }

    status_bar::show(ctx, studio);
    let viewport_rect = ctx.available_rect();
    show_viewer_overlays(
        ctx,
        studio,
        camera_matrices,
        camera,
        frame,
        viewport_rect,
        &mut actions,
    );

    actions.viewer_state_changed =
        previous_viewer_state != *studio.viewer_state() || actions.camera_action.is_some();
    actions
}

/// 日志、侧栏、设置、gizmo、相机浮层；`viewport_rect` 为 3D 区域在屏幕上的矩形（用于坐标轴 gizmo 定位）。
pub fn show_viewer_overlays(
    ctx: &egui::Context,
    studio: &mut StudioApp,
    camera_matrices: CameraMatrices,
    camera: &OrbitalCamera,
    frame: crate::app::UiFrame<'_>,
    viewport_rect: egui::Rect,
    actions: &mut UiActions,
) {
    let log_entries = studio.log_entries().to_vec();
    let has_current_file = studio.has_current_file();
    let is_rendering = studio.is_rendering();

    let log_outcome = log_panel::show(
        ctx,
        studio.viewer_state_mut(),
        &log_entries,
        frame.config,
        viewport_rect,
    );
    if log_outcome.clear_requested {
        studio.clear_logs();
        actions.commands.push(crate::app::UiCommand::SaveSettings);
    }
    if log_outcome.save_settings {
        actions.commands.push(crate::app::UiCommand::SaveSettings);
    }
    side_panel::show(
        ctx,
        studio.viewer_state_mut(),
        has_current_file,
        is_rendering,
        actions,
        viewport_rect,
        side_panel::SidePanelFrame {
            document: frame.document,
            slicers: frame.slicers,
            config: frame.config,
        },
    );
    if settings_dialog::show(ctx, frame.settings_open, frame.config) {
        actions.commands.push(crate::app::UiCommand::SaveSettings);
    }
    scad_scene::gizmo::paint_overlay(
        ctx,
        studio.viewer_state().show_axis_gizmo,
        camera_matrices.view,
        viewport_rect,
    );

    camera_overlay::show(
        ctx,
        camera,
        actions,
        frame.config,
        studio.viewer_state().camera_overlay_open,
        viewport_rect,
    );
}
