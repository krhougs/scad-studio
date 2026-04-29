mod dispatcher;
mod in_process;
mod mpsc_transport;
pub mod plan_extraction;
mod runtime;
mod session;
mod websocket;

pub use app_server_transport::ClientTransport;
pub use dispatcher::{
    HostRequestDispatcher, ServerPushSink, agent_error_type, validate_cadquery_confirmation,
};
pub use in_process::{
    AbortDecision, AbortStrategy, GUI_SHUTDOWN_TIMEOUT, InProcessHost, JoinThenAbort,
    evaluate_shutdown,
};
pub use mpsc_transport::{MpscTransportAdapter, MpscTransportHarness};
pub use plan_extraction::{
    ExtractedPlan, export_handle_for, extract_object_name, extract_plan_from_json_block,
    extract_plan_from_selection, extract_plan_proposal, latest_saved_cad_plan, parse_plan_package,
    validate_saved_plan_confirmation,
};
pub use runtime::spawn_in_process_mpsc_host;
pub use session::HostSession;
pub use websocket::{WebSocketHostConfig, run_websocket_host, run_websocket_host_once};
