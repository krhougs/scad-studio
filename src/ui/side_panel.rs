use crate::{
    app::{UiActions, UiCommand, ViewerState},
    document::DocumentState,
    export::{ExportFormat, SlicerInstall},
};

pub fn show(
    ctx: &egui::Context,
    viewer_state: &ViewerState,
    has_current_file: bool,
    document: &mut DocumentState,
    slicers: &[SlicerInstall],
    actions: &mut UiActions,
) {
    egui::SidePanel::right("side_panel")
        .resizable(true)
        .default_width(300.0)
        .width_range(240.0..=420.0)
        .show_animated(ctx, viewer_state.shows_side_panel(has_current_file), |ui| {
            ui.heading("查看器控制");
            ui.separator();
            egui::CollapsingHeader::new("参数编辑器")
                .default_open(true)
                .show(ui, |ui| crate::ui::param_editor::show(ui, document));
            preset_section(ui, document, actions);
            export_section(ui, document, slicers, actions);
        });
}

fn preset_section(ui: &mut egui::Ui, document: &mut DocumentState, actions: &mut UiActions) {
    egui::CollapsingHeader::new("预设")
        .default_open(true)
        .show(ui, |ui| {
            if document.preset_names().is_empty() {
                ui.label("当前没有可用预设。");
            }
            for preset in document.preset_names() {
                let selected = document.selected_preset.as_deref() == Some(preset.as_str());
                if ui.selectable_label(selected, &preset).clicked() {
                    document.selected_preset = Some(preset.clone());
                    let _ = document.apply_preset(&preset);
                }
            }
            ui.separator();
            ui.label("保存当前参数为预设");
            ui.text_edit_singleline(&mut document.preset_name_input);
            ui.horizontal(|ui| {
                if ui.button("保存").clicked() && !document.preset_name_input.trim().is_empty() {
                    actions
                        .commands
                        .push(UiCommand::SavePreset(document.preset_name_input.trim().to_string()));
                }
                let can_delete = document.selected_preset.is_some();
                if ui
                    .add_enabled(can_delete, egui::Button::new("删除"))
                    .clicked()
                {
                    actions.commands.push(UiCommand::DeletePreset(
                        document.selected_preset.clone().unwrap_or_default(),
                    ));
                }
            });
        });
}

fn export_section(
    ui: &mut egui::Ui,
    document: &mut DocumentState,
    slicers: &[SlicerInstall],
    actions: &mut UiActions,
) {
    egui::CollapsingHeader::new("导出")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("格式");
                ui.selectable_value(&mut document.export_format, ExportFormat::Stl, "STL");
                ui.selectable_value(&mut document.export_format, ExportFormat::ThreeMf, "3MF");
            });
            if ui.button("导出模型").clicked() {
                actions.commands.push(UiCommand::ExportModel);
            }
            if slicers.is_empty() {
                ui.label("未检测到切片软件，可在设置中手动填写路径。");
            }
            for slicer in slicers {
                if ui.button(format!("发送到 {}", slicer.name)).clicked() {
                    actions
                        .commands
                        .push(UiCommand::SendToSlicer(slicer.name.clone()));
                }
            }
        });
}
