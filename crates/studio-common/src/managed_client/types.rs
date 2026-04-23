use app_server_protocol::{
    CommandSuccess, PathHandle, RequestId, ServerCapabilities, SessionToken, SubscriptionId,
    WatchSubscribeRequest, WorkspaceCurrentResponse, WorkspaceEntry, WorkspaceListResponse,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_INTERACTIVE_TIMEOUT_MS: u64 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientError {
    InvalidHandle,
    NotReady,
    DecodeError { context: String },
    UnknownRequest { request_id: RequestId },
    TransportClosed,
    Cancelled,
    ProtocolError { code: String, message: String },
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientEvent {
    HandshakeAccepted {
        session_token: SessionToken,
        server_capabilities: ServerCapabilities,
        negotiated_version: u16,
    },
    RequestSucceeded {
        request_id: RequestId,
        payload: CommandSuccess,
    },
    RequestFailed {
        request_id: RequestId,
        error: ClientError,
    },
    RequestTimedOut {
        request_id: RequestId,
    },
    WatchEvent {
        request_id: RequestId,
        payload: WatchEventPayload,
    },
    WatchResubscribed {
        request_id: RequestId,
    },
    TransportOpen,
    TransportClosed {
        reason: TransportCloseReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum WatchEventPayload {
    Changed {
        subscription_id: SubscriptionId,
        window_start_ms: u64,
        window_end_ms: u64,
        changed_paths: Vec<PathHandle>,
    },
    Error {
        subscription_id: SubscriptionId,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchParams {
    pub request: WatchSubscribeRequest,
    pub throttle_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportCloseReason {
    pub code: u16,
    pub reason: String,
    pub was_clean: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportStatus {
    Connecting,
    Open,
    Reconnecting,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientTimeouts {
    pub workspace_current: Option<u64>,
    pub workspace_list: Option<u64>,
    pub preview_request: Option<u64>,
    pub file_read: Option<u64>,
    pub file_write_text: Option<u64>,
    pub config_load: Option<u64>,
    pub config_save: Option<u64>,
    pub slicer_list: Option<u64>,
    pub export_run: Option<u64>,
    /// watch 订阅握手超时；订阅 ack 返回后不再受此约束，活跃订阅本身不设超时。
    pub watch: Option<u64>,
}

impl Default for ClientTimeouts {
    fn default() -> Self {
        Self {
            workspace_current: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            workspace_list: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            preview_request: None,
            file_read: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            file_write_text: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            config_load: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            config_save: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            slicer_list: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
            export_run: None,
            watch: Some(DEFAULT_INTERACTIVE_TIMEOUT_MS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewPhase {
    Pending,
    Ready,
    Error,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewTaskState {
    pub request_id: RequestId,
    pub target: PathHandle,
    pub phase: PreviewPhase,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchLifecycleSummary {
    pub active_subscriptions: u32,
    pub last_event_at_ms: Option<u64>,
    pub resubscribe_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientSnapshot {
    pub workspace_current: Option<WorkspaceCurrentResponse>,
    pub workspace_list: Option<WorkspaceListResponse>,
    pub current_directory_entries: Vec<WorkspaceEntry>,
    pub preview_tasks: Vec<PreviewTaskState>,
    pub active_preview_target: Option<PathHandle>,
    pub preview_error: Option<String>,
    pub watch_lifecycle: WatchLifecycleSummary,
    pub last_error: Option<ClientError>,
    pub transport_status: TransportStatus,
}
