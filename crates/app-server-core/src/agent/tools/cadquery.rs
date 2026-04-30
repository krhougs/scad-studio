mod args;
mod support;

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use serde_json::json;

use crate::agent::tools::AgentToolCall;

use super::{AgentToolRunContext, CadQueryModelContract, CadQueryToolRuntime, tool_error_json};
use args::{
    analyze_args, doc_update_path_for_execute, dry_run_request_args, execute_request_args,
    existing_model_path, resolve_selection_args, result_id_arg, source_request_args,
    validate_contract_for_run, validate_execute_product_contract, validate_execute_scope,
};
use support::{
    analyze_success, contract_json, contract_warnings, resolve_selection_success, result_success,
    run_success, source_contract, target_type_from_path,
};

pub(super) async fn analyze_source(
    workspace_root: &Path,
    call: &AgentToolCall,
    runtime: Option<&dyn CadQueryToolRuntime>,
) -> String {
    let args = match analyze_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let source_path = match existing_model_path(workspace_root, &args.target_path, call).await {
        Ok(path) => path,
        Err(result) => return result,
    };
    let source = match fs::read_to_string(&source_path).await {
        Ok(source) => source,
        Err(error) => {
            return tool_error_json(
                call,
                &format!("读取 CadQuery 源码失败: {error}"),
                "not_found",
            );
        }
    };
    let request = super::CadQueryToolRunRequest {
        target_path: args.target_path.clone(),
        target_type: target_type_from_path(&args.target_path),
        code: source.clone(),
        params_json: "{}".into(),
        export_formats: Vec::new(),
        export_targets: Vec::new(),
        doc_update_path: None,
        plan_ref: None,
        reason: None,
    };
    let contract = match source_contract_for_runtime(call, &request, runtime).await {
        Ok(contract) => contract,
        Err(result) => return result,
    };
    analyze_success(
        workspace_root,
        call,
        &args.target_path,
        args.include_paired_doc,
        args.include_dependencies,
        &source,
        contract.has_model_description,
    )
    .await
    .to_string()
}

pub(super) async fn check_source(
    call: &AgentToolCall,
    runtime: Option<&dyn CadQueryToolRuntime>,
) -> String {
    let request = match source_request_args(call) {
        Ok(request) => request,
        Err(result) => return result,
    };
    let contract = match source_contract_for_runtime(call, &request, runtime).await {
        Ok(contract) => contract,
        Err(result) => return result,
    };
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery source contract checked",
        "contract": contract_json(&contract),
        "warnings": contract_warnings(&contract)
    })
    .to_string()
}

pub(super) async fn dry_run(
    _workspace_root: &Path,
    call: &AgentToolCall,
    runtime: Option<&dyn CadQueryToolRuntime>,
) -> String {
    let request = match dry_run_request_args(call) {
        Ok(request) => request,
        Err(result) => return result,
    };
    if let Err(result) = validate_contract_for_run(call, &request) {
        return result;
    }
    let Some(runtime) = runtime else {
        return tool_error_json(
            call,
            "CadQuery runtime is not configured",
            "permission_denied",
        );
    };
    match runtime.dry_run(request).await {
        Ok(result) => run_success(call, result, false).to_string(),
        Err(error) => {
            runtime_error_json(call, error.error_type, error.message, error.retry_allowed)
        }
    }
}

pub(super) async fn execute(
    _workspace_root: &Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
    runtime: Option<&dyn CadQueryToolRuntime>,
    committed: &AtomicBool,
) -> String {
    if committed.load(Ordering::SeqCst) {
        return record_plan_failure_for_tool_error(
            _workspace_root,
            context,
            tool_error_json(
                call,
                "cadquery_execute already committed in this run",
                "permission_denied",
            ),
        )
        .await;
    }
    let mut request = match execute_request_args(call) {
        Ok(request) => request,
        Err(result) => {
            return record_plan_failure_for_tool_error(_workspace_root, context, result).await;
        }
    };
    if let Err(result) = validate_contract_for_run(call, &request) {
        return record_plan_failure_for_tool_error(_workspace_root, context, result).await;
    }
    if let Err(result) = validate_execute_scope(call, &request, context) {
        return record_plan_failure_for_tool_error(_workspace_root, context, result).await;
    }
    let Some(runtime) = runtime else {
        return record_plan_failure_for_tool_error(
            _workspace_root,
            context,
            tool_error_json(
                call,
                "CadQuery runtime is not configured",
                "permission_denied",
            ),
        )
        .await;
    };
    let contract = match source_contract_for_runtime(call, &request, Some(runtime)).await {
        Ok(contract) => contract,
        Err(result) => {
            return record_plan_failure_for_tool_error(_workspace_root, context, result).await;
        }
    };
    if let Err(result) = validate_execute_product_contract(call, &request, &contract) {
        return record_plan_failure_for_tool_error(_workspace_root, context, result).await;
    }
    match doc_update_path_for_execute(_workspace_root, call, &request, context).await {
        Ok(doc_update_path) => request.doc_update_path = doc_update_path,
        Err(result) => {
            return record_plan_failure_for_tool_error(_workspace_root, context, result).await;
        }
    }
    match runtime.execute(request).await {
        Ok(result) => {
            let plan_result_path = context
                .execution_scope
                .as_ref()
                .and_then(|scope| scope.plan_result_path.clone());
            let plan_result_warning = match plan_result_path.as_deref() {
                Some(path) => append_plan_result_success(_workspace_root, path, context, &result)
                    .await
                    .err(),
                None => None,
            };
            committed.store(true, Ordering::SeqCst);
            let mut response = run_success(call, result, true);
            if let Some(path) = plan_result_path {
                response["plan_result_path"] = json!(path);
            }
            if let Some(warning) = plan_result_warning {
                response["plan_result_update_warning"] = json!(warning);
            }
            response.to_string()
        }
        Err(error) => {
            let raw =
                runtime_error_json(call, error.error_type, error.message, error.retry_allowed);
            record_plan_failure_for_tool_error(_workspace_root, context, raw).await
        }
    }
}

async fn source_contract_for_runtime(
    call: &AgentToolCall,
    request: &super::CadQueryToolRunRequest,
    runtime: Option<&dyn CadQueryToolRuntime>,
) -> Result<support::SourceContract, String> {
    let mut contract = source_contract(&request.target_path, request.target_type, &request.code);
    if let Some(runtime) = runtime
        && let Some(result) = runtime.model_contract(request).await
    {
        let model_contract = result.map_err(|error| {
            runtime_error_json(call, error.error_type, error.message, error.retry_allowed)
        })?;
        apply_model_contract(&mut contract, model_contract);
    }
    Ok(contract)
}

fn apply_model_contract(
    contract: &mut support::SourceContract,
    model_contract: CadQueryModelContract,
) {
    contract.has_model_description = model_contract.has_model_description;
}

pub(super) async fn record_plan_failure_for_tool_error(
    workspace_root: &Path,
    context: &AgentToolRunContext,
    result_json: String,
) -> String {
    let Some(path) = context
        .execution_scope
        .as_ref()
        .and_then(|scope| scope.plan_result_path.as_deref())
    else {
        return result_json;
    };
    let mut value: serde_json::Value = match serde_json::from_str(&result_json) {
        Ok(value) => value,
        Err(_) => return result_json,
    };
    let error_type = value
        .get("error_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown_error");
    let message = value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Agent tool failed");
    match append_plan_result_failure(workspace_root, path, context, error_type, message).await {
        Ok(()) => {
            value["plan_result_path"] = json!(path);
        }
        Err(warning) => {
            value["plan_result_update_warning"] = json!(warning);
        }
    }
    value.to_string()
}

pub(super) fn get_result(
    call: &AgentToolCall,
    runtime: Option<&dyn CadQueryToolRuntime>,
) -> String {
    let result_id = match result_id_arg(call) {
        Ok(result_id) => result_id,
        Err(result) => return result,
    };
    let Some(runtime) = runtime else {
        return tool_error_json(
            call,
            "CadQuery runtime is not configured",
            "permission_denied",
        );
    };
    let Some(result) = runtime.get_result(&result_id) else {
        return tool_error_json(call, "CadQuery result was not found", "not_found");
    };
    result_success(call, &result).to_string()
}

pub(super) fn resolve_selection(
    call: &AgentToolCall,
    runtime: Option<&dyn CadQueryToolRuntime>,
) -> String {
    let args = match resolve_selection_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let Some(runtime) = runtime else {
        return tool_error_json(
            call,
            "CadQuery runtime is not configured",
            "permission_denied",
        );
    };
    let Some(result) = runtime.get_result(&args.result_id) else {
        return tool_error_json(call, "CadQuery result was not found", "not_found");
    };
    resolve_selection_success(call, &result.mesh, &args.selection_ref).to_string()
}

fn runtime_error_json(
    call: &AgentToolCall,
    error_type: String,
    message: String,
    retry_allowed: bool,
) -> String {
    json!({
        "status": "error",
        "tool_call_id": call.id,
        "tool": call.function_name,
        "message": message,
        "error_type": error_type,
        "retry_allowed": retry_allowed,
        "diagnostics": {
            "traceback": serde_json::Value::Null
        }
    })
    .to_string()
}

async fn append_plan_result_success(
    workspace_root: &Path,
    plan_result_path: &str,
    context: &AgentToolRunContext,
    result: &super::CadQueryToolRunResult,
) -> Result<(), String> {
    let record = format!(
        "\n## Agent Run {}\n\n- status: succeeded\n- run_id: `{}`\n- result_id: `{}`\n- build_id: `{}`\n- committed_files:\n{}\n- outputs:\n{}\n",
        context.run_id.as_deref().unwrap_or("unknown"),
        context.run_id.as_deref().unwrap_or("unknown"),
        result.mesh.result_id,
        result.mesh.build_id,
        markdown_list(&result.committed_files),
        markdown_list(&result.exports)
    );
    append_plan_result_record(workspace_root, plan_result_path, &record).await
}

async fn append_plan_result_failure(
    workspace_root: &Path,
    plan_result_path: &str,
    context: &AgentToolRunContext,
    error_type: &str,
    message: &str,
) -> Result<(), String> {
    let record = format!(
        "\n## Agent Run {}\n\n- status: failed\n- run_id: `{}`\n- error_type: `{}`\n- message: {}\n",
        context.run_id.as_deref().unwrap_or("unknown"),
        context.run_id.as_deref().unwrap_or("unknown"),
        error_type,
        message
    );
    append_plan_result_record(workspace_root, plan_result_path, &record).await
}

async fn append_plan_result_record(
    workspace_root: &Path,
    plan_result_path: &str,
    record: &str,
) -> Result<(), String> {
    validate_plan_result_path(plan_result_path)?;
    reject_plan_result_symlink_components(workspace_root, plan_result_path).await?;
    let absolute = workspace_root.join(plan_result_path);
    let metadata = fs::symlink_metadata(&absolute)
        .await
        .map_err(|error| format!("读取 plan-result.md 失败: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("plan-result.md must not be a symlink".into());
    }
    if !metadata.is_file() {
        return Err("plan-result.md must be a regular file".into());
    }
    reject_hard_linked_plan_result(&metadata)?;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&absolute)
        .await
        .map_err(|error| format!("更新 plan-result.md 失败: {error}"))?;
    file.write_all(record.as_bytes())
        .await
        .map_err(|error| format!("更新 plan-result.md 失败: {error}"))
}

async fn reject_plan_result_symlink_components(
    workspace_root: &Path,
    plan_result_path: &str,
) -> Result<(), String> {
    let mut current = workspace_root.to_path_buf();
    for segment in plan_result_path.split('/') {
        current.push(segment);
        if fs::symlink_metadata(&current)
            .await
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err("plan-result path must not use symlink components".into());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn reject_hard_linked_plan_result(metadata: &std::fs::Metadata) -> Result<(), String> {
    if metadata.nlink() > 1 {
        return Err("plan-result.md must not be hard-linked".into());
    }
    Ok(())
}

#[cfg(windows)]
fn reject_hard_linked_plan_result(_metadata: &std::fs::Metadata) -> Result<(), String> {
    match _metadata.number_of_links() {
        Some(count) if count > 1 => Err("plan-result.md must not be hard-linked".into()),
        Some(_) => Ok(()),
        None => Err("plan-result.md hard-link status is unavailable".into()),
    }
}

#[cfg(not(any(unix, windows)))]
fn reject_hard_linked_plan_result(_metadata: &std::fs::Metadata) -> Result<(), String> {
    Err("plan-result.md hard-link status is unavailable".into())
}

fn validate_plan_result_path(path: &str) -> Result<(), String> {
    if path.starts_with('/') || path.contains(':') || path.split('/').any(|part| part == "..") {
        return Err("plan-result path must be workspace-relative".into());
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() == 3 && parts[0] == "plans" && parts[2] == "plan-result.md" {
        Ok(())
    } else {
        Err("plan-result path must be plans/<id>/plan-result.md".into())
    }
}

fn markdown_list(items: &[String]) -> String {
    if items.is_empty() {
        "  - none".into()
    } else {
        items
            .iter()
            .map(|item| format!("  - {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
