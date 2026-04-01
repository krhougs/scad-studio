#![allow(dead_code)]

#[path = "../src/params.rs"]
mod params;
#[path = "../src/presets.rs"]
mod presets;

use params::{ParameterStore, ParameterValue, parse_parameters};
use presets::{delete_preset, load_presets, preset_path_for_source, save_preset};

#[test]
fn preset_path_uses_matching_scad_json_name() {
    let path = preset_path_for_source(std::path::Path::new("/tmp/example.scad"));

    assert_eq!(path, std::path::PathBuf::from("/tmp/example.scad.json"));
}

#[test]
fn save_load_and_delete_presets_round_trip() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("scad-studio-presets-{suffix}"));
    let source_path = root.join("widget.scad");
    std::fs::create_dir_all(&root).expect("temp dir should exist");
    std::fs::write(&source_path, "size = 10; // [1:1:20]\n").expect("source should exist");
    let preset_path = preset_path_for_source(&source_path);

    let parsed = parse_parameters("size = 10; // [1:1:20]\nname = \"A\"; // [A, B]\n");
    let mut store = ParameterStore::from_parsed(parsed);
    store.set_value("size", ParameterValue::Number(14.0)).unwrap();
    store.set_value("name", ParameterValue::Text("B".into())).unwrap();

    save_preset(&preset_path, "draft", &store).expect("preset should be written");
    let loaded = load_presets(&preset_path).expect("preset should load");
    assert_eq!(
        loaded.presets["draft"]["size"],
        ParameterValue::Number(14.0)
    );

    delete_preset(&preset_path, "draft").expect("preset should delete");
    let deleted = load_presets(&preset_path).expect("preset file should still parse");
    assert!(deleted.presets.is_empty());

    let _ = std::fs::remove_file(&preset_path);
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_dir(&root);
}
