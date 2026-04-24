use std::path::PathBuf;

use app_server_core::{app_config_from_dto, app_config_to_dto, config_file_path};
use app_server_protocol::DisplayUnitDto;
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
fn config_file_path_uses_platform_config_directory() {
    let path = config_file_path().expect("config path should resolve");

    assert!(path.ends_with("scad-studio/config.json"));
}

#[test]
fn config_dto_round_trip_preserves_host_local_paths_and_layout() {
    let config = AppConfig {
        openscad_path: Some(PathBuf::from(
            "/Applications/OpenSCAD.app/Contents/MacOS/OpenSCAD",
        )),
        slicers: vec![SlicerConfig {
            name: "PrusaSlicer".into(),
            path: PathBuf::from("/usr/bin/prusa-slicer"),
        }],
        recent_workspaces: vec![PathBuf::from("/tmp/workspace-a")],
        floating_panel_opacity: 0.7,
        left_panel_width: 300.0,
        right_panel_width: 340.0,
        display_unit: studio_common::DisplayUnit::Centimeter,
        camera_overlay_pos: Some([1.0, 2.0]),
        ..AppConfig::default()
    };

    let dto = app_config_to_dto(&config).expect("config should convert to DTO");
    assert_eq!(dto.display_unit, DisplayUnitDto::Centimeter);
    assert_eq!(
        dto.openscad_path.as_ref().map(|path| path.as_str()),
        Some("/Applications/OpenSCAD.app/Contents/MacOS/OpenSCAD")
    );

    let decoded = app_config_from_dto(dto).expect("DTO should convert to config");
    assert_eq!(decoded.openscad_path, config.openscad_path);
    assert_eq!(decoded.slicers, config.slicers);
    assert_eq!(decoded.recent_workspaces, config.recent_workspaces);
    assert_eq!(decoded.display_unit, config.display_unit);
    assert_eq!(decoded.camera_overlay_pos, config.camera_overlay_pos);
}
