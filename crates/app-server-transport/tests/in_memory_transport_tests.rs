use app_server_protocol::{
    CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCapabilities, ClientCommand,
    ClientPlatform, ClientRequestEnvelope, ProtocolError, ProtocolErrorCode, ProtocolVersionRange,
    RequestId, ServerCapabilities, ServerPushEnvelope, ServerPushEvent, SessionToken,
    SubscriptionId, WatchChangedEvent, WatchSubscribeRequest, WatchSubscriptionAck,
    WatchUnsubscribeRequest, WorkspaceCurrentResponse, WorkspaceId, web_file_read_capability,
};
use app_server_transport::{
    ClientEnvelope, ClientTransport, InMemoryTransport, ServerEnvelope, TransportErrorFrame,
};

#[test]
fn request_response_roundtrip() {
    let (mut transport, harness) = InMemoryTransport::pair();
    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(1),
            command: ClientCommand::WorkspaceCurrent,
        })
        .unwrap();

    let message = harness.pop_client_message().unwrap().unwrap();
    assert!(matches!(message, ClientEnvelope::Request(_)));

    harness
        .push_server_message(ServerEnvelope::Response(
            app_server_protocol::ServerResponseEnvelope {
                request_id: RequestId(1),
                result: Ok(app_server_protocol::CommandSuccess::WorkspaceCurrent(
                    WorkspaceCurrentResponse {
                        workspace_id: WorkspaceId::new("ws"),
                        root_name: "workspace".into(),
                    },
                )),
            },
        ))
        .unwrap();

    let message = transport.next_server_message().unwrap().unwrap();
    assert!(matches!(message, ServerEnvelope::Response(_)));
}

#[test]
fn handshake_and_reconnect_roundtrip() {
    let (mut transport, harness) = InMemoryTransport::pair();
    let hello = handshake_request();
    transport.handshake(hello.clone()).unwrap();
    assert!(matches!(
        harness.pop_client_message().unwrap().unwrap(),
        ClientEnvelope::Handshake(_)
    ));

    transport.reconnect(hello).unwrap();
    assert!(matches!(
        harness.pop_client_message().unwrap().unwrap(),
        ClientEnvelope::Reconnect(_)
    ));

    harness
        .push_server_message(ServerEnvelope::HandshakeAck(handshake_response()))
        .unwrap();
    assert!(matches!(
        transport.next_server_message().unwrap().unwrap(),
        ServerEnvelope::HandshakeAck(_)
    ));
}

#[test]
fn cancel_is_sent_as_request() {
    let (mut transport, harness) = InMemoryTransport::pair();
    transport.cancel(RequestId(7), RequestId(3)).unwrap();
    let message = harness.pop_client_message().unwrap().unwrap();
    match message {
        ClientEnvelope::Request(envelope) => match envelope.command {
            ClientCommand::Cancel(request) => assert_eq!(request.request_id, RequestId(3)),
            other => panic!("unexpected command: {other:?}"),
        },
        other => panic!("unexpected envelope: {other:?}"),
    }
}

#[test]
fn subscription_push_and_unsubscribe_flow() {
    let (mut transport, harness) = InMemoryTransport::pair();
    transport
        .subscribe(RequestId(11), WatchSubscribeRequest { directory: None })
        .unwrap();
    let message = harness.pop_client_message().unwrap().unwrap();
    assert!(matches!(message, ClientEnvelope::Request(_)));

    harness
        .push_server_message(ServerEnvelope::Response(
            app_server_protocol::ServerResponseEnvelope {
                request_id: RequestId(11),
                result: Ok(app_server_protocol::CommandSuccess::WatchSubscribed(
                    WatchSubscriptionAck {
                        subscription_id: SubscriptionId("sub-1".into()),
                    },
                )),
            },
        ))
        .unwrap();
    assert!(matches!(
        transport.next_server_message().unwrap().unwrap(),
        ServerEnvelope::Response(_)
    ));

    harness
        .push_server_message(ServerEnvelope::Push(ServerPushEnvelope {
            event: ServerPushEvent::WatchChanged(WatchChangedEvent {
                subscription_id: SubscriptionId("sub-1".into()),
                changed_paths: vec![],
            }),
        }))
        .unwrap();
    assert!(matches!(
        transport.next_server_message().unwrap().unwrap(),
        ServerEnvelope::Push(_)
    ));

    transport
        .unsubscribe(
            RequestId(12),
            WatchUnsubscribeRequest {
                subscription_id: SubscriptionId("sub-1".into()),
            },
        )
        .unwrap();
    let message = harness.pop_client_message().unwrap().unwrap();
    assert!(matches!(message, ClientEnvelope::Request(_)));

    harness
        .push_server_message(ServerEnvelope::Push(ServerPushEnvelope {
            event: ServerPushEvent::WatchChanged(WatchChangedEvent {
                subscription_id: SubscriptionId("sub-1".into()),
                changed_paths: vec![],
            }),
        }))
        .unwrap();
    assert!(transport.next_server_message().unwrap().is_none());
}

#[test]
fn close_stops_future_requests() {
    let (mut transport, harness) = InMemoryTransport::pair();
    transport.close().unwrap();
    assert!(matches!(
        harness.pop_client_message().unwrap().unwrap(),
        ClientEnvelope::Close
    ));
    let error = transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(1),
            command: ClientCommand::WorkspaceCurrent,
        })
        .expect_err("closed transport should reject new requests");
    assert_eq!(error.to_string(), "transport is closed");
}

#[test]
fn propagates_transport_error_frames() {
    let (mut transport, harness) = InMemoryTransport::pair();
    harness
        .push_server_message(ServerEnvelope::TransportError(TransportErrorFrame {
            message: "wire disconnected".into(),
        }))
        .unwrap();
    let message = transport.next_server_message().unwrap().unwrap();
    assert!(matches!(message, ServerEnvelope::TransportError(_)));

    harness
        .inject_protocol_error(
            RequestId(8),
            ProtocolError::new(ProtocolErrorCode::Cancelled, "cancelled"),
        )
        .unwrap();
    let message = transport.next_server_message().unwrap().unwrap();
    assert!(matches!(message, ServerEnvelope::Response(_)));
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-app".into(),
            platform: ClientPlatform::Desktop,
            protocol_version: ProtocolVersionRange::new(1, 2),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![
                app_server_protocol::PreviewRequestKind::GeometryArtifact,
            ],
        },
    }
}

fn handshake_response() -> CapabilityHandshakeResponse {
    CapabilityHandshakeResponse {
        negotiated_version: 2,
        session_token: SessionToken("session-1".into()),
        server_capabilities: ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(1, 2),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![
                app_server_protocol::PreviewRequestKind::GeometryArtifact,
            ],
            supports_session_reclaim: true,
            cadquery: true,
            agent: false,
            selection_sync: false,
        },
    }
}
