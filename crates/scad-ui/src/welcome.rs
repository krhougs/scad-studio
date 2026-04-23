use std::path::PathBuf;

use crate::{
    theme::palette,
    widgets::{filled_small_button, section_header},
};

#[derive(Debug, Clone)]
pub enum WelcomeAction {
    OpenFolder,
    OpenRecent(PathBuf),
}

pub fn show_welcome(ui: &mut egui::Ui, recent_workspaces: &[PathBuf]) -> Option<WelcomeAction> {
    let mut action = None;
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        egui::Frame::group(ui.style())
            .fill(palette::BG_WINDOW)
            .stroke(egui::Stroke::new(1.0, palette::STROKE_DIM))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(20))
            .show(ui, |ui| {
                ui.set_width(460.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("SCAD Studio")
                            .size(28.0)
                            .strong()
                            .color(palette::TEXT_BRIGHT),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "打开一个 Workspace 文件夹，开始在多文档工作区中浏览模型、文档和 Agent 对话。",
                        )
                        .color(palette::TEXT_SECONDARY)
                        .size(13.0),
                    );
                    ui.add_space(20.0);
                    if filled_small_button(ui, "打开文件夹").clicked() {
                        action = Some(WelcomeAction::OpenFolder);
                    }
                    ui.add_space(20.0);
                    section_header(ui, "最近打开");
                    ui.add_space(6.0);
                    if recent_workspaces.is_empty() {
                        ui.label(
                            egui::RichText::new("暂无最近工作区")
                                .italics()
                                .color(palette::TEXT_SECONDARY),
                        );
                        return;
                    }
                    for path in recent_workspaces {
                        let label = path.display().to_string();
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(label).color(palette::TEXT_PRIMARY),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(6)),
                            )
                            .clicked()
                        {
                            action = Some(WelcomeAction::OpenRecent(path.clone()));
                        }
                    }
                });
            });
    });
    action
}

pub fn show_empty_workspace(ui: &mut egui::Ui, workspace_name: Option<&str>) {
    let title = workspace_name.unwrap_or("当前工作区");
    ui.vertical_centered(|ui| {
        ui.add_space(72.0);
        ui.label(
            egui::RichText::new(title)
                .size(24.0)
                .strong()
                .color(palette::TEXT_BRIGHT),
        );
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("右侧当前没有打开的文档。请从左侧 Files 面板选择文件进入工作区。")
                .size(13.0)
                .color(palette::TEXT_SECONDARY),
        );
    });
}
