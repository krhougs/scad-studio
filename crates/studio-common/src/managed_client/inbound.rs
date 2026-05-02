use app_server_protocol::{
    CapabilityHandshakeResponse, CommandSuccess, ProtocolError, ProtocolErrorCode, RequestId,
    ServerPushEnvelope, ServerPushEvent, ServerResponseEnvelope, TransportErrorFrame,
    WatchChangedEvent, WatchErrorEvent, WatchSubscriptionAck,
};

use crate::AppServerTransportPort;

use super::ManagedClient;
use super::pending::{PendingKind, PendingRequestInfo};
use super::types::{
    ClientError, ClientEvent, PreviewErrorSummary, PreviewPhase, TransportCloseReason,
    TransportStatus, WatchEventPayload,
};

impl<T: AppServerTransportPort> ManagedClient<T> {
    pub(super) fn handle_handshake_ack(&mut self, ack: CapabilityHandshakeResponse) {
        let was_reconnecting = self.transport_status == TransportStatus::Reconnecting;
        self.transport_status = TransportStatus::Open;
        self.pending_handshake = None;
        self.llm_configured = ack.server_capabilities.llm_configured;
        self.agent_provider = ack.server_capabilities.agent_provider.clone();
        self.agent_model_registry = ack.server_capabilities.agent_model_registry.clone();
        self.events.push_back(ClientEvent::HandshakeAccepted {
            session_token: ack.session_token.clone(),
            server_capabilities: ack.server_capabilities.clone(),
            negotiated_version: ack.negotiated_version,
        });
        if was_reconnecting {
            self.events.push_back(ClientEvent::TransportOpen);
        }
    }

    pub(super) fn handle_response(&mut self, response: ServerResponseEnvelope) {
        if self.watches.contains_key(&response.request_id) {
            match response.result {
                Ok(CommandSuccess::WatchSubscribed(ack)) => {
                    self.register_watch_ack(response.request_id, ack);
                }
                Ok(other) => {
                    self.last_error = Some(ClientError::UnknownRequest {
                        request_id: response.request_id,
                    });
                    let _ = other;
                }
                Err(error) => {
                    let client_error = ClientError::ProtocolError {
                        code: protocol_error_code_label(error.code).into(),
                        message: error.message,
                    };
                    self.last_error = Some(client_error.clone());
                    self.events.push_back(ClientEvent::RequestFailed {
                        request_id: response.request_id,
                        error: client_error,
                    });
                }
            }
            return;
        }
        let Some(info) = self.pending.remove(&response.request_id) else {
            self.last_error = Some(ClientError::UnknownRequest {
                request_id: response.request_id,
            });
            return;
        };
        if info.cancelled {
            self.last_error = Some(ClientError::UnknownRequest {
                request_id: response.request_id,
            });
            return;
        }
        match response.result {
            Ok(success) => self.handle_success(response.request_id, info, success),
            Err(error) => self.handle_protocol_error(response.request_id, info, error),
        }
    }

    fn handle_success(
        &mut self,
        request_id: RequestId,
        info: PendingRequestInfo,
        success: CommandSuccess,
    ) {
        match &success {
            CommandSuccess::WorkspaceCurrent(response) => {
                self.workspace_current = Some(response.clone());
            }
            CommandSuccess::WorkspaceList(response) => {
                self.workspace_list = Some(response.clone());
            }
            CommandSuccess::PreviewReady(_) => {
                if self.is_latest_preview(request_id) {
                    if let Some(task) = self.preview_tasks.get_mut(&request_id) {
                        task.phase = PreviewPhase::Ready;
                    }
                    self.preview_error = None;
                } else {
                    self.preview_tasks.remove(&request_id);
                }
            }
            CommandSuccess::CadQueryResultReady(response) => {
                self.upsert_cadquery_result(response.clone());
            }
            CommandSuccess::CadQueryMesh(payload) => {
                self.upsert_cadquery_result(app_server_protocol::CadQueryResultReady {
                    result_id: payload.result_id.clone(),
                    build_id: payload.build_id.clone(),
                    part_count: payload.parts.len() as u32,
                    face_count: payload
                        .parts
                        .iter()
                        .map(|part| part.faces.len() as u32)
                        .sum(),
                    edge_count: payload
                        .parts
                        .iter()
                        .map(|part| part.edges.len() as u32)
                        .sum(),
                    vertex_count: payload
                        .parts
                        .iter()
                        .map(|part| part.vertices.len() as u32)
                        .sum(),
                    artifact_relation: payload.artifact_relation.clone(),
                });
            }
            CommandSuccess::ChatCreated(response) => {
                self.current_chat_session = Some(response.session_id.clone());
                self.current_chat_history.clear();
                self.latest_chat_history_request = None;
                self.pending_chat_session = None;
            }
            CommandSuccess::ChatList(response) => {
                self.chat_sessions = response.sessions.clone();
                if self.pending_chat_session.is_none() {
                    self.current_chat_session = response.active_chat_id.clone();
                }
            }
            CommandSuccess::ChatHistory(response) => {
                if self.pending_chat_session.as_ref() == Some(&response.session_id) {
                    self.current_chat_session = Some(response.session_id.clone());
                    self.pending_chat_session = None;
                }
                if self.latest_chat_history_request == Some(request_id) {
                    let mesh_results = response
                        .messages
                        .iter()
                        .filter_map(|message| message.mesh_result.clone())
                        .collect::<Vec<_>>();
                    self.current_chat_history = response.messages.clone();
                    for result in mesh_results {
                        self.upsert_cadquery_result(result);
                    }
                    self.latest_chat_history_request = None;
                }
            }
            CommandSuccess::ChatAck(response) => {
                self.current_chat_session = Some(response.session_id.clone());
            }
            CommandSuccess::ChatArchived(response) => {
                self.chat_sessions
                    .retain(|session| session.session_id != response.session_id);
                if self.current_chat_session.as_ref() == Some(&response.session_id) {
                    self.current_chat_session = None;
                    self.current_chat_history.clear();
                }
            }
            CommandSuccess::AgentStarted(response) => {
                self.agent_run = Some(response.clone());
            }
            CommandSuccess::AgentModelRegistry(response) => {
                self.agent_model_registry = Some(response.clone());
                self.agent_provider = agent_provider_capability(response);
                self.llm_configured = !response.providers.is_empty();
            }
            CommandSuccess::AgentPlanConfirmed(response) => {
                self.agent_run = Some(response.clone());
            }
            CommandSuccess::AgentCancelled(_) | CommandSuccess::AgentPlanRejected => {}
            CommandSuccess::SelectionUpdated(_) => {
                if let PendingKind::SelectionUpdate { snapshot } = &info.kind {
                    self.current_selection = snapshot.clone();
                }
            }
            _ => {}
        }
        let _ = info;
        self.events.push_back(ClientEvent::RequestSucceeded {
            request_id,
            payload: success,
        });
    }

    fn handle_protocol_error(
        &mut self,
        request_id: RequestId,
        info: PendingRequestInfo,
        error: ProtocolError,
    ) {
        let code = protocol_error_code_label(error.code);
        let client_error = ClientError::ProtocolError {
            code: code.into(),
            message: error.message,
        };
        if let PendingKind::Preview { .. } = info.kind {
            if self.is_latest_preview(request_id) {
                if let Some(task) = self.preview_tasks.get_mut(&request_id) {
                    task.phase = PreviewPhase::Error;
                }
                self.preview_error = Some(match &client_error {
                    ClientError::ProtocolError { code, message } => PreviewErrorSummary {
                        code: code.clone(),
                        message: message.clone(),
                    },
                    _ => PreviewErrorSummary {
                        code: "unknown".into(),
                        message: String::new(),
                    },
                });
            } else {
                self.preview_tasks.remove(&request_id);
                self.last_error = Some(client_error.clone());
                self.events.push_back(ClientEvent::RequestFailed {
                    request_id,
                    error: client_error,
                });
                return;
            }
        }
        self.last_error = Some(client_error.clone());
        self.events.push_back(ClientEvent::RequestFailed {
            request_id,
            error: client_error,
        });
    }

    pub(super) fn is_latest_preview(&self, request_id: RequestId) -> bool {
        self.preview_tasks
            .keys()
            .max_by_key(|id| id.0)
            .map(|latest| *latest == request_id)
            .unwrap_or(false)
    }

    fn register_watch_ack(&mut self, request_id: RequestId, ack: WatchSubscriptionAck) {
        let is_resubscribe = self
            .watches
            .get(&request_id)
            .map(|entry| entry.awaiting_resubscribe)
            .unwrap_or(false);
        if let Some(entry) = self.watches.get_mut(&request_id) {
            entry.subscription_id = Some(ack.subscription_id);
            entry.awaiting_resubscribe = false;
        }
        self.pending.remove(&request_id);
        if is_resubscribe {
            self.watch_resubscribe_count = self.watch_resubscribe_count.saturating_add(1);
            self.events
                .push_back(ClientEvent::WatchResubscribed { request_id });
        }
    }

    pub(super) fn handle_transport_error(&mut self, frame: TransportErrorFrame) {
        let reason = TransportCloseReason {
            code: 0,
            reason: frame.message.clone(),
            was_clean: false,
        };
        self.last_error = Some(ClientError::ProtocolError {
            code: "transport_error".into(),
            message: frame.message,
        });
        self.mark_transport_closed(reason);
    }

    pub(super) fn handle_transport_closed(&mut self) {
        let reason = TransportCloseReason {
            code: 0,
            reason: "server closed".into(),
            was_clean: true,
        };
        self.mark_transport_closed(reason);
    }

    pub(super) fn handle_push(&mut self, push: ServerPushEnvelope) {
        match push.event {
            ServerPushEvent::WatchChanged(event) => self.handle_watch_changed(event),
            ServerPushEvent::WatchError(event) => self.handle_watch_error(event),
            ServerPushEvent::ChatListChanged(response) => {
                self.chat_sessions = response.sessions.clone();
                self.current_chat_session = response.active_chat_id.clone();
                self.events.push_back(ClientEvent::SnapshotChanged);
            }
            event => self.handle_agent_event(event),
        }
    }

    fn handle_agent_event(&mut self, event: ServerPushEvent) {
        if let ServerPushEvent::AgentMeshReady(event) = &event {
            self.upsert_cadquery_result(event.result.clone());
        }
        if let ServerPushEvent::AgentDone(event) = &event {
            if self
                .agent_run
                .as_ref()
                .is_some_and(|run| run.run_id == event.run_id)
            {
                self.agent_run = None;
            }
        }
        self.agent_events.push(event.clone());
        self.events
            .push_back(ClientEvent::AgentEvent { payload: event });
    }

    fn upsert_cadquery_result(&mut self, ready: app_server_protocol::CadQueryResultReady) {
        self.cadquery_results
            .retain(|existing| existing.result_id != ready.result_id);
        self.cadquery_results.push(ready);
    }

    fn handle_watch_changed(&mut self, event: WatchChangedEvent) {
        let now_ms = self.last_tick_ms;
        let Some((_request_id, entry)) = self
            .watches
            .iter_mut()
            .find(|(_, entry)| entry.subscription_id.as_ref() == Some(&event.subscription_id))
        else {
            return;
        };
        entry.accumulator.ingest(now_ms, event.changed_paths);
    }

    fn handle_watch_error(&mut self, event: WatchErrorEvent) {
        let Some((&request_id, _)) = self
            .watches
            .iter()
            .find(|(_, entry)| entry.subscription_id.as_ref() == Some(&event.subscription_id))
        else {
            return;
        };
        let payload = WatchEventPayload::Error {
            subscription_id: event.subscription_id,
            message: event.message,
        };
        self.watch_last_event_at_ms = Some(self.last_tick_ms);
        self.events.push_back(ClientEvent::WatchEvent {
            request_id,
            payload,
        });
    }
}

fn agent_provider_capability(
    registry: &app_server_protocol::AgentModelRegistryResponse,
) -> Option<app_server_protocol::AgentProviderCapabilities> {
    let provider = registry
        .providers
        .iter()
        .find(|provider| provider.id == registry.active_provider_id)?;
    let model = provider
        .models
        .iter()
        .find(|model| model.id == registry.active_model_id)?;
    Some(app_server_protocol::AgentProviderCapabilities {
        provider: provider.kind.clone(),
        model: Some(model.id.clone()),
        native_web_search_enabled: model.native_web_search_applied,
        search_sources_supported: model.search_sources_supported,
    })
}

fn protocol_error_code_label(code: ProtocolErrorCode) -> &'static str {
    match code {
        ProtocolErrorCode::InvalidCommand => "invalid_command",
        ProtocolErrorCode::InvalidPathHandle => "invalid_path_handle",
        ProtocolErrorCode::UnsupportedFileTypeForClient => "unsupported_file_type_for_client",
        ProtocolErrorCode::StalePathHandle => "stale_path_handle",
        ProtocolErrorCode::Cancelled => "cancelled",
        ProtocolErrorCode::SessionExpired => "session_expired",
        ProtocolErrorCode::UnsupportedProtocolVersion => "unsupported_protocol_version",
        ProtocolErrorCode::NotFound => "not_found",
        ProtocolErrorCode::Internal => "internal",
        ProtocolErrorCode::InvalidWireFrame => "invalid_wire_frame",
        ProtocolErrorCode::UnsupportedWireVersion => "unsupported_wire_version",
        ProtocolErrorCode::InvalidNumericValue => "invalid_numeric_value",
        ProtocolErrorCode::InvalidHostLocalPath => "invalid_host_local_path",
        ProtocolErrorCode::AgentBusy => "agent_busy",
    }
}
