use app_server_protocol::{
    PathHandle, PreviewArtifact, PreviewReadyResponse, PreviewRenderedImagePayload, PreviewRequest,
    PreviewRequestKind, PreviewUnit, WorkspaceId,
};
use studio_common::PreviewState;

#[test]
fn preview_state_defaults_to_idle_labels() {
    let state = PreviewState::default();

    assert_eq!(state.status_label(), "preview idle");
    assert_eq!(state.summary_label(), "No preview requested yet.");
    assert!(state.target().is_none());
    assert!(state.current_request().is_none());
    assert!(state.last_result().is_none());
    assert!(state.last_error().is_none());
}

#[test]
fn starting_request_tracks_target_and_pending_labels() {
    let mut state = PreviewState::default();
    let request = geometry_request(["meshes", "benchy.stl"]);

    state.start(request.clone());

    assert_eq!(state.status_label(), "preview pending");
    assert_eq!(state.summary_label(), "Waiting for mesh response…");
    assert_eq!(state.target(), Some(&request.source));
    assert_eq!(state.current_request(), Some(&request));
    assert!(state.last_result().is_none());
    assert!(state.last_error().is_none());
}

#[test]
fn ready_mesh_clears_pending_request_and_formats_summary() {
    let mut state = PreviewState::default();
    state.start(geometry_request(["meshes", "benchy.stl"]));

    let ready = mesh_ready_response();
    state.complete(ready.clone());

    assert_eq!(state.status_label(), "preview ready");
    assert_eq!(
        state.summary_label(),
        "vertices: 4 · indices: 6 · triangles: 2"
    );
    assert!(state.current_request().is_none());
    assert_eq!(state.last_result(), Some(&ready));
    assert!(state.last_error().is_none());
}

#[test]
fn skipped_preview_exposes_reason_without_target() {
    let mut state = PreviewState::default();

    state.skip("No previewable file found in current directory.");

    assert_eq!(state.status_label(), "preview skipped");
    assert_eq!(
        state.summary_label(),
        "No previewable file found in current directory."
    );
    assert!(state.target().is_none());
    assert!(state.current_request().is_none());
    assert!(state.last_result().is_none());
    assert_eq!(
        state.last_error(),
        Some("No previewable file found in current directory.")
    );
}

#[test]
fn failing_preview_clears_pending_request_and_keeps_error_message() {
    let mut state = PreviewState::default();
    let request = geometry_request(["meshes", "broken.stl"]);
    state.start(request.clone());

    state.fail("preview generation failed");

    assert_eq!(state.status_label(), "preview error");
    assert_eq!(state.summary_label(), "preview generation failed");
    assert_eq!(state.target(), Some(&request.source));
    assert!(state.current_request().is_none());
    assert!(state.last_result().is_none());
    assert_eq!(state.last_error(), Some("preview generation failed"));
}

fn geometry_request(path_segments: impl IntoIterator<Item = &'static str>) -> PreviewRequest {
    PreviewRequest {
        source: PathHandle::new(WorkspaceId::new("workspace-preview"), path_segments)
            .expect("valid path handle"),
        defines: Vec::new(),
        kind: PreviewRequestKind::GeometryArtifact,
        configured_openscad_path: None,
    }
}

fn mesh_ready_response() -> PreviewReadyResponse {
    PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact: PreviewArtifact::Mesh(app_server_protocol::PreviewMeshPayload {
            unit: PreviewUnit::Millimeter,
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            normals: Vec::new(),
            vertex_colors: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3],
        }),
    }
}

#[test]
fn unsupported_artifact_reports_unsupported_status() {
    let mut state = PreviewState::default();
    state.start(geometry_request(["renders", "demo.3mf"]));

    let ready = PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact: PreviewArtifact::RenderedImage(PreviewRenderedImagePayload {
            bytes: vec![1, 2, 3],
            media_type: "image/png".into(),
            width: 64,
            height: 64,
        }),
    };
    state.complete(ready.clone());

    assert_eq!(state.status_label(), "preview unsupported");
    assert!(state.summary_label().starts_with("unexpected artifact:"));
    assert!(state.summary_label().contains("RenderedImagePayload"));
    assert!(state.current_request().is_none());
    assert_eq!(state.last_result(), Some(&ready));
    assert!(state.last_error().is_some());
}
