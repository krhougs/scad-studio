#![allow(dead_code)]

#[path = "../src/app.rs"]
mod app;
#[path = "../src/camera.rs"]
mod camera;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/document.rs"]
mod document;
#[path = "../src/export.rs"]
mod export;
#[path = "../src/params.rs"]
mod params;
#[path = "../src/presets.rs"]
mod presets;
#[path = "../src/openscad.rs"]
mod openscad;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/gizmo.rs"]
mod gizmo;
#[path = "../src/mesh.rs"]
mod mesh;

use app::{LogLevel, StudioApp};
use std::path::PathBuf;

#[test]
fn side_panel_is_hidden_without_file_and_visible_after_opening_file() {
    let mut studio = StudioApp::default();

    assert!(
        !studio
            .viewer_state()
            .shows_side_panel(studio.current_file().is_some())
    );

    studio.set_current_file(PathBuf::from("/tmp/example.scad"));

    assert!(
        studio
            .viewer_state()
            .shows_side_panel(studio.current_file().is_some())
    );
}

#[test]
fn error_logs_expand_log_panel_automatically() {
    let mut studio = StudioApp::default();

    assert!(!studio.viewer_state().log_panel_open);

    studio.push_log(LogLevel::Error, "OpenSCAD 编译失败");

    assert!(studio.viewer_state().log_panel_open);
    assert_eq!(studio.log_entries().len(), 1);
}
