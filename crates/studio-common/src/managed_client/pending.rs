use app_server_protocol::{PathHandle, RequestId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingKind {
    WorkspaceCurrent,
    WorkspaceList,
    Preview { target: PathHandle },
    FileRead,
    FileWriteText,
    ConfigLoad,
    ConfigSave,
    SlicerList,
    ExportRun,
    Cancel { target: RequestId },
}

#[derive(Debug, Clone)]
pub struct PendingRequestInfo {
    pub kind: PendingKind,
    pub deadline_ms: Option<u64>,
    #[allow(dead_code)]
    pub issued_at_ms: u64,
    pub envelope_bytes: Vec<u8>,
}
