use crate::canonicalize_or_original;
use app_server_protocol::{
    PathHandle, ProtocolError, ProtocolErrorCode, WorkspaceCurrentResponse, WorkspaceEntry,
    WorkspaceEntryKind, WorkspaceId, WorkspaceListResponse,
};
use std::collections::HashMap;
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
        .map(|entry| build_workspace_entry(&workspace_root, &workspace_id, entry))
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    mark_case_conflicts(&mut entries);
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(WorkspaceListResponse {
        directory: directory.cloned(),
        entries,
    })
}

fn build_workspace_entry(
    workspace_root: &Path,
    workspace_id: &WorkspaceId,
    entry: fs::DirEntry,
) -> Result<WorkspaceEntry, ProtocolError> {
    let name = entry.file_name().to_string_lossy().to_string();
    let raw_path = entry.path();
    let path = canonicalize_or_original(raw_path.clone());
    let kind = if path.is_dir() {
        WorkspaceEntryKind::Directory
    } else {
        WorkspaceEntryKind::File
    };
    let relative = match path.strip_prefix(workspace_root) {
        Ok(relative) => relative,
        Err(_) => {
            return Ok(WorkspaceEntry {
                name,
                path: None,
                kind,
                path_error: Some("路径不在当前 workspace 内".into()),
            });
        }
    };
    let segments = match portable_segments(relative) {
        Ok(segments) => segments,
        Err(error) => {
            return Ok(WorkspaceEntry {
                name,
                path: None,
                kind,
                path_error: Some(error),
            });
        }
    };
    match PathHandle::new(workspace_id.clone(), segments) {
        Ok(handle) => Ok(WorkspaceEntry {
            name,
            path: Some(handle),
            kind,
            path_error: None,
        }),
        Err(error) => Ok(WorkspaceEntry {
            name,
            path: None,
            kind,
            path_error: Some(error.to_string()),
        }),
    }
}

fn portable_segments(relative: &Path) -> Result<Vec<String>, String> {
    let mut segments = Vec::new();
    for component in relative.components() {
        let value = component
            .as_os_str()
            .to_str()
            .ok_or_else(|| "path component must be UTF-8".to_string())?;
        if value.contains('\u{fffd}') {
            return Err("path component must be UTF-8".into());
        }
        segments.push(value.to_string());
    }
    Ok(segments)
}

fn mark_case_conflicts(entries: &mut [WorkspaceEntry]) {
    let mut counts = HashMap::<String, usize>::new();
    for entry in entries.iter().filter_map(|entry| entry.path.as_ref()) {
        *counts.entry(entry.case_fold_key()).or_default() += 1;
    }
    for entry in entries {
        let Some(path) = &entry.path else {
            continue;
        };
        if counts.get(&path.case_fold_key()).copied().unwrap_or(0) > 1 {
            entry.path = None;
            entry.path_error = Some("case-insensitive path conflict".into());
        }
    }
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

pub fn resolve_workspace_write_path(
    workspace_root: &Path,
    handle: &PathHandle,
) -> Result<PathBuf, ProtocolError> {
    let workspace_root = canonicalize_or_original(workspace_root.to_path_buf());
    let mut path = workspace_root.clone();
    for segment in handle.path_segments() {
        path.push(segment);
    }
    let parent = path.parent().ok_or_else(|| {
        ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, "写入路径缺少父目录")
    })?;
    let parent = parent.canonicalize().map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidPathHandle,
            format!("写入路径父目录不存在或不可访问: {error}"),
        )
    })?;
    if !parent.starts_with(&workspace_root) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidPathHandle,
            "写入路径父目录不在当前 workspace 内",
        ));
    }
    let file_name = path.file_name().ok_or_else(|| {
        ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, "写入路径缺少文件名")
    })?;
    Ok(parent.join(file_name))
}
