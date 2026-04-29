use app_server_protocol::{
    AgentCancelRequest, AgentCancelledResponse, AgentDoneEvent, AgentErrorEvent, AgentErrorType,
    AgentInvokeRequest, AgentMode, AgentPlanPackageRef, AgentPlanSavedEvent, AgentStartedResponse,
    AgentTokenEvent, AgentToolResultEvent, AgentToolStartEvent, CURRENT_PROTOCOL_VERSION,
    CadQueryFeatureFaces, CadQueryMeshPayload, CadQueryObjectKind, CadQueryPartMesh,
    CadQueryResultReady, CancelRequest, CapabilityHandshakeRequest, CapabilityHandshakeResponse,
    ChatAckResponse, ChatArchiveRequest, ChatArchivedResponse, ChatCreateRequest,
    ChatCreatedResponse, ChatHistoryRequest, ChatHistoryResponse, ChatListRequest,
    ChatListResponse, ChatMessageRecord, ChatRole, ChatSendRequest, ChatSessionId,
    ChatSessionSummary, ChatToolCallRecord, ChatToolResultRecord, ClientCapabilities,
    ClientCommand, ClientEnvelope, ClientPlatform, ClientRequestEnvelope, CommandSuccess,
    EdgeGroup, FaceGroup, FileReadCapability, FileReadContents, FileReadResponse, PathHandle,
    PreviewArtifact, PreviewArtifact3mf, PreviewArtifactStl, PreviewMeshPayload,
    PreviewReadyResponse, PreviewRenderedImagePayload, PreviewRequest, PreviewRequestKind,
    PreviewResponseFormat, PreviewUnit, ProtocolError, ProtocolErrorCode, ProtocolVersionRange,
    RequestId, SelectionKind, SelectionRef, SelectionUpdateRequest, SelectionUpdateResponse,
    ServerCapabilities, ServerEnvelope, ServerPushEnvelope, ServerPushEvent,
    ServerResponseEnvelope, SessionToken, SubscriptionId, VertexPoint, WatchChangedEvent,
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
            protocol_version: ProtocolVersionRange::new(1, CURRENT_PROTOCOL_VERSION),
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
        negotiated_version: CURRENT_PROTOCOL_VERSION,
        session_token: SessionToken("session-1".into()),
        server_capabilities: ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(1, CURRENT_PROTOCOL_VERSION),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![
                PreviewRequestKind::GeometryArtifact,
                PreviewRequestKind::RenderedImage,
            ],
            supports_session_reclaim: true,
            cadquery: true,
            agent: false,
            selection_sync: false,
            llm_configured: false,
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

    let artifact = PreviewArtifact::Stl(PreviewArtifactStl {
        bytes: vec![4, 5, 6],
        media_type: "model/stl".into(),
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

#[test]
fn preview_response_format_stl_uses_stable_discriminant() {
    let encoded = borsh::to_vec(&PreviewResponseFormat::Stl).expect("format encodes");
    assert_eq!(encoded, vec![3]);
}

#[test]
fn cadquery_payload_roundtrips_and_ready_counts_are_lightweight() {
    let payload = CadQueryMeshPayload {
        result_id: "cq_1".into(),
        build_id: valid_sha256_build_id(),
        unit: PreviewUnit::Millimeter,
        root_ref_text: "@assembly[full]".into(),
        root_object_kind: CadQueryObjectKind::Assembly,
        parts: vec![CadQueryPartMesh {
            name: "top_lid".into(),
            object_kind: CadQueryObjectKind::Part,
            ref_text: "@part[top_lid]".into(),
            instance_path: Some("full/top_lid".into()),
            transform: Some([
                1.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0, 4.0, 0.0, 0.0, 0.0, 1.0,
            ]),
            faces: vec![FaceGroup {
                face_idx: 0,
                positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                features: vec!["top_surface".into()],
                ambiguous: false,
            }],
            edges: vec![EdgeGroup {
                edge_idx: 0,
                polyline: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                adjacent_faces: vec![0],
            }],
            vertices: vec![VertexPoint {
                vertex_idx: 0,
                position: [0.0, 0.0, 0.0],
                adjacent_edges: vec![0],
            }],
            feature_map: vec![CadQueryFeatureFaces {
                feature: "top_surface".into(),
                face_indices: vec![0],
            }],
        }],
    };
    let decoded: CadQueryMeshPayload =
        borsh::from_slice(&borsh::to_vec(&payload).unwrap()).unwrap();
    assert_eq!(decoded, payload);

    let ready = CadQueryResultReady {
        result_id: payload.result_id.clone(),
        build_id: payload.build_id.clone(),
        part_count: 1,
        face_count: 1,
        edge_count: 1,
        vertex_count: 1,
    };
    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(42),
        result: Ok(CommandSuccess::CadQueryResultReady(ready.clone())),
    });
    let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);
}

#[test]
fn cadquery_mesh_frame_rejects_non_finite_values_before_encoding() {
    let mut payload = cadquery_sample_payload();
    payload.parts[0].faces[0].positions[1] = f32::NAN;
    let frame = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(1),
        result: Ok(CommandSuccess::CadQueryMesh(payload)),
    });
    let error = encode_server_frame(&frame).expect_err("NaN must fail validation");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidNumericValue);
}

#[test]
fn cadquery_mesh_frame_rejects_invalid_lengths_and_indices() {
    let mut payload = cadquery_sample_payload();
    payload.parts[0].faces[0].positions = vec![0.0, 1.0];
    let error = encode_server_frame(&cadquery_mesh_frame(payload))
        .expect_err("invalid xyz length must fail validation");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);

    let mut payload = cadquery_sample_payload();
    payload.parts[0].feature_map[0].face_indices = vec![99];
    let error = encode_server_frame(&cadquery_mesh_frame(payload))
        .expect_err("invalid feature face index must fail validation");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn cadquery_mesh_frame_rejects_out_of_range_topology_ids() {
    let mut payload = cadquery_sample_payload();
    payload.parts[0].faces[0].face_idx = 9;
    let error = encode_server_frame(&cadquery_mesh_frame(payload)).expect_err("face_idx must fail");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);

    let mut payload = cadquery_sample_payload();
    payload.parts[0].edges[0].edge_idx = 9;
    let error = encode_server_frame(&cadquery_mesh_frame(payload)).expect_err("edge_idx must fail");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);

    let mut payload = cadquery_sample_payload();
    payload.parts[0].vertices[0].vertex_idx = 9;
    let error =
        encode_server_frame(&cadquery_mesh_frame(payload)).expect_err("vertex_idx must fail");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn cadquery_mesh_frame_rejects_invalid_build_id_shape() {
    let mut payload = cadquery_sample_payload();
    payload.build_id = "build".into();
    let error = encode_server_frame(&cadquery_mesh_frame(payload))
        .expect_err("invalid build_id must fail validation");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);
}

fn cadquery_sample_payload() -> CadQueryMeshPayload {
    CadQueryMeshPayload {
        result_id: "cq_bad".into(),
        build_id: valid_sha256_build_id(),
        unit: PreviewUnit::Millimeter,
        root_ref_text: "@part[top_lid]".into(),
        root_object_kind: CadQueryObjectKind::Part,
        parts: vec![CadQueryPartMesh {
            name: "top_lid".into(),
            object_kind: CadQueryObjectKind::Part,
            ref_text: "@part[top_lid]".into(),
            instance_path: None,
            transform: None,
            faces: vec![FaceGroup {
                face_idx: 0,
                positions: vec![0.0, 0.0, 0.0],
                normals: vec![0.0, 0.0, 1.0],
                features: Vec::new(),
                ambiguous: false,
            }],
            edges: vec![EdgeGroup {
                edge_idx: 0,
                polyline: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                adjacent_faces: vec![0],
            }],
            vertices: vec![VertexPoint {
                vertex_idx: 0,
                position: [0.0, 0.0, 0.0],
                adjacent_edges: vec![0],
            }],
            feature_map: vec![CadQueryFeatureFaces {
                feature: "top_surface".into(),
                face_indices: vec![0],
            }],
        }],
    }
}

fn valid_sha256_build_id() -> String {
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
}

fn cadquery_mesh_frame(payload: CadQueryMeshPayload) -> ServerEnvelope {
    ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(1),
        result: Ok(CommandSuccess::CadQueryMesh(payload)),
    })
}

#[test]
fn chat_agent_and_selection_payloads_roundtrip() {
    let session_id = ChatSessionId("main".into());
    let related_file = PathHandle::new(WorkspaceId::new("ws"), ["parts", "top_lid.py"]).unwrap();
    let create = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(50),
        command: ClientCommand::ChatCreate(ChatCreateRequest {
            title: "main".into(),
            goal: Some("lid iteration".into()),
            related_files: vec![related_file.clone()],
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&create).unwrap()).unwrap();
    assert_eq!(decoded, create);

    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(50),
        result: Ok(CommandSuccess::ChatCreated(ChatCreatedResponse {
            session_id: session_id.clone(),
            title: "main".into(),
        })),
    });
    let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);

    let list = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(51),
        command: ClientCommand::ChatList(ChatListRequest {
            include_archived: false,
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&list).unwrap()).unwrap();
    assert_eq!(decoded, list);

    let history = ChatHistoryResponse {
        session_id: session_id.clone(),
        messages: vec![ChatMessageRecord {
            message_id: "msg-1".into(),
            ts_ms: 1_000,
            role: ChatRole::User,
            content: "make it taller".into(),
            related_files: vec![related_file.clone()],
            tool_call_id: None,
            tool_calls: vec![ChatToolCallRecord {
                tool_call_id: "tool-1".into(),
                tool_name: "cadquery".into(),
                args_json: "{}".into(),
            }],
            tool_result: Some(ChatToolResultRecord {
                tool_call_id: "tool-1".into(),
                tool_name: "cadquery".into(),
                result_json: "{\"ok\":true}".into(),
            }),
            mesh_result: None,
        }],
    };
    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(52),
        result: Ok(CommandSuccess::ChatHistory(history)),
    });
    let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);

    let send = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(53),
        command: ClientCommand::ChatSend(ChatSendRequest {
            session_id: session_id.clone(),
            content: "preview this".into(),
            related_files: vec![related_file],
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&send).unwrap()).unwrap();
    assert_eq!(decoded, send);

    let archive = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(54),
        command: ClientCommand::ChatArchive(ChatArchiveRequest {
            session_id: session_id.clone(),
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&archive).unwrap()).unwrap();
    assert_eq!(decoded, archive);
}

#[test]
fn agent_push_events_and_busy_error_roundtrip() {
    let session_id = ChatSessionId("main".into());
    let invoke = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(60),
        command: ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            prompt: "make a taller lid".into(),
            mode: AgentMode::Plan,
            plan_ref: None,
            context_refs: Vec::new(),
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&invoke).unwrap()).unwrap();
    assert_eq!(decoded, invoke);

    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(60),
        result: Ok(CommandSuccess::AgentStarted(AgentStartedResponse {
            session_id: session_id.clone(),
            run_id: "run-1".into(),
        })),
    });
    let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);

    let token = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::AgentToken(AgentTokenEvent {
            session_id: session_id.clone(),
            run_id: "run-1".into(),
            text: "draft".into(),
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&token).unwrap()).unwrap();
    assert_eq!(decoded, token);

    let tool_start = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::AgentToolStart(AgentToolStartEvent {
            session_id: session_id.clone(),
            run_id: "run-1".into(),
            tool_call_id: "tool-1".into(),
            tool_name: "cadquery.preview".into(),
            args_json: "{\"target\":\"parts/top_lid.py\"}".into(),
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&tool_start).unwrap()).unwrap();
    assert_eq!(decoded, tool_start);

    let done = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::AgentDone(AgentDoneEvent {
            session_id: session_id.clone(),
            run_id: "run-1".into(),
            cancelled: false,
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&done).unwrap()).unwrap();
    assert_eq!(decoded, done);

    let error = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(61),
        result: Err(ProtocolError::new(ProtocolErrorCode::AgentBusy, "busy")),
    });
    let decoded = decode_server_frame(&encode_server_frame(&error).unwrap()).unwrap();
    assert_eq!(decoded, error);

    let cancel = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(62),
        command: ClientCommand::AgentCancel(AgentCancelRequest {
            run_id: Some("run-1".into()),
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&cancel).unwrap()).unwrap();
    assert_eq!(decoded, cancel);

    let cancelled = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(62),
        result: Ok(CommandSuccess::AgentCancelled(AgentCancelledResponse {
            run_id: Some("run-1".into()),
        })),
    });
    let decoded = decode_server_frame(&encode_server_frame(&cancelled).unwrap()).unwrap();
    assert_eq!(decoded, cancelled);

    let tool_result = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::AgentToolResult(AgentToolResultEvent {
            session_id: session_id.clone(),
            run_id: "run-1".into(),
            tool_call_id: "tool-1".into(),
            tool_name: "cadquery".into(),
            result_json: "{\"ok\":true}".into(),
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&tool_result).unwrap()).unwrap();
    assert_eq!(decoded, tool_result);

    let plan_ref = workspace_path(["plans", "2026050100-add-lid-vents"]);
    let saved = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::AgentPlanSaved(AgentPlanSavedEvent {
            session_id: session_id.clone(),
            run_id: "run-1".into(),
            package: AgentPlanPackageRef {
                plan_id: "2026050100-add-lid-vents".into(),
                plan_ref: plan_ref.clone(),
                request_path: workspace_path(["plans", "2026050100-add-lid-vents", "request.md"]),
                plan_path: workspace_path(["plans", "2026050100-add-lid-vents", "plan.md"]),
                result_path: workspace_path([
                    "plans",
                    "2026050100-add-lid-vents",
                    "plan-result.md",
                ]),
            },
            title: "Add lid vents".into(),
            status: "planned".into(),
            target_path: workspace_path(["parts", "top_lid.py"]),
            target_type: CadQueryObjectKind::Part,
            affected_files: vec![workspace_path(["parts", "top_lid.py"])],
            new_files: Vec::new(),
            change_description: "Add vents".into(),
            export_targets: vec![workspace_path(["outputs", "top_lid.step"])],
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&saved).unwrap()).unwrap();
    assert_eq!(decoded, saved);

    let run_plan = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(63),
        command: ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            prompt: "run plan".into(),
            mode: AgentMode::Agent,
            plan_ref: Some(plan_ref),
            context_refs: vec!["@part[top_lid]".into()],
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&run_plan).unwrap()).unwrap();
    assert_eq!(decoded, run_plan);

    let agent_error = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::AgentError(AgentErrorEvent {
            session_id,
            run_id: Some("run-1".into()),
            error_type: AgentErrorType::LlmError,
            message: "provider failed".into(),
        }),
    });
    let decoded = decode_server_frame(&encode_server_frame(&agent_error).unwrap()).unwrap();
    assert_eq!(decoded, agent_error);
}

fn workspace_path<const N: usize>(segments: [&str; N]) -> PathHandle {
    PathHandle::new(WorkspaceId::new("workspace"), segments).expect("workspace path")
}

#[test]
fn selection_update_payload_roundtrips() {
    let selection = SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[top_lid:f_0]".into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.top_surface]".into()),
        build_id: Some(valid_sha256_build_id()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    };
    let envelope = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(70),
        command: ClientCommand::SelectionUpdate(SelectionUpdateRequest {
            selections: vec![selection],
            active_index: Some(0),
        }),
    });
    let decoded = decode_client_frame(&encode_client_frame(&envelope).unwrap()).unwrap();
    assert_eq!(decoded, envelope);

    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(70),
        result: Ok(CommandSuccess::SelectionUpdated(SelectionUpdateResponse {
            accepted_count: 1,
        })),
    });
    let decoded = decode_server_frame(&encode_server_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);

    let summary = ChatListResponse {
        sessions: vec![ChatSessionSummary {
            session_id: ChatSessionId("main".into()),
            title: "main".into(),
            archived: false,
            message_count: 1,
            related_files: Vec::new(),
        }],
    };
    let bytes = borsh::to_vec(&summary).unwrap();
    let decoded: ChatListResponse = borsh::from_slice(&bytes).unwrap();
    assert_eq!(decoded, summary);

    let ack = ChatAckResponse {
        session_id: ChatSessionId("main".into()),
        message_id: "msg-1".into(),
    };
    let decoded: ChatAckResponse = borsh::from_slice(&borsh::to_vec(&ack).unwrap()).unwrap();
    assert_eq!(decoded, ack);

    let archived = ChatArchivedResponse {
        session_id: ChatSessionId("main".into()),
    };
    let decoded: ChatArchivedResponse =
        borsh::from_slice(&borsh::to_vec(&archived).unwrap()).unwrap();
    assert_eq!(decoded, archived);

    let history_request = ChatHistoryRequest {
        session_id: ChatSessionId("main".into()),
        limit: Some(50),
    };
    let decoded: ChatHistoryRequest =
        borsh::from_slice(&borsh::to_vec(&history_request).unwrap()).unwrap();
    assert_eq!(decoded, history_request);
}
