mod cadquery;
mod file_write;
mod readonly;
mod registry;
mod semantic;
mod semantic_chat;
mod semantic_export;
mod tool_path_policy;

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use app_server_protocol::{
    AgentMode, CadQueryExportFormat, CadQueryMeshPayload, CadQueryObjectKind, ChatSessionId,
    SelectionRef,
};
use serde_json::json;

use super::plan_package::ParsedPlanPackage;
use crate::llm::{LlmError, LlmMessage, LlmProvider, LlmResponse, LlmToolCall};
pub use registry::{
    AgentSemanticStore, AgentToolCategory, AgentToolPathPolicy, AgentToolPermission, AgentToolSpec,
    CadQueryModelFilePolicy, OutputPathPolicy, agent_tool_definitions_for_mode,
    agent_tool_permission, agent_tool_specs,
};
use tool_path_policy::{
    normalize_scope_paths, validate_registry_tool_intent, validate_tool_path_policy,
};

const MAX_TOOL_ROUNDS: usize = 10;
pub trait ToolExecutor: Send + Sync {
    fn execute(&self, call: &LlmToolCall, context: &AgentToolRunContext) -> String;
}

pub trait ToolLoopObserver: Send + Sync {
    fn tool_start(&self, call: &LlmToolCall);
    fn tool_result(&self, call: &LlmToolCall, result: &str);
}

pub trait CadQueryToolRuntime: Send + Sync {
    fn dry_run(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError>;

    fn execute(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError>;

    fn get_result(&self, result_id: &str) -> Option<CadQueryToolCachedResult>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadQueryToolRunRequest {
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub code: String,
    pub params_json: String,
    pub export_formats: Vec<CadQueryExportFormat>,
    pub export_targets: Vec<String>,
    pub doc_update_path: Option<String>,
    pub plan_ref: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CadQueryToolCachedResult {
    pub mesh: CadQueryMeshPayload,
    pub exports: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CadQueryToolRunResult {
    pub mesh: CadQueryMeshPayload,
    pub committed_files: Vec<String>,
    pub exports: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadQueryToolRuntimeError {
    pub error_type: String,
    pub message: String,
    pub retry_allowed: bool,
}

impl CadQueryToolRuntimeError {
    pub fn new(
        error_type: impl Into<String>,
        message: impl Into<String>,
        retry_allowed: bool,
    ) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            retry_allowed,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentExecutionScope {
    pub target_path: Option<String>,
    pub target_type: Option<CadQueryObjectKind>,
    pub affected_files: Vec<String>,
    pub new_files: Vec<String>,
    pub export_targets: Vec<String>,
    pub plan_ref: Option<String>,
    pub plan_result_path: Option<String>,
}

impl AgentExecutionScope {
    pub fn new(
        affected_files: Vec<String>,
        new_files: Vec<String>,
        export_targets: Vec<String>,
    ) -> Self {
        Self {
            target_path: None,
            target_type: None,
            affected_files: normalize_scope_paths(affected_files),
            new_files: normalize_scope_paths(new_files),
            export_targets: normalize_scope_paths(export_targets),
            plan_ref: None,
            plan_result_path: None,
        }
    }

    pub fn from_plan_package(plan: &ParsedPlanPackage) -> Self {
        Self {
            target_path: Some(plan.target_path.clone()),
            target_type: Some(plan.target_type),
            affected_files: normalize_scope_paths(plan.affected_files.clone()),
            new_files: normalize_scope_paths(plan.new_files.clone()),
            export_targets: normalize_scope_paths(plan.export_targets.clone()),
            plan_ref: Some(plan.plan_ref.clone()),
            plan_result_path: Some(plan.result_path.clone()),
        }
    }

    pub fn for_plan(
        plan_ref: impl Into<String>,
        plan_result_path: impl Into<String>,
        target_path: impl Into<String>,
        target_type: CadQueryObjectKind,
        affected_files: Vec<String>,
        new_files: Vec<String>,
        export_targets: Vec<String>,
    ) -> Self {
        Self {
            target_path: Some(target_path.into()),
            target_type: Some(target_type),
            affected_files: normalize_scope_paths(affected_files),
            new_files: normalize_scope_paths(new_files),
            export_targets: normalize_scope_paths(export_targets),
            plan_ref: Some(plan_ref.into()),
            plan_result_path: Some(plan_result_path.into()),
        }
    }

    pub(super) fn contains_affected_file(&self, path: &str) -> bool {
        self.affected_files
            .iter()
            .any(|candidate| candidate == path)
    }

    pub(super) fn contains_new_file(&self, path: &str) -> bool {
        self.new_files.iter().any(|candidate| candidate == path)
    }

    fn contains_export_target(&self, path: &str) -> bool {
        self.export_targets
            .iter()
            .any(|candidate| candidate == path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolRunContext {
    pub workspace_root: PathBuf,
    pub session_id: Option<ChatSessionId>,
    pub run_id: Option<String>,
    pub mode: AgentMode,
    pub selections: Vec<SelectionRef>,
    pub active_selection_index: Option<u32>,
    pub context_refs: Vec<String>,
    pub execution_scope: Option<AgentExecutionScope>,
}

impl AgentToolRunContext {
    pub fn new(workspace_root: PathBuf, mode: AgentMode) -> Self {
        Self {
            workspace_root,
            session_id: None,
            run_id: None,
            mode,
            selections: Vec::new(),
            active_selection_index: None,
            context_refs: Vec::new(),
            execution_scope: None,
        }
    }
}

pub struct NoopToolLoopObserver;

impl ToolLoopObserver for NoopToolLoopObserver {
    fn tool_start(&self, _call: &LlmToolCall) {}

    fn tool_result(&self, _call: &LlmToolCall, _result: &str) {}
}

pub struct WorkspaceToolExecutor {
    workspace_root: PathBuf,
    cadquery_runtime: Option<Arc<dyn CadQueryToolRuntime>>,
    cadquery_committed: AtomicBool,
}

impl WorkspaceToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            cadquery_runtime: None,
            cadquery_committed: AtomicBool::new(false),
        }
    }

    pub fn with_cadquery_runtime(mut self, runtime: Arc<dyn CadQueryToolRuntime>) -> Self {
        self.cadquery_runtime = Some(runtime);
        self
    }
}

impl ToolExecutor for WorkspaceToolExecutor {
    fn execute(&self, call: &LlmToolCall, context: &AgentToolRunContext) -> String {
        if let Some(result) = validate_direct_executor_permission(call, context) {
            return record_tool_error_for_context(&self.workspace_root, call, context, result);
        }
        match call.function_name.as_str() {
            "read_file" => readonly::read_file(&self.workspace_root, call),
            "list_directory" => readonly::list_directory(&self.workspace_root, call),
            "search_files" => readonly::search_files(&self.workspace_root, call),
            "get_project_context" => readonly::get_project_context(&self.workspace_root, call),
            "get_selection" => readonly::get_selection(call, context),
            "resolve_ref" => readonly::resolve_ref(&self.workspace_root, call, context),
            "save_cad_plan" => semantic::save_cad_plan(&self.workspace_root, call, context),
            "update_chat_summary" => {
                semantic_chat::update_chat_summary(&self.workspace_root, call, context)
            }
            "write_file" => file_write::write_file(&self.workspace_root, call, context),
            "patch_file" => file_write::patch_file(&self.workspace_root, call, context),
            "copy_file" => file_write::copy_file(&self.workspace_root, call, context),
            "cadquery_analyze_source" => cadquery::analyze_source(&self.workspace_root, call),
            "cadquery_check_source" => cadquery::check_source(call),
            "cadquery_dry_run" => {
                cadquery::dry_run(&self.workspace_root, call, self.cadquery_runtime.as_deref())
            }
            "cadquery_execute" => cadquery::execute(
                &self.workspace_root,
                call,
                context,
                self.cadquery_runtime.as_deref(),
                &self.cadquery_committed,
            ),
            "cadquery_get_result" => cadquery::get_result(call, self.cadquery_runtime.as_deref()),
            "cadquery_resolve_selection" => {
                cadquery::resolve_selection(call, self.cadquery_runtime.as_deref())
            }
            _ => tool_error_json(
                call,
                "tool is registered but not implemented by this executor",
                "unsupported_tool",
            ),
        }
    }
}

fn validate_direct_executor_permission(
    call: &LlmToolCall,
    context: &AgentToolRunContext,
) -> Option<String> {
    let spec = agent_tool_specs()
        .into_iter()
        .find(|spec| spec.definition.name == call.function_name)?;
    let permission = agent_tool_permission(
        &call.function_name,
        context.mode,
        context.execution_scope.is_some(),
    );
    if !permission.allowed {
        return Some(tool_error_json(
            call,
            permission.reason,
            "permission_denied",
        ));
    }
    validate_tool_path_policy(
        &call.function_name,
        &call.arguments,
        &spec.path_policy,
        context.execution_scope.as_ref(),
    )
    .err()
    .map(|error| tool_error_json(call, &error.message, error.error_type))
}

pub fn run_tool_loop_with_registry(
    initial_messages: Vec<LlmMessage>,
    context: AgentToolRunContext,
    provider: &dyn LlmProvider,
    executor: &dyn ToolExecutor,
    observer: &dyn ToolLoopObserver,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<LlmResponse, LlmError> {
    run_tool_loop_with_registry_and_reasoning(
        initial_messages,
        context,
        provider,
        executor,
        observer,
        on_token,
        &|_| true,
    )
}

pub fn run_tool_loop_with_registry_and_reasoning(
    initial_messages: Vec<LlmMessage>,
    context: AgentToolRunContext,
    provider: &dyn LlmProvider,
    executor: &dyn ToolExecutor,
    observer: &dyn ToolLoopObserver,
    on_token: &dyn Fn(&str) -> bool,
    on_reasoning: &dyn Fn(&str) -> bool,
) -> Result<LlmResponse, LlmError> {
    let tools = agent_tool_definitions_for_mode(context.mode);
    let mut messages = initial_messages;
    let mut last_content = String::new();
    for _ in 0..MAX_TOOL_ROUNDS {
        let response = provider.stream_chat_with_reasoning(
            messages.clone(),
            &tools,
            on_token,
            on_reasoning,
        )?;
        if response.content.trim().is_empty()
            && !response.has_tool_calls()
            && response.reasoning_content.is_some()
        {
            messages.push(reasoning_only_retry_message());
            continue;
        }
        if !response.has_tool_calls() {
            return Ok(response);
        }
        last_content = response.content.clone();
        messages.push(LlmMessage::assistant_response(
            response.content.clone(),
            response.reasoning_content.clone(),
            response.tool_calls.clone(),
        ));
        for call in &response.tool_calls {
            observer.tool_start(call);
            let result = execute_registry_tool(call, &context, executor);
            observer.tool_result(call, &result);
            messages.push(LlmMessage::tool_result(call.id.clone(), result));
        }
    }
    Ok(LlmResponse {
        content: if last_content.is_empty() {
            "Agent reached maximum tool call rounds.".into()
        } else {
            last_content
        },
        reasoning_content: None,
        tool_calls: Vec::new(),
    })
}

fn reasoning_only_retry_message() -> LlmMessage {
    LlmMessage::new(
        "user",
        "上一轮只返回了 reasoning_content，没有正文或工具调用。请现在停止只思考的输出：如果任务需要读取、生成或修改项目文件，必须调用可用工具；否则返回面向用户的正文。",
    )
}

fn execute_registry_tool(
    call: &LlmToolCall,
    context: &AgentToolRunContext,
    executor: &dyn ToolExecutor,
) -> String {
    let spec = agent_tool_specs()
        .into_iter()
        .find(|spec| spec.definition.name == call.function_name);
    let permission = agent_tool_permission(
        &call.function_name,
        context.mode,
        context.execution_scope.is_some(),
    );
    if !permission.allowed {
        return record_tool_error_for_context(
            &context.workspace_root,
            call,
            context,
            tool_error_json(call, permission.reason, "permission_denied"),
        );
    }
    if let Some(spec) = spec
        && let Err(error) = validate_tool_path_policy(
            &call.function_name,
            &call.arguments,
            &spec.path_policy,
            context.execution_scope.as_ref(),
        )
    {
        return record_tool_error_for_context(
            &context.workspace_root,
            call,
            context,
            tool_error_json(call, &error.message, error.error_type),
        );
    }
    if let Err(error) = validate_registry_tool_intent(
        &call.function_name,
        &call.arguments,
        context.execution_scope.as_ref(),
    ) {
        return record_tool_error_for_context(
            &context.workspace_root,
            call,
            context,
            tool_error_json(call, &error.message, error.error_type),
        );
    }
    executor.execute(call, context)
}

fn record_tool_error_for_context(
    workspace_root: &std::path::Path,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
    result: String,
) -> String {
    if call.function_name == "cadquery_execute" {
        cadquery::record_plan_failure_for_tool_error(workspace_root, context, result)
    } else {
        result
    }
}

fn tool_error_json(call: &LlmToolCall, message: &str, error_type: &str) -> String {
    json!({
        "status": "error",
        "tool_call_id": call.id,
        "tool": call.function_name,
        "message": message,
        "error_type": error_type,
        "retry_allowed": false
    })
    .to_string()
}
