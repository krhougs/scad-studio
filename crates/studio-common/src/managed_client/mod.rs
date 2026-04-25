mod dispatch;
mod envelopes;
mod inbound;
mod pending;
mod types;
mod watch;

use std::collections::{HashMap, VecDeque};

use app_server_protocol::{CapabilityHandshakeRequest, PathHandle, RequestId};

use crate::AppServerTransportPort;

use pending::{PendingKind, PendingRequestInfo};
pub use types::{
    ClientError, ClientEvent, ClientSnapshot, ClientTimeouts, PreviewErrorSummary, PreviewPhase,
    PreviewTaskState, TransportCloseReason, TransportStatus, WatchEventPayload,
    WatchLifecycleSummary, WatchParams,
};
use watch::WatchRegistryEntry;

use envelopes::{
    InboundFrame, build_cancel_envelope, decode_inbound, encode_handshake,
    encode_reconnect_envelope,
};
use watch::WatchAccumulator;

pub struct ManagedClient<T: AppServerTransportPort> {
    pub(super) transport: T,
    pub(super) timeouts: ClientTimeouts,
    pub(super) next_request_id: u64,
    pub(super) outbound: VecDeque<Vec<u8>>,
    pub(super) pending: HashMap<RequestId, PendingRequestInfo>,
    pub(super) watches: HashMap<RequestId, WatchRegistryEntry>,
    pub(super) events: VecDeque<ClientEvent>,
    pub(super) transport_status: TransportStatus,
    pub(super) last_tick_ms: u64,
    pub(super) last_error: Option<ClientError>,
    pub(super) workspace_current: Option<app_server_protocol::WorkspaceCurrentResponse>,
    pub(super) workspace_list: Option<app_server_protocol::WorkspaceListResponse>,
    pub(super) preview_tasks: HashMap<RequestId, PreviewTaskState>,
    pub(super) active_preview_target: Option<PathHandle>,
    pub(super) preview_error: Option<PreviewErrorSummary>,
    pub(super) watch_last_event_at_ms: Option<u64>,
    pub(super) watch_resubscribe_count: u32,
    pub(super) pending_handshake: Option<Vec<u8>>,
}

impl<T: AppServerTransportPort> ManagedClient<T> {
    pub fn new(transport: T) -> Self {
        Self::with_timeouts(transport, ClientTimeouts::default())
    }

    pub fn with_timeouts(transport: T, timeouts: ClientTimeouts) -> Self {
        Self {
            transport,
            timeouts,
            next_request_id: 1,
            outbound: VecDeque::new(),
            pending: HashMap::new(),
            watches: HashMap::new(),
            events: VecDeque::new(),
            transport_status: TransportStatus::Connecting,
            last_tick_ms: 0,
            last_error: None,
            workspace_current: None,
            workspace_list: None,
            preview_tasks: HashMap::new(),
            active_preview_target: None,
            preview_error: None,
            watch_last_event_at_ms: None,
            watch_resubscribe_count: 0,
            pending_handshake: None,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn begin_handshake(
        &mut self,
        params: CapabilityHandshakeRequest,
    ) -> Result<(), ClientError> {
        let envelope = if self.transport_status == TransportStatus::Reconnecting {
            encode_reconnect_envelope(&params)
        } else {
            encode_handshake(&params)
        };
        self.pending_handshake = Some(envelope.clone());
        self.outbound.push_back(envelope);
        if self.transport_status == TransportStatus::Reconnecting {
            self.enqueue_replay_after_handshake();
        } else {
            self.transport_status = TransportStatus::Connecting;
        }
        Ok(())
    }

    fn enqueue_replay_after_handshake(&mut self) {
        let mut pending_ids: Vec<RequestId> = self.pending.keys().copied().collect();
        pending_ids.sort_by_key(|id| id.0);
        for request_id in pending_ids {
            if let Some(info) = self.pending.get(&request_id) {
                if matches!(info.kind, PendingKind::WatchSubscribe) {
                    continue;
                }
                self.outbound.push_back(info.envelope_bytes.clone());
            }
        }
        let mut watch_ids: Vec<RequestId> = self.watches.keys().copied().collect();
        watch_ids.sort_by_key(|id| id.0);
        for request_id in watch_ids {
            if let Some(entry) = self.watches.get(&request_id) {
                self.outbound.push_back(entry.envelope_bytes.clone());
            }
        }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.last_tick_ms = now_ms;
        self.expire_timeouts(now_ms);
        self.flush_watch_windows(now_ms);
    }

    pub fn drain_events(&mut self) -> Vec<ClientEvent> {
        self.events.drain(..).collect()
    }

    pub fn snapshot(&self) -> ClientSnapshot {
        let active_subscriptions = self
            .watches
            .values()
            .filter(|entry| entry.subscription_id.is_some())
            .count() as u32;
        let current_directory_entries = self
            .workspace_list
            .as_ref()
            .map(|list| list.entries.clone())
            .unwrap_or_default();
        let mut preview_tasks: Vec<PreviewTaskState> =
            self.preview_tasks.values().cloned().collect();
        preview_tasks.sort_by_key(|task| task.request_id.0);
        ClientSnapshot {
            workspace_current: self.workspace_current.clone(),
            workspace_list: self.workspace_list.clone(),
            current_directory_entries,
            preview_tasks,
            active_preview_target: self.active_preview_target.clone(),
            preview_error: self.preview_error.clone(),
            watch_lifecycle: WatchLifecycleSummary {
                active_subscriptions,
                last_event_at_ms: self.watch_last_event_at_ms,
                resubscribe_count: self.watch_resubscribe_count,
            },
            last_error: self.last_error.clone(),
            transport_status: self.transport_status,
        }
    }

    pub fn mark_transport_closed(&mut self, reason: TransportCloseReason) {
        self.transport_status = TransportStatus::Reconnecting;
        self.outbound.clear();
        for entry in self.watches.values_mut() {
            entry.subscription_id = None;
            entry.awaiting_resubscribe = true;
            entry.accumulator = WatchAccumulator::default();
        }
        self.events
            .push_back(ClientEvent::TransportClosed { reason });
    }

    pub fn fail_preview_decode(&mut self, request_id: RequestId, message: String) {
        if !self.is_latest_preview(request_id) {
            self.preview_tasks.remove(&request_id);
            return;
        }
        let Some(task) = self.preview_tasks.get_mut(&request_id) else {
            return;
        };
        task.phase = PreviewPhase::Error;
        self.preview_error = Some(PreviewErrorSummary {
            code: "decode_error".into(),
            message,
        });
    }

    pub fn next_outbound(&mut self) -> Option<Vec<u8>> {
        self.outbound.pop_front()
    }

    pub fn receive_inbound(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        let frame = decode_inbound(bytes).map_err(|err| {
            self.last_error = Some(err.clone());
            err
        })?;
        match frame {
            InboundFrame::HandshakeAck(ack) => self.handle_handshake_ack(ack),
            InboundFrame::Response(response) => self.handle_response(response),
            InboundFrame::Push(push) => self.handle_push(push),
            InboundFrame::TransportError(frame) => self.handle_transport_error(frame),
            InboundFrame::Closed => self.handle_transport_closed(),
        }
        Ok(())
    }

    pub fn cancel(&mut self, target: RequestId) -> Result<RequestId, ClientError> {
        if self.transport_status == TransportStatus::Closed {
            return Err(ClientError::TransportClosed);
        }
        let cancel_id = self.allocate_request_id();
        let envelope_bytes = build_cancel_envelope(cancel_id, target);
        self.pending.insert(
            cancel_id,
            PendingRequestInfo {
                kind: PendingKind::Cancel { target },
                deadline_ms: self.deadline_for(self.timeouts.workspace_current),
                issued_at_ms: self.last_tick_ms,
                envelope_bytes: envelope_bytes.clone(),
                cancelled: false,
            },
        );
        if self.transport_status == TransportStatus::Open {
            self.outbound.push_back(envelope_bytes);
            if let Some(info) = self.pending.remove(&target) {
                self.finalize_pending_cancellation(target, &info);
                self.events.push_back(ClientEvent::RequestFailed {
                    request_id: target,
                    error: ClientError::Cancelled,
                });
            }
        } else {
            let target_kind = self.pending.get_mut(&target).map(|info| {
                info.cancelled = true;
                info.kind.clone()
            });
            if let Some(kind) = target_kind {
                self.finalize_cancellation_by_kind(target, &kind);
                self.events.push_back(ClientEvent::RequestFailed {
                    request_id: target,
                    error: ClientError::Cancelled,
                });
            }
        }
        Ok(cancel_id)
    }

    fn expire_timeouts(&mut self, now_ms: u64) {
        let expired: Vec<RequestId> = self
            .pending
            .iter()
            .filter_map(|(id, info)| match info.deadline_ms {
                Some(deadline) if now_ms >= deadline => Some(*id),
                _ => None,
            })
            .collect();
        for request_id in expired {
            if let Some(info) = self.pending.remove(&request_id) {
                match info.kind {
                    PendingKind::Preview { .. } => {
                        if let Some(task) = self.preview_tasks.get_mut(&request_id) {
                            task.phase = PreviewPhase::TimedOut;
                        }
                        self.preview_error = Some(PreviewErrorSummary {
                            code: "timeout".into(),
                            message: "preview timed out".into(),
                        });
                    }
                    PendingKind::WatchSubscribe => {
                        self.watches.remove(&request_id);
                    }
                    _ => {}
                }
                self.events
                    .push_back(ClientEvent::RequestTimedOut { request_id });
            }
        }
    }

    fn flush_watch_windows(&mut self, now_ms: u64) {
        let mut emissions: Vec<(RequestId, WatchEventPayload)> = Vec::new();
        for (request_id, entry) in self.watches.iter_mut() {
            let Some(subscription_id) = entry.subscription_id.clone() else {
                continue;
            };
            let Some(window_start) = entry.accumulator.window_start_ms() else {
                continue;
            };
            if entry.accumulator.is_empty() {
                continue;
            }
            if now_ms < window_start.saturating_add(entry.throttle_ms as u64) {
                continue;
            }
            if let Some(payload) = entry.accumulator.take_window(subscription_id, now_ms) {
                emissions.push((*request_id, payload));
            }
        }
        for (request_id, payload) in emissions {
            self.watch_last_event_at_ms = Some(now_ms);
            self.events.push_back(ClientEvent::WatchEvent {
                request_id,
                payload,
            });
        }
    }

    fn finalize_pending_cancellation(&mut self, target: RequestId, info: &PendingRequestInfo) {
        self.finalize_cancellation_by_kind(target, &info.kind);
    }

    fn finalize_cancellation_by_kind(&mut self, target: RequestId, kind: &PendingKind) {
        if let PendingKind::Preview { .. } = kind {
            if let Some(task) = self.preview_tasks.get_mut(&target) {
                task.phase = PreviewPhase::Cancelled;
            }
        }
    }
}
