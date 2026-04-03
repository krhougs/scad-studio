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
#[path = "../src/gizmo.rs"]
mod gizmo;
#[path = "../src/mesh.rs"]
mod mesh;
#[path = "../src/openscad.rs"]
mod openscad;
#[path = "../src/params.rs"]
mod params;
#[path = "../src/presets.rs"]
mod presets;
#[path = "../src/three_mf.rs"]
mod three_mf;
#[path = "../src/ui/mod.rs"]
mod ui;

use app::{LogLevel, StudioApp};
use std::path::PathBuf;

#[test]
fn side_panel_is_visible_by_default_regardless_of_file_state() {
    let mut studio = StudioApp::default();

    // 无文件时参数面板仍然可见（显示"请先加载模型"）
    assert!(
        studio
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
