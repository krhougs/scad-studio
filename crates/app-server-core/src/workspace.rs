use crate::canonicalize_or_original;
use app_server_protocol::{
    PathHandle, ProtocolError, ProtocolErrorCode, WorkspaceCurrentResponse, WorkspaceEntry,
    WorkspaceEntryKind, WorkspaceId, WorkspaceListResponse,
};
use std::fs;
use std::path::{Path, PathBuf};

pub fn current_workspace(
    workspace_root: &Path,
    workspace_id: WorkspaceId,
) -> WorkspaceCurrentResponse {
    let workspace_root = canonicalize_or_original(workspace_root.to_path_buf());
    let root_name = workspace_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace")
        .to_string();
    WorkspaceCurrentResponse {
        workspace_id,
        root_name,
    }
}

pub fn list_workspace_entries(
    workspace_root: &Path,
    workspace_id: WorkspaceId,
    directory: Option<&PathHandle>,
) -> Result<WorkspaceListResponse, ProtocolError> {
    let workspace_root = canonicalize_or_original(workspace_root.to_path_buf());
    let target_dir = match directory {
        Some(handle) => resolve_workspace_path(&workspace_root, handle)?,
        None => workspace_root.clone(),
    };
    let mut entries = fs::read_dir(&target_dir)
        .map_err(|error| {
            ProtocolError::new(
                ProtocolErrorCode::NotFound,
                format!("读取目录失败: {error}"),
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| {
            let path = canonicalize_or_original(entry.path());
            let relative = path.strip_prefix(&workspace_root).map_err(|_| {
                ProtocolError::new(
                    ProtocolErrorCode::InvalidPathHandle,
                    "路径不在当前 workspace 内",
                )
            })?;
            let segments = relative
                .components()
                .filter_map(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .map(|value| value.to_string())
                })
                .collect::<Vec<_>>();
            let handle = PathHandle::new(workspace_id.clone(), segments).map_err(|error| {
                ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, error.to_string())
            })?;
            let kind = if path.is_dir() {
                WorkspaceEntryKind::Directory
            } else {
                WorkspaceEntryKind::File
            };
            Ok(WorkspaceEntry { path: handle, kind })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    entries.sort_by(|left, right| left.path.display_path().cmp(&right.path.display_path()));
    Ok(WorkspaceListResponse {
        directory: directory.cloned(),
        entries,
    })
}

pub fn resolve_workspace_path(
    workspace_root: &Path,
    handle: &PathHandle,
) -> Result<PathBuf, ProtocolError> {
    let workspace_root = canonicalize_or_original(workspace_root.to_path_buf());
    let mut path = workspace_root.clone();
    for segment in handle.path_segments() {
        path.push(segment);
    }
    let resolved = canonicalize_or_original(path);
    if !resolved.starts_with(&workspace_root) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidPathHandle,
            "路径不在当前 workspace 内",
        ));
    }
    Ok(resolved)
}
