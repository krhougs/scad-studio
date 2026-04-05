use crate::app::{ColorMode, ProjectionMode, RenderMode, StudioApp, UiActions, ViewerState};
use scad_ui::theme;
use scad_ui::widgets::{selectable_button, toggle_button, toolbar_label};

pub fn show(
    ctx: &egui::Context,
    studio: &mut StudioApp,
    actions: &mut UiActions,
    settings_open: &mut bool,
) {
    show_toolbar(ctx, studio, actions, Some(settings_open));
}

pub fn show_embedded(ctx: &egui::Context, studio: &mut StudioApp, actions: &mut UiActions) {
    show_toolbar(ctx, studio, actions, None);
}

fn show_toolbar(
    ctx: &egui::Context,
    studio: &mut StudioApp,
    actions: &mut UiActions,
    settings_open: Option<&mut bool>,
) {
    let has_current_file = studio.has_current_file();
    egui::TopBottomPanel::top("toolbar")
        .frame(theme::panel_bar_frame(8, 4))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(settings_open) = settings_open {
                    file_group(ui, actions, settings_open);
                    ui.add(egui::Separator::default().spacing(12.0).vertical());
                }

                // 渲染模式
                render_mode_group(ui, studio.viewer_state_mut());

                ui.add(egui::Separator::default().spacing(12.0).vertical());

                // 颜色模式
                color_mode_group(ui, studio.viewer_state_mut());

                ui.add(egui::Separator::default().spacing(12.0).vertical());

                // 投影
                projection_group(ui, studio.viewer_state_mut());

                ui.add(egui::Separator::default().spacing(12.0).vertical());

                // 环境与效果
                toggle_group(ui, studio.viewer_state_mut());

                ui.add(egui::Separator::default().spacing(12.0).vertical());

                // 面板切换
                panel_group(ui, studio.viewer_state_mut(), has_current_file);
            });
        });
}

fn file_group(ui: &mut egui::Ui, actions: &mut UiActions, settings_open: &mut bool) {
    if ui
        .add(
            egui::Button::new(egui::RichText::new("\u{1f4c2} 打开").size(13.0))
                .corner_radius(egui::CornerRadius::same(4)),
        )
        .clicked()
    {
        actions.open_file = true;
    }
    if ui
        .add(
            egui::Button::new(egui::RichText::new("\u{2699}").size(14.0))
                .corner_radius(egui::CornerRadius::same(4)),
        )
        .on_hover_text("设置")
        .clicked()
    {
        *settings_open = true;
    }
}

fn render_mode_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    toolbar_label(ui, "渲染");
    if selectable_button(ui, vs.render_mode == RenderMode::Solid, "Solid").clicked() {
        vs.render_mode = RenderMode::Solid;
    }
    let wire_btn = selectable_button(ui, vs.render_mode == RenderMode::Wireframe, "Wire");
    if !vs.wireframe_supported {
        // disabled state — just show dimmed
    } else if wire_btn.clicked() {
        vs.render_mode = RenderMode::Wireframe;
    }
    if selectable_button(ui, vs.render_mode == RenderMode::XRay, "X-Ray").clicked() {
        vs.render_mode = RenderMode::XRay;
    }
}

fn color_mode_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    toolbar_label(ui, "颜色");
    if selectable_button(ui, vs.color_mode == ColorMode::Mono, "Mono").clicked() {
        vs.color_mode = ColorMode::Mono;
    }
    if selectable_button(ui, vs.color_mode == ColorMode::Color, "Color").clicked() {
        vs.color_mode = ColorMode::Color;
    }
}

fn projection_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    toolbar_label(ui, "投影");
    if selectable_button(
        ui,
        vs.projection_mode == ProjectionMode::Perspective,
        "透视",
    )
    .clicked()
    {
        vs.projection_mode = ProjectionMode::Perspective;
    }
    if selectable_button(
        ui,
        vs.projection_mode == ProjectionMode::Orthographic,
        "正交",
    )
    .clicked()
    {
        vs.projection_mode = ProjectionMode::Orthographic;
    }
}

fn toggle_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    if toggle_button(ui, vs.show_grid, "网格").clicked() {
        vs.show_grid = !vs.show_grid;
    }
    if toggle_button(ui, vs.show_build_plate, "底板").clicked() {
        vs.show_build_plate = !vs.show_build_plate;
    }
    if toggle_button(ui, vs.show_axis_gizmo, "坐标轴").clicked() {
        vs.show_axis_gizmo = !vs.show_axis_gizmo;
    }
    if toggle_button(ui, vs.shadows_enabled, "阴影").clicked() {
        vs.shadows_enabled = !vs.shadows_enabled;
    }
    if toggle_button(ui, vs.fog_enabled, "雾效").clicked() {
        vs.fog_enabled = !vs.fog_enabled;
    }
    if toggle_button(ui, vs.clip_plane_enabled, "剖切").clicked() {
        vs.clip_plane_enabled = !vs.clip_plane_enabled;
    }
}

fn panel_group(ui: &mut egui::Ui, vs: &mut ViewerState, _has_current_file: bool) {
    if toggle_button(ui, vs.side_panel_open, "参数面板")
        .on_hover_text(if vs.side_panel_open {
            "隐藏参数面板"
        } else {
            "显示参数面板"
        })
        .clicked()
    {
        vs.toggle_side_panel();
    }
    if toggle_button(ui, vs.camera_overlay_open, "相机面板").clicked() {
        vs.camera_overlay_open = !vs.camera_overlay_open;
    }
    if toggle_button(ui, vs.log_panel_open, "日志").clicked() {
        vs.toggle_log_panel();
    }
}
