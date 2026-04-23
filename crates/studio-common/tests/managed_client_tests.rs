use std::collections::VecDeque;

use app_server_protocol::{
    CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCapabilities, ClientEnvelope,
    ClientPlatform, ClientRequestEnvelope, CommandSuccess, PathHandle, PreviewRequestKind,
    ProtocolVersionRange, RequestId, ServerCapabilities, ServerEnvelope, ServerPushEnvelope,
    ServerPushEvent, ServerResponseEnvelope, SessionToken, SubscriptionId, WatchChangedEvent,
    WatchSubscribeRequest, WatchSubscriptionAck, WorkspaceCurrentResponse, WorkspaceId,
    web_file_read_capability,
};
use serde_json::Value;
use studio_common::{
    AppServerTransportError, AppServerTransportEvent, AppServerTransportPort, ClientError,
    ClientEvent, ClientTimeouts, ManagedClient, TransportCloseReason, TransportStatus, WatchParams,
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
            protocol_version: ProtocolVersionRange::new(1, 1),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}

fn handshake_response() -> CapabilityHandshakeResponse {
    CapabilityHandshakeResponse {
        negotiated_version: 1,
        session_token: SessionToken("session-1".into()),
        server_capabilities: ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(1, 1),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            supports_session_reclaim: true,
        },
    }
}

fn encode_handshake_ack(ack: &CapabilityHandshakeResponse) -> Vec<u8> {
    serde_json::to_vec(&ServerEnvelope::HandshakeAck(ack.clone())).expect("handshake ack encodes")
}

fn encode_response(envelope: &ServerResponseEnvelope) -> Vec<u8> {
    serde_json::to_vec(&ServerEnvelope::Response(envelope.clone())).expect("response encodes")
}

fn encode_push(envelope: &ServerPushEnvelope) -> Vec<u8> {
    serde_json::to_vec(&ServerEnvelope::Push(envelope.clone())).expect("push encodes")
}

fn encode_server_envelope(envelope: &ServerEnvelope) -> Vec<u8> {
    serde_json::to_vec(envelope).expect("server envelope encodes")
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

#[test]
fn dispatch_before_handshake_returns_not_ready() {
    let mut client = ManagedClient::new(FakeTransport::default());
    let err = client
        .dispatch_workspace_current()
        .expect_err("should be not ready");
    assert!(matches!(err, ClientError::NotReady));
    assert_eq!(client.snapshot().transport_status, TransportStatus::Connecting);
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
    let frame: Value = serde_json::from_slice(&outbound[0]).unwrap();
    assert_eq!(frame["kind"], "request");
    assert_eq!(frame["payload"]["request_id"], 1);
    assert_eq!(frame["payload"]["command"]["command"], "workspace.current");

    let envelope: ClientEnvelope =
        serde_json::from_slice(&outbound[0]).expect("outbound bytes decode as ClientEnvelope");
    match envelope {
        ClientEnvelope::Request(request) => {
            assert_eq!(request.request_id, RequestId(1));
        }
        other => panic!("expected ClientEnvelope::Request, got {other:?}"),
    }
}

#[test]
fn handshake_wire_format_matches_protocol_envelope() {
    let mut client = ManagedClient::new(FakeTransport::default());
    client.begin_handshake(handshake_request()).unwrap();
    let outbound = drain_outbound(&mut client);
    assert_eq!(outbound.len(), 1);
    let frame: Value = serde_json::from_slice(&outbound[0]).unwrap();
    assert_eq!(frame["kind"], "handshake");

    let envelope: ClientEnvelope =
        serde_json::from_slice(&outbound[0]).expect("handshake bytes decode as ClientEnvelope");
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
    assert!(events_after_close
        .iter()
        .any(|event| matches!(event, ClientEvent::TransportClosed { .. })));

    client.begin_handshake(handshake_request()).unwrap();
    let queued = drain_outbound(&mut client);
    assert_eq!(queued.len(), 4, "reconnect + two pending + watch");

    let frame0: Value = serde_json::from_slice(&queued[0]).unwrap();
    assert_eq!(frame0["kind"], "reconnect");
    let reconnect_envelope: ClientEnvelope =
        serde_json::from_slice(&queued[0]).expect("reconnect decodes as ClientEnvelope");
    assert!(matches!(reconnect_envelope, ClientEnvelope::Reconnect(_)));
    let frame1: Value = serde_json::from_slice(&queued[1]).unwrap();
    assert_eq!(frame1["kind"], "request");
    assert_eq!(frame1["payload"]["request_id"], req1.0);
    let frame2: Value = serde_json::from_slice(&queued[2]).unwrap();
    assert_eq!(frame2["kind"], "request");
    assert_eq!(frame2["payload"]["request_id"], req2.0);
    let frame3: Value = serde_json::from_slice(&queued[3]).unwrap();
    assert_eq!(frame3["kind"], "request");
    assert_eq!(frame3["payload"]["request_id"], watch_id.0);
    assert_eq!(frame3["payload"]["command"]["command"], "watch.subscribe");

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
    assert!(events
        .iter()
        .any(|event| matches!(event, ClientEvent::TransportOpen)));
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
    assert_eq!(client.snapshot().transport_status, TransportStatus::Reconnecting);
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
    assert_eq!(json, "{\"type\":\"unknown_request\",\"payload\":{\"request_id\":42}}");

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
