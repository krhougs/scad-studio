use crate::{PathHandle, WorkspaceId};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

pub const DEFAULT_SESSION_RECONNECT_WINDOW_MS: u64 = 30_000;
pub const CURRENT_PROTOCOL_VERSION: u16 = 14;
// Web 客户端默认无拒绝扩展名。核心产品流是：
//   `.scad` → 服务端 OpenSCAD CLI → `.3mf` bytes → 前端 → 解码 + 渲染。
// `.scad` 是源码文本（ScadSplitViewer 要读取）；`.stl` / `.3mf` 是预览
// artifact bytes（MeshViewer 走 `PreviewRequest` 拿，但工作区里已有的
// `.stl` / `.3mf` 文件也允许直接被 `FileRead` 消费，例如将来做 "导入
// 既有模型" 或校验工作区内容）。如果 client 真的要限制某扩展，仍可通过
// 显式 `denied_extensions` 覆写。
pub const WEB_FILE_READ_DENY_EXTENSIONS: &[&str] = &[];

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct RequestId(pub u64);

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct SubscriptionId(pub String);

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct SessionToken(pub String);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum ClientPlatform {
    Web = 1,
    Other = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum PreviewRequestKind {
    GeometryArtifact = 0,
    RenderedImage = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ClientCapabilities {
    pub client_name: String,
    pub platform: ClientPlatform,
    pub protocol_version: ProtocolVersionRange,
    pub file_read: FileReadCapability,
    pub supported_preview_kinds: Vec<PreviewRequestKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ServerCapabilities {
    pub protocol_version: ProtocolVersionRange,
    pub reconnect_window_ms: u64,
    pub supports_watch: bool,
    pub supported_preview_kinds: Vec<PreviewRequestKind>,
    pub supports_session_reclaim: bool,
    pub cadquery: bool,
    pub agent: bool,
    pub selection_sync: bool,
    pub llm_configured: bool,
    #[serde(default)]
    pub agent_provider: Option<AgentProviderCapabilities>,
    #[serde(default)]
    pub agent_model_registry: Option<AgentModelRegistryResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentProviderCapabilities {
    pub provider: String,
    pub model: Option<String>,
    pub native_web_search_enabled: bool,
    pub search_sources_supported: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentModelSource {
    Manual = 0,
    Discovered = 1,
    DiscoveredWithOverride = 2,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentModelDiscoveryStatus {
    Disabled = 0,
    NotStarted = 1,
    Succeeded = 2,
    Failed = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentModelDiscoveryState {
    pub enabled: bool,
    pub status: AgentModelDiscoveryStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentModelRegistryModel {
    pub id: String,
    pub label: Option<String>,
    pub source: AgentModelSource,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
    pub native_web_search_enabled: bool,
    pub native_web_search_applied: bool,
    pub web_search_supported: bool,
    pub web_search_unsupported_reason: Option<String>,
    pub search_sources_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentModelRegistryProvider {
    pub id: String,
    pub kind: String,
    pub label: Option<String>,
    pub discovery: AgentModelDiscoveryState,
    pub models: Vec<AgentModelRegistryModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentModelRegistryResponse {
    pub active_provider_id: String,
    pub active_model_id: String,
    pub active_reasoning_effort: Option<String>,
    pub active_reasoning_effort_applied: bool,
    pub active_service_label: Option<String>,
    pub active_service_label_applied: bool,
    pub reasoning_effort_options: Vec<String>,
    pub service_label_options: Vec<String>,
    pub providers: Vec<AgentModelRegistryProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentModelSelectRequest {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentModelParamsUpdateRequest {
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CapabilityHandshakeRequest {
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CapabilityHandshakeResponse {
    pub negotiated_version: u16,
    pub session_token: SessionToken,
    pub server_capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WorkspaceCurrentResponse {
    pub workspace_id: WorkspaceId,
    pub root_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WorkspaceListRequest {
    pub directory: Option<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum WorkspaceEntryKind {
    Directory = 0,
    File = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WorkspaceEntry {
    pub name: String,
    pub path: Option<PathHandle>,
    pub kind: WorkspaceEntryKind,
    pub path_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WorkspaceListResponse {
    pub directory: Option<PathHandle>,
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FileReadRequest {
    pub path: PathHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum FileReadContents {
    Utf8Text(String) = 0,
    Binary(#[serde(with = "serde_bytes")] Vec<u8>) = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FileReadResponse {
    pub path: PathHandle,
    pub media_type: String,
    pub contents: FileReadContents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FileWriteTextRequest {
    pub path: PathHandle,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FileWriteTextResponse {
    pub path: PathHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct HostLocalPath(pub String);

impl HostLocalPath {
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.is_empty() || value.contains('\0') {
            return Err(ProtocolError::new(
                ProtocolErrorCode::InvalidHostLocalPath,
                "host-local path 不能为空或包含 NUL",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_path_buf(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.0)
    }
}

pub type WorkspacePortablePath = PathHandle;

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum DisplayUnitDto {
    #[default]
    Millimeter = 0,
    Centimeter = 1,
    Inch = 2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlicerConfigDto {
    pub name: String,
    pub path: HostLocalPath,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AppConfigDto {
    pub openscad_path: Option<HostLocalPath>,
    pub slicers: Vec<SlicerConfigDto>,
    pub recent_workspaces: Vec<HostLocalPath>,
    pub floating_panel_opacity: f32,
    pub left_panel_width: f32,
    pub right_panel_width: f32,
    pub display_unit: DisplayUnitDto,
    pub camera_overlay_pos: Option<[f32; 2]>,
    pub camera_overlay_size: Option<[f32; 2]>,
    pub param_panel_pos: Option<[f32; 2]>,
    pub param_panel_size: Option<[f32; 2]>,
    pub log_panel_pos: Option<[f32; 2]>,
    pub log_panel_size: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ConfigLoadResponse {
    pub config: AppConfigDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ConfigSaveRequest {
    pub config: AppConfigDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlicerInstallRecord {
    pub name: String,
    pub path: HostLocalPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlicerListRequest {
    pub configured: Vec<SlicerInstallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlicerListResponse {
    pub slicers: Vec<SlicerInstallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ExportRunRequest {
    pub configured_openscad_path: Option<HostLocalPath>,
    pub configured_slicers: Vec<SlicerInstallRecord>,
    pub source: PathHandle,
    pub defines: Vec<String>,
    pub output_path: WorkspacePortablePath,
    pub format: crate::ExportFormat,
    pub slicer_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ExportRunResponse {
    pub output_path: WorkspacePortablePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PreviewRequest {
    pub source: PathHandle,
    pub defines: Vec<String>,
    pub kind: PreviewRequestKind,
    #[serde(default)]
    pub configured_openscad_path: Option<HostLocalPath>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum PreviewResponseFormat {
    Mesh = 0,
    ThreeMf = 1,
    RenderedImage = 2,
    Stl = 3,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum PreviewUnit {
    Millimeter = 0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PreviewMeshPayload {
    pub unit: PreviewUnit,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub vertex_colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PreviewArtifact3mf {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PreviewArtifactStl {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PreviewRenderedImagePayload {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "format", content = "payload", rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum PreviewArtifact {
    Mesh(PreviewMeshPayload) = 0,
    ThreeMf(PreviewArtifact3mf) = 1,
    RenderedImage(PreviewRenderedImagePayload) = 2,
    Stl(PreviewArtifactStl) = 3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PreviewReadyResponse {
    pub requested_kind: PreviewRequestKind,
    pub artifact: PreviewArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryExecuteRequest {
    pub target_path: PathHandle,
    pub target_type: CadQueryObjectKind,
    pub code: String,
    pub export_formats: Vec<CadQueryExportFormat>,
    pub params_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryPreviewRequest {
    pub target_path: PathHandle,
    pub export_formats: Vec<CadQueryExportFormat>,
    pub params_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryResultGetRequest {
    pub result_id: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum CadQueryExportFormat {
    Step = 0,
    Stl = 1,
    ThreeMf = 2,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum CadQueryObjectKind {
    Part = 0,
    Component = 1,
    Assembly = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryResultReady {
    pub result_id: String,
    pub build_id: String,
    pub part_count: u32,
    pub face_count: u32,
    pub edge_count: u32,
    pub vertex_count: u32,
    pub artifact_relation: Option<CadQueryArtifactRelation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryMeshPayload {
    pub result_id: String,
    pub build_id: String,
    pub unit: PreviewUnit,
    pub root_ref_text: String,
    pub root_object_kind: CadQueryObjectKind,
    pub artifact_relation: Option<CadQueryArtifactRelation>,
    pub parts: Vec<CadQueryPartMesh>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryArtifactRelation {
    pub source_path: String,
    pub exports: Vec<CadQueryArtifactExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryArtifactExport {
    pub name: String,
    pub path: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryPartMesh {
    pub name: String,
    pub object_kind: CadQueryObjectKind,
    pub ref_text: String,
    pub instance_path: Option<String>,
    pub transform: Option<[f32; 16]>,
    pub faces: Vec<FaceGroup>,
    pub edges: Vec<EdgeGroup>,
    pub vertices: Vec<VertexPoint>,
    pub feature_map: Vec<CadQueryFeatureFaces>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CadQueryFeatureFaces {
    pub feature: String,
    pub face_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FaceGroup {
    pub face_idx: u32,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub features: Vec<String>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct EdgeGroup {
    pub edge_idx: u32,
    pub polyline: Vec<f32>,
    pub adjacent_faces: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct VertexPoint {
    pub vertex_idx: u32,
    pub position: [f32; 3],
    pub adjacent_edges: Vec<u32>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct ChatSessionId(pub String);

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(transparent)]
pub struct AgentId(pub String);

impl From<String> for AgentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AgentId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(transparent)]
pub struct AgentTurnId(pub String);

impl From<String> for AgentTurnId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AgentTurnId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
)]
#[serde(transparent)]
pub struct AgentEventId(pub u64);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentProviderType {
    #[serde(rename = "anthropic")]
    Anthropic = 0,
    #[serde(rename = "openai_responses")]
    OpenAiResponses = 1,
    #[serde(rename = "openai_completions")]
    OpenAiCompletions = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct BoundAgentModel {
    pub provider_id: String,
    pub provider_type: AgentProviderType,
    pub model_id: String,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentRuntimeStatus {
    Idle = 0,
    Running = 1,
    Done = 2,
    Failed = 3,
    Cancelled = 4,
    Interrupted = 5,
    FailedNeedsRecovery = 6,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatCreateInitialTurn {
    pub mode: AgentMode,
    pub plan_ref: Option<PathHandle>,
    #[serde(default)]
    pub context_refs: Vec<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum ChatRole {
    User = 0,
    Assistant = 1,
    Tool = 2,
    Meta = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatCreateRequest {
    pub title: String,
    pub goal: Option<String>,
    pub related_files: Vec<PathHandle>,
    #[serde(default)]
    pub client_request_id: Option<String>,
    #[serde(default)]
    pub initial_user_message: Option<String>,
    #[serde(default)]
    pub requested_model: Option<BoundAgentModel>,
    #[serde(default)]
    pub initial_turn: Option<ChatCreateInitialTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatCreatedResponse {
    pub session_id: ChatSessionId,
    pub agent_id: AgentId,
    pub title: String,
    #[serde(default)]
    pub initial_turn: Option<AgentStartedResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatListRequest {
    pub include_archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatSessionSummary {
    pub session_id: ChatSessionId,
    pub agent_id: AgentId,
    pub title: String,
    pub archived: bool,
    pub message_count: u32,
    pub related_files: Vec<PathHandle>,
    #[serde(default)]
    pub bound_model: Option<BoundAgentModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatListResponse {
    pub sessions: Vec<ChatSessionSummary>,
    #[serde(default)]
    pub active_chat_id: Option<ChatSessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatSendRequest {
    pub session_id: ChatSessionId,
    pub content: String,
    pub related_files: Vec<PathHandle>,
    #[serde(default)]
    pub client_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatAckResponse {
    pub session_id: ChatSessionId,
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatHistoryRequest {
    pub session_id: ChatSessionId,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatMessageRecord {
    pub message_id: String,
    pub ts_ms: u64,
    pub role: ChatRole,
    pub content: String,
    pub related_files: Vec<PathHandle>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<ChatToolCallRecord>,
    pub tool_result: Option<ChatToolResultRecord>,
    pub mesh_result: Option<CadQueryResultReady>,
    #[serde(default)]
    pub search_sources: Vec<AgentSearchSource>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<AgentId>,
    #[serde(default)]
    pub turn_id: Option<AgentTurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentSearchSource {
    pub title: String,
    pub url: String,
    pub start_index: Option<u32>,
    pub end_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatToolCallRecord {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatToolResultRecord {
    pub tool_call_id: String,
    pub tool_name: String,
    pub result_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatHistoryResponse {
    pub session_id: ChatSessionId,
    pub messages: Vec<ChatMessageRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatArchiveRequest {
    pub session_id: ChatSessionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChatArchivedResponse {
    pub session_id: ChatSessionId,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentMode {
    Agent = 0,
    Plan = 1,
}

/// Deprecated: use `AgentMode` and `AgentInvokeRequest::plan_ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentCadQueryConfirmation {
    pub request: CadQueryExecuteRequest,
    pub plan_ref: Option<PathHandle>,
    pub affected_files: Vec<PathHandle>,
    pub new_files: Vec<PathHandle>,
    pub export_targets: Vec<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentInvokeRequest {
    pub session_id: ChatSessionId,
    #[serde(default)]
    pub client_request_id: Option<String>,
    pub prompt: String,
    pub mode: AgentMode,
    pub plan_ref: Option<PathHandle>,
    #[serde(default)]
    pub context_refs: Vec<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub service_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentStartTurnRequest {
    pub agent_id: AgentId,
    #[serde(default)]
    pub client_request_id: Option<String>,
    pub prompt: String,
    pub mode: AgentMode,
    pub plan_ref: Option<PathHandle>,
    #[serde(default)]
    pub context_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentStartedResponse {
    pub session_id: ChatSessionId,
    pub agent_id: AgentId,
    pub run_id: String,
    pub turn_id: AgentTurnId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentCancelRequest {
    pub agent_id: AgentId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentCancelledResponse {
    pub agent_id: AgentId,
    pub cancelled: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentErrorType {
    LlmError = 0,
    LlmRefused = 1,
    PermissionDenied = 2,
    FileConflict = 3,
    PythonImportError = 4,
    CadQueryBuildError = 5,
    TessellationError = 6,
    TopologyMappingError = 7,
    ExportError = 8,
    Timeout = 9,
    PersistenceError = 10,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentTokenEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentReasoningEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentToolStartEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub args_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentToolResultEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub result_json: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum AgentHostedToolActivityStatus {
    Requested = 0,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentHostedToolActivityEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub provider_id: String,
    pub provider_kind: AgentProviderType,
    pub tool_type: String,
    pub status: AgentHostedToolActivityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentMeshReadyEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub result: CadQueryResultReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentErrorEvent {
    pub session_id: ChatSessionId,
    pub run_id: Option<String>,
    pub error_type: AgentErrorType,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentDoneEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum AgentEventPayload {
    StateChanged {
        state: AgentRuntimeStatus,
    } = 0,
    Token {
        text: String,
    } = 1,
    Reasoning {
        text: String,
    } = 2,
    ToolStart {
        tool_call_id: String,
        tool_name: String,
        args_json: String,
    } = 3,
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        result_json: String,
    } = 4,
    Error {
        error_type: AgentErrorType,
        message: String,
    } = 5,
    Done {
        cancelled: bool,
    } = 6,
    HostedToolActivity {
        provider_id: String,
        provider_kind: AgentProviderType,
        tool_type: String,
        status: AgentHostedToolActivityStatus,
    } = 7,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentEventRecord {
    pub event_id: AgentEventId,
    pub agent_id: AgentId,
    pub turn_id: Option<AgentTurnId>,
    pub ts_ms: u64,
    pub payload: AgentEventPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentSnapshotRequest {
    pub agent_id: AgentId,
    pub since_event_id: Option<AgentEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentSnapshotResponse {
    pub agent_id: AgentId,
    pub chat_id: ChatSessionId,
    pub bound_model: Option<BoundAgentModel>,
    pub model_lock_reason: Option<String>,
    pub state: AgentRuntimeStatus,
    pub active_turn_id: Option<AgentTurnId>,
    pub since_event_id: Option<AgentEventId>,
    pub events: Vec<AgentEventRecord>,
    pub current_text: String,
    pub current_reasoning: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentSubscribeRequest {
    pub agent_id: AgentId,
    pub since_event_id: Option<AgentEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentSubscribeResponse {
    pub agent_id: AgentId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentPlanProposedEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    #[serde(default)]
    pub plan_ref: Option<PathHandle>,
    pub target_path: PathHandle,
    pub target_type: CadQueryObjectKind,
    pub affected_files: Vec<PathHandle>,
    #[serde(default)]
    pub new_files: Vec<PathHandle>,
    pub change_description: String,
    pub export_targets: Vec<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentPlanPackageRef {
    pub plan_id: String,
    pub plan_ref: PathHandle,
    pub request_path: PathHandle,
    pub plan_path: PathHandle,
    pub result_path: PathHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentPlanSavedEvent {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub package: AgentPlanPackageRef,
    pub title: String,
    pub status: String,
    pub target_path: PathHandle,
    pub target_type: CadQueryObjectKind,
    pub affected_files: Vec<PathHandle>,
    #[serde(default)]
    pub new_files: Vec<PathHandle>,
    pub change_description: String,
    pub export_targets: Vec<PathHandle>,
}

/// Deprecated: use `AgentInvokeRequest { mode: AgentMode::Agent, plan_ref }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentPlanConfirmRequest {
    pub session_id: ChatSessionId,
    pub run_id: String,
    pub confirmed_cadquery: AgentCadQueryConfirmation,
}

/// Deprecated: use Agent mode chat flow instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AgentPlanRejectRequest {
    pub session_id: ChatSessionId,
    pub run_id: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum SelectionKind {
    Component = 0,
    Part = 1,
    Assembly = 2,
    Instance = 3,
    Feature = 4,
    Face = 5,
    Edge = 6,
    Vertex = 7,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SelectionRef {
    pub kind: SelectionKind,
    pub ref_text: String,
    pub owner_ref_text: Option<String>,
    pub owner_object_kind: Option<CadQueryObjectKind>,
    pub instance_path: Option<String>,
    pub candidate_feature_ref: Option<String>,
    pub build_id: Option<String>,
    pub result_id: Option<String>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SelectionUpdateRequest {
    pub selections: Vec<SelectionRef>,
    pub active_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SelectionUpdateResponse {
    pub accepted_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WatchSubscribeRequest {
    pub directory: Option<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WatchUnsubscribeRequest {
    pub subscription_id: SubscriptionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WatchSubscriptionAck {
    pub subscription_id: SubscriptionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WatchChangedEvent {
    pub subscription_id: SubscriptionId,
    pub changed_paths: Vec<PathHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct WatchErrorEvent {
    pub subscription_id: SubscriptionId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CancelRequest {
    pub request_id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SessionReclaimRequest {
    pub session_token: SessionToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SessionReclaimedResponse {
    pub workspace: Option<WorkspaceCurrentResponse>,
    pub reclaimed_capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "command", content = "payload")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum ClientCommand {
    #[serde(rename = "workspace.current")]
    WorkspaceCurrent = 0,
    #[serde(rename = "workspace.list")]
    WorkspaceList(WorkspaceListRequest) = 1,
    #[serde(rename = "config.load")]
    ConfigLoad = 2,
    #[serde(rename = "config.save")]
    ConfigSave(ConfigSaveRequest) = 3,
    #[serde(rename = "file.read")]
    FileRead(FileReadRequest) = 4,
    #[serde(rename = "file.write_text")]
    FileWriteText(FileWriteTextRequest) = 5,
    #[serde(rename = "preview.request")]
    PreviewRequest(PreviewRequest) = 6,
    #[serde(rename = "slicer.list")]
    SlicerList(SlicerListRequest) = 7,
    #[serde(rename = "export.run")]
    ExportRun(ExportRunRequest) = 8,
    #[serde(rename = "watch.subscribe")]
    WatchSubscribe(WatchSubscribeRequest) = 9,
    #[serde(rename = "watch.unsubscribe")]
    WatchUnsubscribe(WatchUnsubscribeRequest) = 10,
    #[serde(rename = "cancel")]
    Cancel(CancelRequest) = 11,
    #[serde(rename = "session.reclaim")]
    SessionReclaim(SessionReclaimRequest) = 12,
    #[serde(rename = "cadquery.execute")]
    CadQueryExecute(CadQueryExecuteRequest) = 13,
    #[serde(rename = "cadquery.preview")]
    CadQueryPreview(CadQueryPreviewRequest) = 14,
    #[serde(rename = "cadquery.result.get")]
    CadQueryResultGet(CadQueryResultGetRequest) = 15,
    #[serde(rename = "chat.create")]
    ChatCreate(ChatCreateRequest) = 16,
    #[serde(rename = "chat.list")]
    ChatList(ChatListRequest) = 17,
    #[serde(rename = "chat.send")]
    ChatSend(ChatSendRequest) = 18,
    #[serde(rename = "chat.history")]
    ChatHistory(ChatHistoryRequest) = 19,
    #[serde(rename = "chat.archive")]
    ChatArchive(ChatArchiveRequest) = 20,
    #[serde(rename = "agent.invoke")]
    AgentInvoke(AgentInvokeRequest) = 21,
    #[serde(rename = "agent.cancel")]
    AgentCancel(AgentCancelRequest) = 22,
    #[serde(rename = "selection.update")]
    SelectionUpdate(SelectionUpdateRequest) = 23,
    #[serde(rename = "agent.plan.confirm")]
    AgentPlanConfirm(AgentPlanConfirmRequest) = 24,
    #[serde(rename = "agent.plan.reject")]
    AgentPlanReject(AgentPlanRejectRequest) = 25,
    #[serde(rename = "agent.model.registry")]
    AgentModelRegistry = 26,
    #[serde(rename = "agent.model.select")]
    AgentModelSelect(AgentModelSelectRequest) = 27,
    #[serde(rename = "agent.model.params.update")]
    AgentModelParamsUpdate(AgentModelParamsUpdateRequest) = 28,
    #[serde(rename = "agent.start_turn")]
    AgentStartTurn(AgentStartTurnRequest) = 29,
    #[serde(rename = "agent.snapshot")]
    AgentSnapshot(AgentSnapshotRequest) = 30,
    #[serde(rename = "agent.subscribe")]
    AgentSubscribe(AgentSubscribeRequest) = 31,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ClientRequestEnvelope {
    pub request_id: RequestId,
    pub command: ClientCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum CommandSuccess {
    WorkspaceCurrent(WorkspaceCurrentResponse) = 0,
    WorkspaceList(WorkspaceListResponse) = 1,
    ConfigLoaded(ConfigLoadResponse) = 2,
    ConfigSaved = 3,
    FileRead(FileReadResponse) = 4,
    FileWritten(FileWriteTextResponse) = 5,
    PreviewReady(PreviewReadyResponse) = 6,
    SlicerListed(SlicerListResponse) = 7,
    ExportRun(ExportRunResponse) = 8,
    WatchSubscribed(WatchSubscriptionAck) = 9,
    WatchUnsubscribed(WatchSubscriptionAck) = 10,
    CancelAccepted(CancelRequest) = 11,
    SessionReclaimed(SessionReclaimedResponse) = 12,
    CadQueryResultReady(CadQueryResultReady) = 13,
    CadQueryMesh(CadQueryMeshPayload) = 14,
    ChatCreated(ChatCreatedResponse) = 15,
    ChatList(ChatListResponse) = 16,
    ChatAck(ChatAckResponse) = 17,
    ChatHistory(ChatHistoryResponse) = 18,
    ChatArchived(ChatArchivedResponse) = 19,
    AgentStarted(AgentStartedResponse) = 20,
    AgentCancelled(AgentCancelledResponse) = 21,
    SelectionUpdated(SelectionUpdateResponse) = 22,
    AgentPlanConfirmed(AgentStartedResponse) = 23,
    AgentPlanRejected = 24,
    AgentModelRegistry(AgentModelRegistryResponse) = 25,
    AgentSnapshot(AgentSnapshotResponse) = 26,
    AgentSubscribed(AgentSubscribeResponse) = 27,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
pub enum ProtocolErrorCode {
    InvalidCommand = 0,
    InvalidPathHandle = 1,
    UnsupportedFileTypeForClient = 2,
    StalePathHandle = 3,
    Cancelled = 4,
    SessionExpired = 5,
    UnsupportedProtocolVersion = 6,
    NotFound = 7,
    Internal = 8,
    InvalidWireFrame = 9,
    UnsupportedWireVersion = 10,
    InvalidNumericValue = 11,
    InvalidHostLocalPath = 12,
    AgentBusy = 13,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
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

    pub fn code(&self) -> ProtocolErrorCode {
        self.code
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ServerResponseEnvelope {
    pub request_id: RequestId,
    pub result: Result<CommandSuccess, ProtocolError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "event", content = "payload")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum ServerPushEvent {
    #[serde(rename = "watch.changed")]
    WatchChanged(WatchChangedEvent) = 0,
    #[serde(rename = "watch.error")]
    WatchError(WatchErrorEvent) = 1,
    #[serde(rename = "agent.token")]
    AgentToken(AgentTokenEvent) = 2,
    #[serde(rename = "agent.tool_start")]
    AgentToolStart(AgentToolStartEvent) = 3,
    #[serde(rename = "agent.tool_result")]
    AgentToolResult(AgentToolResultEvent) = 4,
    #[serde(rename = "agent.mesh_ready")]
    AgentMeshReady(AgentMeshReadyEvent) = 5,
    #[serde(rename = "agent.error")]
    AgentError(AgentErrorEvent) = 6,
    #[serde(rename = "agent.done")]
    AgentDone(AgentDoneEvent) = 7,
    #[serde(rename = "agent.plan_proposed")]
    AgentPlanProposed(AgentPlanProposedEvent) = 8,
    #[serde(rename = "agent.plan_saved")]
    AgentPlanSaved(AgentPlanSavedEvent) = 9,
    #[serde(rename = "agent.reasoning")]
    AgentReasoning(AgentReasoningEvent) = 10,
    #[serde(rename = "chat.list_changed")]
    ChatListChanged(ChatListResponse) = 11,
    #[serde(rename = "agent.hosted_tool_activity")]
    AgentHostedToolActivity(AgentHostedToolActivityEvent) = 12,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ServerPushEnvelope {
    pub event: ServerPushEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum ClientEnvelope {
    Handshake(CapabilityHandshakeRequest) = 0,
    Reconnect(CapabilityHandshakeRequest) = 1,
    Request(ClientRequestEnvelope) = 2,
    Close = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct TransportErrorFrame {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum ServerEnvelope {
    HandshakeAck(CapabilityHandshakeResponse) = 0,
    Response(ServerResponseEnvelope) = 1,
    Push(ServerPushEnvelope) = 2,
    TransportError(TransportErrorFrame) = 3,
    Closed = 4,
}
