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
  | "invalid_host_local_path";

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
  | { event: "watch.error"; payload: WatchErrorEvent };

export interface ServerPushEnvelope {
  event: ServerPushEvent;
}

export type ServerEnvelope =
  | { kind: "handshake_ack"; payload: CapabilityHandshakeResponse }
  | { kind: "response"; payload: ServerResponseEnvelope }
  | { kind: "push"; payload: ServerPushEnvelope }
  | { kind: "transport_error"; payload: { message: string } }
  | { kind: "closed" };
