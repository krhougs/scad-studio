use crate::{
    app::StudioApp,
    welcome::{WelcomeAction, WelcomeTab},
};
use scad_ui::{tab_system::TabContext, theme};

pub fn show(ctx: &egui::Context, app: &mut StudioApp) -> Option<WelcomeAction> {
    let mut action = None;
    egui::TopBottomPanel::top("studio_tab_bar")
        .exact_height(34.0)
        .frame(theme::panel_bar_frame(8, 4))
        .show(ctx, |ui| {
            app.tabs_mut().show_tab_bar(ui);
        });
    app.ensure_welcome_tab();
    egui::CentralPanel::default().show(ctx, |ui| {
        let mut tab_ctx = TabContext::default();
        app.tabs_mut().show_active_content(ui, &mut tab_ctx);
    });
    if let Some(welcome_tab) = app.tabs_mut().active_tab_as_mut::<WelcomeTab>() {
        action = welcome_tab.take_action();
    }
    action
}
