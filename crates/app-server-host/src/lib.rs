mod cadquery_env;
mod dispatcher;
pub mod plan_extraction;
mod session;
mod websocket;

pub use app_server_transport::ClientTransport;
pub use cadquery_env::{cadquery_python_path, verify_cadquery_runner_environment};
pub use dispatcher::{
    HostRequestDispatcher, ServerPushSink, agent_error_type, validate_cadquery_confirmation,
    watch_changed_paths_to_handles,
};
pub use plan_extraction::{
    ExtractedPlan, execution_scope_from_plan_ref, export_handle_for, extract_object_name,
    extract_plan_from_json_block, extract_plan_from_selection, extract_plan_proposal,
    latest_saved_cad_plan, parse_plan_package, validate_saved_plan_confirmation,
};
pub use session::HostSession;
pub use websocket::{WebSocketHostConfig, run_websocket_host, run_websocket_host_once};
