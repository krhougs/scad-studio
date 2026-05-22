export { default as initProtocolWasm } from "../generated/app_server_protocol_wasm.js";
export * from "../generated/app_server_protocol_wasm.js";

export type WorkspaceId = string;
export type RequestId = number;
export type SubscriptionId = string;
export type SessionToken = string;
export const CURRENT_PROTOCOL_VERSION = 14;

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

export type ClientPlatform = "web" | "other";
export type PreviewRequestKind = "geometry_artifact" | "rendered_image";
export type PreviewResponseFormat = "mesh" | "three_mf" | "rendered_image" | "stl";
export type PreviewUnit = "millimeter";
export type ExportFormat = "stl" | "three_mf";
export type CadQueryExportFormat = "step" | "stl" | "three_mf";
export type CadQueryObjectKind = "part" | "component" | "assembly";
export type ChatRole = "user" | "assistant" | "tool" | "meta";
export type ChatSessionId = string;
export type AgentId = string;
export type AgentTurnId = string;
export type AgentEventId = number;
export type AgentMode = "agent" | "plan";
export type AgentProviderType =
  | "anthropic"
  | "openai_responses"
  | "openai_completions";
export type AgentRuntimeStatus =
  | "idle"
  | "running"
  | "done"
  | "failed"
  | "cancelled"
  | "interrupted"
  | "failed_needs_recovery";
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
  | "timeout"
  | "persistence_error";
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
  llm_configured: boolean;
  agent_provider: AgentProviderCapabilities | null;
  agent_model_registry?: AgentModelRegistryResponse | null;
}

export interface AgentProviderCapabilities {
  provider: string;
  model: string | null;
  native_web_search_enabled: boolean;
  search_sources_supported: boolean;
}

export type AgentModelSource =
  | "manual"
  | "discovered"
  | "discovered_with_override";

export type AgentModelDiscoveryStatus =
  | "disabled"
  | "not_started"
  | "succeeded"
  | "failed";

export interface AgentModelDiscoveryState {
  enabled: boolean;
  status: AgentModelDiscoveryStatus;
  error: string | null;
}

export interface AgentModelRegistryModel {
  id: string;
  label: string | null;
  source: AgentModelSource;
  reasoning_effort: string | null;
  service_label: string | null;
  native_web_search_enabled: boolean;
  native_web_search_applied: boolean;
  web_search_supported: boolean;
  web_search_unsupported_reason: string | null;
  search_sources_supported: boolean;
}

export interface AgentModelRegistryProvider {
  id: string;
  kind: string;
  label: string | null;
  discovery: AgentModelDiscoveryState;
  models: AgentModelRegistryModel[];
}

export interface AgentModelRegistryResponse {
  active_provider_id: string;
  active_model_id: string;
  active_reasoning_effort: string | null;
  active_reasoning_effort_applied: boolean;
  active_service_label: string | null;
  active_service_label_applied: boolean;
  reasoning_effort_options: string[];
  service_label_options: string[];
  providers: AgentModelRegistryProvider[];
}

export interface AgentModelSelectRequest {
  provider_id: string;
  model_id: string;
}

export interface AgentModelParamsUpdateRequest {
  provider_id: string;
  model_id: string;
  reasoning_effort: string | null;
  service_label: string | null;
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
  artifact_relation: CadQueryArtifactRelation | null;
}

export interface CadQueryMeshPayload {
  result_id: string;
  build_id: string;
  unit: PreviewUnit;
  root_ref_text: string;
  root_object_kind: CadQueryObjectKind;
  artifact_relation: CadQueryArtifactRelation | null;
  parts: CadQueryPartMesh[];
}

export interface CadQueryArtifactRelation {
  source_path: string;
  exports: CadQueryArtifactExport[];
}

export interface CadQueryArtifactExport {
  name: string;
  path: string;
  hash: string;
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
  client_request_id?: string | null;
  initial_user_message?: string | null;
  requested_model?: BoundAgentModel | null;
  initial_turn?: ChatCreateInitialTurn | null;
}

export interface ChatCreateInitialTurn {
  mode: AgentMode;
  plan_ref: PathHandle | null;
}

export interface ChatCreatedResponse {
  session_id: ChatSessionId;
  agent_id: AgentId;
  title: string;
  initial_turn?: AgentStartedResponse | null;
}

export interface ChatListRequest {
  include_archived: boolean;
}

export interface ChatSessionSummary {
  session_id: ChatSessionId;
  agent_id: AgentId;
  title: string;
  archived: boolean;
  message_count: number;
  related_files: PathHandle[];
  bound_model?: BoundAgentModel | null;
}

export interface ChatListResponse {
  sessions: ChatSessionSummary[];
  active_chat_id?: ChatSessionId | null;
}

export interface ChatSendRequest {
  session_id: ChatSessionId;
  content: string;
  related_files: PathHandle[];
  client_request_id?: string | null;
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
  search_sources: AgentSearchSource[];
  run_id: string | null;
  agent_id: string | null;
  turn_id: string | null;
}

export interface AgentSearchSource {
  title: string;
  url: string;
  start_index: number | null;
  end_index: number | null;
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
  client_request_id?: string | null;
  prompt: string;
  mode: AgentMode;
  plan_ref: PathHandle | null;
  provider_id?: string | null;
  model_id?: string | null;
  reasoning_effort?: string | null;
  service_label?: string | null;
}

export interface BoundAgentModel {
  provider_id: string;
  provider_type: AgentProviderType;
  model_id: string;
  reasoning_effort: string | null;
  service_label: string | null;
}

export interface AgentStartTurnRequest {
  agent_id: AgentId;
  client_request_id?: string | null;
  prompt: string;
  mode: AgentMode;
  plan_ref: PathHandle | null;
}

/** Deprecated: use AgentInvokeRequest { mode: "agent", plan_ref }. */
export interface AgentCadQueryConfirmation {
  request: CadQueryExecuteRequest;
  plan_ref: PathHandle | null;
  affected_files: PathHandle[];
  new_files: PathHandle[];
  export_targets: PathHandle[];
}

export interface AgentStartedResponse {
  session_id: ChatSessionId;
  agent_id: AgentId;
  run_id: string;
  turn_id: AgentTurnId;
}

export interface AgentCancelRequest {
  agent_id: AgentId;
}

export interface AgentCancelledResponse {
  agent_id: AgentId;
  cancelled: boolean;
}

export interface AgentTokenEvent {
  session_id: ChatSessionId;
  run_id: string;
  text: string;
}

export interface AgentReasoningEvent {
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

export type AgentHostedToolActivityStatus = "requested";

export interface AgentHostedToolActivityEvent {
  session_id: ChatSessionId;
  run_id: string;
  provider_id: string;
  provider_kind: AgentProviderType;
  tool_type: string;
  status: AgentHostedToolActivityStatus;
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

export type AgentEventPayload =
  | { event: "state_changed"; payload: { state: AgentRuntimeStatus } }
  | { event: "token"; payload: { text: string } }
  | { event: "reasoning"; payload: { text: string } }
  | {
      event: "tool_start";
      payload: { tool_call_id: string; tool_name: string; args_json: string };
    }
  | {
      event: "tool_result";
      payload: { tool_call_id: string; tool_name: string; result_json: string };
    }
  | {
      event: "hosted_tool_activity";
      payload: {
        provider_id: string;
        provider_kind: AgentProviderType;
        tool_type: string;
        status: AgentHostedToolActivityStatus;
      };
    }
  | {
      event: "error";
      payload: { error_type: AgentErrorType; message: string };
    }
  | { event: "done"; payload: { cancelled: boolean } };

export interface AgentEventRecord {
  event_id: AgentEventId;
  agent_id: AgentId;
  turn_id: AgentTurnId | null;
  ts_ms: number;
  payload: AgentEventPayload;
}

export interface AgentSnapshotRequest {
  agent_id: AgentId;
  since_event_id: AgentEventId | null;
}

export interface AgentSnapshotResponse {
  agent_id: AgentId;
  chat_id: ChatSessionId;
  bound_model: BoundAgentModel | null;
  model_lock_reason: string | null;
  state: AgentRuntimeStatus;
  active_turn_id: AgentTurnId | null;
  since_event_id: AgentEventId | null;
  events: AgentEventRecord[];
  current_text: string;
  current_reasoning: string;
  error: string | null;
}

export interface AgentSubscribeRequest {
  agent_id: AgentId;
  since_event_id: AgentEventId | null;
}

export interface AgentSubscribeResponse {
  agent_id: AgentId;
}

export interface AgentPlanProposedEvent {
  session_id: ChatSessionId;
  run_id: string;
  plan_ref: PathHandle | null;
  target_path: PathHandle;
  target_type: CadQueryObjectKind;
  affected_files: PathHandle[];
  new_files: PathHandle[];
  change_description: string;
  export_targets: PathHandle[];
}

export interface AgentPlanPackageRef {
  plan_id: string;
  plan_ref: PathHandle;
  request_path: PathHandle;
  plan_path: PathHandle;
  result_path: PathHandle;
}

export interface AgentPlanSavedEvent {
  session_id: ChatSessionId;
  run_id: string;
  package: AgentPlanPackageRef;
  title: string;
  status: string;
  target_path: PathHandle;
  target_type: CadQueryObjectKind;
  affected_files: PathHandle[];
  new_files: PathHandle[];
  change_description: string;
  export_targets: PathHandle[];
}

/** Deprecated: use AgentInvokeRequest { mode: "agent", plan_ref }. */
export interface AgentPlanConfirmRequest {
  session_id: ChatSessionId;
  run_id: string;
  confirmed_cadquery: AgentCadQueryConfirmation;
}

/** Deprecated: use Agent mode chat flow instead. */
export interface AgentPlanRejectRequest {
  session_id: ChatSessionId;
  run_id: string;
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
  | { type: "agent_plan_confirmed"; payload: AgentStartedResponse }
  | { type: "agent_plan_rejected" }
  | { type: "agent_model_registry"; payload: AgentModelRegistryResponse }
  | { type: "agent_snapshot"; payload: AgentSnapshotResponse }
  | { type: "agent_subscribed"; payload: AgentSubscribeResponse }
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
  | { event: "agent.reasoning"; payload: AgentReasoningEvent }
  | { event: "agent.tool_start"; payload: AgentToolStartEvent }
  | { event: "agent.tool_result"; payload: AgentToolResultEvent }
  | { event: "agent.hosted_tool_activity"; payload: AgentHostedToolActivityEvent }
  | { event: "agent.mesh_ready"; payload: AgentMeshReadyEvent }
  | { event: "agent.error"; payload: AgentErrorEvent }
  | { event: "agent.done"; payload: AgentDoneEvent }
  | { event: "agent.plan_proposed"; payload: AgentPlanProposedEvent }
  | { event: "agent.plan_saved"; payload: AgentPlanSavedEvent }
  | { event: "chat.list_changed"; payload: ChatListResponse };

export interface ServerPushEnvelope {
  event: ServerPushEvent;
}

export type ServerEnvelope =
  | { kind: "handshake_ack"; payload: CapabilityHandshakeResponse }
  | { kind: "response"; payload: ServerResponseEnvelope }
  | { kind: "push"; payload: ServerPushEnvelope }
  | { kind: "transport_error"; payload: { message: string } }
  | { kind: "closed" };
