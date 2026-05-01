use std::collections::VecDeque;

use app_server_protocol::{
    CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCapabilities, ClientCommand,
    ClientPlatform, ClientRequestEnvelope, CommandSuccess, PreviewRequestKind,
    ProtocolVersionRange, RequestId, ServerCapabilities, ServerResponseEnvelope, SessionToken,
    WorkspaceCurrentResponse, WorkspaceId, web_file_read_capability,
};
use studio_common::{
    AppServerClient, AppServerClientEvent, AppServerTransportError, AppServerTransportEvent,
    AppServerTransportPort,
};

#[derive(Default)]
struct FakeTransport {
    handshakes: Vec<CapabilityHandshakeRequest>,
    reconnects: Vec<CapabilityHandshakeRequest>,
    requests: Vec<ClientRequestEnvelope>,
    queued_events: VecDeque<Result<Option<AppServerTransportEvent>, AppServerTransportError>>,
    closes: usize,
}

impl FakeTransport {
    fn queue_event(&mut self, event: AppServerTransportEvent) {
        self.queued_events.push_back(Ok(Some(event)));
    }
}

impl AppServerTransportPort for FakeTransport {
    fn handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        self.handshakes.push(request);
        Ok(())
    }

    fn reconnect(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        self.reconnects.push(request);
        Ok(())
    }

    fn request(&mut self, request: ClientRequestEnvelope) -> Result<(), AppServerTransportError> {
        self.requests.push(request);
        Ok(())
    }

    fn subscribe(
        &mut self,
        request_id: RequestId,
        request: app_server_protocol::WatchSubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::WatchSubscribe(request),
        })
    }

    fn unsubscribe(
        &mut self,
        request_id: RequestId,
        request: app_server_protocol::WatchUnsubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::WatchUnsubscribe(request),
        })
    }

    fn cancel(
        &mut self,
        request_id: RequestId,
        target_request_id: RequestId,
    ) -> Result<(), AppServerTransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::Cancel(app_server_protocol::CancelRequest {
                request_id: target_request_id,
            }),
        })
    }

    fn poll_server_event(
        &mut self,
    ) -> Result<Option<AppServerTransportEvent>, AppServerTransportError> {
        match self.queued_events.pop_front() {
            Some(result) => result,
            None => Ok(None),
        }
    }

    fn close(&mut self) -> Result<(), AppServerTransportError> {
        self.closes += 1;
        Ok(())
    }
}

#[test]
fn handshake_poll_updates_session_state() {
    let mut transport = FakeTransport::default();
    transport.queue_event(AppServerTransportEvent::HandshakeAck(handshake_response()));
    let mut client = AppServerClient::new(transport);
    let request = handshake_request();

    client.begin_handshake(request.clone()).unwrap();

    assert_eq!(client.transport().handshakes.as_slice(), &[request]);

    let event = client.poll().unwrap().expect("handshake event");
    assert!(matches!(event, AppServerClientEvent::HandshakeAccepted(_)));
    assert_eq!(
        client.session_token().map(|token| token.0.as_str()),
        Some("session-1")
    );
    assert_eq!(client.negotiated_version(), Some(2));
    assert_eq!(
        client
            .server_capabilities()
            .expect("capabilities should be stored")
            .supported_preview_kinds,
        vec![PreviewRequestKind::GeometryArtifact]
    );
}

#[test]
fn request_and_poll_track_workspace_response() {
    let mut transport = FakeTransport::default();
    transport.queue_event(AppServerTransportEvent::Response(ServerResponseEnvelope {
        request_id: RequestId(1),
        result: Ok(CommandSuccess::WorkspaceCurrent(WorkspaceCurrentResponse {
            workspace_id: WorkspaceId::new("workspace"),
            root_name: "workspace".into(),
        })),
    }));
    let mut client = AppServerClient::new(transport);

    let request_id = client
        .send_command(ClientCommand::WorkspaceCurrent)
        .unwrap();

    assert_eq!(request_id, RequestId(1));
    assert_eq!(client.transport().requests.len(), 1);
    assert_eq!(client.transport().requests[0].request_id, RequestId(1));

    let event = client.poll().unwrap().expect("response event");
    match event {
        AppServerClientEvent::Response(response) => {
            assert_eq!(response.request_id, RequestId(1));
        }
        other => panic!("unexpected event: {other:?}"),
    }

    assert_eq!(
        client
            .current_workspace()
            .map(|workspace| workspace.root_name.as_str()),
        Some("workspace")
    );
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-common-tests".into(),
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
        },
    }
}
