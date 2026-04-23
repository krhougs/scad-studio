use std::path::PathBuf;

use crate::app::{LeftPanelTab, StudioApp};
use crate::macos_fused_titlebar;
use scad_ui::{
    document_tabs,
    file_tree::FileTreeAction,
    panel_switcher::{self, PanelSwitchItem},
    theme::palette,
    viewer_viewport,
};

#[derive(Debug, Clone)]
pub enum LeftPanelAction {
    OpenFile(PathBuf),
    LoadDirectory(PathBuf),
    SentChat(String),
}

pub fn show(ui: &mut egui::Ui, app: &mut StudioApp) -> Option<LeftPanelAction> {
    let mut action = None;
    let items = [
        PanelSwitchItem {
            label: "Chat",
            active: app.left_panel_tab() == LeftPanelTab::Chat,
        },
        PanelSwitchItem {
            label: "Files",
            active: app.left_panel_tab() == LeftPanelTab::Files,
        },
    ];
    let _ = viewer_viewport::allocate_filled_strip_ui(
        ui,
        egui::vec2(ui.available_width(), document_tabs::rail_height()),
        document_tabs::rail_margin(),
        document_tabs::rail_fill_color(),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.horizontal(|ui| {
                let inset = macos_fused_titlebar::traffic_lights_left_inset(
                    !app.root_viewport_fullscreen(),
                );
                if inset > 0.0 {
                    ui.add_space(inset);
                }
                if let Some(index) = panel_switcher::show_panel_switcher(ui, &items) {
                    app.set_left_panel_tab(match index {
                        0 => LeftPanelTab::Chat,
                        _ => LeftPanelTab::Files,
                    });
                }
                macos_fused_titlebar::horizontal_drag_tail(ui, 8.0);
            });
        },
    );
    ui.add_space(palette::TAB_STRIP_GAP_BELOW);
    match app.left_panel_tab() {
        LeftPanelTab::Chat => {
            if let Some(scad_ui::chat_panel::ChatAction::SendMessage(message)) =
                app.chat_panel_mut().show(ui)
            {
                action = Some(LeftPanelAction::SentChat(message));
            }
        }
        LeftPanelTab::Files => {
            if let Some(tree) = app.file_tree_mut() {
                let mut open_file = None;
                egui::ScrollArea::vertical()
                    .id_salt("studio_left_file_tree")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match tree.show(ui) {
                            Some(FileTreeAction::OpenFile(path)) => open_file = Some(path),
                            Some(FileTreeAction::LoadDirectory(path)) => {
                                action = Some(LeftPanelAction::LoadDirectory(path));
                            }
                            Some(FileTreeAction::Select(_)) | None => {}
                        }
                    });
                if let Some(path) = open_file {
                    action = Some(LeftPanelAction::OpenFile(path));
                }
            } else {
                ui.label("打开 Workspace 后显示目录树。");
            }
        }
    }
    action
}
