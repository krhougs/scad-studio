use std::collections::VecDeque;

use app_server_protocol::{
    AgentCancelRequest, AgentCancelledResponse, AgentDoneEvent, AgentErrorEvent, AgentErrorType,
    AgentEventId, AgentEventPayload, AgentEventRecord, AgentId, AgentInvokeRequest, AgentMode,
    AgentModelDiscoveryState, AgentModelDiscoveryStatus, AgentModelRegistryModel,
    AgentModelRegistryProvider, AgentModelRegistryResponse, AgentModelSource,
    AgentProviderCapabilities, AgentRuntimeStatus, AgentSnapshotRequest, AgentSnapshotResponse,
    AgentStartedResponse, AgentTokenEvent, AgentTurnId, CadQueryArtifactExport,
    CadQueryArtifactRelation, CadQueryResultReady, CapabilityHandshakeRequest,
    CapabilityHandshakeResponse, ChatCreatedResponse, ChatHistoryResponse, ChatListResponse,
    ChatMessageRecord, ChatRole, ChatSessionId, ChatSessionSummary, ClientCapabilities,
    ClientCommand, ClientEnvelope, ClientPlatform, ClientRequestEnvelope, CommandSuccess,
    PathHandle, PreviewRequest, PreviewRequestKind, ProtocolError, ProtocolErrorCode,
    ProtocolVersionRange, RequestId, SelectionKind, SelectionRef, SelectionUpdateRequest,
    SelectionUpdateResponse, ServerCapabilities, ServerEnvelope, ServerPushEnvelope,
    ServerPushEvent, ServerResponseEnvelope, SessionToken, SubscriptionId, WatchChangedEvent,
    WatchSubscribeRequest, WatchSubscriptionAck, WorkspaceCurrentResponse, WorkspaceId,
    decode_client_frame, encode_server_frame, web_file_read_capability,
};
use studio_common::{
    AppServerTransportError, AppServerTransportEvent, AppServerTransportPort, ClientError,
    ClientEvent, ClientTimeouts, ManagedClient, PreviewPhase, TransportCloseReason,
    TransportStatus, WatchParams,
};

#[derive(Default)]
struct FakeTransport {
    _pending: VecDeque<AppServerTransportEvent>,
}

impl AppServerTransportPort for FakeTransport {
    fn handshake(
        &mut self,
        _request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn reconnect(
        &mut self,
        _request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn request(&mut self, _request: ClientRequestEnvelope) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn subscribe(
        &mut self,
        _request_id: RequestId,
        _request: WatchSubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn unsubscribe(
        &mut self,
        _request_id: RequestId,
        _request: app_server_protocol::WatchUnsubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn cancel(
        &mut self,
        _request_id: RequestId,
        _target_request_id: RequestId,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn poll_server_event(
        &mut self,
    ) -> Result<Option<AppServerTransportEvent>, AppServerTransportError> {
        Ok(self._pending.pop_front())
    }

    fn close(&mut self) -> Result<(), AppServerTransportError> {
        Ok(())
    }
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "managed-client-tests".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(3, 3),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}

fn handshake_response() -> CapabilityHandshakeResponse {
    CapabilityHandshakeResponse {
        negotiated_version: 2,
        session_token: SessionToken("session-1".into()),
        server_capabilities: ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(3, 3),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            supports_session_reclaim: true,
            cadquery: true,
            agent: false,
            selection_sync: false,
            llm_configured: false,
            agent_provider: None,
            agent_model_registry: None,
        },
    }
}

fn encode_handshake_ack(ack: &CapabilityHandshakeResponse) -> Vec<u8> {
    encode_server_frame(&ServerEnvelope::HandshakeAck(ack.clone())).expect("handshake ack encodes")
}

fn encode_response(envelope: &ServerResponseEnvelope) -> Vec<u8> {
    encode_server_frame(&ServerEnvelope::Response(envelope.clone())).expect("response encodes")
}

fn encode_push(envelope: &ServerPushEnvelope) -> Vec<u8> {
    encode_server_frame(&ServerEnvelope::Push(envelope.clone())).expect("push encodes")
}

fn encode_server_envelope(envelope: &ServerEnvelope) -> Vec<u8> {
    encode_server_frame(envelope).expect("server envelope encodes")
}

fn workspace_current_success(request_id: RequestId) -> ServerResponseEnvelope {
    ServerResponseEnvelope {
        request_id,
        result: Ok(CommandSuccess::WorkspaceCurrent(WorkspaceCurrentResponse {
            workspace_id: WorkspaceId::new("workspace"),
            root_name: "workspace".into(),
        })),
    }
}

fn workspace_current_response_bytes(request_id: RequestId) -> Vec<u8> {
    encode_response(&workspace_current_success(request_id))
}

fn open_client_with_handshake(client: &mut ManagedClient<FakeTransport>) {
    client.begin_handshake(handshake_request()).unwrap();
    drain_outbound(client);
    client
        .receive_inbound(&encode_handshake_ack(&handshake_response()))
        .unwrap();
    let _ = client.drain_events();
}

fn drain_outbound(client: &mut ManagedClient<FakeTransport>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while let Some(bytes) = client.next_outbound() {
        out.push(bytes);
    }
    out
}

fn path_handle(name: &str) -> PathHandle {
    PathHandle::new(WorkspaceId::new("workspace"), [name]).expect("path handle")
}

fn sample_subscription_id() -> SubscriptionId {
    SubscriptionId("watch-1".into())
}

fn sample_agent_model_registry() -> AgentModelRegistryResponse {
    AgentModelRegistryResponse {
        active_provider_id: "openai".into(),
        active_model_id: "gpt-5.2".into(),
        active_reasoning_effort: Some("high".into()),
        active_reasoning_effort_applied: true,
        active_service_label: Some("flex".into()),
        active_service_label_applied: true,
        reasoning_effort_options: vec!["low".into(), "medium".into(), "high".into()],
        service_label_options: vec!["default".into(), "flex".into()],
        providers: vec![AgentModelRegistryProvider {
            id: "openai".into(),
            kind: "openai_responses".into(),
            label: None,
            discovery: AgentModelDiscoveryState {
                enabled: true,
                status: AgentModelDiscoveryStatus::Succeeded,
                error: None,
            },
            models: vec![AgentModelRegistryModel {
                id: "gpt-5.2".into(),
                label: Some("GPT 5.2".into()),
                source: AgentModelSource::DiscoveredWithOverride,
                reasoning_effort: Some("high".into()),
                service_label: Some("flex".into()),
                native_web_search_enabled: true,
                native_web_search_applied: true,
                web_search_supported: true,
                web_search_unsupported_reason: None,
                search_sources_supported: false,
            }],
        }],
    }
}

#[test]
fn dispatch_before_handshake_returns_not_ready() {
    let mut client = ManagedClient::new(FakeTransport::default());
    let err = client
        .dispatch_workspace_current()
        .expect_err("should be not ready");
    assert!(matches!(err, ClientError::NotReady));
    assert_eq!(
        client.snapshot().transport_status,
        TransportStatus::Connecting
    );
}

#[test]
fn dispatch_after_handshake_enqueues_envelope() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let request_id = client
        .dispatch_workspace_current()
        .expect("dispatch ok after handshake");
    assert_eq!(request_id, RequestId(1));

    let outbound = drain_outbound(&mut client);
    assert_eq!(outbound.len(), 1);
    match decode_client_frame(&outbound[0]).expect("outbound bytes decode as ClientEnvelope") {
        ClientEnvelope::Request(request) => {
            assert_eq!(request.request_id, RequestId(1));
            assert!(matches!(request.command, ClientCommand::WorkspaceCurrent));
        }
        other => panic!("expected ClientEnvelope::Request, got {other:?}"),
    }
}

#[test]
fn handshake_provider_capability_updates_snapshot() {
    let mut client = ManagedClient::new(FakeTransport::default());
    client.begin_handshake(handshake_request()).unwrap();
    let mut ack = handshake_response();
    ack.server_capabilities.llm_configured = true;
    ack.server_capabilities.agent_provider = Some(AgentProviderCapabilities {
        provider: "openai_responses".into(),
        model: Some("gpt-5.2".into()),
        native_web_search_enabled: true,
        search_sources_supported: false,
    });

    client
        .receive_inbound(&encode_handshake_ack(&ack))
        .expect("handshake ack");

    let snapshot = client.snapshot();
    let provider = snapshot.agent_provider.expect("provider capability");
    assert_eq!(provider.provider, "openai_responses");
    assert_eq!(provider.model.as_deref(), Some("gpt-5.2"));
    assert!(provider.native_web_search_enabled);
    assert!(!provider.search_sources_supported);
    assert!(snapshot.llm_configured);
}

#[test]
fn agent_model_registry_response_updates_snapshot_and_legacy_provider() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let request_id = client
        .dispatch_agent_model_registry()
        .expect("dispatch agent.model.registry");
    assert_eq!(request_id, RequestId(1));
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id,
            result: Ok(CommandSuccess::AgentModelRegistry(
                sample_agent_model_registry(),
            )),
        }))
        .expect("agent model registry response");

    let snapshot = client.snapshot();
    let registry = snapshot
        .agent_model_registry
        .expect("agent model registry snapshot");
    assert_eq!(registry.active_provider_id, "openai");
    assert_eq!(registry.active_model_id, "gpt-5.2");
    assert_eq!(registry.active_reasoning_effort.as_deref(), Some("high"));
    assert_eq!(registry.active_service_label.as_deref(), Some("flex"));
    let provider = snapshot.agent_provider.expect("legacy provider capability");
    assert_eq!(provider.provider, "openai_responses");
    assert_eq!(provider.model.as_deref(), Some("gpt-5.2"));
    assert!(provider.native_web_search_enabled);
    assert!(snapshot.llm_configured);
}

#[test]
fn handshake_wire_format_matches_protocol_envelope() {
    let mut client = ManagedClient::new(FakeTransport::default());
    client.begin_handshake(handshake_request()).unwrap();
    let outbound = drain_outbound(&mut client);
    assert_eq!(outbound.len(), 1);
    let envelope =
        decode_client_frame(&outbound[0]).expect("handshake bytes decode as ClientEnvelope");
    assert!(matches!(envelope, ClientEnvelope::Handshake(_)));
}

#[test]
fn workspace_current_response_updates_snapshot() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let request_id = client.dispatch_workspace_current().unwrap();
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&workspace_current_response_bytes(request_id))
        .unwrap();

    let events = client.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        ClientEvent::RequestSucceeded { request_id: rid, .. } if *rid == request_id
    )));
    let snapshot = client.snapshot();
    assert!(snapshot.workspace_current.is_some());
}

#[test]
fn cancel_marks_target_failed_and_ignores_later_response() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let target = client.dispatch_workspace_current().unwrap();
    let _ = drain_outbound(&mut client);

    let cancel_id = client.cancel(target).unwrap();
    assert_ne!(cancel_id, target);
    let _ = drain_outbound(&mut client);

    let events = client.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        ClientEvent::RequestFailed { request_id: rid, error: ClientError::Cancelled } if *rid == target
    )));

    client
        .receive_inbound(&workspace_current_response_bytes(target))
        .unwrap();
    let events_after = client.drain_events();
    assert!(!events_after
        .iter()
        .any(|event| matches!(event, ClientEvent::RequestSucceeded { request_id: rid, .. } if *rid == target)));
    match client.snapshot().last_error {
        Some(ClientError::UnknownRequest { request_id }) => assert_eq!(request_id, target),
        other => panic!("expected UnknownRequest in last_error, got {other:?}"),
    }
}

#[test]
fn short_timeout_emits_request_timed_out() {
    let mut timeouts = ClientTimeouts::default();
    timeouts.workspace_current = Some(1000);
    let mut client = ManagedClient::with_timeouts(FakeTransport::default(), timeouts);
    open_client_with_handshake(&mut client);

    client.tick(0);
    let request_id = client.dispatch_workspace_current().unwrap();
    let _ = drain_outbound(&mut client);

    client.tick(1001);

    let events = client.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        ClientEvent::RequestTimedOut { request_id: rid } if *rid == request_id
    )));
}

#[test]
fn watch_events_are_throttled_and_deduplicated() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    client.tick(0);
    let watch_request_id = client
        .subscribe_directory_watch(WatchParams {
            request: WatchSubscribeRequest { directory: None },
            throttle_ms: Some(100),
        })
        .unwrap();
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: watch_request_id,
            result: Ok(CommandSuccess::WatchSubscribed(WatchSubscriptionAck {
                subscription_id: sample_subscription_id(),
            })),
        }))
        .unwrap();

    let path_a = path_handle("a.scad");
    let path_b = path_handle("b.scad");
    client.tick(10);
    for paths in [
        vec![path_a.clone()],
        vec![path_a.clone(), path_b.clone()],
        vec![path_b.clone()],
    ] {
        client
            .receive_inbound(&encode_push(&ServerPushEnvelope {
                event: ServerPushEvent::WatchChanged(WatchChangedEvent {
                    subscription_id: sample_subscription_id(),
                    changed_paths: paths,
                }),
            }))
            .unwrap();
    }

    client.tick(200);
    let events = client.drain_events();
    let watch_events: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, ClientEvent::WatchEvent { .. }))
        .collect();
    assert_eq!(watch_events.len(), 1);
    match &watch_events[0] {
        ClientEvent::WatchEvent {
            payload: studio_common::WatchEventPayload::Changed { changed_paths, .. },
            ..
        } => {
            assert_eq!(changed_paths.len(), 2);
            assert!(changed_paths.contains(&path_a));
            assert!(changed_paths.contains(&path_b));
        }
        other => panic!("unexpected watch event: {other:?}"),
    }
}

#[test]
fn chat_list_changed_push_updates_snapshot_current_chat() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    client
        .receive_inbound(&encode_push(&ServerPushEnvelope {
            event: ServerPushEvent::ChatListChanged(chat_list_response()),
        }))
        .unwrap();

    let snapshot = client.snapshot();
    assert_eq!(
        snapshot.current_chat_session,
        Some(ChatSessionId("main".into()))
    );
    assert_eq!(snapshot.chat_sessions.len(), 1);
    assert!(
        client
            .drain_events()
            .iter()
            .any(|event| matches!(event, ClientEvent::SnapshotChanged))
    );
}

#[test]
fn reconnect_replays_pending_and_resubscribes_watch() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let req1 = client.dispatch_workspace_current().unwrap();
    let req2 = client.dispatch_workspace_current().unwrap();
    let watch_id = client
        .subscribe_directory_watch(WatchParams {
            request: WatchSubscribeRequest { directory: None },
            throttle_ms: Some(100),
        })
        .unwrap();
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: watch_id,
            result: Ok(CommandSuccess::WatchSubscribed(WatchSubscriptionAck {
                subscription_id: sample_subscription_id(),
            })),
        }))
        .unwrap();
    let _ = client.drain_events();

    client.mark_transport_closed(TransportCloseReason {
        code: 1006,
        reason: "disconnect".into(),
        was_clean: false,
    });
    let events_after_close = client.drain_events();
    assert!(
        events_after_close
            .iter()
            .any(|event| matches!(event, ClientEvent::TransportClosed { .. }))
    );

    client.begin_handshake(handshake_request()).unwrap();
    let queued = drain_outbound(&mut client);
    assert_eq!(queued.len(), 4, "reconnect + two pending + watch");

    let reconnect_envelope =
        decode_client_frame(&queued[0]).expect("reconnect decodes as ClientEnvelope");
    assert!(matches!(reconnect_envelope, ClientEnvelope::Reconnect(_)));
    assert_request_id(&queued[1], req1);
    assert_request_id(&queued[2], req2);
    match decode_client_frame(&queued[3]).expect("watch subscribe decodes") {
        ClientEnvelope::Request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::WatchSubscribe(_),
        }) => assert_eq!(request_id, watch_id),
        other => panic!("expected watch subscribe request, got {other:?}"),
    }

    client
        .receive_inbound(&encode_handshake_ack(&handshake_response()))
        .unwrap();
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: watch_id,
            result: Ok(CommandSuccess::WatchSubscribed(WatchSubscriptionAck {
                subscription_id: SubscriptionId("watch-2".into()),
            })),
        }))
        .unwrap();

    let events = client.drain_events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ClientEvent::TransportOpen))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        ClientEvent::WatchResubscribed { request_id: rid } if *rid == watch_id
    )));
}

#[test]
fn inbound_transport_error_marks_reconnecting_and_records_last_error() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let bytes = encode_server_envelope(&ServerEnvelope::TransportError(
        app_server_protocol::TransportErrorFrame {
            message: "upstream hangup".into(),
        },
    ));
    client.receive_inbound(&bytes).unwrap();

    let events = client.drain_events();
    let close_event = events
        .iter()
        .find_map(|event| match event {
            ClientEvent::TransportClosed { reason } => Some(reason.clone()),
            _ => None,
        })
        .expect("TransportClosed event emitted");
    assert!(!close_event.was_clean);
    assert_eq!(close_event.reason, "upstream hangup");

    let snapshot = client.snapshot();
    assert_eq!(snapshot.transport_status, TransportStatus::Reconnecting);
    match snapshot.last_error {
        Some(ClientError::ProtocolError { code, message }) => {
            assert_eq!(code, "transport_error");
            assert_eq!(message, "upstream hangup");
        }
        other => panic!("expected ProtocolError last_error, got {other:?}"),
    }
}

#[test]
fn inbound_closed_marks_reconnecting_with_clean_reason() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let bytes = encode_server_envelope(&ServerEnvelope::Closed);
    client.receive_inbound(&bytes).unwrap();

    let events = client.drain_events();
    let close_event = events
        .iter()
        .find_map(|event| match event {
            ClientEvent::TransportClosed { reason } => Some(reason.clone()),
            _ => None,
        })
        .expect("TransportClosed event emitted");
    assert!(close_event.was_clean);
    assert_eq!(
        client.snapshot().transport_status,
        TransportStatus::Reconnecting
    );
}

#[test]
fn client_error_serde_format_matches_contract() {
    let cancelled = ClientError::Cancelled;
    let json = serde_json::to_string(&cancelled).unwrap();
    assert_eq!(json, "{\"type\":\"cancelled\"}");

    let unknown = ClientError::UnknownRequest {
        request_id: RequestId(42),
    };
    let json = serde_json::to_string(&unknown).unwrap();
    assert_eq!(
        json,
        "{\"type\":\"unknown_request\",\"payload\":{\"request_id\":42}}"
    );

    let protocol = ClientError::ProtocolError {
        code: "internal".into(),
        message: "boom".into(),
    };
    let json = serde_json::to_string(&protocol).unwrap();
    assert_eq!(
        json,
        "{\"type\":\"protocol_error\",\"payload\":{\"code\":\"internal\",\"message\":\"boom\"}}"
    );
}

#[test]
fn client_event_serde_tags_match_contract() {
    let timed_out = ClientEvent::RequestTimedOut {
        request_id: RequestId(7),
    };
    let json = serde_json::to_string(&timed_out).unwrap();
    assert_eq!(
        json,
        "{\"type\":\"request_timed_out\",\"payload\":{\"request_id\":7}}"
    );

    let transport_open = ClientEvent::TransportOpen;
    let json = serde_json::to_string(&transport_open).unwrap();
    assert_eq!(json, "{\"type\":\"transport_open\"}");

    let resubscribed = ClientEvent::WatchResubscribed {
        request_id: RequestId(9),
    };
    let json = serde_json::to_string(&resubscribed).unwrap();
    assert_eq!(
        json,
        "{\"type\":\"watch_resubscribed\",\"payload\":{\"request_id\":9}}"
    );
}

fn preview_request_for(name: &str) -> PreviewRequest {
    PreviewRequest {
        source: path_handle(name),
        defines: Vec::new(),
        kind: PreviewRequestKind::GeometryArtifact,
        configured_openscad_path: None,
    }
}

fn preview_protocol_error_bytes(request_id: RequestId, message: &str) -> Vec<u8> {
    encode_response(&ServerResponseEnvelope {
        request_id,
        result: Err(ProtocolError::new(ProtocolErrorCode::Internal, message)),
    })
}

#[test]
fn stale_preview_response_does_not_overwrite_current_preview_state() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let req_a = client
        .dispatch_preview_request(preview_request_for("a.scad"))
        .expect("dispatch preview A");
    let _ = drain_outbound(&mut client);

    let req_b = client
        .dispatch_preview_request(preview_request_for("b.scad"))
        .expect("dispatch preview B");
    let _ = drain_outbound(&mut client);

    let snapshot_before = client.snapshot();
    assert_eq!(
        snapshot_before.active_preview_target,
        Some(path_handle("b.scad"))
    );
    assert!(snapshot_before.preview_error.is_none());

    client
        .receive_inbound(&preview_protocol_error_bytes(req_a, "stale preview boom"))
        .expect("receive stale preview error");

    let snapshot_after = client.snapshot();
    assert!(
        snapshot_after.preview_error.is_none(),
        "stale preview error must not overwrite preview_error, got {:?}",
        snapshot_after.preview_error
    );
    assert_eq!(
        snapshot_after.active_preview_target,
        Some(path_handle("b.scad")),
        "stale preview error must not flip active_preview_target"
    );
    assert!(
        snapshot_after
            .preview_tasks
            .iter()
            .all(|task| task.request_id != req_a),
        "stale preview A should be removed from preview_tasks"
    );
    assert!(
        snapshot_after
            .preview_tasks
            .iter()
            .any(|task| task.request_id == req_b),
        "active preview B should remain in preview_tasks"
    );

    let events = client.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        ClientEvent::RequestFailed { request_id: rid, .. } if *rid == req_a
    )));
}

#[test]
fn fail_preview_decode_moves_latest_preview_to_error() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let request_id = client
        .dispatch_preview_request(preview_request_for("broken.stl"))
        .expect("dispatch preview");
    let _ = drain_outbound(&mut client);

    client.fail_preview_decode(request_id, "stl decode failed".into());

    let snapshot = client.snapshot();
    let task = snapshot
        .preview_tasks
        .iter()
        .find(|task| task.request_id == request_id)
        .expect("preview task remains");
    assert_eq!(task.phase, PreviewPhase::Error);
    assert_eq!(
        snapshot
            .preview_error
            .as_ref()
            .map(|error| error.message.as_str()),
        Some("stl decode failed")
    );
}

#[test]
fn fail_preview_decode_ignores_stale_preview_request() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let req_a = client
        .dispatch_preview_request(preview_request_for("a.stl"))
        .expect("dispatch preview A");
    let _ = drain_outbound(&mut client);
    let req_b = client
        .dispatch_preview_request(preview_request_for("b.stl"))
        .expect("dispatch preview B");
    let _ = drain_outbound(&mut client);

    client.fail_preview_decode(req_a, "stale decode failed".into());

    let snapshot = client.snapshot();
    assert!(snapshot.preview_error.is_none());
    assert!(
        snapshot
            .preview_tasks
            .iter()
            .all(|task| task.request_id != req_a)
    );
    assert!(
        snapshot
            .preview_tasks
            .iter()
            .any(|task| task.request_id == req_b && task.phase == PreviewPhase::Pending)
    );
}

#[test]
fn chat_agent_and_selection_successes_update_snapshot() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let chat_create_id = client
        .dispatch_chat_create(app_server_protocol::ChatCreateRequest {
            title: "main".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: None,
            initial_user_message: None,
            requested_model: None,
            initial_turn: None,
        })
        .expect("dispatch chat.create");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: chat_create_id,
            result: Ok(CommandSuccess::ChatCreated(ChatCreatedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                title: "main".into(),
                initial_turn: None,
            })),
        }))
        .unwrap();

    let chat_list_id = client
        .dispatch_chat_list(app_server_protocol::ChatListRequest {
            include_archived: false,
        })
        .expect("dispatch chat.list");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: chat_list_id,
            result: Ok(CommandSuccess::ChatList(chat_list_response())),
        }))
        .unwrap();

    let selection_request = sample_selection_request();
    let selection_id = client
        .dispatch_selection_update(selection_request)
        .expect("dispatch selection.update");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: selection_id,
            result: Ok(CommandSuccess::SelectionUpdated(SelectionUpdateResponse {
                accepted_count: 1,
            })),
        }))
        .unwrap();

    let invoke_id = client
        .dispatch_agent_invoke(AgentInvokeRequest {
            session_id: ChatSessionId("main".into()),
            client_request_id: None,
            prompt: "inspect".into(),
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        })
        .expect("dispatch agent.invoke");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: invoke_id,
            result: Ok(CommandSuccess::AgentStarted(AgentStartedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                run_id: "agent-1".into(),
                turn_id: "agent-1".into(),
            })),
        }))
        .unwrap();
    push_agent_token_and_done(&mut client);

    let snapshot = client.snapshot();
    assert_eq!(snapshot.chat_sessions.len(), 1);
    assert_eq!(
        snapshot.current_chat_session,
        Some(ChatSessionId("main".into()))
    );
    assert_eq!(snapshot.current_selection.selections.len(), 1);
    assert!(snapshot.agent_run.is_none());
    assert_eq!(snapshot.agent_events.len(), 2);
}

#[test]
fn agent_snapshot_response_updates_structured_agent_events() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);
    let request_id = client
        .dispatch_agent_snapshot(AgentSnapshotRequest {
            agent_id: AgentId("agent-1".into()),
            since_event_id: None,
        })
        .expect("dispatch agent.snapshot");
    drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id,
            result: Ok(CommandSuccess::AgentSnapshot(AgentSnapshotResponse {
                agent_id: AgentId("agent-1".into()),
                chat_id: ChatSessionId("chat-1".into()),
                bound_model: None,
                model_lock_reason: None,
                state: AgentRuntimeStatus::Done,
                active_turn_id: None,
                since_event_id: None,
                events: vec![AgentEventRecord {
                    event_id: AgentEventId(1),
                    agent_id: AgentId("agent-1".into()),
                    turn_id: Some(AgentTurnId("turn-1".into())),
                    ts_ms: 1000,
                    payload: AgentEventPayload::Token { text: "hi".into() },
                }],
                current_text: "hi".into(),
                current_reasoning: String::new(),
                error: None,
            })),
        }))
        .unwrap();

    let snapshot = client.snapshot();
    assert_eq!(
        snapshot.agent_runtime_status,
        Some(AgentRuntimeStatus::Done)
    );
    assert_eq!(snapshot.agent_event_records.len(), 1);
    assert_eq!(
        snapshot.agent_event_records[0].agent_id,
        AgentId("agent-1".into())
    );
}

#[test]
fn chat_created_initial_turn_marks_agent_running() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);
    let request_id = client
        .dispatch_chat_create(app_server_protocol::ChatCreateRequest {
            title: "main".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: None,
            initial_user_message: None,
            requested_model: None,
            initial_turn: None,
        })
        .expect("dispatch chat.create");
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id,
            result: Ok(CommandSuccess::ChatCreated(ChatCreatedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                title: "main".into(),
                initial_turn: Some(AgentStartedResponse {
                    session_id: ChatSessionId("main".into()),
                    agent_id: "agent-main".into(),
                    run_id: "agent-1".into(),
                    turn_id: "agent-1".into(),
                }),
            })),
        }))
        .unwrap();

    assert_eq!(
        client.snapshot().agent_runtime_status,
        Some(AgentRuntimeStatus::Running)
    );
    assert_eq!(
        client.snapshot().agent_run.as_ref().map(|run| &run.run_id),
        Some(&"agent-1".to_string())
    );
}

#[test]
fn chat_created_retry_without_initial_turn_keeps_running_agent() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);
    let first_request_id = client
        .dispatch_chat_create(app_server_protocol::ChatCreateRequest {
            title: "main".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: None,
            initial_user_message: None,
            requested_model: None,
            initial_turn: None,
        })
        .expect("dispatch first chat.create");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: first_request_id,
            result: Ok(CommandSuccess::ChatCreated(ChatCreatedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                title: "main".into(),
                initial_turn: Some(AgentStartedResponse {
                    session_id: ChatSessionId("main".into()),
                    agent_id: "agent-main".into(),
                    run_id: "agent-1".into(),
                    turn_id: "agent-1".into(),
                }),
            })),
        }))
        .unwrap();
    let retry_request_id = client
        .dispatch_chat_create(app_server_protocol::ChatCreateRequest {
            title: "main".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: None,
            initial_user_message: None,
            requested_model: None,
            initial_turn: None,
        })
        .expect("dispatch retry chat.create");
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: retry_request_id,
            result: Ok(CommandSuccess::ChatCreated(ChatCreatedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                title: "main".into(),
                initial_turn: None,
            })),
        }))
        .unwrap();

    assert_eq!(
        client.snapshot().agent_run.as_ref().map(|run| &run.run_id),
        Some(&"agent-1".to_string())
    );
    assert_eq!(
        client.snapshot().agent_runtime_status,
        Some(AgentRuntimeStatus::Running)
    );
}

#[test]
fn agent_cancel_ack_keeps_run_until_done_event() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let invoke_id = client
        .dispatch_agent_invoke(AgentInvokeRequest {
            session_id: ChatSessionId("main".into()),
            client_request_id: None,
            prompt: "inspect".into(),
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        })
        .expect("dispatch agent.invoke");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: invoke_id,
            result: Ok(CommandSuccess::AgentStarted(AgentStartedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                run_id: "agent-1".into(),
                turn_id: "agent-1".into(),
            })),
        }))
        .unwrap();

    let cancel_id = client
        .dispatch_agent_cancel(AgentCancelRequest {
            agent_id: "agent-main".into(),
        })
        .expect("dispatch agent.cancel");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: cancel_id,
            result: Ok(CommandSuccess::AgentCancelled(AgentCancelledResponse {
                agent_id: "agent-main".into(),
                cancelled: true,
            })),
        }))
        .unwrap();

    assert_eq!(
        client.snapshot().agent_run.as_ref().map(|run| &run.run_id),
        Some(&"agent-1".to_string())
    );

    client
        .receive_inbound(&encode_push(&ServerPushEnvelope {
            event: ServerPushEvent::AgentDone(AgentDoneEvent {
                session_id: ChatSessionId("main".into()),
                run_id: "agent-1".into(),
                cancelled: true,
            }),
        }))
        .unwrap();
    assert!(client.snapshot().agent_run.is_none());
}

#[test]
fn agent_error_for_current_run_clears_active_run() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);
    let invoke_id = client
        .dispatch_agent_invoke(AgentInvokeRequest {
            session_id: ChatSessionId("main".into()),
            client_request_id: None,
            prompt: "inspect".into(),
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        })
        .expect("dispatch agent.invoke");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: invoke_id,
            result: Ok(CommandSuccess::AgentStarted(AgentStartedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                run_id: "agent-1".into(),
                turn_id: "agent-1".into(),
            })),
        }))
        .unwrap();

    client
        .receive_inbound(&encode_push(&ServerPushEnvelope {
            event: ServerPushEvent::AgentError(AgentErrorEvent {
                session_id: ChatSessionId("main".into()),
                run_id: Some("agent-1".into()),
                error_type: AgentErrorType::PersistenceError,
                message: "persist failed".into(),
            }),
        }))
        .unwrap();

    assert!(client.snapshot().agent_run.is_none());
    assert_eq!(
        client.snapshot().agent_runtime_status,
        Some(AgentRuntimeStatus::Failed)
    );
}

#[test]
fn stale_agent_error_does_not_mark_current_run_failed() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);
    let invoke_id = client
        .dispatch_agent_invoke(AgentInvokeRequest {
            session_id: ChatSessionId("main".into()),
            client_request_id: None,
            prompt: "inspect".into(),
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        })
        .expect("dispatch agent.invoke");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: invoke_id,
            result: Ok(CommandSuccess::AgentStarted(AgentStartedResponse {
                session_id: ChatSessionId("main".into()),
                agent_id: "agent-main".into(),
                run_id: "agent-2".into(),
                turn_id: "agent-2".into(),
            })),
        }))
        .unwrap();

    client
        .receive_inbound(&encode_push(&ServerPushEnvelope {
            event: ServerPushEvent::AgentError(AgentErrorEvent {
                session_id: ChatSessionId("main".into()),
                run_id: Some("agent-1".into()),
                error_type: AgentErrorType::PersistenceError,
                message: "old run failed".into(),
            }),
        }))
        .unwrap();

    assert_eq!(
        client.snapshot().agent_run.as_ref().map(|run| &run.run_id),
        Some(&"agent-2".to_string())
    );
    assert_eq!(
        client.snapshot().agent_runtime_status,
        Some(AgentRuntimeStatus::Running)
    );
}

#[test]
fn chat_history_response_replaces_snapshot_history() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let request_id = client
        .dispatch_chat_history(app_server_protocol::ChatHistoryRequest {
            session_id: ChatSessionId("main".into()),
            limit: Some(50),
        })
        .expect("dispatch chat.history");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id,
            result: Ok(CommandSuccess::ChatHistory(ChatHistoryResponse {
                session_id: ChatSessionId("main".into()),
                messages: vec![chat_message("msg-1", ChatRole::User, "make lid taller")],
            })),
        }))
        .unwrap();

    let snapshot = client.snapshot();
    assert_eq!(snapshot.current_chat_history.len(), 1);
    assert_eq!(snapshot.current_chat_history[0].content, "make lid taller");
}

#[test]
fn chat_history_response_restores_cadquery_results_from_mesh_records() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);
    let ready = cadquery_ready("cq_chat_history");
    let mut message = chat_message("msg-1", ChatRole::Tool, "agent tool completed");
    message.mesh_result = Some(ready.clone());

    let request_id = client
        .dispatch_chat_history(app_server_protocol::ChatHistoryRequest {
            session_id: ChatSessionId("main".into()),
            limit: Some(50),
        })
        .expect("dispatch chat.history");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id,
            result: Ok(CommandSuccess::ChatHistory(ChatHistoryResponse {
                session_id: ChatSessionId("main".into()),
                messages: vec![message],
            })),
        }))
        .unwrap();

    let snapshot = client.snapshot();
    assert_eq!(snapshot.cadquery_results, vec![ready]);
}

#[test]
fn chat_created_clears_previous_session_history() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let history_request_id = client
        .dispatch_chat_history(app_server_protocol::ChatHistoryRequest {
            session_id: ChatSessionId("main".into()),
            limit: Some(50),
        })
        .expect("dispatch chat.history");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: history_request_id,
            result: Ok(CommandSuccess::ChatHistory(ChatHistoryResponse {
                session_id: ChatSessionId("main".into()),
                messages: vec![chat_message("msg-1", ChatRole::User, "old chat")],
            })),
        }))
        .unwrap();

    let create_request_id = client
        .dispatch_chat_create(app_server_protocol::ChatCreateRequest {
            title: "new chat".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: None,
            initial_user_message: None,
            requested_model: None,
            initial_turn: None,
        })
        .expect("dispatch chat.create");
    let _ = drain_outbound(&mut client);
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: create_request_id,
            result: Ok(CommandSuccess::ChatCreated(ChatCreatedResponse {
                session_id: ChatSessionId("new-chat".into()),
                agent_id: "agent-new-chat".into(),
                title: "new chat".into(),
                initial_turn: None,
            })),
        }))
        .unwrap();

    let snapshot = client.snapshot();
    assert_eq!(
        snapshot.current_chat_session,
        Some(ChatSessionId("new-chat".into()))
    );
    assert!(
        snapshot.current_chat_history.is_empty(),
        "new chat should not render the previous session history",
    );
}

#[test]
fn stale_chat_history_response_does_not_replace_newer_selection() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let main_request_id = client
        .dispatch_chat_history(app_server_protocol::ChatHistoryRequest {
            session_id: ChatSessionId("main".into()),
            limit: Some(50),
        })
        .expect("dispatch main chat.history");
    let other_request_id = client
        .dispatch_chat_select(
            ChatSessionId("other".into()),
            app_server_protocol::ChatHistoryRequest {
                session_id: ChatSessionId("other".into()),
                limit: Some(50),
            },
        )
        .expect("dispatch other chat.select");
    let _ = drain_outbound(&mut client);

    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: other_request_id,
            result: Ok(CommandSuccess::ChatHistory(ChatHistoryResponse {
                session_id: ChatSessionId("other".into()),
                messages: vec![chat_message("msg-other", ChatRole::User, "newer chat")],
            })),
        }))
        .unwrap();
    client
        .receive_inbound(&encode_response(&ServerResponseEnvelope {
            request_id: main_request_id,
            result: Ok(CommandSuccess::ChatHistory(ChatHistoryResponse {
                session_id: ChatSessionId("main".into()),
                messages: vec![chat_message("msg-main", ChatRole::User, "stale chat")],
            })),
        }))
        .unwrap();

    let snapshot = client.snapshot();
    assert_eq!(
        snapshot.current_chat_session,
        Some(ChatSessionId("other".into()))
    );
    assert_eq!(snapshot.current_chat_history.len(), 1);
    assert_eq!(snapshot.current_chat_history[0].content, "newer chat");
}

#[test]
fn cancel_during_reconnect_is_deferred_until_handshake_replay() {
    let mut client = ManagedClient::new(FakeTransport::default());
    open_client_with_handshake(&mut client);

    let req_a = client.dispatch_workspace_current().unwrap();
    let _ = drain_outbound(&mut client);

    client.mark_transport_closed(TransportCloseReason {
        code: 1006,
        reason: "disconnect".into(),
        was_clean: false,
    });
    assert!(
        client.next_outbound().is_none(),
        "mark_transport_closed clears outbound"
    );
    let _ = client.drain_events();

    assert_eq!(
        client.snapshot().transport_status,
        TransportStatus::Reconnecting
    );
    let cancel_id = client.cancel(req_a).expect("cancel during reconnect");
    assert_ne!(cancel_id, req_a);

    assert!(
        client.next_outbound().is_none(),
        "cancel during reconnect must not enqueue outbound immediately"
    );

    let events_after_cancel = client.drain_events();
    assert!(events_after_cancel.iter().any(|event| matches!(
        event,
        ClientEvent::RequestFailed { request_id: rid, error: ClientError::Cancelled } if *rid == req_a
    )));

    client.begin_handshake(handshake_request()).unwrap();
    let queued = drain_outbound(&mut client);
    assert_eq!(
        queued.len(),
        3,
        "expected reconnect + A replay + cancel replay"
    );

    let first: ClientEnvelope = decode_client_frame(&queued[0]).expect("reconnect decodes");
    assert!(matches!(first, ClientEnvelope::Reconnect(_)));

    match decode_client_frame(&queued[1]).expect("A replay decodes") {
        ClientEnvelope::Request(ClientRequestEnvelope {
            request_id,
            command,
        }) => {
            assert_eq!(request_id, req_a);
            assert!(matches!(command, ClientCommand::WorkspaceCurrent));
        }
        other => panic!("expected request envelope for A, got {other:?}"),
    }
    match decode_client_frame(&queued[2]).expect("cancel replay decodes") {
        ClientEnvelope::Request(ClientRequestEnvelope {
            request_id,
            command,
        }) => {
            assert_eq!(request_id, cancel_id);
            match command {
                ClientCommand::Cancel(cancel) => assert_eq!(cancel.request_id, req_a),
                other => panic!("expected Cancel command, got {other:?}"),
            }
        }
        other => panic!("expected request envelope for cancel, got {other:?}"),
    }
}

fn chat_list_response() -> ChatListResponse {
    ChatListResponse {
        active_chat_id: Some(ChatSessionId("main".into())),
        sessions: vec![ChatSessionSummary {
            session_id: ChatSessionId("main".into()),
            agent_id: "agent-main".into(),
            title: "main".into(),
            archived: false,
            message_count: 1,
            related_files: Vec::new(),
            bound_model: None,
        }],
    }
}

fn sample_selection_request() -> SelectionUpdateRequest {
    SelectionUpdateRequest {
        selections: vec![SelectionRef {
            kind: SelectionKind::Face,
            ref_text: "@face[top_lid:f_0]".into(),
            owner_ref_text: Some("@part[top_lid]".into()),
            owner_object_kind: Some(app_server_protocol::CadQueryObjectKind::Part),
            instance_path: None,
            candidate_feature_ref: Some("@feature[top_lid.top_surface]".into()),
            build_id: Some("sha256:build".into()),
            result_id: Some("cq_1".into()),
            ambiguous: false,
        }],
        active_index: Some(0),
    }
}

fn push_agent_token_and_done(client: &mut ManagedClient<FakeTransport>) {
    let session_id = ChatSessionId("main".into());
    client
        .receive_inbound(&encode_push(&ServerPushEnvelope {
            event: ServerPushEvent::AgentToken(AgentTokenEvent {
                session_id: session_id.clone(),
                run_id: "agent-1".into(),
                text: "received".into(),
            }),
        }))
        .unwrap();
    client
        .receive_inbound(&encode_push(&ServerPushEnvelope {
            event: ServerPushEvent::AgentDone(AgentDoneEvent {
                session_id,
                run_id: "agent-1".into(),
                cancelled: false,
            }),
        }))
        .unwrap();
}

fn chat_message(id: &str, role: ChatRole, content: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: id.into(),
        ts_ms: 1,
        role,
        content: content.into(),
        related_files: Vec::new(),
        tool_call_id: None,
        tool_calls: Vec::new(),
        tool_result: None,
        mesh_result: None,
        search_sources: Vec::new(),
        run_id: None,
        agent_id: None,
        turn_id: None,
    }
}

fn cadquery_ready(result_id: &str) -> CadQueryResultReady {
    CadQueryResultReady {
        result_id: result_id.into(),
        build_id: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        part_count: 1,
        face_count: 2,
        edge_count: 3,
        vertex_count: 4,
        artifact_relation: Some(CadQueryArtifactRelation {
            source_path: "parts/top_lid.py".into(),
            exports: vec![CadQueryArtifactExport {
                name: "step".into(),
                path: "outputs/top_lid.step".into(),
                hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .into(),
            }],
        }),
    }
}

fn assert_request_id(bytes: &[u8], expected: RequestId) {
    match decode_client_frame(bytes).expect("request decodes") {
        ClientEnvelope::Request(request) => assert_eq!(request.request_id, expected),
        other => panic!("expected request envelope, got {other:?}"),
    }
}

#[test]
fn watch_subscribe_respects_handshake_timeout() {
    let mut timeouts = ClientTimeouts::default();
    timeouts.watch = Some(1000);
    let mut client = ManagedClient::with_timeouts(FakeTransport::default(), timeouts);
    open_client_with_handshake(&mut client);

    client.tick(0);
    let watch_request_id = client
        .subscribe_directory_watch(WatchParams {
            request: WatchSubscribeRequest { directory: None },
            throttle_ms: Some(100),
        })
        .expect("subscribe during open");
    let _ = drain_outbound(&mut client);

    client.tick(1001);
    let events = client.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        ClientEvent::RequestTimedOut { request_id: rid } if *rid == watch_request_id
    )));

    let snapshot = client.snapshot();
    assert_eq!(
        snapshot.watch_lifecycle.active_subscriptions, 0,
        "timed-out watch subscribe should not leave a registry entry"
    );

    let push_bytes = encode_push(&ServerPushEnvelope {
        event: ServerPushEvent::WatchChanged(WatchChangedEvent {
            subscription_id: sample_subscription_id(),
            changed_paths: vec![path_handle("late.scad")],
        }),
    });
    client.receive_inbound(&push_bytes).unwrap();
    let late_events = client.drain_events();
    assert!(
        !late_events
            .iter()
            .any(|event| matches!(event, ClientEvent::WatchEvent { .. })),
        "late push for timed-out subscription must not produce watch events"
    );
}
