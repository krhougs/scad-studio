mod path_policy;

use std::path::Path;
use tokio::fs;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::agent::tools::AgentToolCall;

use super::{AgentToolRunContext, tool_error_json};
use path_policy::{
    ExistingFilePolicy, WriteTarget, WriteTargetPolicy, is_cadquery_model_path, safe_existing_file,
    safe_write_target, validate_existing_affected_scope,
};

pub(super) async fn write_file(
    workspace_root: &Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
) -> String {
    let args = match write_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let target = match safe_write_target(
        workspace_root,
        &args.path,
        call,
        context,
        WriteTargetPolicy::WriteFile,
    )
    .await
    {
        Ok(target) => target,
        Err(result) => return result,
    };
    if let Err(result) = validate_write_conflict(&target, args.expected_hash.as_deref(), call).await
    {
        return result;
    }
    if let Err(result) = validate_text_bytes(args.contents.as_bytes(), call) {
        return result;
    }
    if let Err(error) = fs::write(&target.absolute, args.contents.as_bytes()).await {
        return tool_error_json(call, &format!("写入文件失败: {error}"), "file_conflict");
    }
    file_write_success(call, &args.path, args.contents.as_bytes(), !target.existed).to_string()
}

pub(super) async fn patch_file(
    workspace_root: &Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
) -> String {
    let args = match patch_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let target = match safe_existing_file(
        workspace_root,
        &args.path,
        call,
        ExistingFilePolicy::PatchTarget,
    )
    .await
    {
        Ok(target) => target,
        Err(result) => return result,
    };
    if let Err(result) = validate_existing_affected_scope(&target.relative, context, call) {
        return result;
    }
    let current = match read_text(&target.absolute, call).await {
        Ok(text) => text,
        Err(result) => return result,
    };
    if sha256_text(&current) != args.expected_hash {
        return tool_error_json(
            call,
            "expected_hash does not match current file",
            "file_conflict",
        );
    }
    let patched = match apply_exact_patch(&current, &args.search, &args.replace, call) {
        Ok(text) => text,
        Err(result) => return result,
    };
    if let Err(error) = fs::write(&target.absolute, patched.as_bytes()).await {
        return tool_error_json(call, &format!("写入 patch 失败: {error}"), "file_conflict");
    }
    file_write_success(call, &args.path, patched.as_bytes(), false).to_string()
}

pub(super) async fn copy_file(
    workspace_root: &Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
) -> String {
    let args = match copy_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let source = match safe_existing_file(
        workspace_root,
        &args.source_path,
        call,
        ExistingFilePolicy::CopySource,
    )
    .await
    {
        Ok(path) => path,
        Err(result) => return result,
    };
    let target = match safe_write_target(
        workspace_root,
        &args.target_path,
        call,
        context,
        WriteTargetPolicy::CopyTarget,
    )
    .await
    {
        Ok(target) => target,
        Err(result) => return result,
    };
    if let Err(result) = validate_copy_model_boundary(&source.relative, &target.relative, call) {
        return result;
    }
    let bytes = match read_text_bytes(&source.absolute, call).await {
        Ok(bytes) => bytes,
        Err(result) => return result,
    };
    if let Err(result) = validate_copy_request(
        &source.absolute,
        &target,
        &bytes,
        args.expected_source_hash.as_deref(),
        call,
    ) {
        return result;
    }
    if let Err(error) = fs::write(&target.absolute, &bytes).await {
        return tool_error_json(call, &format!("复制文件失败: {error}"), "file_conflict");
    }
    file_write_success(call, &args.target_path, &bytes, true).to_string()
}

struct WriteArgs {
    path: String,
    contents: String,
    expected_hash: Option<String>,
}

struct PatchArgs {
    path: String,
    expected_hash: String,
    search: String,
    replace: String,
}

struct CopyArgs {
    source_path: String,
    target_path: String,
    expected_source_hash: Option<String>,
}

fn write_args(call: &AgentToolCall) -> Result<WriteArgs, String> {
    let value = parse_object(call)?;
    Ok(WriteArgs {
        path: required_string(&value, "path", call)?,
        contents: string_arg(&value, "contents", call)?,
        expected_hash: optional_string(&value, "expected_hash", call)?,
    })
}

fn patch_args(call: &AgentToolCall) -> Result<PatchArgs, String> {
    let value = parse_object(call)?;
    Ok(PatchArgs {
        path: required_string(&value, "path", call)?,
        expected_hash: required_string(&value, "expected_hash", call)?,
        search: required_string(&value, "search", call)?,
        replace: string_arg(&value, "replace", call)?,
    })
}

fn copy_args(call: &AgentToolCall) -> Result<CopyArgs, String> {
    let value = parse_object(call)?;
    Ok(CopyArgs {
        source_path: required_string(&value, "source_path", call)?,
        target_path: required_string(&value, "target_path", call)?,
        expected_source_hash: optional_string(&value, "expected_source_hash", call)?,
    })
}

fn parse_object(call: &AgentToolCall) -> Result<Value, String> {
    serde_json::from_str(&call.arguments).map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid tool arguments: {error}"),
            "invalid_arguments",
        )
    })
}

fn required_string(value: &Value, key: &str, call: &AgentToolCall) -> Result<String, String> {
    let text = string_arg(value, key, call)?;
    if text.is_empty() {
        Err(tool_error_json(
            call,
            &format!("missing required string argument '{key}'"),
            "invalid_arguments",
        ))
    } else {
        Ok(text.to_owned())
    }
}

fn string_arg(value: &Value, key: &str, call: &AgentToolCall) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            tool_error_json(
                call,
                &format!("missing required string argument '{key}'"),
                "invalid_arguments",
            )
        })
}

fn optional_string(
    value: &Value,
    key: &str,
    call: &AgentToolCall,
) -> Result<Option<String>, String> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|text| Some(text.to_owned()))
        .ok_or_else(|| {
            tool_error_json(
                call,
                &format!("'{key}' must be a string"),
                "invalid_arguments",
            )
        })
}

fn validate_copy_request(
    source: &Path,
    target: &WriteTarget,
    bytes: &[u8],
    expected_hash: Option<&str>,
    call: &AgentToolCall,
) -> Result<(), String> {
    if let Some(expected) = expected_hash
        && sha256_bytes(bytes) != expected
    {
        return Err(tool_error_json(
            call,
            "expected_source_hash does not match source",
            "file_conflict",
        ));
    }
    if source == target.absolute || target.existed {
        return Err(tool_error_json(
            call,
            "copy target already exists or equals source",
            "file_conflict",
        ));
    }
    Ok(())
}

fn validate_copy_model_boundary(
    source: &str,
    target: &str,
    call: &AgentToolCall,
) -> Result<(), String> {
    if is_cadquery_model_path(target) && !is_cadquery_model_path(source) {
        return Err(tool_error_json(
            call,
            "CadQuery model .py copy targets require a CadQuery model .py source",
            "permission_denied",
        ));
    }
    Ok(())
}

async fn validate_write_conflict(
    target: &WriteTarget,
    expected_hash: Option<&str>,
    call: &AgentToolCall,
) -> Result<(), String> {
    if !target.existed {
        return if expected_hash.is_some() {
            Err(tool_error_json(
                call,
                "expected_hash was provided but target does not exist",
                "file_conflict",
            ))
        } else {
            Ok(())
        };
    }
    let Some(expected) = expected_hash else {
        return Err(tool_error_json(
            call,
            "expected_hash is required when overwriting an existing file",
            "file_conflict",
        ));
    };
    let current = read_text_bytes(&target.absolute, call).await?;
    if sha256_bytes(&current) == expected {
        Ok(())
    } else {
        Err(tool_error_json(
            call,
            "expected_hash does not match current file",
            "file_conflict",
        ))
    }
}

async fn read_text(path: &Path, call: &AgentToolCall) -> Result<String, String> {
    String::from_utf8(read_text_bytes(path, call).await?).map_err(|_| {
        tool_error_json(
            call,
            "file must contain valid UTF-8 text",
            "invalid_arguments",
        )
    })
}

async fn read_text_bytes(path: &Path, call: &AgentToolCall) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).await.map_err(|error| {
        tool_error_json(call, &format!("读取文件失败: {error}"), "file_conflict")
    })?;
    validate_text_bytes(&bytes, call)?;
    Ok(bytes)
}

fn validate_text_bytes(bytes: &[u8], call: &AgentToolCall) -> Result<(), String> {
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        Err(tool_error_json(
            call,
            "file contents must be UTF-8 text",
            "invalid_arguments",
        ))
    } else {
        Ok(())
    }
}

fn apply_exact_patch(
    current: &str,
    search: &str,
    replace: &str,
    call: &AgentToolCall,
) -> Result<String, String> {
    validate_text_bytes(search.as_bytes(), call)?;
    validate_text_bytes(replace.as_bytes(), call)?;
    let matches = current.match_indices(search).count();
    if matches != 1 {
        return Err(tool_error_json(
            call,
            "patch search text must match exactly once",
            "file_conflict",
        ));
    }
    Ok(current.replacen(search, replace, 1))
}

fn file_write_success(call: &AgentToolCall, path: &str, bytes: &[u8], created: bool) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "file write completed",
        "path": path,
        "hash": sha256_bytes(bytes),
        "created": created,
        "conflict": false
    })
}

fn sha256_text(text: &str) -> String {
    sha256_bytes(text.as_bytes())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
