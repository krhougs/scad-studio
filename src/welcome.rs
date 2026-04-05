use std::{any::Any, path::PathBuf};

use scad_ui::{
    tab_system::{TabContext, TabId, WorkTab},
    theme::palette,
    widgets::{filled_small_button, section_header},
};

#[derive(Debug, Clone)]
pub enum WelcomeAction {
    OpenFolder,
    OpenRecent(PathBuf),
}

pub struct WelcomeTab {
    recent_workspaces: Vec<PathBuf>,
    pending_action: Option<WelcomeAction>,
}

impl WelcomeTab {
    pub const ID: TabId = 1;

    pub fn new(recent_workspaces: Vec<PathBuf>) -> Self {
        Self {
            recent_workspaces,
            pending_action: None,
        }
    }

    pub fn tab_id() -> TabId {
        Self::ID
    }

    pub fn set_recent_workspaces(&mut self, recent_workspaces: Vec<PathBuf>) {
        self.recent_workspaces = recent_workspaces;
    }

    pub fn take_action(&mut self) -> Option<WelcomeAction> {
        self.pending_action.take()
    }
}

impl WorkTab for WelcomeTab {
    fn id(&self) -> TabId {
        Self::ID
    }

    fn title(&self) -> &str {
        "欢迎"
    }

    fn is_closable(&self) -> bool {
        false
    }

    fn show(&mut self, ui: &mut egui::Ui, _ctx: &mut TabContext<'_>) {
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
                                "打开一个 Workspace 文件夹，开始在多标签工作区中浏览模型、文档和 Agent 对话。",
                            )
                            .color(palette::TEXT_SECONDARY)
                            .size(13.0),
                        );
                        ui.add_space(20.0);
                        if filled_small_button(ui, "打开文件夹").clicked() {
                            self.pending_action = Some(WelcomeAction::OpenFolder);
                        }
                        ui.add_space(20.0);
                        section_header(ui, "最近打开");
                        ui.add_space(6.0);
                        if self.recent_workspaces.is_empty() {
                            ui.label(
                                egui::RichText::new("暂无最近工作区")
                                    .italics()
                                    .color(palette::TEXT_SECONDARY),
                            );
                            return;
                        }
                        for path in &self.recent_workspaces {
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
                                self.pending_action = Some(WelcomeAction::OpenRecent(path.clone()));
                            }
                        }
                    });
                });
        });
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
