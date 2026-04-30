use std::path::{Path, PathBuf};

use app_server_protocol::{
    AgentMode, CadQueryExportFormat, CadQueryObjectKind, PathHandle, WorkspaceId,
};
use serde_json::Value;

use crate::llm::LlmToolCall;

use super::super::{AgentToolRunContext, CadQueryToolRunRequest, tool_error_json};
use super::support::{SourceContract, is_model_path, source_contract, target_type_label};

#[derive(Debug, Clone)]
pub(super) struct AnalyzeArgs {
    pub(super) target_path: String,
    pub(super) include_paired_doc: bool,
    pub(super) include_dependencies: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ResolveSelectionArgs {
    pub(super) result_id: String,
    pub(super) selection_ref: String,
}

pub(super) fn analyze_args(call: &LlmToolCall) -> Result<AnalyzeArgs, String> {
    let value = parse_object(call)?;
    Ok(AnalyzeArgs {
        target_path: required_string(&value, "target_path", call)?,
        include_paired_doc: bool_arg(&value, "include_paired_doc").unwrap_or(false),
        include_dependencies: bool_arg(&value, "include_dependencies").unwrap_or(false),
    })
}

pub(super) fn source_request_args(call: &LlmToolCall) -> Result<CadQueryToolRunRequest, String> {
    let value = parse_object(call)?;
    let target_path = required_model_path(&value, "target_path", call)?;
    let target_type = target_type_arg(&value, call)?;
    Ok(CadQueryToolRunRequest {
        target_path,
        target_type,
        code: required_string(&value, "code", call)?,
        params_json: optional_string(&value, "params_json").unwrap_or_else(|| "{}".into()),
        export_formats: Vec::new(),
        export_targets: Vec::new(),
        doc_update_path: None,
        plan_ref: optional_string(&value, "plan_ref"),
        reason: optional_string(&value, "reason"),
    })
}

pub(super) fn dry_run_request_args(call: &LlmToolCall) -> Result<CadQueryToolRunRequest, String> {
    let request = source_request_args(call)?;
    validate_params_json(&request.params_json, call)?;
    Ok(request)
}

pub(super) fn execute_request_args(call: &LlmToolCall) -> Result<CadQueryToolRunRequest, String> {
    let value = parse_object(call)?;
    let mut request = source_request_args(call)?;
    request.export_formats = export_formats_arg(&value, call)?;
    request.export_targets = export_targets_arg(&value, call)?;
    validate_params_json(&request.params_json, call)?;
    validate_export_request(&request, call)?;
    Ok(request)
}

pub(super) fn result_id_arg(call: &LlmToolCall) -> Result<String, String> {
    let value = parse_object(call)?;
    required_string(&value, "result_id", call)
}

pub(super) fn resolve_selection_args(call: &LlmToolCall) -> Result<ResolveSelectionArgs, String> {
    let value = parse_object(call)?;
    let selection_ref = required_string(&value, "selection_ref", call)?;
    if selection_ref.starts_with("@selector[") || selection_ref.starts_with("@subshape[") {
        return Err(tool_error_json(
            call,
            "selector and subshape refs are internal and cannot be returned as MVP visible refs",
            "invalid_arguments",
        ));
    }
    Ok(ResolveSelectionArgs {
        result_id: required_string(&value, "result_id", call)?,
        selection_ref,
    })
}

pub(super) fn validate_contract_for_run(
    call: &LlmToolCall,
    request: &CadQueryToolRunRequest,
) -> Result<(), String> {
    let contract = source_contract(&request.target_path, request.target_type, &request.code);
    if contract.has_build_function
        && contract.has_refs
        && contract.target_type_matches
        && contract.unsafe_calls.is_empty()
        && contract.invalid_imports.is_empty()
    {
        Ok(())
    } else {
        Err(tool_error_json(
            call,
            &format!(
                "CadQuery source contract is not satisfied: {}. Required source shape includes a module-level REFS dict with type '{}' and a non-empty \"features\" map chosen from the actual model semantics, plus a build(params=None) function.",
                contract_failure_summary(&contract),
                target_type_label(request.target_type),
            ),
            "invalid_arguments",
        ))
    }
}

pub(super) fn validate_execute_product_contract(
    call: &LlmToolCall,
    request: &CadQueryToolRunRequest,
    contract: &SourceContract,
) -> Result<(), String> {
    if !contract.has_model_description {
        return Err(tool_error_json(
            call,
            "CadQuery model source must include MODEL_DESCRIPTION and MODEL_DETAILS fields purpose, key_dimensions, intended_use, assumptions, interaction_notes, and manufacturing_or_placement_constraints.",
            "invalid_arguments",
        ));
    }
    if request.export_formats.is_empty() || request.export_targets.is_empty() {
        return Err(tool_error_json(
            call,
            "cadquery_execute requires export_formats and export_targets so the committed .py source and derived .step output stay synchronized.",
            "permission_denied",
        ));
    }
    if !request
        .export_formats
        .iter()
        .any(|format| matches!(format, CadQueryExportFormat::Step))
        || !request
            .export_targets
            .iter()
            .any(|target| target.ends_with(".step"))
    {
        return Err(tool_error_json(
            call,
            "cadquery_execute requires a step export format and matching outputs/*.step export target.",
            "permission_denied",
        ));
    }
    Ok(())
}

fn contract_failure_summary(contract: &super::support::SourceContract) -> String {
    let mut missing = Vec::new();
    if !contract.has_build_function {
        missing.push("missing build(params=None)");
    }
    if !contract.has_refs {
        missing.push("missing REFS.features");
    }
    if !contract.target_type_matches {
        missing.push("REFS.type does not match target_type");
    }
    if !contract.invalid_imports.is_empty() {
        missing.push("invalid imports");
    }
    if !contract.unsafe_calls.is_empty() {
        missing.push("unsafe calls");
    }
    missing.join(", ")
}

pub(super) fn validate_execute_scope(
    call: &LlmToolCall,
    request: &CadQueryToolRunRequest,
    context: &AgentToolRunContext,
) -> Result<(), String> {
    if context.mode != AgentMode::Agent {
        return Err(tool_error_json(
            call,
            "cadquery_execute requires Agent mode",
            "permission_denied",
        ));
    }
    let Some(scope) = &context.execution_scope else {
        return Ok(());
    };
    if let Some(target_path) = &scope.target_path
        && target_path != &request.target_path
    {
        return Err(tool_error_json(
            call,
            "target_path does not match execution scope target_path",
            "permission_denied",
        ));
    }
    if let Some(target_type) = scope.target_type
        && target_type != request.target_type
    {
        return Err(tool_error_json(
            call,
            "target_type does not match execution scope target_type",
            "permission_denied",
        ));
    }
    if !scope.affected_files.contains(&request.target_path)
        && !scope.new_files.contains(&request.target_path)
    {
        return Err(tool_error_json(
            call,
            "target_path is outside execution affected_files / new_files",
            "permission_denied",
        ));
    }
    if !scope.export_targets.is_empty() {
        let mut requested = request.export_targets.clone();
        let mut allowed = scope.export_targets.clone();
        requested.sort();
        allowed.sort();
        if requested != allowed {
            return Err(tool_error_json(
                call,
                "export_targets must match execution scope export_targets",
                "permission_denied",
            ));
        }
    }
    if request
        .export_targets
        .iter()
        .any(|target| !scope.export_targets.contains(target))
    {
        return Err(tool_error_json(
            call,
            "export target is outside execution export_targets",
            "permission_denied",
        ));
    }
    Ok(())
}

pub(super) async fn doc_update_path_for_execute(
    workspace_root: &Path,
    call: &LlmToolCall,
    request: &CadQueryToolRunRequest,
    context: &AgentToolRunContext,
) -> Result<Option<String>, String> {
    let Some(path) = request.target_path.strip_suffix(".py") else {
        return Ok(None);
    };
    let doc_path = format!("{path}.md");
    let absolute = workspace_root.join(&doc_path);
    if !tokio::fs::try_exists(&absolute).await.unwrap_or(false) {
        return Ok(None);
    }
    let Some(scope) = &context.execution_scope else {
        return Ok(None);
    };
    if !scope.affected_files.contains(&doc_path) && !scope.new_files.contains(&doc_path) {
        return Err(tool_error_json(
            call,
            "paired CadQuery document must be in execution affected_files / new_files",
            "permission_denied",
        ));
    }
    reject_symlink_workspace_path(workspace_root, &doc_path, call).await?;
    reject_hard_link(&absolute, call).await?;
    Ok(Some(doc_path))
}

pub(super) async fn existing_model_path(
    root: &Path,
    relative: &str,
    call: &LlmToolCall,
) -> Result<PathBuf, String> {
    let handle = path_handle(relative, call)?;
    if !is_model_path(&handle.display_path()) {
        return Err(tool_error_json(
            call,
            "target_path must be a CadQuery .py model source",
            "invalid_arguments",
        ));
    }
    reject_symlink_segments(root, handle.path_segments(), call).await?;
    crate::resolve_workspace_path(root, &handle)
        .await
        .map_err(|error| tool_error_json(call, &error.message, "permission_denied"))
}

async fn reject_symlink_workspace_path(
    root: &Path,
    path: &str,
    call: &LlmToolCall,
) -> Result<(), String> {
    let handle = path_handle(path, call)?;
    reject_symlink_segments(root, handle.path_segments(), call).await
}

async fn reject_symlink_segments(
    root: &Path,
    segments: &[String],
    call: &LlmToolCall,
) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for segment in segments {
        current.push(segment);
        if tokio::fs::symlink_metadata(&current)
            .await
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(tool_error_json(
                call,
                "CadQuery source path must not use symlinks",
                "permission_denied",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
async fn reject_hard_link(path: &Path, call: &LlmToolCall) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    if tokio::fs::metadata(path)
        .await
        .map(|metadata| metadata.nlink() > 1)
        .unwrap_or(false)
    {
        return Err(tool_error_json(
            call,
            "CadQuery document update target must not be a hard link",
            "permission_denied",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
async fn reject_hard_link(_path: &Path, _call: &LlmToolCall) -> Result<(), String> {
    Ok(())
}

fn parse_object(call: &LlmToolCall) -> Result<Value, String> {
    serde_json::from_str(&call.arguments).map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid tool arguments: {error}"),
            "invalid_arguments",
        )
    })
}

fn required_model_path(value: &Value, key: &str, call: &LlmToolCall) -> Result<String, String> {
    let handle = path_handle(&required_string(value, key, call)?, call)?;
    let path = handle.display_path();
    if is_model_path(&path) {
        Ok(path)
    } else {
        Err(tool_error_json(
            call,
            "target_path must be a CadQuery .py model source",
            "invalid_arguments",
        ))
    }
}

fn required_string(value: &Value, key: &str, call: &LlmToolCall) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            tool_error_json(
                call,
                &format!("missing required string argument '{key}'"),
                "invalid_arguments",
            )
        })
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn bool_arg(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn target_type_arg(value: &Value, call: &LlmToolCall) -> Result<CadQueryObjectKind, String> {
    match required_string(value, "target_type", call)?.as_str() {
        "part" => Ok(CadQueryObjectKind::Part),
        "component" => Ok(CadQueryObjectKind::Component),
        "assembly" => Ok(CadQueryObjectKind::Assembly),
        _ => Err(tool_error_json(
            call,
            "target_type must be part, component or assembly",
            "invalid_arguments",
        )),
    }
}

fn export_formats_arg(
    value: &Value,
    call: &LlmToolCall,
) -> Result<Vec<CadQueryExportFormat>, String> {
    value
        .get("export_formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|format| match format.as_str() {
            Some("step") => Ok(CadQueryExportFormat::Step),
            Some("stl") => Ok(CadQueryExportFormat::Stl),
            Some("3mf") => Ok(CadQueryExportFormat::ThreeMf),
            _ => Err(tool_error_json(
                call,
                "export_formats must contain step, stl or 3mf",
                "invalid_arguments",
            )),
        })
        .collect()
}

fn export_targets_arg(value: &Value, call: &LlmToolCall) -> Result<Vec<String>, String> {
    let Some(items) = value.get("export_targets").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .map(|item| {
            let Some(path) = item.as_str() else {
                return Err(tool_error_json(
                    call,
                    "export_targets must be an array of strings",
                    "invalid_arguments",
                ));
            };
            let normalized = path_handle(path, call)?.display_path();
            if normalized.split('/').next() == Some("outputs") {
                Ok(normalized)
            } else {
                Err(tool_error_json(
                    call,
                    "export target must be under outputs/",
                    "permission_denied",
                ))
            }
        })
        .collect()
}

fn validate_export_request(
    request: &CadQueryToolRunRequest,
    call: &LlmToolCall,
) -> Result<(), String> {
    match (
        request.export_formats.is_empty(),
        request.export_targets.is_empty(),
    ) {
        (true, true) => return Ok(()),
        (true, false) => {
            return export_pairing_error(call, "export_targets require export_formats");
        }
        (false, true) => {
            return export_pairing_error(call, "export_formats require export_targets");
        }
        (false, false) => {}
    }
    let target_formats = export_target_formats(&request.export_targets, call)?;
    if !same_formats(&request.export_formats, &target_formats) {
        return export_pairing_error(call, "export_formats must match export_targets extensions");
    }
    if !targets_match_runner_outputs(request) {
        return export_pairing_error(call, "export_targets must match runner output filenames");
    }
    Ok(())
}

fn export_target_formats(
    targets: &[String],
    call: &LlmToolCall,
) -> Result<Vec<CadQueryExportFormat>, String> {
    targets
        .iter()
        .map(|target| export_format_for_target(target, call))
        .collect()
}

fn same_formats(expected: &[CadQueryExportFormat], actual: &[CadQueryExportFormat]) -> bool {
    expected.iter().all(|format| actual.contains(format))
        && actual.iter().all(|format| expected.contains(format))
}

fn targets_match_runner_outputs(request: &CadQueryToolRunRequest) -> bool {
    request.export_targets.iter().all(|target| {
        request
            .export_formats
            .iter()
            .any(|format| target == &expected_export_target(&request.target_path, format))
    })
}

fn export_pairing_error(call: &LlmToolCall, message: &str) -> Result<(), String> {
    Err(tool_error_json(call, message, "permission_denied"))
}

fn export_format_for_target(
    target: &str,
    call: &LlmToolCall,
) -> Result<CadQueryExportFormat, String> {
    let lower = target.to_ascii_lowercase();
    if lower.ends_with(".step") || lower.ends_with(".stp") {
        Ok(CadQueryExportFormat::Step)
    } else if lower.ends_with(".stl") {
        Ok(CadQueryExportFormat::Stl)
    } else if lower.ends_with(".3mf") {
        Ok(CadQueryExportFormat::ThreeMf)
    } else {
        Err(tool_error_json(
            call,
            "export_targets extensions must be step, stl or 3mf",
            "permission_denied",
        ))
    }
}

fn expected_export_target(target_path: &str, format: &CadQueryExportFormat) -> String {
    let stem = Path::new(target_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("cadquery");
    let extension = match format {
        CadQueryExportFormat::Step => "step",
        CadQueryExportFormat::Stl => "stl",
        CadQueryExportFormat::ThreeMf => "3mf",
    };
    format!("outputs/{stem}.{extension}")
}

fn validate_params_json(params: &str, call: &LlmToolCall) -> Result<(), String> {
    serde_json::from_str::<Value>(params)
        .map(|_| ())
        .map_err(|error| {
            tool_error_json(
                call,
                &format!("params_json is invalid: {error}"),
                "invalid_arguments",
            )
        })
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
