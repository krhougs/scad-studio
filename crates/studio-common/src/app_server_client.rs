use app_server_protocol::{
    CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCommand, ClientRequestEnvelope,
    CommandSuccess, RequestId, ServerCapabilities, ServerPushEnvelope, ServerResponseEnvelope,
    SessionToken, WatchSubscribeRequest, WatchUnsubscribeRequest, WorkspaceCurrentResponse,
};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum AppServerTransportEvent {
    HandshakeAck(CapabilityHandshakeResponse),
    Response(ServerResponseEnvelope),
    Push(ServerPushEnvelope),
    TransportError(String),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServerTransportError {
    message: String,
}

impl AppServerTransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AppServerTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppServerTransportError {}

impl From<String> for AppServerTransportError {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

pub trait AppServerTransportPort {
    fn handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError>;
    fn reconnect(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError>;
    fn request(&mut self, request: ClientRequestEnvelope) -> Result<(), AppServerTransportError>;
    fn subscribe(
        &mut self,
        request_id: RequestId,
        request: WatchSubscribeRequest,
    ) -> Result<(), AppServerTransportError>;
    fn unsubscribe(
        &mut self,
        request_id: RequestId,
        request: WatchUnsubscribeRequest,
    ) -> Result<(), AppServerTransportError>;
    fn cancel(
        &mut self,
        request_id: RequestId,
        target_request_id: RequestId,
    ) -> Result<(), AppServerTransportError>;
    fn poll_server_event(
        &mut self,
    ) -> Result<Option<AppServerTransportEvent>, AppServerTransportError>;
    fn close(&mut self) -> Result<(), AppServerTransportError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppServerClientEvent {
    HandshakeAccepted(CapabilityHandshakeResponse),
    Response(ServerResponseEnvelope),
    Push(ServerPushEnvelope),
    TransportError(String),
    Closed,
}

pub struct AppServerClient<T> {
    transport: T,
    next_request_id: u64,
    handshake: Option<CapabilityHandshakeResponse>,
    current_workspace: Option<WorkspaceCurrentResponse>,
}

impl<T> AppServerClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_request_id: 1,
            handshake: None,
            current_workspace: None,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn session_token(&self) -> Option<&SessionToken> {
        self.handshake
            .as_ref()
            .map(|response| &response.session_token)
    }

    pub fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.handshake
            .as_ref()
            .map(|response| &response.server_capabilities)
    }

    pub fn negotiated_version(&self) -> Option<u16> {
        self.handshake
            .as_ref()
            .map(|response| response.negotiated_version)
    }

    pub fn current_workspace(&self) -> Option<&WorkspaceCurrentResponse> {
        self.current_workspace.as_ref()
    }
}

impl<T: AppServerTransportPort> AppServerClient<T> {
    pub fn begin_handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        self.transport.handshake(request)
    }

    pub fn reconnect(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        self.transport.reconnect(request)
    }

    pub fn send_command(
        &mut self,
        command: ClientCommand,
    ) -> Result<RequestId, AppServerTransportError> {
        let request_id = self.allocate_request_id();
        self.transport.request(ClientRequestEnvelope {
            request_id,
            command,
        })?;
        Ok(request_id)
    }

    pub fn subscribe(
        &mut self,
        request: WatchSubscribeRequest,
    ) -> Result<RequestId, AppServerTransportError> {
        let request_id = self.allocate_request_id();
        self.transport.subscribe(request_id, request)?;
        Ok(request_id)
    }

    pub fn unsubscribe(
        &mut self,
        request: WatchUnsubscribeRequest,
    ) -> Result<RequestId, AppServerTransportError> {
        let request_id = self.allocate_request_id();
        self.transport.unsubscribe(request_id, request)?;
        Ok(request_id)
    }

    pub fn cancel(
        &mut self,
        target_request_id: RequestId,
    ) -> Result<RequestId, AppServerTransportError> {
        let request_id = self.allocate_request_id();
        self.transport.cancel(request_id, target_request_id)?;
        Ok(request_id)
    }

    pub fn close(&mut self) -> Result<(), AppServerTransportError> {
        self.transport.close()
    }

    pub fn poll(&mut self) -> Result<Option<AppServerClientEvent>, AppServerTransportError> {
        let Some(event) = self.transport.poll_server_event()? else {
            return Ok(None);
        };
        Ok(Some(match event {
            AppServerTransportEvent::HandshakeAck(response) => {
                self.handshake = Some(response.clone());
                AppServerClientEvent::HandshakeAccepted(response)
            }
            AppServerTransportEvent::Response(response) => {
                self.capture_response(&response);
                AppServerClientEvent::Response(response)
            }
            AppServerTransportEvent::Push(push) => AppServerClientEvent::Push(push),
            AppServerTransportEvent::TransportError(message) => {
                AppServerClientEvent::TransportError(message)
            }
            AppServerTransportEvent::Closed => AppServerClientEvent::Closed,
        }))
    }

    fn allocate_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }

    fn capture_response(&mut self, response: &ServerResponseEnvelope) {
        let Ok(success) = &response.result else {
            return;
        };
        match success {
            CommandSuccess::WorkspaceCurrent(workspace) => {
                self.current_workspace = Some(workspace.clone());
            }
            CommandSuccess::SessionReclaimed(reclaimed) => {
                self.current_workspace = reclaimed.workspace.clone();
                if let Some(handshake) = self.handshake.as_mut() {
                    handshake.server_capabilities = reclaimed.reclaimed_capabilities.clone();
                }
            }
            _ => {}
        }
    }
}
