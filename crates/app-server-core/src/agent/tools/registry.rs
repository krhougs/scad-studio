mod schemas;

use app_server_protocol::AgentMode;

use crate::llm::LlmToolDefinition;
use schemas::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentToolCategory {
    ReadOnlyContext,
    SemanticState,
    FileWrite,
    CadQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSemanticStore {
    CadPlan,
    ChatSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadQueryModelFilePolicy {
    ReadOnly,
    Denied,
    CopyOnly,
    CadQueryToolOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputPathPolicy {
    Denied,
    ReadOnly,
    DeclaredOutputsOnly,
    TemporaryResultCacheOnly,
    ConfirmationOutputsOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolPathPolicy {
    pub allowed_roots: Vec<&'static str>,
    pub denied_roots: Vec<&'static str>,
    pub text_only: bool,
    pub requires_confirmation_scope: bool,
    pub cadquery_model_file: CadQueryModelFilePolicy,
    pub output_paths: OutputPathPolicy,
    pub semantic_store: Option<AgentSemanticStore>,
}

#[derive(Debug, Clone)]
pub struct AgentToolSpec {
    pub definition: LlmToolDefinition,
    pub category: AgentToolCategory,
    pub allowed_modes: Vec<AgentMode>,
    pub requires_confirmation: bool,
    pub automatic_llm_tool: bool,
    pub path_policy: AgentToolPathPolicy,
    pub success_schema: serde_json::Value,
    pub error_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolPermission {
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub reason: &'static str,
}

pub fn agent_tool_specs() -> Vec<AgentToolSpec> {
    vec![
        spec(
            "read_file",
            "Read a bounded UTF-8 text file from the workspace.",
            read_file_input_schema(),
            read_file_success_schema(),
            AgentToolCategory::ReadOnlyContext,
            readonly_ops(),
            false,
            workspace_text_read_policy(),
        ),
        spec(
            "list_directory",
            "List workspace directory entries with optional recursion and filtering.",
            list_directory_input_schema(),
            list_directory_success_schema(),
            AgentToolCategory::ReadOnlyContext,
            readonly_ops(),
            false,
            workspace_list_policy(),
        ),
        spec(
            "search_files",
            "Search text files in safe workspace ranges.",
            search_files_input_schema(),
            search_files_success_schema(),
            AgentToolCategory::ReadOnlyContext,
            readonly_ops(),
            false,
            workspace_text_read_policy(),
        ),
        spec(
            "get_project_context",
            "Summarize CadQuery project folders without reading large file bodies.",
            empty_input_schema(),
            project_context_success_schema(),
            AgentToolCategory::ReadOnlyContext,
            readonly_ops(),
            false,
            workspace_list_policy(),
        ),
        spec(
            "get_selection",
            "Return the current Viewer selection snapshot.",
            empty_input_schema(),
            selection_success_schema(),
            AgentToolCategory::ReadOnlyContext,
            readonly_ops(),
            false,
            no_path_policy(),
        ),
        spec(
            "resolve_ref",
            "Resolve a visible MVP Ref to owner files and stability risks.",
            resolve_ref_input_schema(),
            resolve_ref_success_schema(),
            AgentToolCategory::ReadOnlyContext,
            readonly_ops(),
            false,
            workspace_text_read_policy(),
        ),
        spec(
            "save_cad_plan",
            "Persist a structured Markdown CAD Plan under plans/.",
            save_cad_plan_input_schema(),
            save_cad_plan_success_schema(),
            AgentToolCategory::SemanticState,
            vec![AgentMode::Plan],
            false,
            cad_plan_policy(),
        ),
        spec(
            "update_chat_summary",
            "Update Chat session metadata through ChatStore, not by editing JSONL.",
            update_chat_summary_input_schema(),
            update_chat_summary_success_schema(),
            AgentToolCategory::SemanticState,
            vec![AgentMode::Agent],
            false,
            chat_summary_policy(),
        ),
        spec(
            "write_file",
            "Write a confirmed non-model text file.",
            write_file_input_schema(),
            file_write_success_schema(),
            AgentToolCategory::FileWrite,
            vec![AgentMode::Agent],
            true,
            confirmed_text_write_policy(CadQueryModelFilePolicy::Denied),
        ),
        spec(
            "patch_file",
            "Apply a deterministic patch with hash-based conflict checks.",
            patch_file_input_schema(),
            file_write_success_schema(),
            AgentToolCategory::FileWrite,
            vec![AgentMode::Agent],
            true,
            confirmed_text_write_policy(CadQueryModelFilePolicy::Denied),
        ),
        spec(
            "copy_file",
            "Copy a confirmed file without changing CadQuery source content.",
            copy_file_input_schema(),
            file_write_success_schema(),
            AgentToolCategory::FileWrite,
            vec![AgentMode::Agent],
            true,
            confirmed_text_write_policy(CadQueryModelFilePolicy::CopyOnly),
        ),
        cadquery_analyze_source_spec(),
        cadquery_check_source_spec(),
        cadquery_dry_run_spec(),
        cadquery_execute_spec(),
        cadquery_get_result_spec(),
        cadquery_resolve_selection_spec(),
    ]
}

pub fn agent_tool_definitions_for_mode(mode: AgentMode) -> Vec<LlmToolDefinition> {
    agent_tool_specs()
        .into_iter()
        .filter(|spec| spec.automatic_llm_tool && spec.allowed_modes.iter().any(|op| *op == mode))
        .map(|spec| spec.definition)
        .collect()
}

pub fn agent_tool_permission(
    tool_name: &str,
    mode: AgentMode,
    has_confirmation: bool,
) -> AgentToolPermission {
    let Some(spec) = agent_tool_specs()
        .into_iter()
        .find(|spec| spec.definition.name == tool_name)
    else {
        return denied(false, "unknown tool");
    };
    if !spec.allowed_modes.iter().any(|op| *op == mode) {
        return denied(
            spec.requires_confirmation,
            "tool is not allowed for this mode",
        );
    }
    if spec.requires_confirmation && !has_confirmation {
        return denied(true, "tool requires confirmed execution scope");
    }
    AgentToolPermission {
        allowed: true,
        requires_confirmation: spec.requires_confirmation,
        reason: "allowed",
    }
}

fn spec(
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
    success_schema: serde_json::Value,
    category: AgentToolCategory,
    allowed_modes: Vec<AgentMode>,
    requires_confirmation: bool,
    path_policy: AgentToolPathPolicy,
) -> AgentToolSpec {
    AgentToolSpec {
        definition: LlmToolDefinition {
            name: name.into(),
            description: description.into(),
            parameters,
        },
        category,
        allowed_modes,
        requires_confirmation,
        automatic_llm_tool: true,
        path_policy,
        success_schema,
        error_schema: tool_error_schema(),
    }
}

fn cadquery_analyze_source_spec() -> AgentToolSpec {
    spec(
        "cadquery_analyze_source",
        "Analyze existing CadQuery source and paired design notes without executing Python.",
        cadquery_analyze_source_input_schema(),
        cadquery_analyze_source_success_schema(),
        AgentToolCategory::CadQuery,
        readonly_ops(),
        false,
        cadquery_read_policy(OutputPathPolicy::Denied),
    )
}

fn cadquery_check_source_spec() -> AgentToolSpec {
    spec(
        "cadquery_check_source",
        "Check proposed complete CadQuery source against the static model contract.",
        cadquery_check_source_input_schema(),
        cadquery_check_source_success_schema(),
        AgentToolCategory::CadQuery,
        vec![AgentMode::Plan, AgentMode::Agent],
        false,
        cadquery_read_policy(OutputPathPolicy::Denied),
    )
}

fn cadquery_dry_run_spec() -> AgentToolSpec {
    spec(
        "cadquery_dry_run",
        "Run proposed CadQuery code in staging and cache a temporary preview result.",
        cadquery_dry_run_input_schema(),
        cadquery_run_success_schema(),
        AgentToolCategory::CadQuery,
        vec![AgentMode::Agent],
        false,
        cadquery_read_policy(OutputPathPolicy::TemporaryResultCacheOnly),
    )
}

fn cadquery_execute_spec() -> AgentToolSpec {
    spec(
        "cadquery_execute",
        "Commit confirmed CadQuery source and confirmed outputs through staging.",
        cadquery_execute_input_schema(),
        cadquery_execute_success_schema(),
        AgentToolCategory::CadQuery,
        vec![AgentMode::Agent],
        true,
        cadquery_execute_policy(),
    )
}

fn cadquery_get_result_spec() -> AgentToolSpec {
    spec(
        "cadquery_get_result",
        "Return a lightweight CadQuery result summary without full mesh arrays.",
        cadquery_get_result_input_schema(),
        cadquery_get_result_success_schema(),
        AgentToolCategory::CadQuery,
        readonly_ops(),
        false,
        result_cache_policy(),
    )
}

fn cadquery_resolve_selection_spec() -> AgentToolSpec {
    spec(
        "cadquery_resolve_selection",
        "Map a result-local selection to owner, feature and stability risk.",
        cadquery_resolve_selection_input_schema(),
        resolve_ref_success_schema(),
        AgentToolCategory::CadQuery,
        readonly_ops(),
        false,
        result_cache_policy(),
    )
}

fn denied(requires_confirmation: bool, reason: &'static str) -> AgentToolPermission {
    AgentToolPermission {
        allowed: false,
        requires_confirmation,
        reason,
    }
}

fn readonly_ops() -> Vec<AgentMode> {
    vec![AgentMode::Agent, AgentMode::Plan]
}

fn no_path_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        allowed_roots: Vec::new(),
        denied_roots: Vec::new(),
        text_only: false,
        requires_confirmation_scope: false,
        cadquery_model_file: CadQueryModelFilePolicy::Denied,
        output_paths: OutputPathPolicy::Denied,
        semantic_store: None,
    }
}

fn workspace_text_read_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        allowed_roots: vec![""],
        denied_roots: unsafe_workspace_roots(),
        text_only: true,
        requires_confirmation_scope: false,
        cadquery_model_file: CadQueryModelFilePolicy::ReadOnly,
        output_paths: OutputPathPolicy::ReadOnly,
        semantic_store: None,
    }
}

fn workspace_list_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        text_only: false,
        ..workspace_text_read_policy()
    }
}

fn cad_plan_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        allowed_roots: vec!["plans"],
        denied_roots: vec!["chats", "outputs"],
        text_only: true,
        requires_confirmation_scope: false,
        cadquery_model_file: CadQueryModelFilePolicy::Denied,
        output_paths: OutputPathPolicy::DeclaredOutputsOnly,
        semantic_store: Some(AgentSemanticStore::CadPlan),
    }
}

fn chat_summary_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        denied_roots: vec!["chats"],
        semantic_store: Some(AgentSemanticStore::ChatSummary),
        ..no_path_policy()
    }
}

fn confirmed_text_write_policy(model_policy: CadQueryModelFilePolicy) -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        allowed_roots: vec!["components", "parts", "assemblies", "refs", "docs"],
        denied_roots: vec!["chats", "outputs"],
        text_only: true,
        requires_confirmation_scope: true,
        cadquery_model_file: model_policy,
        output_paths: OutputPathPolicy::Denied,
        semantic_store: None,
    }
}

fn cadquery_read_policy(output_paths: OutputPathPolicy) -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        allowed_roots: vec!["components", "parts", "assemblies"],
        denied_roots: unsafe_workspace_roots(),
        text_only: true,
        requires_confirmation_scope: false,
        cadquery_model_file: CadQueryModelFilePolicy::ReadOnly,
        output_paths,
        semantic_store: None,
    }
}

fn cadquery_execute_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        allowed_roots: vec!["components", "parts", "assemblies"],
        denied_roots: vec!["chats"],
        text_only: true,
        requires_confirmation_scope: true,
        cadquery_model_file: CadQueryModelFilePolicy::CadQueryToolOnly,
        output_paths: OutputPathPolicy::ConfirmationOutputsOnly,
        semantic_store: None,
    }
}

fn result_cache_policy() -> AgentToolPathPolicy {
    AgentToolPathPolicy {
        output_paths: OutputPathPolicy::TemporaryResultCacheOnly,
        ..no_path_policy()
    }
}

fn unsafe_workspace_roots() -> Vec<&'static str> {
    vec![".git", "target", "node_modules", "outputs", ".budn_staging"]
}
