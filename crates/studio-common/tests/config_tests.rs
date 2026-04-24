use std::path::PathBuf;
use studio_common::{AppConfig, SlicerConfig};

#[test]
fn config_json_round_trip_preserves_paths() {
    let config = AppConfig {
        openscad_path: Some(PathBuf::from(
            "/Applications/OpenSCAD.app/Contents/MacOS/OpenSCAD",
        )),
        slicers: vec![SlicerConfig {
            name: "Bambu Studio".into(),
            path: PathBuf::from("/Applications/Bambu Studio.app"),
        }],
        recent_workspaces: vec![
            PathBuf::from("/tmp/workspace-a"),
            PathBuf::from("/tmp/workspace-b"),
        ],
        ..AppConfig::default()
    };

    let json = config.to_json().expect("config should serialize");
    let decoded = AppConfig::from_json(&json).expect("config should deserialize");

    assert_eq!(decoded.openscad_path, config.openscad_path);
    assert_eq!(decoded.slicers, config.slicers);
    assert_eq!(decoded.recent_workspaces, config.recent_workspaces);
}

#[test]
fn config_json_defaults_new_web_layout_fields() {
    let decoded = AppConfig::from_json("{}").expect("config should deserialize");

    assert_eq!(decoded.left_panel_width, 360.0);
    assert_eq!(decoded.right_panel_width, 320.0);
    assert_eq!(decoded.display_unit.to_string(), "millimeter");
}
