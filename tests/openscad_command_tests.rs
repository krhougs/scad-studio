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

use openscad::{
    CliOutputFormat, OpenScadError, build_cli_args, build_preview_job_args, finalize_job,
    resolve_openscad_path,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
fn preview_job_args_force_3mf_output() {
    let (output_path, args) = build_preview_job_args(
        Path::new("/tmp/model.scad"),
        &["height=12".into(), "name=\"fine\"".into()],
    );

    assert_eq!(
        output_path.extension().and_then(|value| value.to_str()),
        Some("3mf")
    );
    assert_eq!(
        args,
        vec![
            "--export-format".to_string(),
            "3mf".to_string(),
            "-o".to_string(),
            output_path.display().to_string(),
            "-D".to_string(),
            "height=12".to_string(),
            "-D".to_string(),
            "name=\"fine\"".to_string(),
            "/tmp/model.scad".to_string(),
        ]
    );
}

#[test]
fn preview_job_uses_3mf_temp_filename() {
    let (output_path, _) = build_preview_job_args(Path::new("/tmp/widget.scad"), &[]);

    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("preview output should have a file name");

    assert!(file_name.starts_with("scad-studio-widget-"));
    assert!(file_name.ends_with(".3mf"));
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

#[test]
fn resolve_openscad_path_keeps_generic_missing_cli_message() {
    let error = resolve_openscad_path(None, None, None).expect_err("missing path should fail");

    assert_eq!(
        error.to_string(),
        "未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH"
    );
}

#[test]
fn finalize_job_cleans_preview_file_when_output_collection_fails() {
    let preview_path = std::env::temp_dir().join(format!(
        "scad-studio-preview-cleanup-{}.3mf",
        std::process::id()
    ));
    fs::write(&preview_path, b"fixture").expect("should create temp preview file");

    let result = finalize_job(
        PathBuf::from("/tmp/example.scad"),
        preview_path.clone(),
        true,
        Err(OpenScadError::new("collect output failed")),
    );

    assert!(result.is_err());
    assert!(
        !preview_path.exists(),
        "preview file should be removed on error"
    );
}
