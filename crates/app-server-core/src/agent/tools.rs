mod cadquery;
mod file_write;
mod readonly;
mod registry;
mod semantic;
mod semantic_chat;
mod semantic_export;
mod tool_path_policy;
mod web;

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use app_server_protocol::{
    AgentMode, CadQueryExportFormat, CadQueryMeshPayload, CadQueryObjectKind, ChatSessionId,
    SelectionRef,
};
use async_trait::async_trait;
use serde_json::json;

use crate::llm::ResolvedWebSearchProvider;

use super::plan_package::ParsedPlanPackage;
pub use registry::{
    AgentSemanticStore, AgentToolCategory, AgentToolPathPolicy, AgentToolPermission, AgentToolSpec,
    CadQueryModelFilePolicy, OutputPathPolicy, agent_tool_definitions_for_mode,
    agent_tool_permission, agent_tool_specs,
};
use tool_path_policy::{
    normalize_scope_paths, validate_registry_tool_intent, validate_tool_path_policy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolCall {
    pub id: String,
    pub function_name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct AgentToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: &AgentToolCall, context: &AgentToolRunContext) -> String;
}

pub trait AgentToolObserver: Send + Sync {
    fn tool_start(&self, call: &AgentToolCall);
    fn tool_result(&self, call: &AgentToolCall, result: &str);
}

#[async_trait]
pub trait CadQueryToolRuntime: Send + Sync {
    async fn model_contract(
        &self,
        _request: &CadQueryToolRunRequest,
    ) -> Option<Result<CadQueryModelContract, CadQueryToolRuntimeError>> {
        None
    }

    async fn dry_run(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError>;

    async fn execute(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError>;

    fn get_result(&self, result_id: &str) -> Option<CadQueryToolCachedResult>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadQueryModelContract {
    pub has_model_description: bool,
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
    pub execution_scope: Option<AgentExecutionScope>,
    pub web_search_available: bool,
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
            execution_scope: None,
            web_search_available: false,
        }
    }
}

pub struct NoopAgentToolObserver;

impl AgentToolObserver for NoopAgentToolObserver {
    fn tool_start(&self, _call: &AgentToolCall) {}

    fn tool_result(&self, _call: &AgentToolCall, _result: &str) {}
}

pub struct WorkspaceToolExecutor {
    workspace_root: PathBuf,
    cadquery_runtime: Option<Arc<dyn CadQueryToolRuntime>>,
    cadquery_committed: AtomicBool,
    http_client: reqwest::Client,
    web_search_provider: Option<ResolvedWebSearchProvider>,
}

impl WorkspaceToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            cadquery_runtime: None,
            cadquery_committed: AtomicBool::new(false),
            http_client: reqwest::Client::new(),
            web_search_provider: None,
        }
    }

    pub fn with_cadquery_runtime(mut self, runtime: Arc<dyn CadQueryToolRuntime>) -> Self {
        self.cadquery_runtime = Some(runtime);
        self
    }

    pub fn with_web_search_provider(mut self, provider: ResolvedWebSearchProvider) -> Self {
        if let Some(ua) = &provider.user_agent {
            if let Ok(client) = reqwest::Client::builder().user_agent(ua).build() {
                self.http_client = client;
            }
        }
        self.web_search_provider = Some(provider);
        self
    }
}

#[async_trait]
impl ToolExecutor for WorkspaceToolExecutor {
    async fn execute(&self, call: &AgentToolCall, context: &AgentToolRunContext) -> String {
        if let Some(result) = validate_direct_executor_permission(call, context) {
            return record_tool_error_for_context(&self.workspace_root, call, context, result)
                .await;
        }
        match call.function_name.as_str() {
            "read_file" => readonly::read_file(&self.workspace_root, call).await,
            "list_directory" => readonly::list_directory(&self.workspace_root, call).await,
            "search_files" => readonly::search_files(&self.workspace_root, call).await,
            "get_project_context" => {
                readonly::get_project_context(&self.workspace_root, call).await
            }
            "get_selection" => readonly::get_selection(call, context),
            "resolve_ref" => readonly::resolve_ref(&self.workspace_root, call, context).await,
            "save_cad_plan" => semantic::save_cad_plan(&self.workspace_root, call, context).await,
            "update_chat_summary" => {
                semantic_chat::update_chat_summary(&self.workspace_root, call, context).await
            }
            "write_file" => file_write::write_file(&self.workspace_root, call, context).await,
            "patch_file" => file_write::patch_file(&self.workspace_root, call, context).await,
            "copy_file" => file_write::copy_file(&self.workspace_root, call, context).await,
            "web_search" => {
                let Some(provider) = &self.web_search_provider else {
                    return tool_error_json(
                        call,
                        "web search provider not configured",
                        "web_search_error",
                    );
                };
                web::web_search(&self.http_client, provider, call).await
            }
            "fetch_url" => web::fetch_url(&self.http_client, call).await,
            "cadquery_analyze_source" => {
                cadquery::analyze_source(
                    &self.workspace_root,
                    call,
                    self.cadquery_runtime.as_deref(),
                )
                .await
            }
            "cadquery_check_source" => {
                cadquery::check_source(call, self.cadquery_runtime.as_deref()).await
            }
            "cadquery_dry_run" => {
                cadquery::dry_run(&self.workspace_root, call, self.cadquery_runtime.as_deref())
                    .await
            }
            "cadquery_execute" => {
                cadquery::execute(
                    &self.workspace_root,
                    call,
                    context,
                    self.cadquery_runtime.as_deref(),
                    &self.cadquery_committed,
                )
                .await
            }
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
    call: &AgentToolCall,
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

pub async fn execute_registered_tool(
    call: &AgentToolCall,
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
        )
        .await;
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
        )
        .await;
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
        )
        .await;
    }
    executor.execute(call, context).await
}

async fn record_tool_error_for_context(
    workspace_root: &std::path::Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
    result: String,
) -> String {
    if call.function_name == "cadquery_execute" {
        cadquery::record_plan_failure_for_tool_error(workspace_root, context, result).await
    } else {
        result
    }
}

fn tool_error_json(call: &AgentToolCall, message: &str, error_type: &str) -> String {
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
