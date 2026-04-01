use crate::app::{ColorMode, ProjectionMode, RenderMode, StudioApp, UiActions, ViewerState};

pub fn show(ctx: &egui::Context, studio: &mut StudioApp, actions: &mut UiActions) {
    let has_current_file = studio.has_current_file();
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("打开文件").clicked() {
                actions.open_file = true;
            }
            ui.separator();
            render_mode_group(ui, studio.viewer_state_mut());
            ui.separator();
            color_mode_group(ui, studio.viewer_state_mut());
            ui.separator();
            projection_group(ui, studio.viewer_state_mut());
            ui.separator();
            environment_group(ui, studio.viewer_state_mut());
            ui.separator();
            effects_group(ui, studio.viewer_state_mut());
            ui.separator();
            panel_group(ui, studio.viewer_state_mut(), has_current_file);
        });
    });
}

fn render_mode_group(ui: &mut egui::Ui, viewer_state: &mut ViewerState) {
    ui.label("渲染");
    ui.selectable_value(&mut viewer_state.render_mode, RenderMode::Solid, "Solid");
    let wire_response = ui.add_enabled(
        viewer_state.wireframe_supported,
        egui::Button::selectable(viewer_state.render_mode == RenderMode::Wireframe, "Wire"),
    );
    if wire_response.clicked() {
        viewer_state.render_mode = RenderMode::Wireframe;
    }
    ui.selectable_value(&mut viewer_state.render_mode, RenderMode::XRay, "X-Ray");
}

fn color_mode_group(ui: &mut egui::Ui, viewer_state: &mut ViewerState) {
    ui.label("颜色");
    ui.selectable_value(&mut viewer_state.color_mode, ColorMode::Mono, "Mono");
    ui.selectable_value(&mut viewer_state.color_mode, ColorMode::Color, "Color");
}

fn projection_group(ui: &mut egui::Ui, viewer_state: &mut ViewerState) {
    ui.label("投影");
    ui.selectable_value(
        &mut viewer_state.projection_mode,
        ProjectionMode::Perspective,
        "Persp",
    );
    ui.selectable_value(
        &mut viewer_state.projection_mode,
        ProjectionMode::Orthographic,
        "Ortho",
    );
}

fn environment_group(ui: &mut egui::Ui, viewer_state: &mut ViewerState) {
    ui.label("环境");
    ui.toggle_value(&mut viewer_state.show_grid, "Grid");
    ui.toggle_value(&mut viewer_state.show_build_plate, "Plate");
    ui.toggle_value(&mut viewer_state.show_axis_gizmo, "Axis");
}

fn effects_group(ui: &mut egui::Ui, viewer_state: &mut ViewerState) {
    ui.label("效果");
    ui.toggle_value(&mut viewer_state.shadows_enabled, "Shadow");
    ui.toggle_value(&mut viewer_state.fog_enabled, "Fog");
    ui.toggle_value(&mut viewer_state.clip_plane_enabled, "Clip");
}

fn panel_group(ui: &mut egui::Ui, viewer_state: &mut ViewerState, has_current_file: bool) {
    let side_label = if viewer_state.side_panel_open {
        "侧栏"
    } else {
        "显示侧栏"
    };
    if ui
        .add_enabled(has_current_file, egui::Button::new(side_label))
        .clicked()
    {
        viewer_state.toggle_side_panel();
    }
    let log_label = if viewer_state.log_panel_open {
        "隐藏日志"
    } else {
        "显示日志"
    };
    if ui.button(log_label).clicked() {
        viewer_state.toggle_log_panel();
    }
}
