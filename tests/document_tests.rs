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

use document::DocumentState;
use export::ExportFormat;
use params::ParameterValue;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn loading_source_builds_parameter_state_and_watch_list() {
    let mut document = DocumentState::default();
    let source_path = PathBuf::from("/tmp/example.scad");
    let source = "height = 12; // [4:1:20]\n";

    document.load_source(source_path.clone(), source);

    assert_eq!(document.current_source(), Some(source_path.as_path()));
    assert_eq!(document.current_defines(), vec!["height=12".to_string()]);
    assert_eq!(
        document.watch_paths(),
        vec![source_path.clone(), PathBuf::from("/tmp/example.scad.json")]
    );
    assert_eq!(document.export_format, ExportFormat::Stl);
}

#[test]
fn reparsing_source_preserves_existing_parameter_override() {
    let mut document = DocumentState::default();
    document.load_source(
        PathBuf::from("/tmp/example.scad"),
        "height = 12; // [4:1:20]\nflag = false; // or true\n",
    );
    document
        .set_parameter("height", ParameterValue::Number(18.0))
        .unwrap();

    document.reload_source(
        "height = 12; // [4:1:20]\nflag = false; // or true\nname = \"A\"; // [A, B]\n",
    );

    assert_eq!(
        document.parameter_value("height"),
        Some(&ParameterValue::Number(18.0))
    );
    assert_eq!(
        document.parameter_value("name"),
        Some(&ParameterValue::Text("A".into()))
    );
}

#[test]
fn applying_preset_updates_parameter_values() {
    let mut document = DocumentState::default();
    document.load_source(
        PathBuf::from("/tmp/example.scad"),
        "height = 12; // [4:1:20]\nname = \"A\"; // [A, B]\n",
    );
    document.presets.presets.insert(
        "fine".into(),
        BTreeMap::from([
            ("height".into(), ParameterValue::Number(19.0)),
            ("name".into(), ParameterValue::Text("B".into())),
        ]),
    );

    document.apply_preset("fine").unwrap();

    assert_eq!(
        document.parameter_value("height"),
        Some(&ParameterValue::Number(19.0))
    );
    assert_eq!(
        document.parameter_value("name"),
        Some(&ParameterValue::Text("B".into()))
    );
}
