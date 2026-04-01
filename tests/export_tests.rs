#![allow(dead_code)]

#[path = "../src/config.rs"]
mod config;
#[path = "../src/document.rs"]
mod document;
#[path = "../src/app.rs"]
mod app;
#[path = "../src/camera.rs"]
mod camera;
#[path = "../src/export.rs"]
mod export;
#[path = "../src/params.rs"]
mod params;
#[path = "../src/presets.rs"]
mod presets;
#[path = "../src/mesh.rs"]
mod mesh;
#[path = "../src/openscad.rs"]
mod openscad;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/gizmo.rs"]
mod gizmo;

use config::{AppConfig, SlicerConfig};
use export::{ExportFormat, build_export_filename, detect_slicer_paths};
use std::path::PathBuf;

#[test]
fn export_filename_uses_selected_format_extension() {
    assert_eq!(
        build_export_filename(std::path::Path::new("/tmp/widget.scad"), ExportFormat::Stl),
        "widget.stl"
    );
    assert_eq!(
        build_export_filename(std::path::Path::new("/tmp/widget.scad"), ExportFormat::ThreeMf),
        "widget.3mf"
    );
}

#[test]
fn manual_slicer_paths_are_returned_before_auto_detected_paths() {
    let config = AppConfig {
        openscad_path: None,
        slicers: vec![SlicerConfig {
            name: "Cura".into(),
            path: PathBuf::from("/custom/Cura.app"),
        }],
    };

    let detected = detect_slicer_paths(&config);

    assert_eq!(detected[0].name, "Cura");
    assert_eq!(detected[0].path, PathBuf::from("/custom/Cura.app"));
}
