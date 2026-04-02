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

use app::LogLevel;
use openscad::collect_process_logs;

#[test]
fn collect_process_logs_ignores_blank_lines_and_tags_stdout_as_info() {
    let logs = collect_process_logs(b"line one\n\nline two\n", b"", true);

    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].level, LogLevel::Info);
    assert_eq!(logs[0].message, "line one");
    assert_eq!(logs[1].level, LogLevel::Info);
    assert_eq!(logs[1].message, "line two");
}

#[test]
fn collect_process_logs_tags_stderr_as_error_when_process_fails() {
    let logs = collect_process_logs(b"", b"warning line\nfatal line\n", false);

    assert_eq!(logs.len(), 2);
    assert!(logs.iter().all(|entry| entry.level == LogLevel::Error));
}
