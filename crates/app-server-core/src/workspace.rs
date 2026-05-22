use crate::canonicalize_or_original;
use app_server_protocol::{
    PathHandle, ProtocolError, ProtocolErrorCode, WorkspaceCurrentResponse, WorkspaceEntry,
    WorkspaceEntryKind, WorkspaceId, WorkspaceListResponse,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn current_workspace(
    workspace_root: &Path,
    workspace_id: WorkspaceId,
) -> WorkspaceCurrentResponse {
    current_workspace_owned(workspace_root.to_path_buf(), workspace_id).await
}

pub async fn current_workspace_owned(
    workspace_root: PathBuf,
    workspace_id: WorkspaceId,
) -> WorkspaceCurrentResponse {
    let workspace_root = canonicalize_or_original(workspace_root).await;
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

pub async fn list_workspace_entries(
    workspace_root: &Path,
    workspace_id: WorkspaceId,
    directory: Option<&PathHandle>,
) -> Result<WorkspaceListResponse, ProtocolError> {
    list_workspace_entries_owned(
        workspace_root.to_path_buf(),
        workspace_id,
        directory.cloned(),
    )
    .await
}

pub async fn list_workspace_entries_owned(
    workspace_root: PathBuf,
    workspace_id: WorkspaceId,
    directory: Option<PathHandle>,
) -> Result<WorkspaceListResponse, ProtocolError> {
    let workspace_root = canonicalize_or_original(workspace_root).await;
    let target_dir = match &directory {
        Some(handle) => {
            resolve_workspace_path_owned(workspace_root.clone(), handle.clone()).await?
        }
        None => workspace_root.clone(),
    };
    let mut read_dir = fs::read_dir(target_dir).await.map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("读取目录失败: {error}"),
        )
    })?;
    let mut entries = Vec::new();
    while let Some(entry) = read_dir.next_entry().await.map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("读取目录失败: {error}"),
        )
    })? {
        entries.push(
            build_workspace_entry(workspace_root.clone(), workspace_id.clone(), entry).await?,
        );
    }
    mark_case_conflicts(&mut entries);
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(WorkspaceListResponse { directory, entries })
}

async fn build_workspace_entry(
    workspace_root: PathBuf,
    workspace_id: WorkspaceId,
    entry: fs::DirEntry,
) -> Result<WorkspaceEntry, ProtocolError> {
    let name = entry.file_name().to_string_lossy().to_string();
    let raw_path = entry.path();
    let path = canonicalize_or_original(raw_path.clone()).await;
    let file_type = entry.file_type().await.map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("读取目录项失败: {error}"),
        )
    })?;
    let kind = if file_type.is_dir() {
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

pub async fn resolve_workspace_path(
    workspace_root: &Path,
    handle: &PathHandle,
) -> Result<PathBuf, ProtocolError> {
    resolve_workspace_path_owned(workspace_root.to_path_buf(), handle.clone()).await
}

pub async fn resolve_workspace_path_owned(
    workspace_root: PathBuf,
    handle: PathHandle,
) -> Result<PathBuf, ProtocolError> {
    let workspace_root = canonicalize_or_original(workspace_root).await;
    let mut path = workspace_root.clone();
    for segment in handle.path_segments() {
        path.push(segment);
    }
    let resolved = canonicalize_or_original(path).await;
    if !resolved.starts_with(&workspace_root) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidPathHandle,
            "路径不在当前 workspace 内",
        ));
    }
    Ok(resolved)
}

pub async fn resolve_workspace_write_path(
    workspace_root: &Path,
    handle: &PathHandle,
) -> Result<PathBuf, ProtocolError> {
    resolve_workspace_write_path_owned(workspace_root.to_path_buf(), handle.clone()).await
}

pub async fn resolve_workspace_write_path_owned(
    workspace_root: PathBuf,
    handle: PathHandle,
) -> Result<PathBuf, ProtocolError> {
    let workspace_root = canonicalize_or_original(workspace_root).await;
    let mut path = workspace_root.clone();
    for segment in handle.path_segments() {
        path.push(segment);
    }
    let parent = path
        .parent()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| {
            ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, "写入路径缺少父目录")
        })?;
    let parent = fs::canonicalize(parent).await.map_err(|error| {
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
