use scad_ui::theme;
use scad_viewer::{
    app::{StudioApp, UiActions},
    ui::toolbar,
};

#[test]
fn embedded_toolbar_wraps_by_block_when_narrow() {
    egui::__run_test_ui(|ui| {
        theme::apply(ui.ctx());
        let mut studio = StudioApp::default();
        let mut actions = UiActions::default();
        let inner = ui.scope_builder(
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(220.0, 300.0),
            )),
            |ui| {
                let mut settings_sink = false;
                toolbar::paint_toolbar_row(ui, &mut studio, &mut actions, false, &mut settings_sink)
            },
        );
        assert!(
            inner.response.rect.height() > 44.0,
            "窄窗口下工具栏应按功能块纵向换行，当前高度为 {}",
            inner.response.rect.height()
        );
    });
}

#[test]
fn embedded_toolbar_stays_single_row_on_wide_width() {
    egui::__run_test_ui(|ui| {
        theme::apply(ui.ctx());
        let mut studio = StudioApp::default();
        let mut actions = UiActions::default();
        let inner = ui.scope_builder(
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 300.0),
            )),
            |ui| {
                let mut settings_sink = false;
                toolbar::paint_toolbar_row(ui, &mut studio, &mut actions, false, &mut settings_sink)
            },
        );
        assert!(
            inner.response.rect.height() <= 48.0,
            "宽窗口下工具栏应单行排布，当前高度为 {}",
            inner.response.rect.height()
        );
    });
}
