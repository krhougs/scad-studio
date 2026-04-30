use std::path::Path;

use app_server_protocol::{CadQueryObjectKind, WorkspaceId};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::agent::plan_package::{SaveCadPlanPackageInput, save_plan_package};

use super::{AgentToolCall, AgentToolRunContext, semantic_export, tool_error_json};

const DENIED_RELATION_ROOTS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "chats",
    "outputs",
    ".budn_staging",
];
pub(super) const PLAN_SCOPE_ROOTS: &[&str] =
    &["components", "parts", "assemblies", "plans", "refs", "docs"];

pub(super) async fn save_cad_plan(
    workspace_root: &Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
) -> String {
    let mut args = match save_plan_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    args.source_chat_session = context.session_id.as_ref().map(|session| session.0.clone());
    let saved = match save_plan_package(workspace_root, &args).await {
        Ok(saved) => saved,
        Err(error) => return tool_error_json(call, &error.message, error.error_type),
    };
    save_plan_success(call, context, args, saved).to_string()
}

type SavePlanArgs = SaveCadPlanPackageInput;

fn save_plan_args(call: &AgentToolCall) -> Result<SavePlanArgs, String> {
    let value = parse_object(call)?;
    let target_path = cadquery_target_arg(&value, "target_path", call)?;
    let affected_files = plan_scope_paths(&value, "affected_files", call)?;
    let args = SavePlanArgs {
        title: non_empty_string_arg(&value, "title", call)?,
        request: non_empty_string_arg(&value, "request", call)?,
        target_ref: non_empty_string_arg(&value, "target_ref", call)?,
        target_path,
        target_type: target_type_arg(&value, call)?,
        affected_files,
        new_files: optional_plan_scope_paths(&value, "new_files", call)?,
        export_targets: export_targets(&value, call)?,
        strategy: non_empty_string_arg(&value, "strategy", call)?,
        risks: optional_string_array(&value, "risks", call)?,
        acceptance: optional_string_array(&value, "acceptance", call)?,
        execution_scope: non_empty_string_arg(&value, "execution_scope", call)?,
        source_chat_session: None,
    };
    validate_target_type_matches_path(&args, call)?;
    validate_plan_execution_scope(&args, call)?;
    semantic_export::validate_plan_export_targets(&args.target_path, &args.export_targets, call)?;
    Ok(args)
}

pub(super) fn parse_object(call: &AgentToolCall) -> Result<Value, String> {
    serde_json::from_str(&call.arguments).map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid tool arguments: {error}"),
            "invalid_arguments",
        )
    })
}

pub(super) fn non_empty_string_arg(
    value: &Value,
    key: &str,
    call: &AgentToolCall,
) -> Result<String, String> {
    let text = value.get(key).and_then(Value::as_str).unwrap_or("").trim();
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

fn plan_scope_paths(value: &Value, key: &str, call: &AgentToolCall) -> Result<Vec<String>, String> {
    let paths = optional_plan_scope_paths(value, key, call)?;
    if paths.is_empty() {
        Err(tool_error_json(
            call,
            &format!("'{key}' must contain at least one path"),
            "invalid_arguments",
        ))
    } else {
        Ok(paths)
    }
}

fn optional_plan_scope_paths(
    value: &Value,
    key: &str,
    call: &AgentToolCall,
) -> Result<Vec<String>, String> {
    optional_string_array(value, key, call)?
        .into_iter()
        .map(|path| normalize_allowed_path(&path, PLAN_SCOPE_ROOTS, call))
        .collect()
}

fn export_targets(value: &Value, call: &AgentToolCall) -> Result<Vec<String>, String> {
    let targets = optional_string_array(value, "export_targets", call)?
        .into_iter()
        .map(|path| normalize_export_target(&path, call))
        .collect::<Result<Vec<_>, _>>()?;
    if targets.is_empty() {
        return Err(tool_error_json(
            call,
            "'export_targets' must contain at least one path",
            "invalid_arguments",
        ));
    }
    Ok(targets)
}

fn validate_plan_execution_scope(args: &SavePlanArgs, call: &AgentToolCall) -> Result<(), String> {
    if args
        .affected_files
        .iter()
        .any(|path| path == &args.target_path)
        || args.new_files.iter().any(|path| path == &args.target_path)
    {
        return Ok(());
    }
    Err(tool_error_json(
        call,
        "target_path must be included in affected_files or new_files",
        "invalid_arguments",
    ))
}

fn cadquery_target_arg(value: &Value, key: &str, call: &AgentToolCall) -> Result<String, String> {
    let path = non_empty_string_arg(value, key, call)?;
    let normalized = normalize_allowed_path(&path, &["components", "parts", "assemblies"], call)?;
    if !normalized.ends_with(".py") {
        return Err(tool_error_json(
            call,
            "target_path must be a CadQuery .py model source",
            "invalid_arguments",
        ));
    }
    Ok(normalized)
}

fn target_type_arg(value: &Value, call: &AgentToolCall) -> Result<CadQueryObjectKind, String> {
    match non_empty_string_arg(value, "target_type", call)?.as_str() {
        "assembly" => Ok(CadQueryObjectKind::Assembly),
        "component" => Ok(CadQueryObjectKind::Component),
        "part" => Ok(CadQueryObjectKind::Part),
        _ => Err(tool_error_json(
            call,
            "target_type must be part, component, or assembly",
            "invalid_arguments",
        )),
    }
}

fn validate_target_type_matches_path(
    args: &SavePlanArgs,
    call: &AgentToolCall,
) -> Result<(), String> {
    let expected = match first_segment(&args.target_path) {
        "assemblies" => CadQueryObjectKind::Assembly,
        "components" => CadQueryObjectKind::Component,
        _ => CadQueryObjectKind::Part,
    };
    if args.target_type == expected {
        Ok(())
    } else {
        Err(tool_error_json(
            call,
            "target_type does not match target_path",
            "invalid_arguments",
        ))
    }
}

pub(super) fn optional_string_array(
    value: &Value,
    key: &str,
    call: &AgentToolCall,
) -> Result<Vec<String>, String> {
    let Some(raw) = value.get(key) else {
        return Ok(Vec::new());
    };
    let Some(array) = raw.as_array() else {
        return Err(tool_error_json(
            call,
            &format!("'{key}' must be an array of strings"),
            "invalid_arguments",
        ));
    };
    array
        .iter()
        .map(|entry| {
            entry.as_str().map(str::to_owned).ok_or_else(|| {
                tool_error_json(
                    call,
                    &format!("'{key}' must be an array of strings"),
                    "invalid_arguments",
                )
            })
        })
        .collect()
}

pub(super) fn path_handle(
    path: &str,
    call: &AgentToolCall,
) -> Result<app_server_protocol::PathHandle, String> {
    app_server_protocol::PathHandle::new(
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

fn normalize_export_target(path: &str, call: &AgentToolCall) -> Result<String, String> {
    let normalized = normalize_workspace_path(path, call)?;
    if first_segment(&normalized) != "outputs" {
        return Err(tool_error_json(
            call,
            "export_targets must be under outputs/",
            "permission_denied",
        ));
    }
    if supported_export_extension(&normalized) {
        Ok(normalized)
    } else {
        Err(tool_error_json(
            call,
            "export_targets must use .step, .stl, or .3mf",
            "invalid_arguments",
        ))
    }
}

fn supported_export_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".step") || lower.ends_with(".stl") || lower.ends_with(".3mf")
}

pub(super) fn normalize_allowed_path(
    path: &str,
    allowed_roots: &[&str],
    call: &AgentToolCall,
) -> Result<String, String> {
    let normalized = normalize_workspace_path(path, call)?;
    let root = first_segment(&normalized);
    if DENIED_RELATION_ROOTS.contains(&root) {
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is denied for this tool"),
            "permission_denied",
        ));
    }
    if allowed_roots.iter().any(|allowed| *allowed == root) {
        Ok(normalized)
    } else {
        Err(tool_error_json(
            call,
            &format!("path root '{root}' is not allowed for this tool"),
            "permission_denied",
        ))
    }
}

pub(super) fn normalize_workspace_path(path: &str, call: &AgentToolCall) -> Result<String, String> {
    let cleaned = path.trim().replace('\\', "/");
    if cleaned.is_empty() || cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(tool_error_json(
            call,
            "path must be workspace-relative",
            "permission_denied",
        ));
    }
    if cleaned.split('/').any(|segment| segment == "..") {
        return Err(tool_error_json(
            call,
            "path must not contain '..'",
            "permission_denied",
        ));
    }
    Ok(cleaned
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

fn save_plan_success(
    call: &AgentToolCall,
    context: &AgentToolRunContext,
    args: SavePlanArgs,
    saved: crate::agent::plan_package::SavedPlanPackage,
) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CAD Plan package saved",
        "plan_id": saved.paths.plan_id,
        "plan_ref": saved.paths.plan_ref,
        "request_path": saved.paths.request_path,
        "plan_path": saved.paths.plan_path,
        "result_path": saved.paths.result_path,
        "hash": sha256_text(&saved.hash_source),
        "summary": args.strategy,
        "target_path": args.target_path,
        "target_type": target_type_label(args.target_type),
        "affected_files": args.affected_files,
        "new_files": args.new_files,
        "export_targets": args.export_targets,
        "plan_status": saved.plan_status,
        "run_id": context.run_id
    })
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn target_type_label(target_type: CadQueryObjectKind) -> &'static str {
    match target_type {
        CadQueryObjectKind::Assembly => "assembly",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Part => "part",
    }
}

pub(super) fn first_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}
