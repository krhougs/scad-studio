use scad_viewer::app::StudioApp;

#[test]
fn viewer_app_defaults_to_closed_log_panel() {
    let app = StudioApp::default();

    assert!(!app.viewer_state().log_panel_open);
}
