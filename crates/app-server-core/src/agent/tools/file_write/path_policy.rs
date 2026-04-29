use std::{
    fs,
    path::{Path, PathBuf},
};

use app_server_protocol::{PathHandle, WorkspaceId};

use crate::llm::LlmToolCall;

use super::super::{AgentExecutionScope, AgentToolRunContext, tool_error_json};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

const WRITE_ALLOWED_ROOTS: &[&str] = &["components", "parts", "assemblies", "refs", "docs"];
const WRITE_DENIED_ROOTS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "outputs",
    ".budn_staging",
    "chats",
];

pub(super) struct WriteTarget {
    pub(super) absolute: PathBuf,
    pub(super) relative: String,
    pub(super) existed: bool,
}

pub(super) struct ExistingFile {
    pub(super) absolute: PathBuf,
    pub(super) relative: String,
}

#[derive(Clone, Copy)]
pub(super) enum WriteTargetPolicy {
    WriteFile,
    CopyTarget,
}

#[derive(Clone, Copy)]
pub(super) enum ExistingFilePolicy {
    PatchTarget,
    CopySource,
}

pub(super) fn safe_write_target(
    root: &Path,
    path: &str,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
    policy: WriteTargetPolicy,
) -> Result<WriteTarget, String> {
    let absolute = resolve_write_path(root, path, call)?;
    let relative = workspace_relative_path(root, &absolute, call)?;
    validate_actual_write_path(&relative, policy.allows_model_target(), call)?;
    let existed = target_status(&absolute, call)?;
    validate_write_execution_scope(
        &relative,
        existed,
        context.execution_scope.as_ref(),
        policy,
        call,
    )?;
    Ok(WriteTarget {
        absolute,
        relative,
        existed,
    })
}

pub(super) fn safe_existing_file(
    root: &Path,
    path: &str,
    call: &LlmToolCall,
    policy: ExistingFilePolicy,
) -> Result<ExistingFile, String> {
    let handle = path_handle(path, call)?;
    let literal = literal_workspace_path(root, &handle);
    validate_existing_literal_file(&literal, call)?;
    let absolute = crate::resolve_workspace_path(root, &handle)
        .map_err(|error| tool_error_json(call, &error.message, "permission_denied"))?;
    let relative = workspace_relative_path(root, &absolute, call)?;
    validate_actual_write_path(&relative, policy.allows_model_source(), call)?;
    Ok(ExistingFile { absolute, relative })
}

pub(super) fn validate_existing_affected_scope(
    relative: &str,
    context: &AgentToolRunContext,
    call: &LlmToolCall,
) -> Result<(), String> {
    let Some(scope) = context.execution_scope.as_ref() else {
        return Ok(());
    };
    if scope.contains_affected_file(relative) {
        return Ok(());
    }
    if scope.contains_new_file(relative) {
        return Err(tool_error_json(
            call,
            "execution new_files cannot patch existing files",
            "file_conflict",
        ));
    }
    Err(tool_error_json(
        call,
        "path is outside execution scope",
        "permission_denied",
    ))
}

fn target_status(path: &Path, call: &LlmToolCall) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(tool_error_json(
            call,
            "target path must not be a symlink",
            "permission_denied",
        )),
        Ok(metadata) if metadata.is_file() => {
            validate_no_hard_link_alias(&metadata, call)?;
            Ok(true)
        }
        Ok(_) => Err(tool_error_json(
            call,
            "target path must be a file",
            "invalid_arguments",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(tool_error_json(
            call,
            &format!("读取目标路径失败: {error}"),
            "file_conflict",
        )),
    }
}

fn validate_existing_literal_file(path: &Path, call: &LlmToolCall) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(tool_error_json(
            call,
            "source path must not be a symlink",
            "permission_denied",
        )),
        Ok(metadata) if metadata.is_file() => validate_no_hard_link_alias(&metadata, call),
        Ok(_) => Err(tool_error_json(
            call,
            "source path must be a file",
            "invalid_arguments",
        )),
        Err(error) => Err(tool_error_json(
            call,
            &format!("读取源文件失败: {error}"),
            "not_found",
        )),
    }
}

#[cfg(unix)]
fn validate_no_hard_link_alias(
    metadata: &std::fs::Metadata,
    call: &LlmToolCall,
) -> Result<(), String> {
    if metadata.nlink() > 1 {
        return Err(tool_error_json(
            call,
            "file write tools reject hard-linked files",
            "permission_denied",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_no_hard_link_alias(
    _metadata: &std::fs::Metadata,
    _call: &LlmToolCall,
) -> Result<(), String> {
    Ok(())
}

fn resolve_write_path(root: &Path, path: &str, call: &LlmToolCall) -> Result<PathBuf, String> {
    let handle = path_handle(path, call)?;
    crate::resolve_workspace_write_path(root, &handle)
        .map_err(|error| tool_error_json(call, &error.message, "invalid_arguments"))
}

fn path_handle(path: &str, call: &LlmToolCall) -> Result<PathHandle, String> {
    PathHandle::new(
        WorkspaceId::new("workspace"),
        path.split('/').map(str::to_owned),
    )
    .map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid workspace path: {error}"),
            "invalid_arguments",
        )
    })
}

fn literal_workspace_path(root: &Path, handle: &PathHandle) -> PathBuf {
    let mut path = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    for segment in handle.path_segments() {
        path.push(segment);
    }
    path
}

fn workspace_relative_path(
    root: &Path,
    absolute: &Path,
    call: &LlmToolCall,
) -> Result<String, String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    absolute
        .strip_prefix(&root)
        .ok()
        .and_then(|relative| {
            relative
                .components()
                .map(|component| component.as_os_str().to_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .map(|segments| segments.join("/"))
        .ok_or_else(|| {
            tool_error_json(call, "path resolves outside workspace", "permission_denied")
        })
}

fn validate_actual_write_path(
    relative: &str,
    allow_cadquery_model: bool,
    call: &LlmToolCall,
) -> Result<(), String> {
    let root = first_path_segment(relative);
    if WRITE_DENIED_ROOTS.contains(&root) || !WRITE_ALLOWED_ROOTS.contains(&root) {
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is not allowed for file write tools"),
            "permission_denied",
        ));
    }
    if !allow_cadquery_model && is_cadquery_model_path(relative) {
        return Err(tool_error_json(
            call,
            "CadQuery model .py files must be modified through CadQuery tools",
            "permission_denied",
        ));
    }
    Ok(())
}

fn validate_write_execution_scope(
    relative: &str,
    existed: bool,
    scope: Option<&AgentExecutionScope>,
    policy: WriteTargetPolicy,
    call: &LlmToolCall,
) -> Result<(), String> {
    let Some(scope) = scope else {
        return Ok(());
    };
    match policy {
        WriteTargetPolicy::WriteFile => validate_write_file_scope(relative, existed, scope, call),
        WriteTargetPolicy::CopyTarget => validate_copy_target_scope(relative, scope, call),
    }
}

fn validate_write_file_scope(
    relative: &str,
    existed: bool,
    scope: &AgentExecutionScope,
    call: &LlmToolCall,
) -> Result<(), String> {
    if existed && scope.contains_affected_file(relative) {
        return Ok(());
    }
    if !existed && scope.contains_new_file(relative) {
        return Ok(());
    }
    if scope.contains_new_file(relative) || scope.contains_affected_file(relative) {
        return Err(tool_error_json(
            call,
            "execution scope file state does not match workspace",
            "file_conflict",
        ));
    }
    Err(tool_error_json(
        call,
        "path is outside execution scope",
        "permission_denied",
    ))
}

fn validate_copy_target_scope(
    relative: &str,
    scope: &AgentExecutionScope,
    call: &LlmToolCall,
) -> Result<(), String> {
    if scope.contains_new_file(relative) {
        Ok(())
    } else {
        Err(tool_error_json(
            call,
            "copy_file target must be in execution new_files",
            "permission_denied",
        ))
    }
}

fn first_path_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}

pub(super) fn is_cadquery_model_path(path: &str) -> bool {
    matches!(
        first_path_segment(path),
        "components" | "parts" | "assemblies"
    ) && path.ends_with(".py")
}

impl WriteTargetPolicy {
    fn allows_model_target(self) -> bool {
        matches!(self, WriteTargetPolicy::CopyTarget)
    }
}

impl ExistingFilePolicy {
    fn allows_model_source(self) -> bool {
        matches!(self, ExistingFilePolicy::CopySource)
    }
}
