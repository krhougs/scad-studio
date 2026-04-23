use crate::{
    ClientEnvelope, ClientTransport, ServerEnvelope, SubscriptionId, TransportError,
    TransportErrorFrame, decode_server_envelope_text, encode_client_envelope_text,
};
use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, ClientCommand, ClientRequestEnvelope, RequestId,
    WatchSubscribeRequest, WatchUnsubscribeRequest,
};
use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{CloseEvent, ErrorEvent, Event, MessageEvent, WebSocket};

#[derive(Default)]
struct WebSocketSharedState {
    queue: VecDeque<ServerEnvelope>,
    is_open: bool,
    is_closed: bool,
}

pub struct WebSocketClientTransport {
    socket: WebSocket,
    state: Rc<RefCell<WebSocketSharedState>>,
    muted_subscriptions: HashSet<SubscriptionId>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(ErrorEvent)>,
    _on_close: Closure<dyn FnMut(CloseEvent)>,
}

impl WebSocketClientTransport {
    pub fn connect(url: &str) -> Result<Self, wasm_bindgen::JsValue> {
        let socket = WebSocket::new(url)?;
        let state = Rc::new(RefCell::new(WebSocketSharedState::default()));

        let on_open = {
            let state = Rc::clone(&state);
            Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
                let mut state = state.borrow_mut();
                state.is_open = true;
                state.is_closed = false;
            }))
        };
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        let on_message = {
            let state = Rc::clone(&state);
            Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |event: MessageEvent| {
                let message = match event.data().as_string() {
                    Some(text) => decode_server_envelope_text(&text).unwrap_or_else(|error| {
                        ServerEnvelope::TransportError(TransportErrorFrame {
                            message: format!("invalid websocket payload: {error}"),
                        })
                    }),
                    None => ServerEnvelope::TransportError(TransportErrorFrame {
                        message: "websocket message was not text".into(),
                    }),
                };
                state.borrow_mut().queue.push_back(message);
            }))
        };
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let on_error = {
            let state = Rc::clone(&state);
            Closure::<dyn FnMut(ErrorEvent)>::wrap(Box::new(move |event: ErrorEvent| {
                state
                    .borrow_mut()
                    .queue
                    .push_back(ServerEnvelope::TransportError(TransportErrorFrame {
                        message: event.message(),
                    }));
            }))
        };
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        let on_close = {
            let state = Rc::clone(&state);
            Closure::<dyn FnMut(CloseEvent)>::wrap(Box::new(move |_| {
                let mut state = state.borrow_mut();
                state.is_open = false;
                if !state.is_closed {
                    state.is_closed = true;
                    state.queue.push_back(ServerEnvelope::Closed);
                }
            }))
        };
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        Ok(Self {
            socket,
            state,
            muted_subscriptions: HashSet::new(),
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
            _on_close: on_close,
        })
    }

    pub fn is_open(&self) -> bool {
        self.state.borrow().is_open
    }

    fn send_message(&self, message: ClientEnvelope) -> Result<(), TransportError> {
        let state = self.state.borrow();
        if state.is_closed {
            return Err(TransportError::Closed);
        }
        if !state.is_open {
            return Err(TransportError::NotReady);
        }
        let payload =
            encode_client_envelope_text(&message).map_err(|_| TransportError::QueuePoisoned)?;
        self.socket
            .send_with_str(&payload)
            .map_err(|_| TransportError::Closed)
    }
}

impl ClientTransport for WebSocketClientTransport {
    fn handshake(&mut self, request: CapabilityHandshakeRequest) -> Result<(), TransportError> {
        self.send_message(ClientEnvelope::Handshake(request))
    }

    fn reconnect(&mut self, request: CapabilityHandshakeRequest) -> Result<(), TransportError> {
        self.send_message(ClientEnvelope::Reconnect(request))
    }

    fn request(&mut self, request: ClientRequestEnvelope) -> Result<(), TransportError> {
        self.send_message(ClientEnvelope::Request(request))
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
        let mut state = self.state.borrow_mut();
        while let Some(message) = state.queue.pop_front() {
            if let ServerEnvelope::Push(app_server_protocol::ServerPushEnvelope {
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
        if self.state.borrow().is_closed {
            return Ok(());
        }
        let _ = self.socket.close();
        let mut state = self.state.borrow_mut();
        state.is_open = false;
        state.is_closed = true;
        state.queue.push_back(ServerEnvelope::Closed);
        Ok(())
    }
}
