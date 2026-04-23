#[cfg(target_arch = "wasm32")]
mod websocket_client;
mod websocket_wire;

use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, ClientCommand, ClientRequestEnvelope, ProtocolError,
    RequestId, ServerPushEnvelope, ServerResponseEnvelope, SubscriptionId, WatchSubscribeRequest,
    WatchUnsubscribeRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
pub use websocket_client::WebSocketClientTransport;
pub use websocket_wire::{
    decode_client_envelope_text, decode_server_envelope_text, encode_client_envelope_text,
    encode_server_envelope_text,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ClientEnvelope {
    Handshake(CapabilityHandshakeRequest),
    Reconnect(CapabilityHandshakeRequest),
    Request(ClientRequestEnvelope),
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportErrorFrame {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ServerEnvelope {
    HandshakeAck(app_server_protocol::CapabilityHandshakeResponse),
    Response(ServerResponseEnvelope),
    Push(ServerPushEnvelope),
    TransportError(TransportErrorFrame),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Closed,
    NotReady,
    QueuePoisoned,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("transport is closed"),
            Self::NotReady => f.write_str("transport is not ready"),
            Self::QueuePoisoned => f.write_str("transport queue poisoned"),
        }
    }
}

impl std::error::Error for TransportError {}

pub trait ClientTransport {
    fn handshake(&mut self, request: CapabilityHandshakeRequest) -> Result<(), TransportError>;
    fn reconnect(&mut self, request: CapabilityHandshakeRequest) -> Result<(), TransportError>;
    fn request(&mut self, request: ClientRequestEnvelope) -> Result<(), TransportError>;
    fn subscribe(
        &mut self,
        request_id: RequestId,
        request: WatchSubscribeRequest,
    ) -> Result<(), TransportError>;
    fn unsubscribe(
        &mut self,
        request_id: RequestId,
        request: WatchUnsubscribeRequest,
    ) -> Result<(), TransportError>;
    fn cancel(
        &mut self,
        request_id: RequestId,
        target_request_id: RequestId,
    ) -> Result<(), TransportError>;
    fn next_server_message(&mut self) -> Result<Option<ServerEnvelope>, TransportError>;
    fn close(&mut self) -> Result<(), TransportError>;
}

#[derive(Clone)]
struct SharedState {
    client_to_server: Arc<Mutex<VecDeque<ClientEnvelope>>>,
    server_to_client: Arc<Mutex<VecDeque<ServerEnvelope>>>,
    closed: Arc<AtomicBool>,
}

pub struct InMemoryTransport {
    shared: SharedState,
    muted_subscriptions: HashSet<SubscriptionId>,
}

pub struct InMemoryTransportHarness {
    shared: SharedState,
}

impl InMemoryTransport {
    pub fn pair() -> (Self, InMemoryTransportHarness) {
        let shared = SharedState {
            client_to_server: Arc::new(Mutex::new(VecDeque::new())),
            server_to_client: Arc::new(Mutex::new(VecDeque::new())),
            closed: Arc::new(AtomicBool::new(false)),
        };
        (
            Self {
                shared: shared.clone(),
                muted_subscriptions: HashSet::new(),
            },
            InMemoryTransportHarness { shared },
        )
    }

    fn send_client_message(&self, message: ClientEnvelope) -> Result<(), TransportError> {
        if self.shared.closed.load(Ordering::SeqCst) {
            return Err(TransportError::Closed);
        }
        let mut queue = self
            .shared
            .client_to_server
            .lock()
            .map_err(|_| TransportError::QueuePoisoned)?;
        queue.push_back(message);
        Ok(())
    }
}

impl ClientTransport for InMemoryTransport {
    fn handshake(&mut self, request: CapabilityHandshakeRequest) -> Result<(), TransportError> {
        self.send_client_message(ClientEnvelope::Handshake(request))
    }

    fn reconnect(&mut self, request: CapabilityHandshakeRequest) -> Result<(), TransportError> {
        self.shared.closed.store(false, Ordering::SeqCst);
        self.send_client_message(ClientEnvelope::Reconnect(request))
    }

    fn request(&mut self, request: ClientRequestEnvelope) -> Result<(), TransportError> {
        self.send_client_message(ClientEnvelope::Request(request))
    }

    fn subscribe(
        &mut self,
        request_id: RequestId,
        request: WatchSubscribeRequest,
    ) -> Result<(), TransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::WatchSubscribe(request),
        })
    }

    fn unsubscribe(
        &mut self,
        request_id: RequestId,
        request: WatchUnsubscribeRequest,
    ) -> Result<(), TransportError> {
        self.muted_subscriptions
            .insert(request.subscription_id.clone());
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::WatchUnsubscribe(request),
        })
    }

    fn cancel(
        &mut self,
        request_id: RequestId,
        target_request_id: RequestId,
    ) -> Result<(), TransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::Cancel(CancelRequest {
                request_id: target_request_id,
            }),
        })
    }

    fn next_server_message(&mut self) -> Result<Option<ServerEnvelope>, TransportError> {
        let mut queue = self
            .shared
            .server_to_client
            .lock()
            .map_err(|_| TransportError::QueuePoisoned)?;
        while let Some(message) = queue.pop_front() {
            if let ServerEnvelope::Push(ServerPushEnvelope {
                event: app_server_protocol::ServerPushEvent::WatchChanged(event),
            }) = &message
                && self.muted_subscriptions.contains(&event.subscription_id)
            {
                continue;
            }
            return Ok(Some(message));
        }
        Ok(None)
    }

    fn close(&mut self) -> Result<(), TransportError> {
        if self.shared.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.send_client_message(ClientEnvelope::Close)?;
        self.shared.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl InMemoryTransportHarness {
    pub fn pop_client_message(&self) -> Result<Option<ClientEnvelope>, TransportError> {
        let mut queue = self
            .shared
            .client_to_server
            .lock()
            .map_err(|_| TransportError::QueuePoisoned)?;
        Ok(queue.pop_front())
    }

    pub fn push_server_message(&self, message: ServerEnvelope) -> Result<(), TransportError> {
        let mut queue = self
            .shared
            .server_to_client
            .lock()
            .map_err(|_| TransportError::QueuePoisoned)?;
        queue.push_back(message);
        Ok(())
    }

    pub fn inject_protocol_error(
        &self,
        request_id: RequestId,
        error: ProtocolError,
    ) -> Result<(), TransportError> {
        self.push_server_message(ServerEnvelope::Response(ServerResponseEnvelope {
            request_id,
            result: Err(error),
        }))
    }
}
