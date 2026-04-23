mod app_server_client;
mod config;
mod document;
mod document_session;
mod document_workspace;
mod managed_client;
mod params;
mod presets;
mod preview_state;
mod watch_lifecycle;
mod workspace;

pub use app_server_client::{
    AppServerClient, AppServerClientEvent, AppServerTransportError, AppServerTransportEvent,
    AppServerTransportPort,
};
pub use managed_client::{
    ClientError, ClientEvent, ClientSnapshot, ClientTimeouts, ManagedClient, TransportCloseReason,
    TransportStatus, WatchEventPayload, WatchParams,
};
pub use config::{AppConfig, ConfigError, SlicerConfig};
pub use document::DocumentState;
pub use document_session::{DocumentDescriptor, DocumentKey, DocumentKind};
pub use document_workspace::{DocumentOpenOutcome, DocumentSlot, DocumentTab, DocumentWorkspace};
pub use params::{ParameterStore, parse_parameters};
pub use presets::preset_path_for_source;
pub use preview_state::PreviewState;
pub use watch_lifecycle::{DirectoryWatchLifecycle, WatchLifecycleRequest};
pub use workspace::{remember_workspace, sanitize_recent_workspaces, workspace_name};

pub use app_server_protocol::{
    ExportFormat, ParameterDefinition, ParameterEntry, ParameterKind, ParameterValue,
    ParsedParameters, PresetFile,
};
