mod agent;
pub mod cadquery;
mod chat;
mod child_terminator;
mod config;
mod export;
mod file;
mod presets;
mod preview;
mod watch;
mod workspace;

pub use agent::{
    AgentBackend, AgentBackendDecision, AgentBackendError, AgentCadQueryCodeInput, AgentLlmRequest,
    AgentTurnDraft, AgentTurnInput, GeneratedCadQueryCode, LocalAgentBackend,
    cadquery_agent_system_prompt, draft_agent_turn, generate_cadquery_code,
    llm_request_for_cadquery_execute, rig_backend_decision,
};
pub use cadquery::{
    CadQueryCommitScope, CadQueryExecuteConfig, CadQueryRunConfig, CadQueryRunResult,
    CadQueryRunnerError, CadQueryRunnerErrorKind, StagedCadQueryProject, cadquery_result_ready,
    execute_cadquery_with_staging, execute_cadquery_with_staging_cancellable,
    execute_cadquery_with_staging_cancellable_scoped, parse_cadquery_success_json,
    run_cadquery_runner, run_cadquery_runner_with_cancel, stage_cadquery_project,
    validate_cadquery_mesh_payload,
};
pub use chat::ChatStore;
pub use child_terminator::{
    ChildTerminator, DefaultChildTerminator, terminate_child, terminate_child_with,
};
pub use config::{
    ConfigError, app_config_from_dto, app_config_to_dto, config_file_path, load_config,
    load_config_dto, load_config_json, save_config, save_config_dto, save_config_json,
};
pub use export::{
    SlicerInstall, build_export_filename, detect_slicer_paths, export_model, send_to_slicer,
};
pub use file::{canonicalize_or_original, read_binary_file, read_file_response, read_text_file};
pub use presets::{PresetError, delete_preset, load_presets, preset_path_for_source, save_preset};
pub use preview::{
    CliOutputFormat, LogEntry, LogLevel, OpenScadError, OpenScadMessage, OpenScadRunner,
    RenderedArtifact, build_cli_args, build_preview_job_args, collect_process_logs,
    detect_openscad_path, finalize_job, preview_artifact, preview_ready_response,
    resolve_openscad_path,
};
pub use watch::{FileWatcher, WatchError, WatchMessage, matches_any_path, matches_path};
pub use workspace::{
    current_workspace, list_workspace_entries, resolve_workspace_path, resolve_workspace_write_path,
};
