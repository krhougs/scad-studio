use app_server_core::llm::LlmToolCall;
use app_server_core::{
    AgentToolRunContext, AgentTurnInput, CadQueryCommitScope, CadQueryRunConfig, CadQueryRunResult,
    CadQueryRunnerError, CadQueryRunnerErrorKind, CadQueryToolCachedResult, CadQueryToolRunRequest,
    CadQueryToolRunResult, CadQueryToolRuntime, CadQueryToolRuntimeError, ChatStore, FileWatcher,
    SlicerInstall, current_workspace, detect_slicer_paths, export_model, list_workspace_entries,
    load_config_dto, preview_ready_response, read_file_response, resolve_workspace_path,
    resolve_workspace_write_path, run_cadquery_runner, run_cadquery_runner_with_cancel,
    save_config_dto, send_to_slicer, stage_cadquery_project,
};
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentCancelRequest, AgentCancelledResponse, AgentDoneEvent,
    AgentErrorEvent, AgentErrorType, AgentInvokeRequest, AgentMeshReadyEvent, AgentMode,
    AgentPlanConfirmRequest, AgentPlanProposedEvent, AgentPlanRejectRequest, AgentStartedResponse,
    AgentTokenEvent, AgentToolResultEvent, AgentToolStartEvent, CURRENT_PROTOCOL_VERSION,
    CadQueryExportFormat, CadQueryMeshPayload, CadQueryObjectKind, CapabilityHandshakeRequest,
    CapabilityHandshakeResponse, ChatRole, ChatToolCallRecord, ChatToolResultRecord, ClientCommand,
    ClientRequestEnvelope, CommandSuccess, ConfigLoadResponse, DEFAULT_SESSION_RECONNECT_WINDOW_MS,
    ExportRunResponse, FileWriteTextResponse, HostLocalPath, PathHandle, PreviewRequestKind,
    ProtocolError, ProtocolErrorCode, ProtocolVersionRange, SelectionUpdateRequest,
    SelectionUpdateResponse, ServerCapabilities, ServerPushEnvelope, ServerPushEvent,
    ServerResponseEnvelope, SessionReclaimedResponse, SessionToken, SubscriptionId,
    WatchChangedEvent, WatchErrorEvent, WatchSubscriptionAck, WorkspaceId, WorkspaceListResponse,
    negotiate_protocol_version,
};
use std::collections::{HashMap, VecDeque};
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

const CADQUERY_RESULT_CACHE_LIMIT: usize = 8;

#[derive(Debug)]
struct CadQueryResultCache {
    limit: usize,
    order: VecDeque<String>,
    results: HashMap<String, CadQueryToolCachedResult>,
}

impl CadQueryResultCache {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            order: VecDeque::new(),
            results: HashMap::new(),
        }
    }

    fn insert(&mut self, _result_id: String, payload: CadQueryMeshPayload) {
        self.insert_cached(CadQueryToolCachedResult {
            mesh: payload,
            exports: Vec::new(),
            warnings: Vec::new(),
        });
    }

    fn insert_cached(&mut self, result: CadQueryToolCachedResult) {
        let result_id = result.mesh.result_id.clone();
        self.order.retain(|existing| existing != &result_id);
        self.order.push_back(result_id.clone());
        self.results.insert(result_id, result);
        while self.order.len() > self.limit {
            if let Some(evicted) = self.order.pop_front() {
                self.results.remove(&evicted);
            }
        }
    }

    fn get(&self, result_id: &str) -> Option<CadQueryMeshPayload> {
        self.results
            .get(result_id)
            .map(|result| result.mesh.clone())
    }

    fn get_cached(&self, result_id: &str) -> Option<CadQueryToolCachedResult> {
        self.results.get(result_id).cloned()
    }
}

pub struct HostRequestDispatcher {
    workspace_id: WorkspaceId,
    workspace_path: Option<PathBuf>,
    denied_extensions: Vec<String>,
    next_subscription_id: u64,
    watchers: HashMap<String, FileWatcher>,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
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
            cadquery_results: Arc::new(Mutex::new(CadQueryResultCache::new(
                CADQUERY_RESULT_CACHE_LIMIT,
            ))),
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
    ) -> Result<CapabilityHandshakeResponse, ProtocolError> {
        let server_capabilities = server_capabilities_for_request(&request);
        let negotiated_version = negotiate_protocol_version(
            request.capabilities.protocol_version,
            server_capabilities.protocol_version,
        )?;
        self.session
            .replace_capabilities(server_capabilities.clone());
        Ok(CapabilityHandshakeResponse {
            negotiated_version,
            session_token: self.session.token().clone(),
            server_capabilities,
        })
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
                if !request.export_formats.is_empty() {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::InvalidCommand,
                        "CadQuery preview 不允许 export_formats；需要写入 outputs 时请切换到 Agent mode 执行 CadQuery",
                    ));
                }
                let workspace_path = self.workspace_root()?.to_path_buf();
                let source_path = resolve_workspace_path(&workspace_path, &request.target_path)?;
                let code = fs::read_to_string(&source_path).map_err(internal_error)?;
                let script = path_handle_to_relative_path(&request.target_path);
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
                let ready = result.ready.clone();
                self.cache_cadquery_mesh(result)?;
                Ok(CommandSuccess::CadQueryResultReady(ready))
            }
            ClientCommand::CadQueryExecute(_request) => Err(ProtocolError::new(
                ProtocolErrorCode::InvalidCommand,
                "CadQuery execute 已迁移到 Agent Execute tool loop；直接协议写入已禁用",
            )),
            ClientCommand::CadQueryResultGet(request) => {
                let payload = self
                    .cadquery_results
                    .lock()
                    .map_err(|_| internal_error("CadQuery result cache lock poisoned"))?
                    .get(&request.result_id)
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
            ClientCommand::AgentPlanConfirm(request) => self.confirm_agent_plan(request),
            ClientCommand::AgentPlanReject(request) => self.reject_agent_plan(request),
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
            mode: request.mode,
            plan_ref: request.plan_ref,
            context_refs: request.context_refs,
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

    // request.run_id identifies the plan-proposing run; confirm creates a new Execute run.
    fn confirm_agent_plan(
        &mut self,
        _request: AgentPlanConfirmRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        Err(deprecated_command(
            "agent.plan.confirm 已废弃；请使用 agent.invoke { mode: Agent, plan_ref } 执行计划",
        ))
    }

    fn reject_agent_plan(
        &mut self,
        _request: AgentPlanRejectRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        Err(deprecated_command(
            "agent.plan.reject 已废弃；Plan package 不再需要确认或拒绝命令",
        ))
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
    mode: AgentMode,
    plan_ref: Option<PathHandle>,
    context_refs: Vec<String>,
    selection_snapshot: SelectionUpdateRequest,
    workspace_root: PathBuf,
    python: PathBuf,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
    agent_runs: Arc<Mutex<AgentRunRegistry>>,
    push_sink: ServerPushSink,
}

struct AgentToolEventRecorder {
    workspace_root: PathBuf,
    push_sink: ServerPushSink,
    run: AgentRunHandle,
}

impl app_server_core::ToolLoopObserver for AgentToolEventRecorder {
    fn tool_start(&self, call: &LlmToolCall) {
        push_llm_tool_start(&self.push_sink, &self.run, call);
        append_llm_tool_call(&self.workspace_root, &self.run, call);
    }

    fn tool_result(&self, call: &LlmToolCall, result: &str) {
        push_llm_tool_result(&self.push_sink, &self.run, call, result);
        append_llm_tool_result(&self.workspace_root, &self.run, call, result);
    }
}

struct HostCadQueryToolRuntime {
    workspace_root: PathBuf,
    python: PathBuf,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
    push_sink: ServerPushSink,
    run: AgentRunHandle,
}

impl CadQueryToolRuntime for HostCadQueryToolRuntime {
    fn dry_run(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let staged = stage_cadquery_project(
            &self.workspace_root,
            &PathBuf::from(&request.target_path),
            &request.code,
        )
        .map_err(cadquery_tool_error)?;
        let result = run_cadquery_runner_with_cancel(
            &CadQueryRunConfig {
                python: self.python.clone(),
                project_root: staged.root().to_path_buf(),
                script: staged.script_arg(),
                output_dir: staged.output_dir(),
                export_formats: Vec::new(),
                params_json: request.params_json,
                timeout: Duration::from_secs(60),
            },
            &|| self.run.cancelled.load(Ordering::SeqCst),
        )
        .map_err(cadquery_tool_error)?;
        self.finish_result(
            result,
            request.target_type,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn execute(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let commit_scope = CadQueryCommitScope::ExactOutputs(
            request.export_targets.iter().map(PathBuf::from).collect(),
        );
        let staged = stage_cadquery_project(
            &self.workspace_root,
            &PathBuf::from(&request.target_path),
            &request.code,
        )
        .map_err(cadquery_tool_error)?;
        let result = run_cadquery_runner_with_cancel(
            &CadQueryRunConfig {
                python: self.python.clone(),
                project_root: staged.root().to_path_buf(),
                script: staged.script_arg(),
                output_dir: staged.output_dir(),
                export_formats: request.export_formats.clone(),
                params_json: request.params_json,
                timeout: Duration::from_secs(60),
            },
            &|| self.run.cancelled.load(Ordering::SeqCst),
        )
        .map_err(cadquery_tool_error)?;
        validate_result_kind(&result.mesh, request.target_type)?;
        if let Some(doc_path) = request.doc_update_path.as_deref() {
            self.preflight_cadquery_doc_update(doc_path)?;
        }
        staged
            .commit_success_with_scope_cancellable(&commit_scope, &|| {
                self.run.cancelled.load(Ordering::SeqCst)
            })
            .map_err(cadquery_tool_error)?;
        let mut committed_files = vec![request.target_path];
        let mut extra_warnings = Vec::new();
        if let Some(doc_path) = request.doc_update_path {
            match self.append_cadquery_doc_update(&doc_path, &result) {
                Ok(()) => committed_files.push(doc_path),
                Err(warning) => extra_warnings.push(warning),
            }
        }
        self.finish_result(
            result,
            request.target_type,
            committed_files,
            request.export_targets,
            extra_warnings,
        )
    }

    fn get_result(&self, result_id: &str) -> Option<CadQueryToolCachedResult> {
        self.cadquery_results
            .lock()
            .ok()
            .and_then(|cache| cache.get_cached(result_id))
    }
}

impl HostCadQueryToolRuntime {
    fn finish_result(
        &self,
        result: CadQueryRunResult,
        expected_type: CadQueryObjectKind,
        committed_files: Vec<String>,
        exports: Vec<String>,
        extra_warnings: Vec<String>,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        validate_result_kind(&result.mesh, expected_type)?;
        let mut warnings = runner_warnings(&result.stderr);
        warnings.extend(extra_warnings);
        let cached = CadQueryToolCachedResult {
            mesh: result.mesh.clone(),
            exports: exports.clone(),
            warnings: warnings.clone(),
        };
        self.cadquery_results
            .lock()
            .map_err(|_| {
                CadQueryToolRuntimeError::new(
                    "cadquery_build_error",
                    "CadQuery result cache lock poisoned",
                    false,
                )
            })?
            .insert_cached(cached);
        push_agent_mesh_ready(&self.push_sink, &self.run, result.ready);
        Ok(CadQueryToolRunResult {
            mesh: result.mesh,
            committed_files,
            exports,
            warnings,
        })
    }

    fn preflight_cadquery_doc_update(
        &self,
        doc_path: &str,
    ) -> Result<(), CadQueryToolRuntimeError> {
        let absolute = self.workspace_root.join(doc_path);
        let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
            CadQueryToolRuntimeError::new(
                "file_conflict",
                format!("读取 CadQuery 说明文档失败: {error}"),
                false,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(CadQueryToolRuntimeError::new(
                "permission_denied",
                "CadQuery 说明文档不能是 symlink",
                false,
            ));
        }
        if is_hard_link(&metadata) {
            return Err(CadQueryToolRuntimeError::new(
                "permission_denied",
                "CadQuery 说明文档不能是 hard link",
                false,
            ));
        }
        fs::OpenOptions::new()
            .append(true)
            .open(&absolute)
            .map(|_| ())
            .map_err(|error| {
                CadQueryToolRuntimeError::new(
                    "file_conflict",
                    format!("CadQuery 说明文档不可写: {error}"),
                    false,
                )
            })
    }

    fn append_cadquery_doc_update(
        &self,
        doc_path: &str,
        result: &CadQueryRunResult,
    ) -> Result<(), String> {
        let absolute = self.workspace_root.join(doc_path);
        let note = format!(
            "\n\n## budn' CadQuery 执行记录\n\n- result_id: `{}`\n- build_id: `{}`\n",
            result.mesh.result_id, result.mesh.build_id
        );
        fs::OpenOptions::new()
            .append(true)
            .open(&absolute)
            .and_then(|mut file| {
                use std::io::Write;
                file.write_all(note.as_bytes())
            })
            .map_err(|error| format!("更新 CadQuery 说明文档失败: {error}"))
    }
}

fn run_agent_worker(worker: AgentWorker) {
    run_text_agent(worker);
}

fn run_text_agent(worker: AgentWorker) {
    let provider = match app_server_core::llm::create_provider() {
        Ok(p) => p,
        Err(error) => {
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::LlmError,
                error.message,
            );
            finish_agent_worker(worker, false);
            return;
        }
    };
    let response_text = match run_text_agent_llm(&worker, provider.as_ref()) {
        Some(text) => text,
        None => {
            finish_agent_worker(worker, false);
            return;
        }
    };
    thread::sleep(Duration::from_millis(120));
    if worker.run.cancelled.load(Ordering::SeqCst) {
        finish_agent_worker(worker, true);
        return;
    }
    let saved_plan = if matches!(worker.mode, AgentMode::Plan) {
        latest_saved_plan_for_worker(&worker)
    } else {
        None
    };
    if matches!(worker.mode, AgentMode::Plan) {
        try_propose_plan(&worker, saved_plan.as_ref());
    }
    append_agent_message(&worker.workspace_root, &worker.run, &response_text);
    finish_agent_worker(worker, false);
}

fn latest_saved_plan_for_worker(worker: &AgentWorker) -> Option<SavedCadPlan> {
    let history = ChatStore::new(worker.workspace_root.clone())
        .history(&worker.run.session_id, None)
        .ok()?;
    latest_saved_cad_plan(&history.messages, &worker.run.run_id)
}

fn try_propose_plan(worker: &AgentWorker, saved_plan: Option<&SavedCadPlan>) {
    if let Some(plan) = saved_plan
        && push_saved_plan_proposal(worker, plan)
    {
        return;
    }
}

fn push_saved_plan_proposal(worker: &AgentWorker, plan: &SavedCadPlan) -> bool {
    let Ok(plan_ref) = plan_target_handle(&worker.workspace_root, &plan.plan_ref) else {
        return false;
    };
    let Ok(target) = plan_target_handle(&worker.workspace_root, &plan.target_path) else {
        return false;
    };
    let affected = path_handles_for(&worker.workspace_root, &plan.affected_paths);
    let new_files = path_handles_for(&worker.workspace_root, &plan.new_paths);
    let export_targets = path_handles_for(&worker.workspace_root, &plan.export_targets);
    if export_targets.is_empty() {
        return false;
    }
    (worker.push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentPlanProposed(AgentPlanProposedEvent {
            session_id: worker.run.session_id.clone(),
            run_id: worker.run.run_id.clone(),
            plan_ref: Some(plan_ref),
            target_path: target.clone(),
            target_type: plan.target_type,
            affected_files: if affected.is_empty() {
                vec![target.clone()]
            } else {
                affected
            },
            new_files,
            change_description: plan.description.clone(),
            export_targets,
        }),
    });
    true
}

fn path_handles_for(workspace_root: &Path, paths: &[String]) -> Vec<PathHandle> {
    paths
        .iter()
        .filter_map(|path| plan_target_handle(workspace_root, path).ok())
        .collect()
}

use crate::plan_extraction::{
    SavedCadPlan, execution_scope_from_plan_ref, latest_saved_cad_plan, plan_target_handle,
};

fn run_text_agent_llm(
    worker: &AgentWorker,
    provider: &dyn app_server_core::llm::LlmProvider,
) -> Option<String> {
    let store = ChatStore::new(worker.workspace_root.clone());
    let history = store
        .history(&worker.run.session_id, Some(8))
        .map(|response| response.messages)
        .unwrap_or_default();
    let mode = app_server_core::mode_for_tool_loop(worker.mode);
    let execution_scope = match execution_scope_for_worker(worker, mode) {
        Ok(scope) => scope,
        Err(error) => {
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::PermissionDenied,
                error.message,
            );
            return None;
        }
    };
    let input = AgentTurnInput {
        mode,
        prompt: worker.prompt.clone(),
        history,
        selections: worker.selection_snapshot.selections.clone(),
        active_selection_index: worker.selection_snapshot.active_index,
        plan_ref: worker.plan_ref.clone(),
        context_refs: worker.context_refs.clone(),
        execution_scope: execution_scope.clone(),
    };
    let cadquery_runtime = Arc::new(HostCadQueryToolRuntime {
        workspace_root: worker.workspace_root.clone(),
        python: worker.python.clone(),
        cadquery_results: Arc::clone(&worker.cadquery_results),
        push_sink: Arc::clone(&worker.push_sink),
        run: worker.run.clone(),
    });
    let tool_executor = app_server_core::WorkspaceToolExecutor::new(worker.workspace_root.clone())
        .with_cadquery_runtime(cadquery_runtime);
    let mut tool_context = AgentToolRunContext::new(worker.workspace_root.clone(), mode);
    tool_context.session_id = Some(worker.run.session_id.clone());
    tool_context.run_id = Some(worker.run.run_id.clone());
    tool_context.selections = worker.selection_snapshot.selections.clone();
    tool_context.active_selection_index = worker.selection_snapshot.active_index;
    tool_context.context_refs = worker.context_refs.clone();
    tool_context.execution_scope = execution_scope;
    let tool_observer = AgentToolEventRecorder {
        workspace_root: worker.workspace_root.clone(),
        push_sink: Arc::clone(&worker.push_sink),
        run: worker.run.clone(),
    };
    let push_sink = Arc::clone(&worker.push_sink);
    let run = worker.run.clone();
    let cancelled = Arc::clone(&worker.run.cancelled);
    match app_server_core::stream_agent_turn_with_tools(
        input,
        provider,
        &tool_executor,
        tool_context,
        &tool_observer,
        &|token| {
            if cancelled.load(Ordering::SeqCst) {
                return false;
            }
            push_agent_token(&push_sink, &run, token);
            true
        },
    ) {
        Ok(draft) => Some(draft.text),
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

fn execution_scope_for_worker(
    worker: &AgentWorker,
    mode: AgentMode,
) -> Result<Option<app_server_core::AgentExecutionScope>, ProtocolError> {
    if mode != AgentMode::Agent {
        return Ok(None);
    }
    let Some(plan_ref) = &worker.plan_ref else {
        return Ok(None);
    };
    execution_scope_from_plan_ref(&worker.workspace_root, plan_ref).map(Some)
}

pub fn validate_cadquery_confirmation(
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
    validate_export_format_targets(confirmation)?;
    if confirmation
        .export_targets
        .iter()
        .any(|path| !path_handle_to_relative_path(path).starts_with("outputs"))
    {
        return Err("CadQuery export_targets 必须位于 outputs/ 目录");
    }
    Ok(())
}

fn validate_export_format_targets(
    confirmation: &AgentCadQueryConfirmation,
) -> Result<(), &'static str> {
    if confirmation.export_targets.is_empty() {
        return Ok(());
    }
    if confirmation.request.export_formats.is_empty() {
        return Err("CadQuery export_targets 非空时必须提供匹配的 export_formats");
    }
    let mut target_formats = Vec::new();
    for target in &confirmation.export_targets {
        let Some(format) = export_format_for_target(target) else {
            return Err("CadQuery export_targets 扩展名不受支持");
        };
        if !export_target_matches_runner_output(&confirmation.request.target_path, target) {
            return Err("CadQuery export_targets 必须匹配 runner 输出文件名");
        }
        if !target_formats.contains(&format) {
            target_formats.push(format);
        }
    }
    if target_formats
        .iter()
        .any(|format| !confirmation.request.export_formats.contains(format))
        || confirmation
            .request
            .export_formats
            .iter()
            .any(|format| !target_formats.contains(format))
    {
        return Err("CadQuery export_formats 必须与 export_targets 扩展名一致");
    }
    Ok(())
}

fn export_format_for_target(target: &PathHandle) -> Option<CadQueryExportFormat> {
    let path = path_handle_to_relative_path(target);
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("step") | Some("stp") => Some(CadQueryExportFormat::Step),
        Some("stl") => Some(CadQueryExportFormat::Stl),
        Some("3mf") => Some(CadQueryExportFormat::ThreeMf),
        _ => None,
    }
}

fn export_target_matches_runner_output(
    target_path: &PathHandle,
    export_target: &PathHandle,
) -> bool {
    let Some(format) = export_format_for_target(export_target) else {
        return false;
    };
    let expected = PathBuf::from("outputs").join(cadquery_export_file_name(
        &cadquery_target_stem(target_path),
        &format,
    ));
    path_handle_to_relative_path(export_target) == expected
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

fn append_llm_tool_call(workspace_root: &Path, run: &AgentRunHandle, call: &LlmToolCall) {
    let _ = ChatStore::new(workspace_root.to_path_buf()).append_tool_call(
        &run.session_id,
        "agent tool started",
        ChatToolCallRecord {
            tool_call_id: call.id.clone(),
            tool_name: call.function_name.clone(),
            args_json: call.arguments.clone(),
        },
    );
}

fn append_llm_tool_result(
    workspace_root: &Path,
    run: &AgentRunHandle,
    call: &LlmToolCall,
    result: &str,
) {
    let _ = ChatStore::new(workspace_root.to_path_buf()).append_tool_result(
        &run.session_id,
        "agent tool completed",
        ChatToolResultRecord {
            tool_call_id: call.id.clone(),
            tool_name: call.function_name.clone(),
            result_json: result.to_owned(),
        },
        None,
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

fn push_llm_tool_start(push_sink: &ServerPushSink, run: &AgentRunHandle, call: &LlmToolCall) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentToolStart(AgentToolStartEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            tool_call_id: call.id.clone(),
            tool_name: call.function_name.clone(),
            args_json: call.arguments.clone(),
        }),
    });
}

fn push_llm_tool_result(
    push_sink: &ServerPushSink,
    run: &AgentRunHandle,
    call: &LlmToolCall,
    result: &str,
) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentToolResult(AgentToolResultEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            tool_call_id: call.id.clone(),
            tool_name: call.function_name.clone(),
            result_json: result.to_owned(),
        }),
    });
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
    let msg = message.into();
    log::error!(
        "[agent run={}] {:?}: {}",
        run.run_id,
        error_type,
        msg
    );
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentError(AgentErrorEvent {
            session_id: run.session_id.clone(),
            run_id: Some(run.run_id.clone()),
            error_type,
            message: msg,
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

pub fn agent_error_type(kind: &CadQueryRunnerErrorKind) -> AgentErrorType {
    match kind {
        CadQueryRunnerErrorKind::Build => AgentErrorType::CadQueryBuildError,
        CadQueryRunnerErrorKind::FileConflict => AgentErrorType::FileConflict,
        CadQueryRunnerErrorKind::Timeout => AgentErrorType::Timeout,
        CadQueryRunnerErrorKind::Cancelled => AgentErrorType::Timeout,
        CadQueryRunnerErrorKind::PermissionDenied => AgentErrorType::PermissionDenied,
        CadQueryRunnerErrorKind::PythonImport => AgentErrorType::PythonImportError,
        CadQueryRunnerErrorKind::InvalidProjectPath
        | CadQueryRunnerErrorKind::Io
        | CadQueryRunnerErrorKind::Runner => AgentErrorType::CadQueryBuildError,
    }
}

fn cadquery_tool_error(error: CadQueryRunnerError) -> CadQueryToolRuntimeError {
    log::error!("[cadquery] {:?}: {}", error.kind, error.message);
    let retry_allowed = matches!(
        error.kind,
        CadQueryRunnerErrorKind::Build
            | CadQueryRunnerErrorKind::PythonImport
            | CadQueryRunnerErrorKind::Runner
            | CadQueryRunnerErrorKind::Timeout
    );
    CadQueryToolRuntimeError::new(
        cadquery_tool_error_type(&error.kind),
        error.message,
        retry_allowed,
    )
}

fn cadquery_tool_error_type(kind: &CadQueryRunnerErrorKind) -> &'static str {
    match kind {
        CadQueryRunnerErrorKind::Build | CadQueryRunnerErrorKind::Runner => "cadquery_build_error",
        CadQueryRunnerErrorKind::PythonImport => "python_import_error",
        CadQueryRunnerErrorKind::FileConflict => "file_conflict",
        CadQueryRunnerErrorKind::Timeout | CadQueryRunnerErrorKind::Cancelled => "timeout",
        CadQueryRunnerErrorKind::PermissionDenied | CadQueryRunnerErrorKind::InvalidProjectPath => {
            "permission_denied"
        }
        CadQueryRunnerErrorKind::Io => "cadquery_build_error",
    }
}

fn validate_result_kind(
    mesh: &CadQueryMeshPayload,
    expected: CadQueryObjectKind,
) -> Result<(), CadQueryToolRuntimeError> {
    if mesh.root_object_kind == expected {
        Ok(())
    } else {
        Err(CadQueryToolRuntimeError::new(
            "topology_mapping_error",
            "CadQuery root object kind does not match target_type",
            true,
        ))
    }
}

fn runner_warnings(stderr: &str) -> Vec<String> {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_owned()]
    }
}

#[cfg(unix)]
fn is_hard_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata.nlink() > 1
}

#[cfg(not(unix))]
fn is_hard_link(_metadata: &fs::Metadata) -> bool {
    false
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

fn invalid_command(message: impl std::fmt::Display) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidCommand, message.to_string())
}

fn deprecated_command(message: impl std::fmt::Display) -> ProtocolError {
    invalid_command(message)
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
        protocol_version: ProtocolVersionRange::new(
            CURRENT_PROTOCOL_VERSION,
            CURRENT_PROTOCOL_VERSION,
        ),
        reconnect_window_ms: DEFAULT_SESSION_RECONNECT_WINDOW_MS,
        supports_watch: true,
        supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        supports_session_reclaim: true,
        cadquery: true,
        agent: true,
        selection_sync: true,
        llm_configured: app_server_core::llm::load_llm_config()
            .ok()
            .flatten()
            .is_some(),
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
