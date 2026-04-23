use app_server_protocol::{PathHandle, PathHandleValidationError, WorkspaceId};

#[test]
fn path_handle_rejects_dot_dot_segment() {
    let error = PathHandle::new(WorkspaceId::new("ws"), ["src", ".."])
        .expect_err("dot dot should be rejected");
    assert_eq!(error, PathHandleValidationError::DotDotSegment);
}

#[test]
fn path_handle_rejects_single_dot_segment() {
    let error = PathHandle::new(WorkspaceId::new("ws"), ["."]).expect_err("single dot should fail");
    assert_eq!(error, PathHandleValidationError::SingleDotSegment);
}

#[test]
fn path_handle_rejects_empty_segment() {
    let error = PathHandle::new(WorkspaceId::new("ws"), [""])
        .expect_err("empty segment should be rejected");
    assert_eq!(error, PathHandleValidationError::EmptySegment);
}

#[test]
fn path_handle_rejects_native_separator() {
    let error = PathHandle::new(WorkspaceId::new("ws"), ["src/main.rs"])
        .expect_err("slash should be rejected");
    assert_eq!(error, PathHandleValidationError::NativeSeparator);

    let error = PathHandle::new(WorkspaceId::new("ws"), [r"src\main.rs"])
        .expect_err("backslash should be rejected");
    assert_eq!(error, PathHandleValidationError::NativeSeparator);
}

#[test]
fn path_handle_nfc_canonical_equivalent() {
    let composed = PathHandle::new(WorkspaceId::new("ws"), ["café"]).unwrap();
    let decomposed = PathHandle::new(WorkspaceId::new("ws"), ["cafe\u{301}"]).unwrap();
    assert_eq!(composed, decomposed);
}
