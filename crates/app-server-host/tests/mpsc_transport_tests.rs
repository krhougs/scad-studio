use app_server_host::{
    AbortDecision, ClientTransport, GUI_SHUTDOWN_TIMEOUT, InProcessHost, JoinThenAbort,
    MpscTransportAdapter, evaluate_shutdown, spawn_in_process_mpsc_host,
};
use app_server_protocol::{
    CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCapabilities, ClientCommand,
    ClientPlatform, ClientRequestEnvelope, ProtocolError, ProtocolErrorCode, ProtocolVersionRange,
    RequestId, ServerCapabilities, ServerPushEnvelope, ServerPushEvent, SessionToken,
    SubscriptionId, WatchChangedEvent, WatchSubscribeRequest, WatchSubscriptionAck,
    WatchUnsubscribeRequest, WorkspaceCurrentResponse, WorkspaceId, web_file_read_capability,
};
use app_server_transport::{ClientEnvelope, ServerEnvelope, TransportErrorFrame};
use std::path::PathBuf;

#[test]
fn mpsc_request_response_roundtrip() {
    let (mut transport, harness) = MpscTransportAdapter::pair();
    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(1),
            command: ClientCommand::WorkspaceCurrent,
        })
        .unwrap();
    assert!(matches!(
        harness.pop_client_message(),
        Some(ClientEnvelope::Request(_))
    ));

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
    assert!(matches!(
        transport.next_server_message().unwrap(),
        Some(ServerEnvelope::Response(_))
    ));
}

#[test]
fn mpsc_handshake_cancel_and_close_flow() {
    let (mut transport, harness) = MpscTransportAdapter::pair();
    transport.handshake(handshake_request()).unwrap();
    assert!(matches!(
        harness.pop_client_message(),
        Some(ClientEnvelope::Handshake(_))
    ));

    transport.cancel(RequestId(2), RequestId(1)).unwrap();
    assert!(matches!(
        harness.pop_client_message(),
        Some(ClientEnvelope::Request(_))
    ));

    transport.close().unwrap();
    assert!(matches!(
        harness.pop_client_message(),
        Some(ClientEnvelope::Close)
    ));
}

#[test]
fn mpsc_subscription_and_transport_error_flow() {
    let (mut transport, harness) = MpscTransportAdapter::pair();
    transport
        .subscribe(RequestId(11), WatchSubscribeRequest { directory: None })
        .unwrap();
    assert!(matches!(
        harness.pop_client_message(),
        Some(ClientEnvelope::Request(_))
    ));

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
        transport.next_server_message().unwrap(),
        Some(ServerEnvelope::Response(_))
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
        transport.next_server_message().unwrap(),
        Some(ServerEnvelope::Push(_))
    ));

    transport
        .unsubscribe(
            RequestId(12),
            WatchUnsubscribeRequest {
                subscription_id: SubscriptionId("sub-1".into()),
            },
        )
        .unwrap();
    assert!(matches!(
        harness.pop_client_message(),
        Some(ClientEnvelope::Request(_))
    ));

    harness
        .push_server_message(ServerEnvelope::TransportError(TransportErrorFrame {
            message: "wire disconnected".into(),
        }))
        .unwrap();
    assert!(matches!(
        transport.next_server_message().unwrap(),
        Some(ServerEnvelope::TransportError(_))
    ));

    harness
        .inject_protocol_error(
            RequestId(8),
            ProtocolError::new(ProtocolErrorCode::Cancelled, "cancelled"),
        )
        .unwrap();
    assert!(matches!(
        transport.next_server_message().unwrap(),
        Some(ServerEnvelope::Response(_))
    ));
}

#[test]
fn in_process_host_rebinds_workspace_and_shutdown_defaults_to_abort() {
    let mut host = InProcessHost::new();
    host.rebind_workspace(PathBuf::from("/tmp/workspace"));
    assert_eq!(
        host.current_workspace(),
        Some(PathBuf::from("/tmp/workspace").as_path())
    );

    let strategy = JoinThenAbort::default();
    assert_eq!(strategy.timeout, GUI_SHUTDOWN_TIMEOUT);
    assert_eq!(evaluate_shutdown(true, &strategy), AbortDecision::CleanExit);
    assert_eq!(evaluate_shutdown(false, &strategy), AbortDecision::Abort);
}

#[test]
fn spawned_in_process_host_serves_handshake_and_workspace_current() {
    let workspace = std::env::temp_dir().join(format!("studio-host-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();

    let (mut host, mut transport) = spawn_in_process_mpsc_host().unwrap();
    host.rebind_workspace(workspace.clone());

    transport.handshake(handshake_request()).unwrap();
    let handshake = recv_server_message(&mut transport).expect("handshake ack");
    assert!(matches!(handshake, ServerEnvelope::HandshakeAck(_)));

    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(41),
            command: ClientCommand::WorkspaceCurrent,
        })
        .unwrap();
    let response = recv_server_message(&mut transport).expect("workspace current response");
    match response {
        ServerEnvelope::Response(envelope) => match envelope.result.unwrap() {
            app_server_protocol::CommandSuccess::WorkspaceCurrent(current) => {
                assert_eq!(
                    current.root_name,
                    workspace.file_name().unwrap().to_string_lossy()
                );
            }
            other => panic!("unexpected response: {other:?}"),
        },
        other => panic!("unexpected server message: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(workspace);
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

fn recv_server_message(transport: &mut dyn ClientTransport) -> Option<ServerEnvelope> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if let Some(message) = transport.next_server_message().unwrap() {
            return Some(message);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    None
}

#[allow(dead_code)]
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
        },
    }
}
