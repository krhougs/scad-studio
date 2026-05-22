use app_server_core::CadQueryRunnerErrorKind;
use app_server_host::{
    agent_error_type, validate_cadquery_confirmation, watch_changed_paths_to_handles,
};
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentErrorType, CadQueryExecuteRequest, CadQueryExportFormat,
    CadQueryObjectKind, PathHandle, WorkspaceId,
};
use std::path::PathBuf;

fn ws() -> WorkspaceId {
    WorkspaceId::new("ws")
}

fn path(segments: &[&str]) -> PathHandle {
    PathHandle::new(ws(), segments.iter().copied()).unwrap()
}

fn relative_paths(handles: &[PathHandle]) -> Vec<String> {
    handles.iter().map(PathHandle::display_path).collect()
}

#[test]
fn watch_changed_paths_keep_actual_relative_files() {
    let watched = path(&[]);
    let root = PathBuf::from("/tmp/workspace");
    let changed = vec![
        root.join("examples").join("cube.scad"),
        root.join("examples").join("cube.scad.json"),
    ];

    let handles = watch_changed_paths_to_handles(&watched, &root, &changed);

    assert_eq!(
        relative_paths(&handles),
        vec!["examples/cube.scad", "examples/cube.scad.json"]
    );
}

#[test]
fn watch_changed_paths_extend_subdirectory_subscription() {
    let watched = path(&["examples"]);
    let root = PathBuf::from("/tmp/workspace/examples");
    let changed = vec![root.join("cube.scad")];

    let handles = watch_changed_paths_to_handles(&watched, &root, &changed);

    assert_eq!(relative_paths(&handles), vec!["examples/cube.scad"]);
}

fn make_confirmation(
    target_segments: &[&str],
    affected: &[&[&str]],
    new_files: &[&[&str]],
    export_formats: Vec<CadQueryExportFormat>,
    export_targets: &[&[&str]],
) -> AgentCadQueryConfirmation {
    AgentCadQueryConfirmation {
        request: CadQueryExecuteRequest {
            target_path: path(target_segments),
            target_type: CadQueryObjectKind::Part,
            code: String::new(),
            export_formats,
            params_json: String::new(),
        },
        plan_ref: None,
        affected_files: affected.iter().map(|s| path(s)).collect(),
        new_files: new_files.iter().map(|s| path(s)).collect(),
        export_targets: export_targets.iter().map(|s| path(s)).collect(),
    }
}

#[test]
fn valid_target_in_affected_files() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![],
        &[],
    );
    assert!(validate_cadquery_confirmation(&c).is_ok());
}

#[test]
fn valid_target_in_new_files() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[],
        &[&["parts", "lid.py"]],
        vec![],
        &[],
    );
    assert!(validate_cadquery_confirmation(&c).is_ok());
}

#[test]
fn target_not_in_affected_or_new_files() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "base.py"]],
        &[],
        vec![],
        &[],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn export_formats_without_export_targets() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![CadQueryExportFormat::Step],
        &[],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn export_targets_outside_outputs_dir() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![],
        &[&["parts", "lid.step"]],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn export_targets_without_export_formats_rejects() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![],
        &[&["outputs", "lid.step"]],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn export_formats_must_match_export_target_extensions() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![CadQueryExportFormat::Step],
        &[&["outputs", "lid.stl"]],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn export_target_extension_must_be_supported() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![CadQueryExportFormat::Step],
        &[&["outputs", "lid.obj"]],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn export_target_filename_must_match_runner_output() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![CadQueryExportFormat::Step],
        &[&["outputs", "custom.step"]],
    );
    assert!(validate_cadquery_confirmation(&c).is_err());
}

#[test]
fn valid_export_targets_in_outputs_dir() {
    let c = make_confirmation(
        &["parts", "lid.py"],
        &[&["parts", "lid.py"]],
        &[],
        vec![CadQueryExportFormat::Step],
        &[&["outputs", "lid.step"]],
    );
    assert!(validate_cadquery_confirmation(&c).is_ok());
}

#[test]
fn empty_affected_and_new_files_rejects() {
    let c = make_confirmation(&["parts", "lid.py"], &[], &[], vec![], &[]);
    assert!(validate_cadquery_confirmation(&c).is_err());
}

// --- agent_error_type mapping tests ---

#[test]
fn build_error_maps_to_cadquery_build_error() {
    assert_eq!(
        agent_error_type(&CadQueryRunnerErrorKind::Build),
        AgentErrorType::CadQueryBuildError,
    );
}

#[test]
fn timeout_maps_to_timeout() {
    assert_eq!(
        agent_error_type(&CadQueryRunnerErrorKind::Timeout),
        AgentErrorType::Timeout,
    );
}

#[test]
fn file_conflict_maps_to_file_conflict() {
    assert_eq!(
        agent_error_type(&CadQueryRunnerErrorKind::FileConflict),
        AgentErrorType::FileConflict,
    );
}

#[test]
fn permission_denied_maps_to_permission_denied() {
    assert_eq!(
        agent_error_type(&CadQueryRunnerErrorKind::PermissionDenied),
        AgentErrorType::PermissionDenied,
    );
}

#[test]
fn python_import_maps_to_python_import_error() {
    assert_eq!(
        agent_error_type(&CadQueryRunnerErrorKind::PythonImport),
        AgentErrorType::PythonImportError,
    );
}

#[test]
fn cancelled_maps_to_timeout_as_fallback() {
    assert_eq!(
        agent_error_type(&CadQueryRunnerErrorKind::Cancelled),
        AgentErrorType::Timeout,
    );
}
