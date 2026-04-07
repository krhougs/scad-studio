#![allow(dead_code)]

#[path = "../src/wrap_line_pack.rs"]
mod wrap_line_pack;
#[path = "../src/app.rs"]
mod app;
#[path = "../src/ui/mod.rs"]
mod ui;

use app::{LogLevel, StudioApp};
use std::path::PathBuf;

#[test]
fn side_panel_is_visible_by_default_regardless_of_file_state() {
    let mut studio = StudioApp::default();

    // 无文件时参数面板仍然可见（显示"请先加载模型"）
    assert!(
        studio.viewer_state().side_panel_open
    );

    studio.set_current_file(PathBuf::from("/tmp/example.scad"));

    assert!(
        studio.viewer_state().side_panel_open
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
