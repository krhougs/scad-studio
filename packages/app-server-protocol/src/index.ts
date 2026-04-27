export { default as initProtocolWasm } from "../generated/app_server_protocol_wasm.js";
export * from "../generated/app_server_protocol_wasm.js";

export type WorkspaceId = string;
export type RequestId = number;
export type SubscriptionId = string;
export type SessionToken = string;

export interface PathHandle {
  workspace_id: WorkspaceId;
  path_segments: string[];
}

export type HostLocalPath = string;

export type DisplayUnitDto = "millimeter" | "centimeter" | "inch";

export interface SlicerConfigDto {
  name: string;
  path: HostLocalPath;
}

export interface AppConfigDto {
  openscad_path: HostLocalPath | null;
  slicers: SlicerConfigDto[];
  recent_workspaces: HostLocalPath[];
  floating_panel_opacity: number;
  left_panel_width: number;
  right_panel_width: number;
  display_unit: DisplayUnitDto;
  camera_overlay_pos: [number, number] | null;
  camera_overlay_size: [number, number] | null;
  param_panel_pos: [number, number] | null;
  param_panel_size: [number, number] | null;
  log_panel_pos: [number, number] | null;
  log_panel_size: [number, number] | null;
}

export interface ConfigSaveRequest {
  config: AppConfigDto;
}

export type ClientPlatform = "desktop" | "web" | "other";
export type PreviewRequestKind = "geometry_artifact" | "rendered_image";
export type PreviewResponseFormat = "mesh" | "three_mf" | "rendered_image" | "stl";
export type PreviewUnit = "millimeter";
export type ExportFormat = "stl" | "three_mf";
export type CadQueryExportFormat = "step" | "stl" | "three_mf";
export type CadQueryObjectKind = "part" | "component" | "assembly";
export type ChatRole = "user" | "assistant" | "tool" | "meta";
export type ChatSessionId = string;
export type AgentOperationLevel = "inform" | "plan" | "execute";
export type AgentErrorType =
  | "llm_error"
  | "llm_refused"
  | "permission_denied"
  | "file_conflict"
  | "python_import_error"
  | "cadquery_build_error"
  | "tessellation_error"
  | "topology_mapping_error"
  | "export_error"
  | "timeout";
export type SelectionKind =
  | "component"
  | "part"
  | "assembly"
  | "instance"
  | "feature"
  | "face"
  | "edge"
  | "vertex";
export type WorkspaceEntryKind = "directory" | "file";
export type ProtocolErrorCode =
  | "invalid_command"
  | "invalid_path_handle"
  | "unsupported_file_type_for_client"
  | "stale_path_handle"
  | "cancelled"
  | "session_expired"
  | "unsupported_protocol_version"
  | "not_found"
  | "internal"
  | "invalid_wire_frame"
  | "unsupported_wire_version"
  | "invalid_numeric_value"
  | "invalid_host_local_path"
  | "agent_busy";

export interface ProtocolVersionRange {
  min: number;
  max: number;
}

export interface FileReadCapability {
  denied_extensions: string[];
}

export interface ClientCapabilities {
  client_name: string;
  platform: ClientPlatform;
  protocol_version: ProtocolVersionRange;
  file_read: FileReadCapability;
  supported_preview_kinds: PreviewRequestKind[];
}

export interface ServerCapabilities {
  protocol_version: ProtocolVersionRange;
  reconnect_window_ms: number;
  supports_watch: boolean;
  supported_preview_kinds: PreviewRequestKind[];
  supports_session_reclaim: boolean;
  cadquery: boolean;
  agent: boolean;
  selection_sync: boolean;
}

export interface CapabilityHandshakeRequest {
  capabilities: ClientCapabilities;
}

export interface CapabilityHandshakeResponse {
  negotiated_version: number;
  session_token: SessionToken;
  server_capabilities: ServerCapabilities;
}

export interface WorkspaceCurrentResponse {
  workspace_id: WorkspaceId;
  root_name: string;
}

export interface WorkspaceListRequest {
  directory: PathHandle | null;
}

export interface WorkspaceEntry {
  name: string;
  path: PathHandle | null;
  kind: WorkspaceEntryKind;
  path_error: string | null;
}

export interface WorkspaceListResponse {
  directory: PathHandle | null;
  entries: WorkspaceEntry[];
}

export interface FileReadRequest {
  path: PathHandle;
}

export type FileReadContents =
  | { kind: "utf8_text"; payload: string }
  | { kind: "binary"; payload: Uint8Array };

export interface FileReadResponse {
  path: PathHandle;
  media_type: string;
  contents: FileReadContents;
}

export interface FileWriteTextRequest {
  path: PathHandle;
  contents: string;
}

export interface FileWriteTextResponse {
  path: PathHandle;
}

export interface SlicerInstallRecord {
  name: string;
  path: HostLocalPath;
}

export interface SlicerListRequest {
  configured: SlicerInstallRecord[];
}

export interface SlicerListResponse {
  slicers: SlicerInstallRecord[];
}

export interface ExportRunRequest {
  configured_openscad_path: HostLocalPath | null;
  configured_slicers: SlicerInstallRecord[];
  source: PathHandle;
  defines: string[];
  output_path: PathHandle;
  format: ExportFormat;
  slicer_name: string | null;
}

export interface ExportRunResponse {
  output_path: PathHandle;
}

export interface PreviewRequest {
  source: PathHandle;
  defines: string[];
  kind: PreviewRequestKind;
  configured_openscad_path: HostLocalPath | null;
}

export interface PreviewMeshPayload {
  unit: PreviewUnit;
  positions: [number, number, number][];
  normals: [number, number, number][];
  vertex_colors: [number, number, number, number][];
  indices: number[];
}

export interface PreviewArtifact3mf {
  bytes: Uint8Array;
  media_type: string;
}

export interface PreviewArtifactStl {
  bytes: Uint8Array;
  media_type: string;
}

export interface PreviewRenderedImagePayload {
  bytes: Uint8Array;
  media_type: string;
  width: number;
  height: number;
}

export type PreviewArtifact =
  | { format: "mesh"; payload: PreviewMeshPayload }
  | { format: "three_mf"; payload: PreviewArtifact3mf }
  | { format: "rendered_image"; payload: PreviewRenderedImagePayload }
  | { format: "stl"; payload: PreviewArtifactStl };

export interface PreviewReadyResponse {
  requested_kind: PreviewRequestKind;
  artifact: PreviewArtifact;
}

export interface CadQueryExecuteRequest {
  target_path: PathHandle;
  target_type: CadQueryObjectKind;
  code: string;
  export_formats: CadQueryExportFormat[];
  params_json: string;
}

export interface CadQueryPreviewRequest {
  target_path: PathHandle;
  export_formats: CadQueryExportFormat[];
  params_json: string;
}

export interface CadQueryResultGetRequest {
  result_id: string;
}

export interface CadQueryResultReady {
  result_id: string;
  build_id: string;
  part_count: number;
  face_count: number;
  edge_count: number;
  vertex_count: number;
}

export interface CadQueryMeshPayload {
  result_id: string;
  build_id: string;
  unit: PreviewUnit;
  root_ref_text: string;
  root_object_kind: CadQueryObjectKind;
  parts: CadQueryPartMesh[];
}

export interface CadQueryPartMesh {
  name: string;
  object_kind: CadQueryObjectKind;
  ref_text: string;
  instance_path: string | null;
  transform: CadQueryTransform | null;
  faces: FaceGroup[];
  edges: EdgeGroup[];
  vertices: VertexPoint[];
  feature_map: CadQueryFeatureFaces[];
}

export type CadQueryTransform = [
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
];

export interface FaceGroup {
  face_idx: number;
  positions: number[];
  normals: number[];
  features: string[];
  ambiguous: boolean;
}

export interface EdgeGroup {
  edge_idx: number;
  polyline: number[];
  adjacent_faces: number[];
}

export interface VertexPoint {
  vertex_idx: number;
  position: [number, number, number];
  adjacent_edges: number[];
}

export interface CadQueryFeatureFaces {
  feature: string;
  face_indices: number[];
}

export interface ChatCreateRequest {
  title: string;
  goal: string | null;
  related_files: PathHandle[];
}

export interface ChatCreatedResponse {
  session_id: ChatSessionId;
  title: string;
}

export interface ChatListRequest {
  include_archived: boolean;
}

export interface ChatSessionSummary {
  session_id: ChatSessionId;
  title: string;
  archived: boolean;
  message_count: number;
  related_files: PathHandle[];
}

export interface ChatListResponse {
  sessions: ChatSessionSummary[];
}

export interface ChatSendRequest {
  session_id: ChatSessionId;
  content: string;
  related_files: PathHandle[];
}

export interface ChatAckResponse {
  session_id: ChatSessionId;
  message_id: string;
}

export interface ChatHistoryRequest {
  session_id: ChatSessionId;
  limit: number | null;
}

export interface ChatMessageRecord {
  message_id: string;
  ts_ms: number;
  role: ChatRole;
  content: string;
  related_files: PathHandle[];
  tool_call_id: string | null;
  tool_calls: ChatToolCallRecord[];
  tool_result: ChatToolResultRecord | null;
  mesh_result: CadQueryResultReady | null;
}

export interface ChatToolCallRecord {
  tool_call_id: string;
  tool_name: string;
  args_json: string;
}

export interface ChatToolResultRecord {
  tool_call_id: string;
  tool_name: string;
  result_json: string;
}

export interface ChatHistoryResponse {
  session_id: ChatSessionId;
  messages: ChatMessageRecord[];
}

export interface ChatArchiveRequest {
  session_id: ChatSessionId;
}

export interface ChatArchivedResponse {
  session_id: ChatSessionId;
}

export interface AgentInvokeRequest {
  session_id: ChatSessionId;
  prompt: string;
  operation: AgentOperationLevel;
  confirmed_cadquery: AgentCadQueryConfirmation | null;
}

export interface AgentCadQueryConfirmation {
  request: CadQueryExecuteRequest;
  plan_ref: PathHandle | null;
  affected_files: PathHandle[];
  new_files: PathHandle[];
  export_targets: PathHandle[];
}

export interface AgentStartedResponse {
  session_id: ChatSessionId;
  run_id: string;
}

export interface AgentCancelRequest {
  run_id: string | null;
}

export interface AgentCancelledResponse {
  run_id: string | null;
}

export interface AgentTokenEvent {
  session_id: ChatSessionId;
  run_id: string;
  text: string;
}

export interface AgentToolStartEvent {
  session_id: ChatSessionId;
  run_id: string;
  tool_call_id: string;
  tool_name: string;
  args_json: string;
}

export interface AgentToolResultEvent {
  session_id: ChatSessionId;
  run_id: string;
  tool_call_id: string;
  tool_name: string;
  result_json: string;
}

export interface AgentMeshReadyEvent {
  session_id: ChatSessionId;
  run_id: string;
  result: CadQueryResultReady;
}

export interface AgentErrorEvent {
  session_id: ChatSessionId;
  run_id: string | null;
  error_type: AgentErrorType;
  message: string;
}

export interface AgentDoneEvent {
  session_id: ChatSessionId;
  run_id: string;
  cancelled: boolean;
}

export interface SelectionRef {
  kind: SelectionKind;
  ref_text: string;
  owner_ref_text: string | null;
  owner_object_kind: CadQueryObjectKind | null;
  instance_path: string | null;
  candidate_feature_ref: string | null;
  build_id: string | null;
  result_id: string | null;
  ambiguous: boolean;
}

export interface SelectionUpdateRequest {
  selections: SelectionRef[];
  active_index: number | null;
}

export interface SelectionUpdateResponse {
  accepted_count: number;
}

export interface WatchSubscribeRequest {
  directory: PathHandle | null;
}

export interface WatchUnsubscribeRequest {
  subscription_id: SubscriptionId;
}

export interface WatchSubscriptionAck {
  subscription_id: SubscriptionId;
}

export interface WatchChangedEvent {
  subscription_id: SubscriptionId;
  changed_paths: PathHandle[];
}

export interface WatchErrorEvent {
  subscription_id: SubscriptionId;
  message: string;
}

export interface CancelRequest {
  request_id: RequestId;
}

export interface SessionReclaimRequest {
  session_token: SessionToken;
}

export interface SessionReclaimedResponse {
  workspace: WorkspaceCurrentResponse | null;
  reclaimed_capabilities: ServerCapabilities;
}

export type ProtocolError = {
  code: ProtocolErrorCode;
  message: string;
};

export type CommandSuccess =
  | { type: "workspace_current"; payload: WorkspaceCurrentResponse }
  | { type: "workspace_list"; payload: WorkspaceListResponse }
  | { type: "config_loaded"; payload: { config: AppConfigDto } }
  | { type: "config_saved" }
  | { type: "file_read"; payload: FileReadResponse }
  | { type: "file_written"; payload: FileWriteTextResponse }
  | { type: "preview_ready"; payload: PreviewReadyResponse }
  | { type: "cad_query_result_ready"; payload: CadQueryResultReady }
  | { type: "cad_query_mesh"; payload: CadQueryMeshPayload }
  | { type: "chat_created"; payload: ChatCreatedResponse }
  | { type: "chat_list"; payload: ChatListResponse }
  | { type: "chat_ack"; payload: ChatAckResponse }
  | { type: "chat_history"; payload: ChatHistoryResponse }
  | { type: "chat_archived"; payload: ChatArchivedResponse }
  | { type: "agent_started"; payload: AgentStartedResponse }
  | { type: "agent_cancelled"; payload: AgentCancelledResponse }
  | { type: "selection_updated"; payload: SelectionUpdateResponse }
  | { type: "slicer_listed"; payload: SlicerListResponse }
  | { type: "export_run"; payload: ExportRunResponse }
  | { type: "watch_subscribed"; payload: WatchSubscriptionAck }
  | { type: "watch_unsubscribed"; payload: WatchSubscriptionAck }
  | { type: "cancel_accepted"; payload: CancelRequest }
  | { type: "session_reclaimed"; payload: SessionReclaimedResponse };

export interface ServerResponseEnvelope {
  request_id: RequestId;
  result: { ok: CommandSuccess } | { err: ProtocolError };
}

export type ServerPushEvent =
  | { event: "watch.changed"; payload: WatchChangedEvent }
  | { event: "watch.error"; payload: WatchErrorEvent }
  | { event: "agent.token"; payload: AgentTokenEvent }
  | { event: "agent.tool_start"; payload: AgentToolStartEvent }
  | { event: "agent.tool_result"; payload: AgentToolResultEvent }
  | { event: "agent.mesh_ready"; payload: AgentMeshReadyEvent }
  | { event: "agent.error"; payload: AgentErrorEvent }
  | { event: "agent.done"; payload: AgentDoneEvent };

export interface ServerPushEnvelope {
  event: ServerPushEvent;
}

export type ServerEnvelope =
  | { kind: "handshake_ack"; payload: CapabilityHandshakeResponse }
  | { kind: "response"; payload: ServerResponseEnvelope }
  | { kind: "push"; payload: ServerPushEnvelope }
  | { kind: "transport_error"; payload: { message: string } }
  | { kind: "closed" };
