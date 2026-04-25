mod codec;
mod export;
mod params;
mod path;
mod presets;
mod protocol;

pub use codec::{
    WIRE_MAGIC, WIRE_VERSION, decode_client_frame, decode_server_frame, encode_client_frame,
    encode_server_frame, validate_f32,
};
pub use export::ExportFormat;
pub use params::{
    ParameterDefinition, ParameterEntry, ParameterKind, ParameterValue, ParsedParameters,
};
pub use path::{PathHandle, PathHandleValidationError, WorkspaceId};
pub use presets::PresetFile;
pub use protocol::{
    AppConfigDto, CancelRequest, CapabilityHandshakeRequest, CapabilityHandshakeResponse,
    ClientCapabilities, ClientCommand, ClientEnvelope, ClientPlatform, ClientRequestEnvelope,
    CommandSuccess, ConfigLoadResponse, ConfigSaveRequest, DEFAULT_SESSION_RECONNECT_WINDOW_MS,
    DisplayUnitDto, ExportRunRequest, ExportRunResponse, FileReadCapability, FileReadContents,
    FileReadRequest, FileReadResponse, FileWriteTextRequest, FileWriteTextResponse, HostLocalPath,
    PreviewArtifact, PreviewArtifact3mf, PreviewArtifactStl, PreviewMeshPayload,
    PreviewReadyResponse, PreviewRenderedImagePayload, PreviewRequest, PreviewRequestKind,
    PreviewResponseFormat, PreviewUnit, ProtocolError, ProtocolErrorCode, ProtocolVersionRange,
    RequestId, ServerCapabilities, ServerEnvelope, ServerPushEnvelope, ServerPushEvent,
    ServerResponseEnvelope, SessionReclaimRequest, SessionReclaimedResponse, SessionToken,
    SlicerConfigDto, SlicerInstallRecord, SlicerListRequest, SlicerListResponse, SubscriptionId,
    TransportErrorFrame, WEB_FILE_READ_DENY_EXTENSIONS, WatchChangedEvent, WatchErrorEvent,
    WatchSubscribeRequest, WatchSubscriptionAck, WatchUnsubscribeRequest, WorkspaceCurrentResponse,
    WorkspaceEntry, WorkspaceEntryKind, WorkspaceListRequest, WorkspaceListResponse,
    WorkspacePortablePath, negotiate_protocol_version, web_file_read_capability,
};
