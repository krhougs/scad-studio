mod agent;
pub mod cadquery;
mod chat;
mod config;
mod export;
mod file;
pub mod llm;
mod presets;
mod preview;
mod watch;
mod workspace;

pub use agent::{
    AgentTurnInput, RigAgentCallbacks, RigAgentError, RigAgentTurnResult,
    build_rig_prompt_and_history, build_turn_context, cadquery_agent_system_prompt,
    extract_cadquery_code,
    plan_package::{
        ParsedPlanPackage, PlanPackageError, PlanPackagePaths, PlanTimestamp,
        SaveCadPlanPackageInput, SavedPlanPackage, collect_plan_packages, parse_plan_package,
        save_plan_package, save_plan_package_with_timestamp, slugify_plan_title,
    },
    run_rig_agent_turn, run_rig_agent_turn_with_config,
    tools::{
        AgentExecutionScope, AgentSemanticStore, AgentToolCall, AgentToolCategory,
        AgentToolDefinition, AgentToolObserver, AgentToolPathPolicy, AgentToolPermission,
        AgentToolRunContext, AgentToolSpec, CadQueryModelContract, CadQueryModelFilePolicy,
        CadQueryToolCachedResult, CadQueryToolRunRequest, CadQueryToolRunResult,
        CadQueryToolRuntime, CadQueryToolRuntimeError, NoopAgentToolObserver, OutputPathPolicy,
        ToolExecutor, WorkspaceToolExecutor, agent_tool_definitions_for_mode,
        agent_tool_permission, agent_tool_specs, execute_registered_tool,
    },
};
pub use cadquery::{
    CadQueryCommitScope, CadQueryContractConfig, CadQueryContractResult, CadQueryExecuteConfig,
    CadQueryRunConfig, CadQueryRunResult, CadQueryRunnerError, CadQueryRunnerErrorKind,
    StagedCadQueryProject, cadquery_result_ready, execute_cadquery_with_staging,
    execute_cadquery_with_staging_cancellable, execute_cadquery_with_staging_cancellable_scoped,
    parse_cadquery_success_json, run_cadquery_contract, run_cadquery_runner,
    run_cadquery_runner_with_cancel, stage_cadquery_project, stage_cadquery_project_owned,
    validate_cadquery_mesh_payload,
};
pub use chat::ChatStore;
pub use config::{
    ConfigError, app_config_from_dto, app_config_to_dto, config_file_path, load_config,
    load_config_dto, load_config_json, save_config, save_config_dto, save_config_json,
};
pub use export::{
    SlicerInstall, build_export_filename, detect_slicer_paths, export_model, send_to_slicer,
};
pub use file::{
    canonicalize_or_original, read_binary_file, read_file_response, read_file_response_owned,
    read_text_file,
};
pub use presets::{PresetError, delete_preset, load_presets, preset_path_for_source, save_preset};
pub use preview::{
    CliOutputFormat, LogEntry, LogLevel, OpenScadError, OpenScadMessage, RenderedArtifact,
    build_cli_args, build_preview_job_args, collect_process_logs, detect_openscad_path,
    finalize_job, preview_artifact, preview_ready_response, resolve_openscad_path,
};
pub use watch::{FileWatcher, WatchError, WatchMessage, matches_any_path, matches_path};
pub use workspace::{
    current_workspace, current_workspace_owned, list_workspace_entries,
    list_workspace_entries_owned, resolve_workspace_path, resolve_workspace_path_owned,
    resolve_workspace_write_path, resolve_workspace_write_path_owned,
};
