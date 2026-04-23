use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCapabilities,
    ClientCommand, ClientPlatform, ClientRequestEnvelope, CommandSuccess, FileReadCapability,
    FileReadContents, FileReadResponse, PathHandle, PreviewArtifact, PreviewArtifact3mf,
    PreviewMeshPayload, PreviewReadyResponse, PreviewRenderedImagePayload, PreviewRequest,
    PreviewRequestKind, PreviewUnit, ProtocolError, ProtocolErrorCode, ProtocolVersionRange,
    RequestId, ServerCapabilities, ServerPushEnvelope, ServerPushEvent, ServerResponseEnvelope,
    SessionToken, SubscriptionId, WatchChangedEvent, WorkspaceCurrentResponse, WorkspaceEntry,
    WorkspaceEntryKind, WorkspaceId, negotiate_protocol_version, web_file_read_capability,
};

#[test]
fn path_handle_serde_roundtrip() {
    let handle = PathHandle::new(WorkspaceId::new("ws"), ["src", "main.scad"]).unwrap();
    let json = serde_json::to_string(&handle).unwrap();
    let decoded: PathHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, handle);
    assert_eq!(decoded.display_path(), "src/main.scad");
}

#[test]
fn handshake_and_command_roundtrip() {
    let capabilities = ClientCapabilities {
        client_name: "studio-web".into(),
        platform: ClientPlatform::Web,
        protocol_version: ProtocolVersionRange::new(1, 3),
        file_read: web_file_read_capability(),
        supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
    };
    let request = CapabilityHandshakeRequest {
        capabilities: capabilities.clone(),
    };
    let json = serde_json::to_string(&request).unwrap();
    let decoded: CapabilityHandshakeRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);

    let path = PathHandle::new(WorkspaceId::new("ws"), ["docs", "readme.md"]).unwrap();
    let request = ClientRequestEnvelope {
        request_id: RequestId(7),
        command: ClientCommand::PreviewRequest(PreviewRequest {
            source: path,
            defines: vec!["height=12".into()],
            kind: PreviewRequestKind::GeometryArtifact,
            configured_openscad_path: None,
        }),
    };
    let json = serde_json::to_string(&request).unwrap();
    let decoded: ClientRequestEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, request);
}

#[test]
fn event_error_and_response_roundtrip() {
    let path = PathHandle::new(WorkspaceId::new("ws"), ["docs", "guide.md"]).unwrap();
    let success = ServerResponseEnvelope {
        request_id: RequestId(2),
        result: Ok(CommandSuccess::FileRead(FileReadResponse {
            path: path.clone(),
            media_type: "text/markdown".into(),
            contents: FileReadContents::Utf8Text("# hi".into()),
        })),
    };
    let json = serde_json::to_string(&success).unwrap();
    let decoded: ServerResponseEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, success);

    let push = ServerPushEnvelope {
        event: ServerPushEvent::WatchChanged(WatchChangedEvent {
            subscription_id: SubscriptionId("sub-1".into()),
            changed_paths: vec![path],
        }),
    };
    let json = serde_json::to_string(&push).unwrap();
    let decoded: ServerPushEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, push);

    let error = ProtocolError::new(ProtocolErrorCode::UnsupportedFileTypeForClient, "blocked");
    let json = serde_json::to_string(&error).unwrap();
    let decoded: ProtocolError = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, error);
}

#[test]
fn protocol_version_negotiates_min_and_max_overlap() {
    let negotiated = negotiate_protocol_version(
        ProtocolVersionRange::new(1, 3),
        ProtocolVersionRange::new(2, 4),
    )
    .unwrap();
    assert_eq!(negotiated, 3);

    let negotiated = negotiate_protocol_version(
        ProtocolVersionRange::new(1, 2),
        ProtocolVersionRange::new(2, 5),
    )
    .unwrap();
    assert_eq!(negotiated, 2);
}

#[test]
fn preview_payload_roundtrip_for_small_and_large_cases() {
    let small = preview_response(1);
    let json = serde_json::to_string(&small).unwrap();
    let decoded: PreviewReadyResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, small);

    let large = preview_response(128);
    let json = serde_json::to_string(&large).unwrap();
    let decoded: PreviewReadyResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, large);
}

fn preview_response(vertex_count: usize) -> PreviewReadyResponse {
    let positions = (0..vertex_count)
        .map(|index| [index as f32, index as f32 + 1.0, index as f32 + 2.0])
        .collect::<Vec<_>>();
    let normals = vec![[0.0, 0.0, 1.0]; vertex_count];
    let colors = vec![[0.2, 0.4, 0.6, 1.0]; vertex_count];
    let indices = (0..vertex_count as u32).collect::<Vec<_>>();
    PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact: PreviewArtifact::Mesh(PreviewMeshPayload {
            unit: PreviewUnit::Millimeter,
            positions,
            normals,
            vertex_colors: colors,
            indices,
        }),
    }
}

#[test]
fn reclaim_and_artifact_variants_roundtrip() {
    let response = CapabilityHandshakeResponse {
        negotiated_version: 2,
        session_token: SessionToken("session-1".into()),
        server_capabilities: ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(1, 2),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![
                PreviewRequestKind::GeometryArtifact,
                PreviewRequestKind::RenderedImage,
            ],
            supports_session_reclaim: true,
        },
    };
    let json = serde_json::to_string(&response).unwrap();
    let decoded: CapabilityHandshakeResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, response);

    let artifact = PreviewArtifact::ThreeMf(PreviewArtifact3mf {
        bytes: vec![1, 2, 3],
        media_type: "model/3mf".into(),
    });
    let json = serde_json::to_string(&artifact).unwrap();
    let decoded: PreviewArtifact = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, artifact);

    let artifact = PreviewArtifact::RenderedImage(PreviewRenderedImagePayload {
        bytes: vec![9, 8, 7],
        media_type: "image/png".into(),
        width: 64,
        height: 64,
    });
    let json = serde_json::to_string(&artifact).unwrap();
    let decoded: PreviewArtifact = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, artifact);

    let response = WorkspaceCurrentResponse {
        workspace_id: WorkspaceId::new("ws"),
        root_name: "workspace".into(),
    };
    let entry = WorkspaceEntry {
        path: PathHandle::new(WorkspaceId::new("ws"), ["src", "main.rs"]).unwrap(),
        kind: WorkspaceEntryKind::File,
    };
    assert_eq!(response.workspace_id.0, "ws");
    assert_eq!(entry.kind, WorkspaceEntryKind::File);

    let cancel = CancelRequest {
        request_id: RequestId(99),
    };
    let json = serde_json::to_string(&cancel).unwrap();
    let decoded: CancelRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, cancel);

    let capability = FileReadCapability {
        denied_extensions: vec![".bin".into()],
    };
    assert_eq!(capability.denied_extensions, vec![".bin"]);
}
