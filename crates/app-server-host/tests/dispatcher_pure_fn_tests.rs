use app_server_core::CadQueryRunnerErrorKind;
use app_server_host::{agent_error_type, validate_cadquery_confirmation};
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentErrorType, CadQueryExecuteRequest, CadQueryExportFormat,
    CadQueryObjectKind, PathHandle, WorkspaceId,
};

fn ws() -> WorkspaceId {
    WorkspaceId::new("ws")
}

fn path(segments: &[&str]) -> PathHandle {
    PathHandle::new(ws(), segments.iter().copied()).unwrap()
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
