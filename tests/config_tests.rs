#![allow(dead_code)]

#[path = "../src/config.rs"]
mod config;

use config::{AppConfig, SlicerConfig};
use std::path::PathBuf;

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
        ..AppConfig::default()
    };

    let json = config.to_json().expect("config should serialize");
    let decoded = AppConfig::from_json(&json).expect("config should deserialize");

    assert_eq!(decoded.openscad_path, config.openscad_path);
    assert_eq!(decoded.slicers, config.slicers);
}

#[test]
fn config_file_path_uses_platform_config_directory() {
    let path = config::config_file_path().expect("config path should resolve");

    assert!(path.ends_with("scad-studio/config.json"));
}
