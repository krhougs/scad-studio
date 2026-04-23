use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, ClientCommand, ClientRequestEnvelope, ProtocolError,
    RequestId, ServerPushEnvelope, ServerResponseEnvelope, SubscriptionId, WatchSubscribeRequest,
    WatchUnsubscribeRequest,
};
use app_server_transport::{ClientEnvelope, ClientTransport, ServerEnvelope, TransportErrorFrame};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub struct MpscTransportAdapter {
    client_tx: UnboundedSender<ClientEnvelope>,
    server_rx: Arc<Mutex<UnboundedReceiver<ServerEnvelope>>>,
    closed: Arc<AtomicBool>,
    muted_subscriptions: HashSet<SubscriptionId>,
}

#[derive(Clone)]
pub struct MpscTransportHarness {
    client_rx: Arc<Mutex<UnboundedReceiver<ClientEnvelope>>>,
    server_tx: UnboundedSender<ServerEnvelope>,
}

impl MpscTransportAdapter {
    pub fn pair() -> (Self, MpscTransportHarness) {
        let (client_tx, client_rx) = unbounded_channel();
        let (server_tx, server_rx) = unbounded_channel();
        (
            Self {
                client_tx,
                server_rx: Arc::new(Mutex::new(server_rx)),
                closed: Arc::new(AtomicBool::new(false)),
                muted_subscriptions: HashSet::new(),
            },
            MpscTransportHarness {
                client_rx: Arc::new(Mutex::new(client_rx)),
                server_tx,
            },
        )
    }

    fn send_client_message(&self, message: ClientEnvelope) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("transport is closed".into());
        }
        self.client_tx
            .send(message)
            .map_err(|_| "client channel is closed".into())
    }
}

impl ClientTransport for MpscTransportAdapter {
    fn handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), app_server_transport::TransportError> {
        self.send_client_message(ClientEnvelope::Handshake(request))
            .map_err(|_| app_server_transport::TransportError::Closed)
    }

    fn reconnect(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<(), app_server_transport::TransportError> {
        self.closed.store(false, Ordering::SeqCst);
        self.send_client_message(ClientEnvelope::Reconnect(request))
            .map_err(|_| app_server_transport::TransportError::Closed)
    }

    fn request(
        &mut self,
        request: ClientRequestEnvelope,
    ) -> Result<(), app_server_transport::TransportError> {
        self.send_client_message(ClientEnvelope::Request(request))
            .map_err(|_| app_server_transport::TransportError::Closed)
    }

    fn subscribe(
        &mut self,
        request_id: RequestId,
        request: WatchSubscribeRequest,
    ) -> Result<(), app_server_transport::TransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::WatchSubscribe(request),
        })
    }

    fn unsubscribe(
        &mut self,
        request_id: RequestId,
        request: WatchUnsubscribeRequest,
    ) -> Result<(), app_server_transport::TransportError> {
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
    ) -> Result<(), app_server_transport::TransportError> {
        self.request(ClientRequestEnvelope {
            request_id,
            command: ClientCommand::Cancel(CancelRequest {
                request_id: target_request_id,
            }),
        })
    }

    fn next_server_message(
        &mut self,
    ) -> Result<Option<ServerEnvelope>, app_server_transport::TransportError> {
        let mut rx = self
            .server_rx
            .lock()
            .map_err(|_| app_server_transport::TransportError::QueuePoisoned)?;
        loop {
            match rx.try_recv() {
                Ok(message) => {
                    if let ServerEnvelope::Push(ServerPushEnvelope {
                        event: app_server_protocol::ServerPushEvent::WatchChanged(event),
                    }) = &message
                        && self.muted_subscriptions.contains(&event.subscription_id)
                    {
                        continue;
                    }
                    return Ok(Some(message));
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => return Ok(None),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    return Ok(Some(ServerEnvelope::TransportError(TransportErrorFrame {
                        message: "server channel is closed".into(),
                    })));
                }
            }
        }
    }

    fn close(&mut self) -> Result<(), app_server_transport::TransportError> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.send_client_message(ClientEnvelope::Close)
            .map_err(|_| app_server_transport::TransportError::Closed)?;
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl MpscTransportHarness {
    pub fn pop_client_message(&self) -> Option<ClientEnvelope> {
        let mut rx = self.client_rx.lock().ok()?;
        rx.try_recv().ok()
    }

    pub fn push_server_message(&self, message: ServerEnvelope) -> Result<(), String> {
        self.server_tx
            .send(message)
            .map_err(|_| "server channel is closed".into())
    }

    pub fn inject_protocol_error(
        &self,
        request_id: RequestId,
        error: ProtocolError,
    ) -> Result<(), String> {
        self.push_server_message(ServerEnvelope::Response(ServerResponseEnvelope {
            request_id,
            result: Err(error),
        }))
    }
}
