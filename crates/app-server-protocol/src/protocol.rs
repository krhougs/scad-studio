use crate::{PathHandle, WorkspaceId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_SESSION_RECONNECT_WINDOW_MS: u64 = 30_000;
// Web 客户端默认无拒绝扩展名。核心产品流是：
//   `.scad` → 服务端 OpenSCAD CLI → `.3mf` bytes → 前端 → 解码 + 渲染。
// `.scad` 是源码文本（ScadSplitViewer 要读取）；`.stl` / `.3mf` 是预览
// artifact bytes（MeshViewer 走 `PreviewRequest` 拿，但工作区里已有的
// `.stl` / `.3mf` 文件也允许直接被 `FileRead` 消费，例如将来做 "导入
// 既有模型" 或校验工作区内容）。如果 client 真的要限制某扩展，仍可通过
// 显式 `denied_extensions` 覆写。
pub const WEB_FILE_READ_DENY_EXTENSIONS: &[&str] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionToken(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersionRange {
    pub min: u16,
    pub max: u16,
}

impl ProtocolVersionRange {
    pub fn new(min: u16, max: u16) -> Self {
        Self { min, max }
    }
}

pub fn negotiate_protocol_version(
    client: ProtocolVersionRange,
    server: ProtocolVersionRange,
) -> Result<u16, ProtocolError> {
    let negotiated = client.max.min(server.max);
    if negotiated < client.min || negotiated < server.min {
        return Err(ProtocolError::new(
            ProtocolErrorCode::UnsupportedProtocolVersion,
            "no overlapping protocol version",
        ));
    }
    Ok(negotiated)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientPlatform {
    Desktop,
    Web,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadCapability {
    pub denied_extensions: Vec<String>,
}

pub fn web_file_read_capability() -> FileReadCapability {
    FileReadCapability {
        denied_extensions: WEB_FILE_READ_DENY_EXTENSIONS
            .iter()
            .map(|value| value.to_string())
            .collect(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewRequestKind {
    GeometryArtifact,
    RenderedImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub client_name: String,
    pub platform: ClientPlatform,
    pub protocol_version: ProtocolVersionRange,
    pub file_read: FileReadCapability,
    pub supported_preview_kinds: Vec<PreviewRequestKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub protocol_version: ProtocolVersionRange,
    pub reconnect_window_ms: u64,
    pub supports_watch: bool,
    pub supported_preview_kinds: Vec<PreviewRequestKind>,
    pub supports_session_reclaim: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityHandshakeRequest {
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityHandshakeResponse {
    pub negotiated_version: u16,
    pub session_token: SessionToken,
    pub server_capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCurrentResponse {
    pub workspace_id: WorkspaceId,
    pub root_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceListRequest {
    pub directory: Option<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub path: PathHandle,
    pub kind: WorkspaceEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceListResponse {
    pub directory: Option<PathHandle>,
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadRequest {
    pub path: PathHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum FileReadContents {
    Utf8Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadResponse {
    pub path: PathHandle,
    pub media_type: String,
    pub contents: FileReadContents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteTextRequest {
    pub path: PathHandle,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteTextResponse {
    pub path: PathHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigLoadResponse {
    pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSaveRequest {
    pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlicerInstallRecord {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlicerListRequest {
    pub configured: Vec<SlicerInstallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlicerListResponse {
    pub slicers: Vec<SlicerInstallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRunRequest {
    pub configured_openscad_path: Option<PathBuf>,
    pub configured_slicers: Vec<SlicerInstallRecord>,
    pub source: PathHandle,
    pub defines: Vec<String>,
    pub output_path: PathBuf,
    pub format: crate::ExportFormat,
    pub slicer_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRunResponse {
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRequest {
    pub source: PathHandle,
    pub defines: Vec<String>,
    pub kind: PreviewRequestKind,
    #[serde(default)]
    pub configured_openscad_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewResponseFormat {
    Mesh,
    ThreeMf,
    RenderedImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewUnit {
    Millimeter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewMeshPayload {
    pub unit: PreviewUnit,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub vertex_colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewArtifact3mf {
    pub bytes: Vec<u8>,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRenderedImagePayload {
    pub bytes: Vec<u8>,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "format", content = "payload", rename_all = "snake_case")]
pub enum PreviewArtifact {
    Mesh(PreviewMeshPayload),
    ThreeMf(PreviewArtifact3mf),
    RenderedImage(PreviewRenderedImagePayload),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewReadyResponse {
    pub requested_kind: PreviewRequestKind,
    pub artifact: PreviewArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchSubscribeRequest {
    pub directory: Option<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchUnsubscribeRequest {
    pub subscription_id: SubscriptionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchSubscriptionAck {
    pub subscription_id: SubscriptionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchChangedEvent {
    pub subscription_id: SubscriptionId,
    pub changed_paths: Vec<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchErrorEvent {
    pub subscription_id: SubscriptionId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelRequest {
    pub request_id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionReclaimRequest {
    pub session_token: SessionToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionReclaimedResponse {
    pub workspace: Option<WorkspaceCurrentResponse>,
    pub reclaimed_capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", content = "payload")]
pub enum ClientCommand {
    #[serde(rename = "workspace.current")]
    WorkspaceCurrent,
    #[serde(rename = "workspace.list")]
    WorkspaceList(WorkspaceListRequest),
    #[serde(rename = "config.load")]
    ConfigLoad,
    #[serde(rename = "config.save")]
    ConfigSave(ConfigSaveRequest),
    #[serde(rename = "file.read")]
    FileRead(FileReadRequest),
    #[serde(rename = "file.write_text")]
    FileWriteText(FileWriteTextRequest),
    #[serde(rename = "preview.request")]
    PreviewRequest(PreviewRequest),
    #[serde(rename = "slicer.list")]
    SlicerList(SlicerListRequest),
    #[serde(rename = "export.run")]
    ExportRun(ExportRunRequest),
    #[serde(rename = "watch.subscribe")]
    WatchSubscribe(WatchSubscribeRequest),
    #[serde(rename = "watch.unsubscribe")]
    WatchUnsubscribe(WatchUnsubscribeRequest),
    #[serde(rename = "cancel")]
    Cancel(CancelRequest),
    #[serde(rename = "session.reclaim")]
    SessionReclaim(SessionReclaimRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRequestEnvelope {
    pub request_id: RequestId,
    pub command: ClientCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CommandSuccess {
    WorkspaceCurrent(WorkspaceCurrentResponse),
    WorkspaceList(WorkspaceListResponse),
    ConfigLoaded(ConfigLoadResponse),
    ConfigSaved,
    FileRead(FileReadResponse),
    FileWritten(FileWriteTextResponse),
    PreviewReady(PreviewReadyResponse),
    SlicerListed(SlicerListResponse),
    ExportRun(ExportRunResponse),
    WatchSubscribed(WatchSubscriptionAck),
    WatchUnsubscribed(WatchSubscriptionAck),
    CancelAccepted(CancelRequest),
    SessionReclaimed(SessionReclaimedResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolErrorCode {
    InvalidCommand,
    InvalidPathHandle,
    UnsupportedFileTypeForClient,
    StalePathHandle,
    Cancelled,
    SessionExpired,
    UnsupportedProtocolVersion,
    NotFound,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: ProtocolErrorCode,
    pub message: String,
}

impl ProtocolError {
    pub fn new(code: ProtocolErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerResponseEnvelope {
    pub request_id: RequestId,
    pub result: Result<CommandSuccess, ProtocolError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload")]
pub enum ServerPushEvent {
    #[serde(rename = "watch.changed")]
    WatchChanged(WatchChangedEvent),
    #[serde(rename = "watch.error")]
    WatchError(WatchErrorEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerPushEnvelope {
    pub event: ServerPushEvent,
}

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
    HandshakeAck(CapabilityHandshakeResponse),
    Response(ServerResponseEnvelope),
    Push(ServerPushEnvelope),
    TransportError(TransportErrorFrame),
    Closed,
}
