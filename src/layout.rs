use std::path::PathBuf;

use crate::{
    app::StudioApp,
    left_panel::{self, LeftPanelAction},
    log_panel, welcome, work_area,
};
use scad_ui::theme;

#[derive(Debug, Clone)]
pub enum LayoutAction {
    OpenFolder,
    OpenRecent(PathBuf),
    OpenFile(PathBuf),
    SentChat(String),
}

pub fn show(
    ctx: &egui::Context,
    app: &mut StudioApp,
    show_studio_chrome: bool,
) -> Option<LayoutAction> {
    let mut action = None;
    if app.left_panel_open() && app.workspace_path().is_some() {
        egui::SidePanel::left("studio_left_panel")
            .resizable(true)
            .default_width(app.left_panel_width())
            .width_range(220.0..=480.0)
            .frame(theme::floating_frame(1.0))
            .show(ctx, |ui| {
                app.set_left_panel_width(ui.available_width());
                if let Some(next) = left_panel::show(ui, app) {
                    action = Some(match next {
                        LeftPanelAction::OpenFile(path) => LayoutAction::OpenFile(path),
                        LeftPanelAction::SentChat(message) => LayoutAction::SentChat(message),
                    });
                }
            });
    }
    if show_studio_chrome {
        log_panel::show(ctx, app);
        egui::TopBottomPanel::bottom("studio_status_bar")
            .exact_height(28.0)
            .frame(theme::panel_bar_frame(10, 3))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(app.status_text());
                });
            });
    }
    if let Some(next) = work_area::show(ctx, app) {
        action = Some(match next {
            welcome::WelcomeAction::OpenFolder => LayoutAction::OpenFolder,
            welcome::WelcomeAction::OpenRecent(path) => LayoutAction::OpenRecent(path),
        });
    }
    action
}
