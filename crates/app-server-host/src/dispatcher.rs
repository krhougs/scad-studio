use app_server_core::{
    AgentCadQueryCodeInput, AgentTurnInput, CadQueryCommitScope, CadQueryExecuteConfig,
    CadQueryRunConfig, CadQueryRunResult, CadQueryRunnerError, CadQueryRunnerErrorKind, ChatStore,
    FileWatcher, SlicerInstall, current_workspace, detect_slicer_paths, draft_agent_turn,
    execute_cadquery_with_staging_cancellable_scoped, export_model, generate_cadquery_code,
    list_workspace_entries, load_config_dto, preview_ready_response, read_file_response,
    resolve_workspace_path, resolve_workspace_write_path, run_cadquery_runner, save_config_dto,
    send_to_slicer, stage_cadquery_project,
};
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentCancelRequest, AgentCancelledResponse, AgentDoneEvent,
    AgentErrorEvent, AgentErrorType, AgentInvokeRequest, AgentMeshReadyEvent, AgentOperationLevel,
    AgentStartedResponse, AgentTokenEvent, AgentToolResultEvent, AgentToolStartEvent,
    CadQueryExecuteRequest, CadQueryExportFormat, CadQueryMeshPayload, CapabilityHandshakeRequest,
    CapabilityHandshakeResponse, ChatRole, ChatToolCallRecord, ChatToolResultRecord, ClientCommand,
    ClientRequestEnvelope, CommandSuccess, ConfigLoadResponse, DEFAULT_SESSION_RECONNECT_WINDOW_MS,
    ExportRunResponse, FileWriteTextResponse, HostLocalPath, PathHandle, PreviewRequestKind,
    ProtocolError, ProtocolErrorCode, ProtocolVersionRange, SelectionUpdateRequest,
    SelectionUpdateResponse, ServerCapabilities, ServerPushEnvelope, ServerPushEvent,
    ServerResponseEnvelope, SessionReclaimedResponse, SessionToken, SubscriptionId,
    WatchChangedEvent, WatchErrorEvent, WatchSubscriptionAck, WorkspaceId, WorkspaceListResponse,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use crate::HostSession;

pub type ServerPushSink = Arc<dyn Fn(ServerPushEnvelope) + Send + Sync>;

pub struct HostRequestDispatcher {
    workspace_id: WorkspaceId,
    workspace_path: Option<PathBuf>,
    denied_extensions: Vec<String>,
    next_subscription_id: u64,
    watchers: HashMap<String, FileWatcher>,
    cadquery_results: Arc<Mutex<HashMap<String, CadQueryMeshPayload>>>,
    agent_runs: Arc<Mutex<AgentRunRegistry>>,
    selection_snapshot: SelectionUpdateRequest,
    push_sink: ServerPushSink,
    session: HostSession,
}

impl HostRequestDispatcher {
    pub fn new(
        workspace_path: Option<PathBuf>,
        denied_extensions: Vec<String>,
        push_sink: ServerPushSink,
    ) -> Self {
        Self::with_session_token(
            workspace_path,
            SessionToken("session-1".into()),
            denied_extensions,
            push_sink,
        )
    }

    pub fn with_session_token(
        workspace_path: Option<PathBuf>,
        session_token: SessionToken,
        denied_extensions: Vec<String>,
        push_sink: ServerPushSink,
    ) -> Self {
        Self {
            workspace_id: WorkspaceId::new("workspace"),
            workspace_path,
            denied_extensions,
            next_subscription_id: 1,
            watchers: HashMap::new(),
            cadquery_results: Arc::new(Mutex::new(HashMap::new())),
            agent_runs: Arc::new(Mutex::new(AgentRunRegistry::default())),
            selection_snapshot: SelectionUpdateRequest {
                selections: Vec::new(),
                active_index: None,
            },
            push_sink,
            session: HostSession::new(session_token, server_capabilities()),
        }
    }

    pub fn rebind_workspace(&mut self, workspace_path: PathBuf) {
        self.workspace_path = Some(workspace_path);
    }

    pub fn handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> CapabilityHandshakeResponse {
        let server_capabilities = server_capabilities_for_request(&request);
        self.session
            .replace_capabilities(server_capabilities.clone());
        CapabilityHandshakeResponse {
            negotiated_version: server_capabilities.protocol_version.max,
            session_token: self.session.token().clone(),
            server_capabilities,
        }
    }

    pub fn dispatch_envelope(&mut self, envelope: ClientRequestEnvelope) -> ServerResponseEnvelope {
        self.session.track_request(envelope.request_id);
        let result = self.dispatch_command(envelope.command);
        self.session.complete_request(&envelope.request_id);
        ServerResponseEnvelope {
            request_id: envelope.request_id,
            result,
        }
    }

    pub fn disconnect(&mut self) {
        self.watchers.clear();
        self.session.disconnect(
            Instant::now(),
            Duration::from_millis(DEFAULT_SESSION_RECONNECT_WINDOW_MS),
        );
    }

    fn dispatch_command(
        &mut self,
        command: ClientCommand,
    ) -> Result<CommandSuccess, ProtocolError> {
        match command {
            ClientCommand::WorkspaceCurrent => {
                let workspace_path = self.workspace_root()?;
                let current = current_workspace(workspace_path, self.workspace_id.clone());
                self.session.bind_workspace(current.clone());
                Ok(CommandSuccess::WorkspaceCurrent(current))
            }
            ClientCommand::WorkspaceList(request) => {
                let workspace_path = self.workspace_root()?;
                let response = list_workspace_entries(
                    workspace_path,
                    self.workspace_id.clone(),
                    request.directory.as_ref(),
                )?;
                self.record_workspace_entries(&response);
                Ok(CommandSuccess::WorkspaceList(response))
            }
            ClientCommand::FileRead(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                self.session.issue_handle(request.path.clone());
                read_file_response(&workspace_path, &request.path, &self.denied_extensions)
                    .map(CommandSuccess::FileRead)
            }
            ClientCommand::FileWriteText(request) => {
                let workspace_path = self.workspace_root()?;
                let resolved = resolve_workspace_write_path(workspace_path, &request.path)?;
                fs::write(&resolved, request.contents).map_err(internal_error)?;
                self.session.issue_handle(request.path.clone());
                Ok(CommandSuccess::FileWritten(FileWriteTextResponse {
                    path: request.path,
                }))
            }
            ClientCommand::ConfigLoad => {
                let config = load_config_dto().map_err(internal_error)?;
                Ok(CommandSuccess::ConfigLoaded(ConfigLoadResponse { config }))
            }
            ClientCommand::ConfigSave(request) => {
                save_config_dto(&request.config).map_err(internal_error)?;
                Ok(CommandSuccess::ConfigSaved)
            }
            ClientCommand::PreviewRequest(request) => {
                let workspace_path = self.workspace_root()?;
                let source_path = resolve_workspace_path(workspace_path, &request.source)?;
                self.session.issue_handle(request.source.clone());
                preview_ready_response(
                    request
                        .configured_openscad_path
                        .map(|path| path.to_path_buf()),
                    &source_path,
                    &request.defines,
                )
                .map(CommandSuccess::PreviewReady)
                .map_err(internal_error)
            }
            ClientCommand::CadQueryPreview(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let source_path = resolve_workspace_path(&workspace_path, &request.target_path)?;
                let code = fs::read_to_string(&source_path).map_err(internal_error)?;
                let script = path_handle_to_relative_path(&request.target_path);
                let commit_scope =
                    default_cadquery_commit_scope(&request.target_path, &request.export_formats);
                let staged = stage_cadquery_project(&workspace_path, &script, &code)
                    .map_err(internal_error)?;
                self.session.issue_handle(request.target_path);
                let result = run_cadquery_runner(&CadQueryRunConfig {
                    python: cadquery_python_path(),
                    project_root: staged.root().to_path_buf(),
                    script: display_relative_path(&script),
                    output_dir: staged.output_dir(),
                    export_formats: request.export_formats,
                    params_json: request.params_json,
                    timeout: Duration::from_secs(60),
                })
                .map_err(internal_error)?;
                staged
                    .commit_outputs_with_scope(&commit_scope)
                    .map_err(cadquery_command_error)?;
                let ready = result.ready.clone();
                self.cache_cadquery_mesh(result)?;
                Ok(CommandSuccess::CadQueryResultReady(ready))
            }
            ClientCommand::CadQueryExecute(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let _target_path =
                    resolve_workspace_write_path(&workspace_path, &request.target_path)?;
                let commit_scope =
                    default_cadquery_commit_scope(&request.target_path, &request.export_formats);
                let target = path_handle_to_relative_path(&request.target_path);
                self.session.issue_handle(request.target_path.clone());
                let result = execute_cadquery_with_staging_cancellable_scoped(
                    &CadQueryExecuteConfig {
                        python: cadquery_python_path(),
                        workspace_root: workspace_path,
                        target_relative_path: target,
                        code: request.code,
                        export_formats: request.export_formats,
                        params_json: request.params_json,
                        timeout: Duration::from_secs(60),
                    },
                    &|| false,
                    &commit_scope,
                )
                .map_err(cadquery_command_error)?;
                let ready = result.ready.clone();
                self.cache_cadquery_mesh(result)?;
                Ok(CommandSuccess::CadQueryResultReady(ready))
            }
            ClientCommand::CadQueryResultGet(request) => {
                let payload = self
                    .cadquery_results
                    .lock()
                    .map_err(|_| internal_error("CadQuery result cache lock poisoned"))?
                    .get(&request.result_id)
                    .cloned()
                    .ok_or_else(|| {
                        ProtocolError::new(
                            ProtocolErrorCode::NotFound,
                            format!("未找到 CadQuery result: {}", request.result_id),
                        )
                    })?;
                Ok(CommandSuccess::CadQueryMesh(payload))
            }
            ClientCommand::ChatCreate(request) => {
                self.issue_handles(&request.related_files);
                self.chat_store()?
                    .create(&request.title, request.goal, request.related_files)
                    .map(CommandSuccess::ChatCreated)
            }
            ClientCommand::ChatList(request) => self
                .chat_store()?
                .list(request.include_archived)
                .map(CommandSuccess::ChatList),
            ClientCommand::ChatSend(request) => {
                self.issue_handles(&request.related_files);
                self.chat_store()?
                    .append_message(
                        &request.session_id,
                        ChatRole::User,
                        &request.content,
                        request.related_files,
                        None,
                    )
                    .map(CommandSuccess::ChatAck)
            }
            ClientCommand::ChatHistory(request) => self
                .chat_store()?
                .history(&request.session_id, request.limit)
                .map(CommandSuccess::ChatHistory),
            ClientCommand::ChatArchive(request) => self
                .chat_store()?
                .archive(&request.session_id)
                .map(CommandSuccess::ChatArchived),
            ClientCommand::AgentInvoke(request) => self.start_agent(request),
            ClientCommand::AgentCancel(request) => self.cancel_agent(request),
            ClientCommand::SelectionUpdate(request) => self
                .update_selection_snapshot(request)
                .map(CommandSuccess::SelectionUpdated),
            ClientCommand::SlicerList(request) => {
                let configured = request
                    .configured
                    .into_iter()
                    .map(|item| SlicerInstall {
                        name: item.name,
                        path: item.path.to_path_buf(),
                    })
                    .collect::<Vec<_>>();
                let slicers = detect_slicer_paths(&configured)
                    .into_iter()
                    .map(|item| {
                        Ok(app_server_protocol::SlicerInstallRecord {
                            name: item.name,
                            path: path_buf_to_host_path(item.path)?,
                        })
                    })
                    .collect::<Result<Vec<_>, ProtocolError>>()?;
                Ok(CommandSuccess::SlicerListed(
                    app_server_protocol::SlicerListResponse { slicers },
                ))
            }
            ClientCommand::ExportRun(request) => {
                let workspace_path = self.workspace_root()?;
                let source_path = resolve_workspace_path(workspace_path, &request.source)?;
                let output_handle = request.output_path.clone();
                let output_path = resolve_workspace_write_path(workspace_path, &output_handle)?;
                let configured_slicers = request
                    .configured_slicers
                    .iter()
                    .map(|item| SlicerInstall {
                        name: item.name.clone(),
                        path: item.path.to_path_buf(),
                    })
                    .collect::<Vec<_>>();
                export_model(
                    request
                        .configured_openscad_path
                        .map(|path| path.to_path_buf()),
                    &source_path,
                    &request.defines,
                    &output_path,
                    request.format,
                )
                .map_err(internal_error)?;
                if let Some(name) = request.slicer_name {
                    let slicer = detect_slicer_paths(&configured_slicers)
                        .into_iter()
                        .find(|item| item.name == name)
                        .ok_or_else(|| {
                            ProtocolError::new(
                                ProtocolErrorCode::NotFound,
                                format!("未找到切片软件 {name}"),
                            )
                        })?;
                    send_to_slicer(&slicer.path, &output_path).map_err(internal_error)?;
                }
                Ok(CommandSuccess::ExportRun(ExportRunResponse {
                    output_path: output_handle,
                }))
            }
            ClientCommand::WatchSubscribe(request) => {
                let workspace_path = self.workspace_root()?;
                let watched_handle = request.directory.unwrap_or_else(|| {
                    app_server_protocol::PathHandle::new(
                        self.workspace_id.clone(),
                        Vec::<String>::new(),
                    )
                    .expect("root workspace handle should be valid")
                });
                let watched_path = resolve_workspace_path(workspace_path, &watched_handle)?;
                let subscription_id = SubscriptionId(format!("sub-{}", self.next_subscription_id));
                self.next_subscription_id += 1;
                let watcher = build_watcher(
                    Arc::clone(&self.push_sink),
                    subscription_id.clone(),
                    watched_handle.clone(),
                    watched_path,
                )?;
                self.watchers.insert(subscription_id.0.clone(), watcher);
                self.session.issue_handle(watched_handle);
                self.session.track_subscription(subscription_id.clone());
                Ok(CommandSuccess::WatchSubscribed(WatchSubscriptionAck {
                    subscription_id,
                }))
            }
            ClientCommand::WatchUnsubscribe(request) => {
                self.watchers.remove(&request.subscription_id.0);
                self.session.untrack_subscription(&request.subscription_id);
                Ok(CommandSuccess::WatchUnsubscribed(WatchSubscriptionAck {
                    subscription_id: request.subscription_id,
                }))
            }
            ClientCommand::Cancel(cancel) => {
                self.session.cancel_request(&cancel.request_id);
                Ok(CommandSuccess::CancelAccepted(cancel))
            }
            ClientCommand::SessionReclaim(request) => {
                if request.session_token != *self.session.token() {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::SessionExpired,
                        "session token 不匹配",
                    ));
                }
                if self.session.can_reclaim(Instant::now()) || self.session.workspace().is_some() {
                    Ok(CommandSuccess::SessionReclaimed(SessionReclaimedResponse {
                        workspace: self.session.workspace().cloned(),
                        reclaimed_capabilities: self.session.capabilities().clone(),
                    }))
                } else {
                    Err(ProtocolError::new(
                        ProtocolErrorCode::SessionExpired,
                        "session reclaim window 已过期",
                    ))
                }
            }
        }
    }

    fn workspace_root(&self) -> Result<&Path, ProtocolError> {
        self.workspace_path.as_deref().ok_or_else(|| {
            ProtocolError::new(ProtocolErrorCode::NotFound, "当前 host 尚未绑定 workspace")
        })
    }

    fn record_workspace_entries(&mut self, response: &WorkspaceListResponse) {
        for entry in &response.entries {
            if let Some(path) = &entry.path {
                self.session.issue_handle(path.clone());
            }
        }
    }

    fn chat_store(&self) -> Result<ChatStore, ProtocolError> {
        Ok(ChatStore::new(self.workspace_root()?.to_path_buf()))
    }

    fn issue_handles(&mut self, handles: &[PathHandle]) {
        for handle in handles {
            self.session.issue_handle(handle.clone());
        }
    }

    fn cache_cadquery_mesh(&self, result: CadQueryRunResult) -> Result<(), ProtocolError> {
        self.cadquery_results
            .lock()
            .map_err(|_| internal_error("CadQuery result cache lock poisoned"))?
            .insert(result.ready.result_id.clone(), result.mesh);
        Ok(())
    }

    fn update_selection_snapshot(
        &mut self,
        request: SelectionUpdateRequest,
    ) -> Result<SelectionUpdateResponse, ProtocolError> {
        if let Some(active_index) = request.active_index {
            if active_index as usize >= request.selections.len() {
                return Err(ProtocolError::new(
                    ProtocolErrorCode::InvalidCommand,
                    "active selection index 超出 selections 范围",
                ));
            }
        }
        let accepted_count = request.selections.len() as u32;
        self.selection_snapshot = request;
        Ok(SelectionUpdateResponse { accepted_count })
    }

    fn start_agent(
        &mut self,
        request: AgentInvokeRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        self.chat_store()?.history(&request.session_id, Some(1))?;
        let run = self
            .agent_runs
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .try_start(request.session_id.clone())?;
        let response = AgentStartedResponse {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
        };
        let worker = AgentWorker {
            run,
            prompt: request.prompt,
            operation: request.operation,
            confirmed_cadquery: request.confirmed_cadquery,
            selection_snapshot: self.selection_snapshot.clone(),
            workspace_root: self.workspace_root()?.to_path_buf(),
            python: cadquery_python_path(),
            cadquery_results: Arc::clone(&self.cadquery_results),
            agent_runs: Arc::clone(&self.agent_runs),
            push_sink: Arc::clone(&self.push_sink),
        };
        thread::spawn(move || run_agent_worker(worker));
        Ok(CommandSuccess::AgentStarted(response))
    }

    fn cancel_agent(
        &mut self,
        request: AgentCancelRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        let cancelled = self
            .agent_runs
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .cancel(request.run_id.as_deref());
        if let Some(run) = cancelled {
            Ok(CommandSuccess::AgentCancelled(AgentCancelledResponse {
                run_id: Some(run.run_id),
            }))
        } else {
            Ok(CommandSuccess::AgentCancelled(AgentCancelledResponse {
                run_id: None,
            }))
        }
    }
}

#[derive(Default)]
struct AgentRunRegistry {
    next_run_id: u64,
    running: Option<AgentRunHandle>,
}

impl AgentRunRegistry {
    fn try_start(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
    ) -> Result<AgentRunHandle, ProtocolError> {
        if self.running.is_some() {
            return Err(ProtocolError::new(
                ProtocolErrorCode::AgentBusy,
                "已有 Agent session 正在运行",
            ));
        }
        self.next_run_id = self.next_run_id.saturating_add(1);
        let run = AgentRunHandle {
            session_id,
            run_id: format!("agent-{}", self.next_run_id),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        self.running = Some(run.clone());
        Ok(run)
    }

    fn cancel(&mut self, requested_run_id: Option<&str>) -> Option<AgentRunHandle> {
        let run = self.running.as_ref()?;
        let should_cancel = requested_run_id
            .map(|requested| requested == run.run_id)
            .unwrap_or(true);
        if should_cancel {
            run.cancelled.store(true, Ordering::SeqCst);
            Some(run.clone())
        } else {
            None
        }
    }

    fn finish_if_current(&mut self, run_id: &str) -> Option<AgentRunHandle> {
        let is_current = self
            .running
            .as_ref()
            .is_some_and(|run| run.run_id == run_id);
        is_current.then(|| self.running.take()).flatten()
    }
}

#[derive(Clone)]
struct AgentRunHandle {
    session_id: app_server_protocol::ChatSessionId,
    run_id: String,
    cancelled: Arc<AtomicBool>,
}

struct AgentWorker {
    run: AgentRunHandle,
    prompt: String,
    operation: AgentOperationLevel,
    confirmed_cadquery: Option<AgentCadQueryConfirmation>,
    selection_snapshot: SelectionUpdateRequest,
    workspace_root: PathBuf,
    python: PathBuf,
    cadquery_results: Arc<Mutex<HashMap<String, CadQueryMeshPayload>>>,
    agent_runs: Arc<Mutex<AgentRunRegistry>>,
    push_sink: ServerPushSink,
}

fn run_agent_worker(worker: AgentWorker) {
    match worker.operation {
        AgentOperationLevel::Execute => run_execute_agent(worker),
        AgentOperationLevel::Inform | AgentOperationLevel::Plan => run_text_agent(worker),
    }
}

fn run_text_agent(worker: AgentWorker) {
    let response_text = agent_response_text(&worker);
    push_agent_token(&worker.push_sink, &worker.run, &response_text);
    thread::sleep(Duration::from_millis(120));
    if worker.run.cancelled.load(Ordering::SeqCst) {
        finish_agent_worker(worker, true);
        return;
    }
    append_agent_message(&worker.workspace_root, &worker.run, &response_text);
    finish_agent_worker(worker, false);
}

fn agent_response_text(worker: &AgentWorker) -> String {
    let store = ChatStore::new(worker.workspace_root.clone());
    let history = store
        .history(&worker.run.session_id, Some(8))
        .map(|response| response.messages)
        .unwrap_or_default();
    let confirmed_target_path = worker
        .confirmed_cadquery
        .as_ref()
        .map(|confirmation| confirmation.request.target_path.display_path().to_owned());
    draft_agent_turn(AgentTurnInput {
        operation: worker.operation,
        prompt: worker.prompt.clone(),
        history,
        selections: worker.selection_snapshot.selections.clone(),
        active_selection_index: worker.selection_snapshot.active_index,
        confirmed_target_path,
    })
    .text
}

fn run_execute_agent(worker: AgentWorker) {
    let Some(confirmation) = execute_confirmation_or_report(&worker) else {
        finish_agent_worker(worker, false);
        return;
    };
    let Some(generated) = generate_cadquery_or_report(&worker, &confirmation) else {
        finish_agent_worker(worker, false);
        return;
    };
    if push_execute_intro_or_cancelled(&worker, &generated.response_text) {
        finish_agent_worker(worker, true);
        return;
    }
    let mut request = confirmation.request;
    request.code = generated.code;
    push_agent_tool_start(&worker.push_sink, &worker.run, &request);
    append_agent_tool_call(&worker.workspace_root, &worker.run, &request);
    let result = execute_confirmed_cadquery(&worker, request, &confirmation.export_targets);
    if handle_execute_result(&worker, result) {
        finish_agent_worker(worker, true);
        return;
    }
    append_agent_message(
        &worker.workspace_root,
        &worker.run,
        &generated.response_text,
    );
    finish_agent_worker(worker, false);
}

fn execute_confirmation_or_report(worker: &AgentWorker) -> Option<AgentCadQueryConfirmation> {
    let Some(confirmation) = worker.confirmed_cadquery.clone() else {
        report_execute_permission_error(
            worker,
            "Execute 需要 confirmed_cadquery 才能写入并执行 CadQuery",
        );
        return None;
    };
    if let Err(message) = validate_cadquery_confirmation(&confirmation) {
        report_execute_permission_error(worker, message);
        return None;
    }
    Some(confirmation)
}

fn report_execute_permission_error(worker: &AgentWorker, message: &str) {
    let response_text = agent_response_text(worker);
    push_agent_error(
        &worker.push_sink,
        &worker.run,
        AgentErrorType::PermissionDenied,
        message,
    );
    append_agent_message(&worker.workspace_root, &worker.run, &response_text);
}

fn generate_cadquery_or_report(
    worker: &AgentWorker,
    confirmation: &AgentCadQueryConfirmation,
) -> Option<app_server_core::GeneratedCadQueryCode> {
    match generate_agent_cadquery(worker, confirmation) {
        Ok(generated) => Some(generated),
        Err(error) => {
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::LlmError,
                error.message,
            );
            None
        }
    }
}

fn push_execute_intro_or_cancelled(worker: &AgentWorker, response_text: &str) -> bool {
    push_agent_token(&worker.push_sink, &worker.run, response_text);
    thread::sleep(Duration::from_millis(120));
    worker.run.cancelled.load(Ordering::SeqCst)
}

fn handle_execute_result(
    worker: &AgentWorker,
    result: Result<CadQueryRunResult, CadQueryRunnerError>,
) -> bool {
    match result {
        Ok(result) => {
            let ready = result.ready.clone();
            if let Ok(mut cache) = worker.cadquery_results.lock() {
                cache.insert(ready.result_id.clone(), result.mesh);
            }
            push_agent_tool_result(&worker.push_sink, &worker.run, &ready);
            append_agent_tool_result(&worker.workspace_root, &worker.run, &ready);
            push_agent_mesh_ready(&worker.push_sink, &worker.run, ready);
            false
        }
        Err(error) if error.kind == CadQueryRunnerErrorKind::Cancelled => true,
        Err(error) => {
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                agent_error_type(&error.kind),
                error.message,
            );
            false
        }
    }
}

fn execute_confirmed_cadquery(
    worker: &AgentWorker,
    request: CadQueryExecuteRequest,
    export_targets: &[PathHandle],
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    let _target = resolve_workspace_write_path(&worker.workspace_root, &request.target_path)
        .map_err(protocol_to_cadquery_error)?;
    let target_relative_path = path_handle_to_relative_path(&request.target_path);
    let commit_scope = CadQueryCommitScope::ExactOutputs(
        export_targets
            .iter()
            .map(path_handle_to_relative_path)
            .collect(),
    );
    execute_cadquery_with_staging_cancellable_scoped(
        &CadQueryExecuteConfig {
            python: worker.python.clone(),
            workspace_root: worker.workspace_root.clone(),
            target_relative_path,
            code: request.code,
            export_formats: request.export_formats,
            params_json: request.params_json,
            timeout: Duration::from_secs(60),
        },
        &|| worker.run.cancelled.load(Ordering::SeqCst),
        &commit_scope,
    )
}

fn generate_agent_cadquery(
    worker: &AgentWorker,
    confirmation: &AgentCadQueryConfirmation,
) -> Result<app_server_core::GeneratedCadQueryCode, app_server_core::AgentBackendError> {
    let store = ChatStore::new(worker.workspace_root.clone());
    let history = store
        .history(&worker.run.session_id, Some(8))
        .map(|response| response.messages)
        .unwrap_or_default();
    generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: worker.prompt.clone(),
        history,
        selections: worker.selection_snapshot.selections.clone(),
        active_selection_index: worker.selection_snapshot.active_index,
        target_display_path: confirmation.request.target_path.display_path(),
        target_type: confirmation.request.target_type,
    })
}

fn validate_cadquery_confirmation(
    confirmation: &AgentCadQueryConfirmation,
) -> Result<(), &'static str> {
    let target = &confirmation.request.target_path;
    if !contains_path(&confirmation.affected_files, target)
        && !contains_path(&confirmation.new_files, target)
    {
        return Err("CadQuery target_path 不在已确认的 affected_files / new_files 范围内");
    }
    if !confirmation.request.export_formats.is_empty() && confirmation.export_targets.is_empty() {
        return Err("CadQuery export_formats 非空时必须提供已确认的 export_targets");
    }
    if confirmation
        .export_targets
        .iter()
        .any(|path| !path_handle_to_relative_path(path).starts_with("outputs"))
    {
        return Err("CadQuery export_targets 必须位于 outputs/ 目录");
    }
    Ok(())
}

fn contains_path(paths: &[PathHandle], target: &PathHandle) -> bool {
    paths.iter().any(|path| path == target)
}

fn finish_agent_worker(worker: AgentWorker, cancelled: bool) {
    let finished = worker
        .agent_runs
        .lock()
        .ok()
        .and_then(|mut registry| registry.finish_if_current(&worker.run.run_id));
    if let Some(run) = finished {
        push_agent_done(&worker.push_sink, &run, cancelled);
    }
}

fn append_agent_message(workspace_root: &Path, run: &AgentRunHandle, content: &str) {
    let _ = ChatStore::new(workspace_root.to_path_buf()).append_message(
        &run.session_id,
        ChatRole::Assistant,
        content,
        Vec::new(),
        None,
    );
}

fn append_agent_tool_call(
    workspace_root: &Path,
    run: &AgentRunHandle,
    request: &CadQueryExecuteRequest,
) {
    let _ = ChatStore::new(workspace_root.to_path_buf()).append_tool_call(
        &run.session_id,
        "cadquery tool started",
        ChatToolCallRecord {
            tool_call_id: tool_call_id(run),
            tool_name: "cadquery".into(),
            args_json: cadquery_tool_args_json(request),
        },
    );
}

fn append_agent_tool_result(
    workspace_root: &Path,
    run: &AgentRunHandle,
    ready: &app_server_protocol::CadQueryResultReady,
) {
    let _ = ChatStore::new(workspace_root.to_path_buf()).append_tool_result(
        &run.session_id,
        "cadquery tool completed",
        ChatToolResultRecord {
            tool_call_id: tool_call_id(run),
            tool_name: "cadquery".into(),
            result_json: cadquery_ready_json(ready),
        },
        Some(ready.clone()),
    );
}

fn push_agent_token(push_sink: &ServerPushSink, run: &AgentRunHandle, text: &str) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentToken(AgentTokenEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            text: text.to_owned(),
        }),
    });
}

fn push_agent_tool_start(
    push_sink: &ServerPushSink,
    run: &AgentRunHandle,
    request: &CadQueryExecuteRequest,
) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentToolStart(AgentToolStartEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            tool_call_id: tool_call_id(run),
            tool_name: "cadquery".into(),
            args_json: cadquery_tool_args_json(request),
        }),
    });
}

fn push_agent_tool_result(
    push_sink: &ServerPushSink,
    run: &AgentRunHandle,
    ready: &app_server_protocol::CadQueryResultReady,
) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentToolResult(AgentToolResultEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            tool_call_id: tool_call_id(run),
            tool_name: "cadquery".into(),
            result_json: cadquery_ready_json(ready),
        }),
    });
}

fn cadquery_tool_args_json(request: &CadQueryExecuteRequest) -> String {
    serde_json::json!({
        "target_path": request.target_path.display_path(),
        "target_type": request.target_type,
        "export_formats": request.export_formats,
    })
    .to_string()
}

fn cadquery_ready_json(ready: &app_server_protocol::CadQueryResultReady) -> String {
    serde_json::json!({
        "result_id": ready.result_id,
        "build_id": ready.build_id,
        "part_count": ready.part_count,
        "face_count": ready.face_count,
        "edge_count": ready.edge_count,
        "vertex_count": ready.vertex_count,
    })
    .to_string()
}

fn push_agent_mesh_ready(
    push_sink: &ServerPushSink,
    run: &AgentRunHandle,
    ready: app_server_protocol::CadQueryResultReady,
) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentMeshReady(AgentMeshReadyEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            result: ready,
        }),
    });
}

fn push_agent_error(
    push_sink: &ServerPushSink,
    run: &AgentRunHandle,
    error_type: AgentErrorType,
    message: impl Into<String>,
) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentError(AgentErrorEvent {
            session_id: run.session_id.clone(),
            run_id: Some(run.run_id.clone()),
            error_type,
            message: message.into(),
        }),
    });
}

fn push_agent_done(push_sink: &ServerPushSink, run: &AgentRunHandle, cancelled: bool) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentDone(AgentDoneEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            cancelled,
        }),
    });
}

fn tool_call_id(run: &AgentRunHandle) -> String {
    format!("{}-cadquery-1", run.run_id)
}

fn agent_error_type(kind: &CadQueryRunnerErrorKind) -> AgentErrorType {
    match kind {
        CadQueryRunnerErrorKind::Build => AgentErrorType::CadQueryBuildError,
        CadQueryRunnerErrorKind::FileConflict => AgentErrorType::FileConflict,
        CadQueryRunnerErrorKind::Timeout => AgentErrorType::Timeout,
        CadQueryRunnerErrorKind::Cancelled => AgentErrorType::Timeout,
        CadQueryRunnerErrorKind::PermissionDenied => AgentErrorType::PermissionDenied,
        CadQueryRunnerErrorKind::InvalidProjectPath
        | CadQueryRunnerErrorKind::Io
        | CadQueryRunnerErrorKind::Runner => AgentErrorType::CadQueryBuildError,
    }
}

fn protocol_to_cadquery_error(error: ProtocolError) -> CadQueryRunnerError {
    CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::InvalidProjectPath,
        message: error.message,
    }
}

fn cadquery_command_error(error: CadQueryRunnerError) -> ProtocolError {
    match error.kind {
        CadQueryRunnerErrorKind::PermissionDenied => {
            ProtocolError::new(ProtocolErrorCode::InvalidCommand, error.message)
        }
        CadQueryRunnerErrorKind::InvalidProjectPath => {
            ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, error.message)
        }
        _ => internal_error(error),
    }
}

fn default_cadquery_commit_scope(
    target_path: &PathHandle,
    formats: &[CadQueryExportFormat],
) -> CadQueryCommitScope {
    let stem = cadquery_target_stem(target_path);
    let paths = formats
        .iter()
        .map(|format| PathBuf::from("outputs").join(cadquery_export_file_name(&stem, format)))
        .collect();
    CadQueryCommitScope::ExactOutputs(paths)
}

fn cadquery_target_stem(target_path: &PathHandle) -> String {
    target_path
        .path_segments()
        .last()
        .and_then(|file_name| Path::new(file_name).file_stem())
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("cadquery")
        .to_owned()
}

fn cadquery_export_file_name(stem: &str, format: &CadQueryExportFormat) -> String {
    let extension = match format {
        CadQueryExportFormat::Step => "step",
        CadQueryExportFormat::Stl => "stl",
        CadQueryExportFormat::ThreeMf => "3mf",
    };
    format!("{stem}.{extension}")
}

fn build_watcher(
    push_sink: ServerPushSink,
    subscription_id: SubscriptionId,
    watched_handle: app_server_protocol::PathHandle,
    watched_path: PathBuf,
) -> Result<FileWatcher, ProtocolError> {
    let watcher = FileWatcher::new(move |message| match message {
        app_server_core::WatchMessage::Changed(_) => {
            (push_sink)(ServerPushEnvelope {
                event: ServerPushEvent::WatchChanged(WatchChangedEvent {
                    subscription_id: subscription_id.clone(),
                    changed_paths: vec![watched_handle.clone()],
                }),
            });
        }
        app_server_core::WatchMessage::Error(message) => {
            (push_sink)(ServerPushEnvelope {
                event: ServerPushEvent::WatchError(WatchErrorEvent {
                    subscription_id: subscription_id.clone(),
                    message,
                }),
            });
        }
    });
    watcher.watch_files(vec![watched_path]);
    Ok(watcher)
}

fn internal_error(error: impl std::fmt::Display) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::Internal, error.to_string())
}

fn path_buf_to_host_path(path: PathBuf) -> Result<HostLocalPath, ProtocolError> {
    let value = path.to_str().ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidHostLocalPath,
            "host-local path 必须是 UTF-8",
        )
    })?;
    HostLocalPath::new(value)
}

fn path_handle_to_relative_path(path: &PathHandle) -> PathBuf {
    path.path_segments().iter().collect()
}

fn display_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn cadquery_python_path() -> PathBuf {
    std::env::var_os("CADQUERY_RUNNER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("python3"))
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        protocol_version: ProtocolVersionRange::new(2, 2),
        reconnect_window_ms: DEFAULT_SESSION_RECONNECT_WINDOW_MS,
        supports_watch: true,
        supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        supports_session_reclaim: true,
        cadquery: true,
        agent: true,
        selection_sync: true,
    }
}

fn server_capabilities_for_request(request: &CapabilityHandshakeRequest) -> ServerCapabilities {
    let mut capabilities = server_capabilities();
    capabilities.supported_preview_kinds = request
        .capabilities
        .supported_preview_kinds
        .iter()
        .filter(|kind| matches!(kind, PreviewRequestKind::GeometryArtifact))
        .cloned()
        .collect();
    if capabilities.supported_preview_kinds.is_empty() {
        capabilities.supported_preview_kinds = vec![PreviewRequestKind::GeometryArtifact];
    }
    capabilities
}
