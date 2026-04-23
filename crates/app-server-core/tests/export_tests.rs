use app_server_core::{SlicerInstall, build_export_filename, detect_slicer_paths};
use app_server_protocol::ExportFormat;
use std::path::PathBuf;

#[test]
fn export_filename_uses_selected_format_extension() {
    assert_eq!(
        build_export_filename(std::path::Path::new("/tmp/widget.scad"), ExportFormat::Stl),
        "widget.stl"
    );
    assert_eq!(
        build_export_filename(
            std::path::Path::new("/tmp/widget.scad"),
            ExportFormat::ThreeMf,
        ),
        "widget.3mf"
    );
}

#[test]
fn manual_slicer_paths_are_returned_before_auto_detected_paths() {
    let detected = detect_slicer_paths(&[SlicerInstall {
        name: "Cura".into(),
        path: PathBuf::from("/custom/Cura.app"),
    }]);

    assert_eq!(detected[0].name, "Cura");
    assert_eq!(detected[0].path, PathBuf::from("/custom/Cura.app"));
}
