mod child_terminator;
mod config;
mod export;
mod file;
mod presets;
mod preview;
mod watch;
mod workspace;

pub use child_terminator::{
    ChildTerminator, DefaultChildTerminator, terminate_child, terminate_child_with,
};
pub use config::{
    ConfigError, config_file_path, load_config, load_config_json, save_config, save_config_json,
};
pub use export::{
    SlicerInstall, build_export_filename, detect_slicer_paths, export_model, send_to_slicer,
};
pub use file::{canonicalize_or_original, read_binary_file, read_file_response, read_text_file};
pub use presets::{PresetError, delete_preset, load_presets, preset_path_for_source, save_preset};
pub use preview::{
    CliOutputFormat, LogEntry, LogLevel, OpenScadError, OpenScadMessage, OpenScadRunner,
    RenderedArtifact, build_cli_args, build_preview_job_args, collect_process_logs,
    detect_openscad_path, finalize_job, mesh_to_preview_payload, preview_artifact,
    preview_ready_response, resolve_openscad_path,
};
pub use watch::{FileWatcher, WatchError, WatchMessage, matches_any_path, matches_path};
pub use workspace::{current_workspace, list_workspace_entries, resolve_workspace_path};
