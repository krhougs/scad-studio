use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCapabilities,
    ClientCommand, ClientEnvelope, ClientPlatform, ClientRequestEnvelope, CommandSuccess,
    FileReadCapability, FileReadContents, FileReadResponse, PathHandle, PreviewArtifact,
    PreviewArtifact3mf, PreviewMeshPayload, PreviewReadyResponse, PreviewRenderedImagePayload,
    PreviewRequest, PreviewRequestKind, PreviewUnit, ProtocolError, ProtocolErrorCode,
    ProtocolVersionRange, RequestId, ServerCapabilities, ServerEnvelope, ServerPushEnvelope,
    ServerPushEvent, ServerResponseEnvelope, SessionToken, SubscriptionId, WatchChangedEvent,
    WorkspaceCurrentResponse, WorkspaceEntry, WorkspaceEntryKind, WorkspaceId, decode_client_frame,
    decode_server_frame, encode_client_frame, encode_server_frame, negotiate_protocol_version,
    web_file_read_capability,
};

#[test]
fn path_handle_borsh_roundtrip() {
    let handle = PathHandle::new(WorkspaceId::new("ws"), ["src", "main.scad"]).unwrap();
    let bytes = borsh::to_vec(&handle).unwrap();
    let decoded: PathHandle = borsh::from_slice(&bytes).unwrap();
    assert_eq!(decoded, handle);
    assert_eq!(decoded.display_path(), "src/main.scad");
}

#[test]
fn handshake_and_command_frame_roundtrip() {
    let request = CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-web".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(1, 3),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    };
    let envelope = ClientEnvelope::Handshake(request.clone());
    let decoded = decode_client_frame(&encode_client_frame(&envelope).unwrap()).unwrap();
    assert_eq!(decoded, envelope);

    let path = PathHandle::new(WorkspaceId::new("ws"), ["docs", "readme.md"]).unwrap();
    let envelope = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(7),
        command: ClientCommand::PreviewRequest(PreviewRequest {
            source: path,
            defines: vec!["height=12".into()],
            kind: PreviewRequestKind::GeometryArtifact,
            configured_openscad_path: None,
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&envelope).unwrap()).unwrap();
    assert_eq!(decoded, envelope);
}

#[test]
fn event_error_and_response_frame_roundtrip() {
    let path = PathHandle::new(WorkspaceId::new("ws"), ["docs", "guide.md"]).unwrap();
    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(2),
        result: Ok(CommandSuccess::FileRead(FileReadResponse {
            path: path.clone(),
            media_type: "text/markdown".into(),
            contents: FileReadContents::Utf8Text("# hi".into()),
        })),
    });
    let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);

    let push = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::WatchChanged(WatchChangedEvent {
            subscription_id: SubscriptionId("sub-1".into()),
            changed_paths: vec![path],
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&push).unwrap()).unwrap();
    assert_eq!(decoded, push);

    let error = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(3),
        result: Err(ProtocolError::new(
            ProtocolErrorCode::UnsupportedFileTypeForClient,
            "blocked",
        )),
    });
    let decoded = decode_server_frame(&encode_server_frame(&error).unwrap()).unwrap();
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
fn preview_payload_roundtrips_for_small_and_large_cases() {
    for vertex_count in [1, 128] {
        let response = ServerEnvelope::Response(ServerResponseEnvelope {
            request_id: RequestId(vertex_count as u64),
            result: Ok(CommandSuccess::PreviewReady(preview_response(vertex_count))),
        });
        let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
        assert_eq!(decoded, response);
    }
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
    let bytes = borsh::to_vec(&response).unwrap();
    let decoded: CapabilityHandshakeResponse = borsh::from_slice(&bytes).unwrap();
    assert_eq!(decoded, response);

    let artifact = PreviewArtifact::ThreeMf(PreviewArtifact3mf {
        bytes: vec![1, 2, 3],
        media_type: "model/3mf".into(),
    });
    let decoded: PreviewArtifact = borsh::from_slice(&borsh::to_vec(&artifact).unwrap()).unwrap();
    assert_eq!(decoded, artifact);

    let artifact = PreviewArtifact::RenderedImage(PreviewRenderedImagePayload {
        bytes: vec![9, 8, 7],
        media_type: "image/png".into(),
        width: 64,
        height: 64,
    });
    let decoded: PreviewArtifact = borsh::from_slice(&borsh::to_vec(&artifact).unwrap()).unwrap();
    assert_eq!(decoded, artifact);

    let response = WorkspaceCurrentResponse {
        workspace_id: WorkspaceId::new("ws"),
        root_name: "workspace".into(),
    };
    let entry = WorkspaceEntry {
        name: "main.rs".into(),
        path: Some(PathHandle::new(WorkspaceId::new("ws"), ["src", "main.rs"]).unwrap()),
        kind: WorkspaceEntryKind::File,
        path_error: None,
    };
    assert_eq!(response.workspace_id.0, "ws");
    assert_eq!(entry.kind, WorkspaceEntryKind::File);

    let cancel = CancelRequest {
        request_id: RequestId(99),
    };
    let decoded: CancelRequest = borsh::from_slice(&borsh::to_vec(&cancel).unwrap()).unwrap();
    assert_eq!(decoded, cancel);

    let capability = FileReadCapability {
        denied_extensions: vec![".bin".into()],
    };
    assert_eq!(capability.denied_extensions, vec![".bin"]);
}
