#![cfg(target_arch = "wasm32")]

use app_server_transport::{ClientTransport, ServerEnvelope, WebSocketClientTransport};
use studio_common::{AppServerTransportError, AppServerTransportEvent, AppServerTransportPort};

pub struct WebSocketAppServerTransportPort {
    inner: WebSocketClientTransport,
}

impl WebSocketAppServerTransportPort {
    pub fn connect(url: &str) -> Result<Self, wasm_bindgen::JsValue> {
        Ok(Self {
            inner: WebSocketClientTransport::connect(url)?,
        })
    }

    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

impl AppServerTransportPort for WebSocketAppServerTransportPort {
    fn handshake(
        &mut self,
        request: app_server_protocol::CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        ClientTransport::handshake(&mut self.inner, request).map_err(map_transport_error)
    }

    fn reconnect(
        &mut self,
        request: app_server_protocol::CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        ClientTransport::reconnect(&mut self.inner, request).map_err(map_transport_error)
    }

    fn request(
        &mut self,
        request: app_server_protocol::ClientRequestEnvelope,
    ) -> Result<(), AppServerTransportError> {
        ClientTransport::request(&mut self.inner, request).map_err(map_transport_error)
    }

    fn subscribe(
        &mut self,
        request_id: app_server_protocol::RequestId,
        request: app_server_protocol::WatchSubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        ClientTransport::subscribe(&mut self.inner, request_id, request)
            .map_err(map_transport_error)
    }

    fn unsubscribe(
        &mut self,
        request_id: app_server_protocol::RequestId,
        request: app_server_protocol::WatchUnsubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        ClientTransport::unsubscribe(&mut self.inner, request_id, request)
            .map_err(map_transport_error)
    }

    fn cancel(
        &mut self,
        request_id: app_server_protocol::RequestId,
        target_request_id: app_server_protocol::RequestId,
    ) -> Result<(), AppServerTransportError> {
        ClientTransport::cancel(&mut self.inner, request_id, target_request_id)
            .map_err(map_transport_error)
    }

    fn poll_server_event(
        &mut self,
    ) -> Result<Option<AppServerTransportEvent>, AppServerTransportError> {
        ClientTransport::next_server_message(&mut self.inner)
            .map_err(map_transport_error)
            .map(|message| {
                message.map(|message| match message {
                    ServerEnvelope::HandshakeAck(response) => {
                        AppServerTransportEvent::HandshakeAck(response)
                    }
                    ServerEnvelope::Response(response) => {
                        AppServerTransportEvent::Response(response)
                    }
                    ServerEnvelope::Push(push) => AppServerTransportEvent::Push(push),
                    ServerEnvelope::TransportError(error) => {
                        AppServerTransportEvent::TransportError(error.message)
                    }
                    ServerEnvelope::Closed => AppServerTransportEvent::Closed,
                })
            })
    }

    fn close(&mut self) -> Result<(), AppServerTransportError> {
        ClientTransport::close(&mut self.inner).map_err(map_transport_error)
    }
}

fn map_transport_error(error: app_server_transport::TransportError) -> AppServerTransportError {
    AppServerTransportError::new(error.to_string())
}
