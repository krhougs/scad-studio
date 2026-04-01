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
#[path = "../src/mesh.rs"]
mod mesh;
#[path = "../src/openscad.rs"]
mod openscad;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/gizmo.rs"]
mod gizmo;

use openscad::{CliOutputFormat, build_cli_args, resolve_openscad_path};
use std::path::{Path, PathBuf};

#[test]
fn build_cli_args_includes_defines_before_source_path() {
    let args = build_cli_args(
        CliOutputFormat::BinaryStl,
        Path::new("/tmp/out.stl"),
        &["height=12".into(), "name=\"fine\"".into()],
        Path::new("/tmp/model.scad"),
    );

    assert_eq!(
        args,
        vec![
            "--export-format".to_string(),
            "binstl".to_string(),
            "-o".to_string(),
            "/tmp/out.stl".to_string(),
            "-D".to_string(),
            "height=12".to_string(),
            "-D".to_string(),
            "name=\"fine\"".to_string(),
            "/tmp/model.scad".to_string(),
        ]
    );
}

#[test]
fn resolve_openscad_path_prefers_configured_path() {
    let resolved = resolve_openscad_path(
        Some(PathBuf::from("/custom/OpenSCAD")),
        Some(PathBuf::from("/env/OpenSCAD")),
        Some(PathBuf::from("/auto/OpenSCAD")),
    )
    .expect("configured path should win");

    assert_eq!(resolved, PathBuf::from("/custom/OpenSCAD"));
}
