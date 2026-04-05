use scad_data::{AppConfig, DocumentState, LogLevel};

#[test]
fn config_defaults_expose_empty_slicer_list() {
    let config = AppConfig::default();

    assert!(config.slicers.is_empty());
}

#[test]
fn document_starts_without_source() {
    let document = DocumentState::default();

    assert!(document.current_source().is_none());
    assert!(matches!(LogLevel::Info, LogLevel::Info));
}
