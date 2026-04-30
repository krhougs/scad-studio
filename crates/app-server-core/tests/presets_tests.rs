use app_server_core::{delete_preset, load_presets, preset_path_for_source, save_preset};
use app_server_protocol::ParameterValue;
use std::collections::BTreeMap;

#[test]
fn preset_path_uses_matching_scad_json_name() {
    let path = preset_path_for_source(std::path::Path::new("/tmp/example.scad"));

    assert_eq!(path, std::path::PathBuf::from("/tmp/example.scad.json"));
}

#[tokio::test]
async fn save_load_and_delete_presets_round_trip() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("scad-studio-presets-{suffix}"));
    let source_path = root.join("widget.scad");
    std::fs::create_dir_all(&root).expect("temp dir should exist");
    std::fs::write(&source_path, "size = 10; // [1:1:20]\n").expect("source should exist");
    let preset_path = preset_path_for_source(&source_path);

    let values = BTreeMap::from([
        ("size".to_string(), ParameterValue::Number(14.0)),
        ("name".to_string(), ParameterValue::Text("B".into())),
    ]);
    save_preset(&preset_path, "draft", &values).await.expect("preset should be written");
    let loaded = load_presets(&preset_path).await.expect("preset should load");
    assert_eq!(
        loaded.presets["draft"]["size"],
        ParameterValue::Number(14.0)
    );

    delete_preset(&preset_path, "draft").await.expect("preset should delete");
    let deleted = load_presets(&preset_path).await.expect("preset file should still parse");
    assert!(deleted.presets.is_empty());

    let _ = std::fs::remove_file(&preset_path);
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_dir(&root);
}
