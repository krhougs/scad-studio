use app_server_protocol::{
    AgentCancelRequest, AgentInvokeRequest, AgentPlanConfirmRequest, AgentPlanRejectRequest,
    CadQueryExecuteRequest, CadQueryPreviewRequest, CadQueryResultGetRequest, ChatArchiveRequest,
    ChatCreateRequest, ChatHistoryRequest, ChatListRequest, ChatSendRequest, ChatSessionId,
    ClientCommand, ConfigSaveRequest, ExportRunRequest, FileReadRequest, FileWriteTextRequest,
    PreviewRequest, RequestId, SelectionUpdateRequest, SlicerListRequest, WorkspaceListRequest,
};

use crate::AppServerTransportPort;

use super::ManagedClient;
use super::envelopes::{build_request_envelope, build_watch_subscribe_envelope};
use super::pending::{PendingKind, PendingRequestInfo};
use super::types::{ClientError, PreviewPhase, PreviewTaskState, TransportStatus, WatchParams};
use super::watch::{DEFAULT_WATCH_THROTTLE_MS, WatchAccumulator, WatchRegistryEntry};

impl<T: AppServerTransportPort> ManagedClient<T> {
    pub fn dispatch_workspace_current(&mut self) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::WorkspaceCurrent,
            PendingKind::WorkspaceCurrent,
            self.timeouts.workspace_current,
        )
    }

    pub fn dispatch_workspace_list(
        &mut self,
        params: WorkspaceListRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::WorkspaceList(params),
            PendingKind::WorkspaceList,
            self.timeouts.workspace_list,
        )
    }

    pub fn dispatch_preview_request(
        &mut self,
        params: PreviewRequest,
    ) -> Result<RequestId, ClientError> {
        let target = params.source.clone();
        let request_id = self.enqueue_command(
            ClientCommand::PreviewRequest(params),
            PendingKind::Preview {
                target: target.clone(),
            },
            self.timeouts.preview_request,
        )?;
        self.preview_tasks.insert(
            request_id,
            PreviewTaskState {
                request_id,
                target: target.clone(),
                phase: PreviewPhase::Pending,
            },
        );
        self.active_preview_target = Some(target);
        self.preview_error = None;
        Ok(request_id)
    }

    pub fn dispatch_file_read(
        &mut self,
        params: FileReadRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::FileRead(params),
            PendingKind::FileRead,
            self.timeouts.file_read,
        )
    }

    pub fn dispatch_file_write_text(
        &mut self,
        params: FileWriteTextRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::FileWriteText(params),
            PendingKind::FileWriteText,
            self.timeouts.file_write_text,
        )
    }

    pub fn dispatch_config_load(&mut self) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ConfigLoad,
            PendingKind::ConfigLoad,
            self.timeouts.config_load,
        )
    }

    pub fn dispatch_config_save(
        &mut self,
        params: ConfigSaveRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ConfigSave(params),
            PendingKind::ConfigSave,
            self.timeouts.config_save,
        )
    }

    pub fn dispatch_slicer_list(
        &mut self,
        params: SlicerListRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::SlicerList(params),
            PendingKind::SlicerList,
            self.timeouts.slicer_list,
        )
    }

    pub fn dispatch_export_run(
        &mut self,
        params: ExportRunRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ExportRun(params),
            PendingKind::ExportRun,
            self.timeouts.export_run,
        )
    }

    pub fn dispatch_cadquery_execute(
        &mut self,
        params: CadQueryExecuteRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::CadQueryExecute(params),
            PendingKind::CadQuery,
            self.timeouts.preview_request,
        )
    }

    pub fn dispatch_cadquery_preview(
        &mut self,
        params: CadQueryPreviewRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::CadQueryPreview(params),
            PendingKind::CadQuery,
            self.timeouts.preview_request,
        )
    }

    pub fn dispatch_cadquery_result_get(
        &mut self,
        params: CadQueryResultGetRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::CadQueryResultGet(params),
            PendingKind::CadQuery,
            self.timeouts.file_read,
        )
    }

    pub fn dispatch_chat_create(
        &mut self,
        params: ChatCreateRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ChatCreate(params),
            PendingKind::Chat,
            self.timeouts.chat,
        )
    }

    pub fn dispatch_chat_list(
        &mut self,
        params: ChatListRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ChatList(params),
            PendingKind::Chat,
            self.timeouts.chat,
        )
    }

    pub fn dispatch_chat_send(
        &mut self,
        params: ChatSendRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ChatSend(params),
            PendingKind::Chat,
            self.timeouts.chat,
        )
    }

    pub fn dispatch_chat_history(
        &mut self,
        params: ChatHistoryRequest,
    ) -> Result<RequestId, ClientError> {
        let request_id = self.enqueue_command(
            ClientCommand::ChatHistory(params),
            PendingKind::Chat,
            self.timeouts.chat,
        )?;
        self.latest_chat_history_request = Some(request_id);
        Ok(request_id)
    }

    pub fn dispatch_chat_select(
        &mut self,
        session_id: ChatSessionId,
        params: ChatHistoryRequest,
    ) -> Result<RequestId, ClientError> {
        self.pending_chat_session = Some(session_id);
        self.dispatch_chat_history(params)
    }

    pub fn dispatch_chat_archive(
        &mut self,
        params: ChatArchiveRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::ChatArchive(params),
            PendingKind::Chat,
            self.timeouts.chat,
        )
    }

    pub fn dispatch_agent_invoke(
        &mut self,
        params: AgentInvokeRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::AgentInvoke(params),
            PendingKind::Agent,
            self.timeouts.agent,
        )
    }

    pub fn dispatch_agent_cancel(
        &mut self,
        params: AgentCancelRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::AgentCancel(params),
            PendingKind::Agent,
            self.timeouts.agent,
        )
    }

    pub fn dispatch_agent_plan_confirm(
        &mut self,
        params: AgentPlanConfirmRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::AgentPlanConfirm(params),
            PendingKind::Agent,
            self.timeouts.agent,
        )
    }

    pub fn dispatch_agent_plan_reject(
        &mut self,
        params: AgentPlanRejectRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::AgentPlanReject(params),
            PendingKind::Agent,
            self.timeouts.agent,
        )
    }

    pub fn dispatch_selection_update(
        &mut self,
        params: SelectionUpdateRequest,
    ) -> Result<RequestId, ClientError> {
        self.enqueue_command(
            ClientCommand::SelectionUpdate(params.clone()),
            PendingKind::SelectionUpdate { snapshot: params },
            self.timeouts.selection_update,
        )
    }

    pub fn subscribe_directory_watch(
        &mut self,
        params: WatchParams,
    ) -> Result<RequestId, ClientError> {
        if self.transport_status != TransportStatus::Open {
            return Err(ClientError::NotReady);
        }
        let request_id = self.allocate_request_id();
        let throttle_ms = params.throttle_ms.unwrap_or(DEFAULT_WATCH_THROTTLE_MS);
        let envelope_bytes = build_watch_subscribe_envelope(request_id, params.request.clone());
        self.watches.insert(
            request_id,
            WatchRegistryEntry {
                request: params.request,
                throttle_ms,
                subscription_id: None,
                envelope_bytes: envelope_bytes.clone(),
                accumulator: WatchAccumulator::default(),
                awaiting_resubscribe: false,
            },
        );
        self.pending.insert(
            request_id,
            PendingRequestInfo {
                kind: PendingKind::WatchSubscribe,
                deadline_ms: self.deadline_for(self.timeouts.watch),
                issued_at_ms: self.last_tick_ms,
                envelope_bytes: envelope_bytes.clone(),
                cancelled: false,
            },
        );
        self.outbound.push_back(envelope_bytes);
        Ok(request_id)
    }

    pub(super) fn enqueue_command(
        &mut self,
        command: ClientCommand,
        kind: PendingKind,
        timeout_ms: Option<u64>,
    ) -> Result<RequestId, ClientError> {
        if self.transport_status != TransportStatus::Open {
            return Err(ClientError::NotReady);
        }
        let request_id = self.allocate_request_id();
        let envelope_bytes = build_request_envelope(request_id, command);
        self.pending.insert(
            request_id,
            PendingRequestInfo {
                kind,
                deadline_ms: self.deadline_for(timeout_ms),
                issued_at_ms: self.last_tick_ms,
                envelope_bytes: envelope_bytes.clone(),
                cancelled: false,
            },
        );
        self.outbound.push_back(envelope_bytes);
        Ok(request_id)
    }

    pub(super) fn allocate_request_id(&mut self) -> RequestId {
        let id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        id
    }

    pub(super) fn deadline_for(&self, timeout_ms: Option<u64>) -> Option<u64> {
        timeout_ms.map(|timeout| self.last_tick_ms.saturating_add(timeout))
    }
}
