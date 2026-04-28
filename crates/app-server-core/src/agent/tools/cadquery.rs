mod args;
mod support;

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use serde_json::json;

use crate::llm::LlmToolCall;

use super::{AgentToolRunContext, CadQueryToolRuntime, tool_error_json};
use args::{
    analyze_args, doc_update_path_for_execute, dry_run_request_args, execute_request_args,
    existing_model_path, resolve_selection_args, result_id_arg, source_request_args,
    validate_contract_for_run, validate_execute_scope,
};
use support::{
    analyze_success, contract_json, contract_warnings, resolve_selection_success, result_success,
    run_success, source_contract,
};

pub(super) fn analyze_source(workspace_root: &Path, call: &LlmToolCall) -> String {
    let args = match analyze_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let source_path = match existing_model_path(workspace_root, &args.target_path, call) {
        Ok(path) => path,
        Err(result) => return result,
    };
    let source = match fs::read_to_string(&source_path) {
        Ok(source) => source,
        Err(error) => {
            return tool_error_json(
                call,
                &format!("读取 CadQuery 源码失败: {error}"),
                "not_found",
            );
        }
    };
    analyze_success(
        workspace_root,
        call,
        &args.target_path,
        args.include_paired_doc,
        args.include_dependencies,
        &source,
    )
    .to_string()
}

pub(super) fn check_source(call: &LlmToolCall) -> String {
    let request = match source_request_args(call) {
        Ok(request) => request,
        Err(result) => return result,
    };
    let contract = source_contract(&request.target_path, request.target_type, &request.code);
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery source contract checked",
        "contract": contract_json(&contract),
        "warnings": contract_warnings(&contract)
    })
    .to_string()
}

pub(super) fn dry_run(
    _workspace_root: &Path,
    call: &LlmToolCall,
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
    match runtime.dry_run(request) {
        Ok(result) => run_success(call, result, false).to_string(),
        Err(error) => {
            runtime_error_json(call, error.error_type, error.message, error.retry_allowed)
        }
    }
}

pub(super) fn execute(
    _workspace_root: &Path,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
    runtime: Option<&dyn CadQueryToolRuntime>,
    committed: &AtomicBool,
) -> String {
    if committed.load(Ordering::SeqCst) {
        return tool_error_json(
            call,
            "cadquery_execute already committed in this run",
            "permission_denied",
        );
    }
    let mut request = match execute_request_args(call) {
        Ok(request) => request,
        Err(result) => return result,
    };
    if let Err(result) = validate_contract_for_run(call, &request) {
        return result;
    }
    if let Err(result) = validate_execute_scope(call, &request, context) {
        return result;
    }
    match doc_update_path_for_execute(_workspace_root, call, &request, context) {
        Ok(doc_update_path) => request.doc_update_path = doc_update_path,
        Err(result) => return result,
    }
    let Some(runtime) = runtime else {
        return tool_error_json(
            call,
            "CadQuery runtime is not configured",
            "permission_denied",
        );
    };
    match runtime.execute(request) {
        Ok(result) => {
            committed.store(true, Ordering::SeqCst);
            run_success(call, result, true).to_string()
        }
        Err(error) => {
            runtime_error_json(call, error.error_type, error.message, error.retry_allowed)
        }
    }
}

pub(super) fn get_result(call: &LlmToolCall, runtime: Option<&dyn CadQueryToolRuntime>) -> String {
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
    call: &LlmToolCall,
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
    call: &LlmToolCall,
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
