use std::path::PathBuf;

use crate::app::{LeftPanelTab, StudioApp};
use scad_ui::{
    chat_panel::ChatAction,
    file_tree::FileTreeAction,
    widgets::{section_header, selectable_button},
};

#[derive(Debug, Clone)]
pub enum LeftPanelAction {
    OpenFile(PathBuf),
    SentChat(String),
}

pub fn show(ui: &mut egui::Ui, app: &mut StudioApp) -> Option<LeftPanelAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        tab_button(ui, app, LeftPanelTab::Chat, "Chat");
        tab_button(ui, app, LeftPanelTab::Files, "Files");
    });
    ui.add_space(12.0);
    match app.left_panel_tab() {
        LeftPanelTab::Chat => {
            if let Some(ChatAction::SendMessage(message)) = app.chat_panel_mut().show(ui) {
                action = Some(LeftPanelAction::SentChat(message));
            }
        }
        LeftPanelTab::Files => {
            section_header(ui, "workspace files");
            if let Some(tree) = app.file_tree_mut() {
                if let Some(FileTreeAction::OpenFile(path)) = tree.show(ui) {
                    action = Some(LeftPanelAction::OpenFile(path));
                }
            } else {
                ui.label("打开 Workspace 后显示目录树。");
            }
        }
    }
    action
}

fn tab_button(ui: &mut egui::Ui, app: &mut StudioApp, tab: LeftPanelTab, label: &str) {
    let selected = app.left_panel_tab() == tab;
    if selectable_button(ui, selected, label).clicked() {
        app.set_left_panel_tab(tab);
    }
}
