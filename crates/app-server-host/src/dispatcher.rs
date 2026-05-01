use app_server_core::{
    AgentToolCall, AgentToolRunContext, AgentTurnInput, CadQueryCommitScope,
    CadQueryContractConfig, CadQueryModelContract, CadQueryRunConfig, CadQueryRunResult,
    CadQueryRunnerError, CadQueryRunnerErrorKind, CadQueryToolCachedResult, CadQueryToolRunRequest,
    CadQueryToolRunResult, CadQueryToolRuntime, CadQueryToolRuntimeError, ChatStore, FileWatcher,
    SlicerInstall, cadquery_result_ready, current_workspace_owned, detect_slicer_paths,
    export_model, list_workspace_entries_owned, load_config_dto, preview_ready_response,
    read_file_response_owned, resolve_workspace_path_owned, resolve_workspace_write_path_owned,
    run_cadquery_contract, run_cadquery_runner, run_cadquery_runner_with_cancel,
    run_rig_agent_turn_with_config, save_config_dto, send_to_slicer, stage_cadquery_project_owned,
};
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentCancelRequest, AgentCancelledResponse, AgentDoneEvent,
    AgentErrorEvent, AgentErrorType, AgentInvokeRequest, AgentMeshReadyEvent, AgentMode,
    AgentModelDiscoveryState, AgentModelDiscoveryStatus, AgentModelParamsUpdateRequest,
    AgentModelRegistryModel, AgentModelRegistryProvider, AgentModelRegistryResponse,
    AgentModelSelectRequest, AgentModelSource, AgentPlanConfirmRequest, AgentPlanProposedEvent,
    AgentPlanRejectRequest, AgentProviderCapabilities, AgentReasoningEvent, AgentStartedResponse,
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

use crate::cadquery_python_path;
use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

use crate::HostSession;

pub type ServerPushSink = Arc<dyn Fn(ServerPushEnvelope) + Send + Sync>;

const CADQUERY_RESULT_CACHE_LIMIT: usize = 8;
const CADQUERY_RUNNER_TIMEOUT: Duration = Duration::from_secs(180);

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

#[derive(Clone, Debug, Default)]
struct AgentModelRuntimeState {
    provider_id: Option<String>,
    model_id: Option<String>,
    reasoning_effort: Option<String>,
    service_label: Option<String>,
}

pub struct HostRequestDispatcher {
    workspace_id: WorkspaceId,
    workspace_path: Option<PathBuf>,
    denied_extensions: Vec<String>,
    next_subscription_id: u64,
    watchers: HashMap<String, FileWatcher>,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
    agent_runs: Arc<Mutex<AgentRunRegistry>>,
    agent_model_state: AgentModelRuntimeState,
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
            agent_model_state: AgentModelRuntimeState::default(),
            selection_snapshot: SelectionUpdateRequest {
                selections: Vec::new(),
                active_index: None,
            },
            push_sink,
            session: HostSession::new(session_token, server_capabilities(None)),
        }
    }

    pub fn rebind_workspace(&mut self, workspace_path: PathBuf) {
        self.workspace_path = Some(workspace_path);
    }

    pub async fn handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<CapabilityHandshakeResponse, ProtocolError> {
        let requested_preview_kinds = request.capabilities.supported_preview_kinds.clone();
        let server_capabilities = server_capabilities_for_request(requested_preview_kinds).await;
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

    pub async fn dispatch_envelope(
        &mut self,
        envelope: ClientRequestEnvelope,
    ) -> ServerResponseEnvelope {
        self.session.track_request(envelope.request_id);
        let result = self.dispatch_command(envelope.command).await;
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

    async fn dispatch_command(
        &mut self,
        command: ClientCommand,
    ) -> Result<CommandSuccess, ProtocolError> {
        match command {
            ClientCommand::WorkspaceCurrent => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let current =
                    current_workspace_owned(workspace_path, self.workspace_id.clone()).await;
                self.session.bind_workspace(current.clone());
                Ok(CommandSuccess::WorkspaceCurrent(current))
            }
            ClientCommand::WorkspaceList(request) => {
                let workspace_path = self.workspace_root()?;
                let response = list_workspace_entries_owned(
                    workspace_path.to_path_buf(),
                    self.workspace_id.clone(),
                    request.directory,
                )
                .await?;
                self.record_workspace_entries(&response);
                Ok(CommandSuccess::WorkspaceList(response))
            }
            ClientCommand::FileRead(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                self.session.issue_handle(request.path.clone());
                read_file_response_owned(
                    workspace_path,
                    request.path,
                    self.denied_extensions.clone(),
                )
                .await
                .map(CommandSuccess::FileRead)
            }
            ClientCommand::FileWriteText(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let resolved =
                    resolve_workspace_write_path_owned(workspace_path, request.path.clone())
                        .await?;
                tokio::fs::write(resolved, request.contents)
                    .await
                    .map_err(internal_error)?;
                self.session.issue_handle(request.path.clone());
                Ok(CommandSuccess::FileWritten(FileWriteTextResponse {
                    path: request.path,
                }))
            }
            ClientCommand::ConfigLoad => {
                let config = load_config_dto().await.map_err(internal_error)?;
                Ok(CommandSuccess::ConfigLoaded(ConfigLoadResponse { config }))
            }
            ClientCommand::ConfigSave(request) => {
                save_config_dto(request.config)
                    .await
                    .map_err(internal_error)?;
                Ok(CommandSuccess::ConfigSaved)
            }
            ClientCommand::PreviewRequest(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let source_path =
                    resolve_workspace_path_owned(workspace_path, request.source.clone()).await?;
                self.session.issue_handle(request.source.clone());
                preview_ready_response(
                    request
                        .configured_openscad_path
                        .map(|path| path.to_path_buf()),
                    source_path,
                    request.defines,
                )
                .await
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
                let source_path = resolve_workspace_path_owned(
                    workspace_path.clone(),
                    request.target_path.clone(),
                )
                .await?;
                let code = tokio::fs::read_to_string(source_path)
                    .await
                    .map_err(internal_error)?;
                let script = path_handle_to_relative_path(&request.target_path);
                let staged = stage_cadquery_project_owned(workspace_path, script.clone(), code)
                    .await
                    .map_err(internal_error)?;
                self.session.issue_handle(request.target_path);
                let result = match run_cadquery_runner(CadQueryRunConfig {
                    python: cadquery_python_path(),
                    project_root: staged.root().to_path_buf(),
                    script: display_relative_path(&script),
                    output_dir: staged.output_dir(),
                    export_formats: request.export_formats,
                    params_json: request.params_json,
                    timeout: CADQUERY_RUNNER_TIMEOUT,
                })
                .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        staged.cleanup().await;
                        return Err(internal_error(error));
                    }
                };
                staged.cleanup().await;
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
                    .create_owned(request.title, request.goal, request.related_files)
                    .await
                    .map(CommandSuccess::ChatCreated)
            }
            ClientCommand::ChatList(request) => self
                .chat_store()?
                .list_owned(request.include_archived)
                .await
                .map(CommandSuccess::ChatList),
            ClientCommand::ChatSend(request) => {
                self.issue_handles(&request.related_files);
                self.chat_store()?
                    .append_message_owned(
                        request.session_id,
                        ChatRole::User,
                        request.content,
                        request.related_files,
                        None,
                    )
                    .await
                    .map(CommandSuccess::ChatAck)
            }
            ClientCommand::ChatHistory(request) => self
                .chat_store()?
                .history_owned(request.session_id, request.limit)
                .await
                .map(CommandSuccess::ChatHistory),
            ClientCommand::ChatArchive(request) => self
                .chat_store()?
                .archive_owned(request.session_id)
                .await
                .map(CommandSuccess::ChatArchived),
            ClientCommand::AgentInvoke(request) => self.start_agent_after_history(request),
            ClientCommand::AgentCancel(request) => self.cancel_agent(request),
            ClientCommand::AgentModelRegistry => {
                let registry = self.agent_model_registry_snapshot().await?;
                Ok(CommandSuccess::AgentModelRegistry(registry))
            }
            ClientCommand::AgentModelSelect(request) => {
                let registry = self.select_agent_model(request).await?;
                Ok(CommandSuccess::AgentModelRegistry(registry))
            }
            ClientCommand::AgentModelParamsUpdate(request) => {
                let registry = self.update_agent_model_params(request).await?;
                Ok(CommandSuccess::AgentModelRegistry(registry))
            }
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
                let slicers = detect_slicer_paths(configured)
                    .await
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
                let workspace_path = self.workspace_root()?.to_path_buf();
                let source_path =
                    resolve_workspace_path_owned(workspace_path.clone(), request.source).await?;
                let output_handle = request.output_path.clone();
                let output_path =
                    resolve_workspace_write_path_owned(workspace_path, output_handle.clone())
                        .await?;
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
                    source_path,
                    request.defines,
                    output_path.clone(),
                    request.format,
                )
                .await
                .map_err(internal_error)?;
                if let Some(name) = request.slicer_name {
                    let slicer = detect_slicer_paths(configured_slicers)
                        .await
                        .into_iter()
                        .find(|item| item.name == name)
                        .ok_or_else(|| {
                            ProtocolError::new(
                                ProtocolErrorCode::NotFound,
                                format!("未找到切片软件 {name}"),
                            )
                        })?;
                    send_to_slicer(slicer.path, output_path.clone())
                        .await
                        .map_err(internal_error)?;
                }
                Ok(CommandSuccess::ExportRun(ExportRunResponse {
                    output_path: output_handle,
                }))
            }
            ClientCommand::WatchSubscribe(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let watched_handle = request.directory.unwrap_or_else(|| {
                    app_server_protocol::PathHandle::new(
                        self.workspace_id.clone(),
                        Vec::<String>::new(),
                    )
                    .expect("root workspace handle should be valid")
                });
                let watched_path =
                    resolve_workspace_path_owned(workspace_path, watched_handle.clone()).await?;
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

    async fn agent_model_registry_snapshot(
        &self,
    ) -> Result<AgentModelRegistryResponse, ProtocolError> {
        let registry = load_agent_model_registry().await?;
        Ok(agent_model_registry_response(
            &registry,
            &self.agent_model_state,
        ))
    }

    async fn select_agent_model(
        &mut self,
        request: AgentModelSelectRequest,
    ) -> Result<AgentModelRegistryResponse, ProtocolError> {
        let registry = load_agent_model_registry().await?;
        ensure_agent_model_exists(&registry, &request.provider_id, &request.model_id)?;
        self.agent_model_state = AgentModelRuntimeState {
            provider_id: Some(request.provider_id),
            model_id: Some(request.model_id),
            reasoning_effort: None,
            service_label: None,
        };
        Ok(agent_model_registry_response(
            &registry,
            &self.agent_model_state,
        ))
    }

    async fn update_agent_model_params(
        &mut self,
        request: AgentModelParamsUpdateRequest,
    ) -> Result<AgentModelRegistryResponse, ProtocolError> {
        let registry = load_agent_model_registry().await?;
        ensure_agent_model_exists(&registry, &request.provider_id, &request.model_id)?;
        let same_model = self.agent_model_state.provider_id.as_deref()
            == Some(request.provider_id.as_str())
            && self.agent_model_state.model_id.as_deref() == Some(request.model_id.as_str());
        self.agent_model_state = AgentModelRuntimeState {
            provider_id: Some(request.provider_id),
            model_id: Some(request.model_id),
            reasoning_effort: request.reasoning_effort.or_else(|| {
                same_model
                    .then(|| self.agent_model_state.reasoning_effort.clone())
                    .flatten()
            }),
            service_label: request.service_label.or_else(|| {
                same_model
                    .then(|| self.agent_model_state.service_label.clone())
                    .flatten()
            }),
        };
        Ok(agent_model_registry_response(
            &registry,
            &self.agent_model_state,
        ))
    }

    fn start_agent_after_history(
        &mut self,
        request: AgentInvokeRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        let run = self
            .agent_runs
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .try_start(request.session_id.clone())?;
        let response = AgentStartedResponse {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
        };
        let model_state = agent_model_state_for_request(&self.agent_model_state, &request);
        let worker = AgentWorker {
            run,
            prompt: request.prompt,
            mode: request.mode,
            plan_ref: request.plan_ref,
            context_refs: request.context_refs,
            model_state,
            selection_snapshot: self.selection_snapshot.clone(),
            workspace_root: self.workspace_root()?.to_path_buf(),
            python: cadquery_python_path(),
            cadquery_results: Arc::clone(&self.cadquery_results),
            agent_runs: Arc::clone(&self.agent_runs),
            push_sink: Arc::clone(&self.push_sink),
        };
        tokio::spawn(run_agent_worker(worker));
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
    model_state: AgentModelRuntimeState,
    selection_snapshot: SelectionUpdateRequest,
    workspace_root: PathBuf,
    python: PathBuf,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
    agent_runs: Arc<Mutex<AgentRunRegistry>>,
    push_sink: ServerPushSink,
}

struct AgentToolEventRecorder {
    workspace_root: PathBuf,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
    push_sink: ServerPushSink,
    run: AgentRunHandle,
    history_writes: Arc<Mutex<Vec<AgentToolHistoryWrite>>>,
}

enum AgentToolHistoryWrite {
    ToolCall(ChatToolCallRecord),
    ToolResult(
        ChatToolResultRecord,
        Option<app_server_protocol::CadQueryResultReady>,
    ),
}

impl app_server_core::AgentToolObserver for AgentToolEventRecorder {
    fn tool_start(&self, call: &AgentToolCall) {
        push_llm_tool_start(&self.push_sink, &self.run, call);
        self.record_history_write(AgentToolHistoryWrite::ToolCall(ChatToolCallRecord {
            tool_call_id: call.id.clone(),
            tool_name: call.function_name.clone(),
            args_json: call.arguments.clone(),
        }));
    }

    fn tool_result(&self, call: &AgentToolCall, result: &str) {
        push_llm_tool_result(&self.push_sink, &self.run, call, result);
        let mesh_result = cadquery_ready_for_tool_result(&self.cadquery_results, result);
        self.record_history_write(AgentToolHistoryWrite::ToolResult(
            ChatToolResultRecord {
                tool_call_id: call.id.clone(),
                tool_name: call.function_name.clone(),
                result_json: result.to_owned(),
            },
            mesh_result,
        ));
    }
}

impl AgentToolEventRecorder {
    fn record_history_write(&self, write: AgentToolHistoryWrite) {
        if let Ok(mut writes) = self.history_writes.lock() {
            writes.push(write);
        }
    }

    async fn flush_history_writes(&self) {
        let writes = self
            .history_writes
            .lock()
            .ok()
            .map(|mut writes| writes.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        let store = ChatStore::new(self.workspace_root.clone());
        for write in writes {
            match write {
                AgentToolHistoryWrite::ToolCall(tool_call) => {
                    let _ = store
                        .append_tool_call_with_run_id(
                            &self.run.session_id,
                            "agent tool started",
                            tool_call,
                            Some(self.run.run_id.clone()),
                        )
                        .await;
                }
                AgentToolHistoryWrite::ToolResult(tool_result, mesh_result) => {
                    let _ = store
                        .append_tool_result_with_run_id(
                            &self.run.session_id,
                            "agent tool completed",
                            tool_result,
                            mesh_result,
                            Some(self.run.run_id.clone()),
                        )
                        .await;
                }
            }
        }
    }
}

struct HostCadQueryToolRuntime {
    workspace_root: PathBuf,
    python: PathBuf,
    cadquery_results: Arc<Mutex<CadQueryResultCache>>,
    push_sink: ServerPushSink,
    run: AgentRunHandle,
}

#[async_trait::async_trait]
impl CadQueryToolRuntime for HostCadQueryToolRuntime {
    async fn model_contract(
        &self,
        request: &CadQueryToolRunRequest,
    ) -> Option<Result<CadQueryModelContract, CadQueryToolRuntimeError>> {
        let result = run_cadquery_contract(CadQueryContractConfig {
            python: self.python.clone(),
            code: request.code.clone(),
            timeout: CADQUERY_RUNNER_TIMEOUT,
        })
        .await
        .map(|contract| CadQueryModelContract {
            has_model_description: contract.has_model_description,
        })
        .map_err(cadquery_tool_error);
        Some(result)
    }

    async fn dry_run(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let staged = stage_cadquery_project_owned(
            self.workspace_root.clone(),
            PathBuf::from(request.target_path.clone()),
            request.code.clone(),
        )
        .await
        .map_err(cadquery_tool_error)?;
        let result = match run_cadquery_runner_with_cancel(
            CadQueryRunConfig {
                python: self.python.clone(),
                project_root: staged.root().to_path_buf(),
                script: staged.script_arg(),
                output_dir: staged.output_dir(),
                export_formats: Vec::new(),
                params_json: request.params_json,
                timeout: CADQUERY_RUNNER_TIMEOUT,
            },
            &|| self.run.cancelled.load(Ordering::SeqCst),
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                staged.cleanup().await;
                return Err(cadquery_tool_error(error));
            }
        };
        staged.cleanup().await;
        self.finish_result(
            result,
            request.target_type,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    async fn execute(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let commit_scope = CadQueryCommitScope::ExactOutputs(
            request
                .export_targets
                .iter()
                .map(|path| PathBuf::from(path.clone()))
                .collect(),
        );
        let staged = stage_cadquery_project_owned(
            self.workspace_root.clone(),
            PathBuf::from(request.target_path.clone()),
            request.code.clone(),
        )
        .await
        .map_err(cadquery_tool_error)?;
        let result = match run_cadquery_runner_with_cancel(
            CadQueryRunConfig {
                python: self.python.clone(),
                project_root: staged.root().to_path_buf(),
                script: staged.script_arg(),
                output_dir: staged.output_dir(),
                export_formats: request.export_formats.clone(),
                params_json: request.params_json,
                timeout: CADQUERY_RUNNER_TIMEOUT,
            },
            &|| self.run.cancelled.load(Ordering::SeqCst),
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                staged.cleanup().await;
                return Err(cadquery_tool_error(error));
            }
        };
        if let Err(error) = validate_result_kind(&result.mesh, request.target_type) {
            staged.cleanup().await;
            return Err(error);
        }
        if let Some(doc_path) = request.doc_update_path.clone() {
            if let Err(error) = self.preflight_cadquery_doc_update(doc_path).await {
                staged.cleanup().await;
                return Err(error);
            }
        }
        staged
            .commit_success_with_scope_cancellable(&commit_scope, &|| {
                self.run.cancelled.load(Ordering::SeqCst)
            })
            .await
            .map_err(cadquery_tool_error)?;
        let mut committed_files = vec![request.target_path];
        let mut extra_warnings = Vec::new();
        if let Some(doc_path) = request.doc_update_path {
            match self
                .append_cadquery_doc_update(doc_path.clone(), result.clone())
                .await
            {
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

    async fn preflight_cadquery_doc_update(
        &self,
        doc_path: String,
    ) -> Result<(), CadQueryToolRuntimeError> {
        let absolute = self.workspace_root.join(doc_path);
        let metadata = tokio::fs::symlink_metadata(absolute.clone())
            .await
            .map_err(|error| {
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
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(absolute)
            .await
            .map(|_| ())
            .map_err(|error| {
                CadQueryToolRuntimeError::new(
                    "file_conflict",
                    format!("CadQuery 说明文档不可写: {error}"),
                    false,
                )
            })
    }

    async fn append_cadquery_doc_update(
        &self,
        doc_path: String,
        result: CadQueryRunResult,
    ) -> Result<(), String> {
        let absolute = self.workspace_root.join(doc_path);
        let note = format!(
            "\n\n## budn' CadQuery 执行记录\n\n- result_id: `{}`\n- build_id: `{}`\n",
            result.mesh.result_id, result.mesh.build_id
        );
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(absolute)
            .await
            .map_err(|error| format!("更新 CadQuery 说明文档失败: {error}"))?;
        let bytes = note.into_bytes();
        file.write_all(&bytes)
            .await
            .map_err(|error| format!("更新 CadQuery 说明文档失败: {error}"))
    }
}

async fn run_agent_worker(worker: AgentWorker) {
    run_text_agent(worker).await;
}

async fn run_text_agent(worker: AgentWorker) {
    let response_text = match run_text_agent_rig(&worker).await {
        Some(text) => text,
        None => {
            finish_agent_worker(worker, false);
            return;
        }
    };
    if worker.run.cancelled.load(Ordering::SeqCst) {
        finish_agent_worker(worker, true);
        return;
    }
    let saved_plan = if matches!(worker.mode, AgentMode::Plan) {
        latest_saved_plan_for_worker(&worker).await
    } else {
        None
    };
    if matches!(worker.mode, AgentMode::Plan) {
        try_propose_plan(&worker, saved_plan.as_ref());
    }
    if !response_text.trim().is_empty() {
        append_agent_message(&worker.workspace_root, &worker.run, &response_text).await;
    }
    finish_agent_worker(worker, false);
}

async fn latest_saved_plan_for_worker(worker: &AgentWorker) -> Option<SavedCadPlan> {
    let store = ChatStore::new(worker.workspace_root.clone());
    let history = store.history(&worker.run.session_id, None).await.ok()?;
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

async fn load_agent_model_registry()
-> Result<app_server_core::llm::AgentProviderRegistry, ProtocolError> {
    app_server_core::llm::load_agent_provider_registry_with_discovery()
        .await
        .map_err(|error| internal_error(error.message))?
        .ok_or_else(|| internal_error("Rig Agent is not configured"))
}

fn ensure_agent_model_exists(
    registry: &app_server_core::llm::AgentProviderRegistry,
    provider_id: &str,
    model_id: &str,
) -> Result<(), ProtocolError> {
    let provider = registry.provider(provider_id).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidCommand,
            format!("unknown agent provider `{provider_id}`"),
        )
    })?;
    if provider.models.iter().any(|model| model.id == model_id) {
        Ok(())
    } else {
        Err(ProtocolError::new(
            ProtocolErrorCode::InvalidCommand,
            format!("unknown agent model `{model_id}`"),
        ))
    }
}

fn agent_model_state_for_request(
    current: &AgentModelRuntimeState,
    request: &AgentInvokeRequest,
) -> AgentModelRuntimeState {
    let provider_id = request
        .provider_id
        .clone()
        .or_else(|| current.provider_id.clone());
    let model_id = request
        .model_id
        .clone()
        .or_else(|| current.model_id.clone());
    let same_model = provider_id == current.provider_id && model_id == current.model_id;
    AgentModelRuntimeState {
        provider_id,
        model_id,
        reasoning_effort: request.reasoning_effort.clone().or_else(|| {
            same_model
                .then(|| current.reasoning_effort.clone())
                .flatten()
        }),
        service_label: request
            .service_label
            .clone()
            .or_else(|| same_model.then(|| current.service_label.clone()).flatten()),
    }
}

fn agent_model_registry_response(
    registry: &app_server_core::llm::AgentProviderRegistry,
    state: &AgentModelRuntimeState,
) -> AgentModelRegistryResponse {
    let (active_provider_id, active_model_id) = active_agent_model_ids(registry, state);
    let active_provider = registry.provider(&active_provider_id);
    let active_model = active_provider.and_then(|provider| {
        provider
            .models
            .iter()
            .find(|model| model.id == active_model_id)
    });
    let active_reasoning_effort = state
        .reasoning_effort
        .clone()
        .or_else(|| active_model.and_then(|model| model.reasoning_effort.clone()));
    let active_service_label = state
        .service_label
        .clone()
        .or_else(|| active_model.and_then(|model| model.service_label.clone()));
    let provider_kind = active_provider.map(|provider| provider.kind);
    AgentModelRegistryResponse {
        active_provider_id,
        active_model_id,
        active_reasoning_effort_applied: active_reasoning_effort_applied(
            provider_kind,
            active_reasoning_effort.as_deref(),
            active_model,
        ),
        active_reasoning_effort,
        active_service_label_applied: active_service_label_applied(
            provider_kind,
            active_service_label.as_deref(),
        ),
        active_service_label,
        reasoning_effort_options: reasoning_effort_options(),
        service_label_options: service_label_options(registry, state),
        providers: registry
            .providers
            .iter()
            .map(agent_provider_registry_provider)
            .collect(),
    }
}

fn active_agent_model_ids(
    registry: &app_server_core::llm::AgentProviderRegistry,
    state: &AgentModelRuntimeState,
) -> (String, String) {
    if let (Some(provider_id), Some(model_id)) = (&state.provider_id, &state.model_id)
        && registry
            .provider(provider_id)
            .is_some_and(|provider| provider.models.iter().any(|model| &model.id == model_id))
    {
        return (provider_id.clone(), model_id.clone());
    }
    (
        registry.active_provider_id.clone(),
        registry.active_model_id.clone(),
    )
}

fn agent_provider_registry_provider(
    provider: &app_server_core::llm::ResolvedAgentProvider,
) -> AgentModelRegistryProvider {
    AgentModelRegistryProvider {
        id: provider.id.clone(),
        kind: provider.kind.as_str().into(),
        label: None,
        discovery: agent_model_discovery_state(&provider.model_discovery_status),
        models: provider
            .models
            .iter()
            .map(agent_model_registry_model)
            .collect(),
    }
}

fn agent_model_registry_model(
    model: &app_server_core::llm::ResolvedAgentModel,
) -> AgentModelRegistryModel {
    AgentModelRegistryModel {
        id: model.id.clone(),
        label: model.label.clone(),
        source: agent_model_source(model.source.clone()),
        reasoning_effort: model.reasoning_effort.clone(),
        service_label: model.service_label.clone(),
        native_web_search_enabled: model.native_web_search,
        native_web_search_applied: model.native_web_search && model.web_search_supported,
        web_search_supported: model.web_search_supported,
        web_search_unsupported_reason: model.web_search_unsupported_reason.clone(),
        search_sources_supported: false,
    }
}

fn agent_model_source(source: app_server_core::llm::AgentModelSource) -> AgentModelSource {
    match source {
        app_server_core::llm::AgentModelSource::Manual => AgentModelSource::Manual,
        app_server_core::llm::AgentModelSource::Discovered => AgentModelSource::Discovered,
        app_server_core::llm::AgentModelSource::DiscoveredWithOverride => {
            AgentModelSource::DiscoveredWithOverride
        }
    }
}

fn agent_model_discovery_state(
    status: &app_server_core::llm::ModelDiscoveryStatus,
) -> AgentModelDiscoveryState {
    match status {
        app_server_core::llm::ModelDiscoveryStatus::Disabled => AgentModelDiscoveryState {
            enabled: false,
            status: AgentModelDiscoveryStatus::Disabled,
            error: None,
        },
        app_server_core::llm::ModelDiscoveryStatus::NotStarted => AgentModelDiscoveryState {
            enabled: true,
            status: AgentModelDiscoveryStatus::NotStarted,
            error: None,
        },
        app_server_core::llm::ModelDiscoveryStatus::Succeeded => AgentModelDiscoveryState {
            enabled: true,
            status: AgentModelDiscoveryStatus::Succeeded,
            error: None,
        },
        app_server_core::llm::ModelDiscoveryStatus::Failed(error) => AgentModelDiscoveryState {
            enabled: true,
            status: AgentModelDiscoveryStatus::Failed,
            error: Some(error.clone()),
        },
    }
}

fn reasoning_effort_options() -> Vec<String> {
    ["minimal", "low", "medium", "high", "xhigh"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn service_label_options(
    registry: &app_server_core::llm::AgentProviderRegistry,
    state: &AgentModelRuntimeState,
) -> Vec<String> {
    let (provider_id, _) = active_agent_model_ids(registry, state);
    match registry
        .provider(&provider_id)
        .map(|provider| provider.kind)
    {
        Some(app_server_core::llm::AgentProviderKind::OpenAiResponses) => {
            ["auto", "default", "flex"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        }
        _ => Vec::new(),
    }
}

fn active_reasoning_effort_applied(
    provider_kind: Option<app_server_core::llm::AgentProviderKind>,
    effort: Option<&str>,
    model: Option<&app_server_core::llm::ResolvedAgentModel>,
) -> bool {
    let Some(effort) = effort else {
        return false;
    };
    match provider_kind {
        Some(app_server_core::llm::AgentProviderKind::OpenAiResponses) => true,
        Some(app_server_core::llm::AgentProviderKind::AnthropicMessages) => {
            model.is_some_and(|model| {
                anthropic_thinking_budget_tokens(effort, model.max_tokens).is_some()
            })
        }
        None => false,
    }
}

fn active_service_label_applied(
    provider_kind: Option<app_server_core::llm::AgentProviderKind>,
    service_label: Option<&str>,
) -> bool {
    matches!(
        provider_kind,
        Some(app_server_core::llm::AgentProviderKind::OpenAiResponses)
    ) && service_label.and_then(openai_service_tier).is_some()
}

fn openai_service_tier(service_label: &str) -> Option<&'static str> {
    match service_label.trim().to_ascii_lowercase().as_str() {
        "auto" => Some("auto"),
        "default" => Some("default"),
        "flex" => Some("flex"),
        _ => None,
    }
}

fn anthropic_thinking_budget_tokens(effort: &str, max_tokens: u64) -> Option<u64> {
    let requested = match effort.trim().to_ascii_lowercase().as_str() {
        "minimal" | "low" => 1024,
        "medium" => 4096,
        "high" => 8192,
        "xhigh" => 16384,
        _ => return None,
    };
    (max_tokens > 1024).then_some(requested.min(max_tokens - 1))
}

use crate::plan_extraction::{
    SavedCadPlan, execution_scope_from_plan_ref, latest_saved_cad_plan, plan_target_handle,
};

async fn run_text_agent_rig(worker: &AgentWorker) -> Option<String> {
    let store = ChatStore::new(worker.workspace_root.clone());
    let history = store
        .history(&worker.run.session_id, None)
        .await
        .map(|response| response.messages)
        .unwrap_or_default();
    let config = match load_agent_model_registry().await.and_then(|registry| {
        let selection = app_server_core::llm::RigAgentConfigSelection {
            provider_id: worker.model_state.provider_id.clone(),
            model_id: worker.model_state.model_id.clone(),
            reasoning_effort: worker.model_state.reasoning_effort.clone(),
            service_label: worker.model_state.service_label.clone(),
        };
        app_server_core::llm::rig_config_from_registry_selection(registry, &selection)
            .map_err(|error| internal_error(error.message))
    }) {
        Ok(config) => config,
        Err(error) if error.message == "Rig Agent is not configured" => {
            let message =
                "Rig Agent is not configured. Set BUDN_AGENT_CONFIG or a provider API key env.";
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::LlmError,
                message,
            );
            append_agent_error_message(
                &worker.workspace_root,
                &worker.run,
                AgentErrorType::LlmError,
                message,
            )
            .await;
            return None;
        }
        Err(error) => {
            let message = error.message;
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::LlmError,
                message.clone(),
            );
            append_agent_error_message(
                &worker.workspace_root,
                &worker.run,
                AgentErrorType::LlmError,
                &message,
            )
            .await;
            return None;
        }
    };
    append_agent_capability_meta(
        &worker.workspace_root,
        &worker.run,
        config.provider_kind.as_str(),
        config.native_web_search,
    )
    .await;
    let mode = worker.mode;
    let execution_scope = match execution_scope_for_worker(worker, mode).await {
        Ok(scope) => scope,
        Err(error) => {
            let message = error.message;
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::PermissionDenied,
                message.clone(),
            );
            append_agent_error_message(
                &worker.workspace_root,
                &worker.run,
                AgentErrorType::PermissionDenied,
                &message,
            )
            .await;
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
        native_web_search_enabled: config.native_web_search,
        execution_scope: execution_scope.clone(),
    };
    let cadquery_runtime = Arc::new(HostCadQueryToolRuntime {
        workspace_root: worker.workspace_root.clone(),
        python: worker.python.clone(),
        cadquery_results: Arc::clone(&worker.cadquery_results),
        push_sink: Arc::clone(&worker.push_sink),
        run: worker.run.clone(),
    });
    let tool_executor: Arc<dyn app_server_core::ToolExecutor> = Arc::new(
        app_server_core::WorkspaceToolExecutor::new(worker.workspace_root.clone())
            .with_cadquery_runtime(cadquery_runtime),
    );
    let mut tool_context = AgentToolRunContext::new(worker.workspace_root.clone(), mode);
    tool_context.session_id = Some(worker.run.session_id.clone());
    tool_context.run_id = Some(worker.run.run_id.clone());
    tool_context.selections = worker.selection_snapshot.selections.clone();
    tool_context.active_selection_index = worker.selection_snapshot.active_index;
    tool_context.context_refs = worker.context_refs.clone();
    tool_context.execution_scope = execution_scope;
    let tool_observer = AgentToolEventRecorder {
        workspace_root: worker.workspace_root.clone(),
        cadquery_results: Arc::clone(&worker.cadquery_results),
        push_sink: Arc::clone(&worker.push_sink),
        run: worker.run.clone(),
        history_writes: Arc::new(Mutex::new(Vec::new())),
    };
    let token_push_sink = Arc::clone(&worker.push_sink);
    let token_run = worker.run.clone();
    let token_cancelled = Arc::clone(&worker.run.cancelled);
    let token_push = Arc::new(move |token: &str| {
        if token_cancelled.load(Ordering::SeqCst) {
            return false;
        }
        push_agent_token(&token_push_sink, &token_run, token);
        true
    });
    let reasoning_push_sink = Arc::clone(&worker.push_sink);
    let reasoning_run = worker.run.clone();
    let reasoning_cancelled = Arc::clone(&worker.run.cancelled);
    let reasoning_push = Arc::new(move |delta: &str| {
        if reasoning_cancelled.load(Ordering::SeqCst) {
            return false;
        }
        push_agent_reasoning(&reasoning_push_sink, &reasoning_run, delta);
        true
    });
    let cancelled_for_rig = Arc::clone(&worker.run.cancelled);
    let turn_result = run_rig_agent_turn_with_config(
        input,
        config,
        tool_executor,
        tool_context,
        &tool_observer,
        app_server_core::RigAgentCallbacks {
            on_token: token_push,
            on_reasoning: reasoning_push,
            cancelled: Arc::new(move || cancelled_for_rig.load(Ordering::SeqCst)),
        },
    )
    .await
    .map_err(|error| error.message);
    tool_observer.flush_history_writes().await;
    match turn_result {
        Ok(draft) => Some(draft.text),
        Err(message) => {
            let error_type = agent_error_type_for_rig_message(&message);
            push_agent_error(&worker.push_sink, &worker.run, error_type, message.clone());
            append_agent_error_message(&worker.workspace_root, &worker.run, error_type, &message)
                .await;
            None
        }
    }
}

fn agent_error_type_for_rig_message(message: &str) -> AgentErrorType {
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        AgentErrorType::Timeout
    } else {
        AgentErrorType::LlmError
    }
}

async fn execution_scope_for_worker(
    worker: &AgentWorker,
    mode: AgentMode,
) -> Result<Option<app_server_core::AgentExecutionScope>, ProtocolError> {
    if mode != AgentMode::Agent {
        return Ok(None);
    }
    let Some(plan_ref) = &worker.plan_ref else {
        return Ok(None);
    };
    execution_scope_from_plan_ref(&worker.workspace_root, plan_ref)
        .await
        .map(Some)
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

async fn append_agent_message(workspace_root: &Path, run: &AgentRunHandle, content: &str) {
    let store = ChatStore::new(workspace_root.to_path_buf());
    let _ = store
        .append_message_with_run_id(
            &run.session_id,
            ChatRole::Assistant,
            content,
            Vec::new(),
            None,
            Some(run.run_id.clone()),
        )
        .await;
}

async fn append_agent_capability_meta(
    workspace_root: &Path,
    run: &AgentRunHandle,
    provider: &str,
    native_web_search_enabled: bool,
) {
    let content = serde_json::json!({
        "type": "agent_run_capabilities",
        "provider": provider,
        "native_web_search_enabled": native_web_search_enabled,
    })
    .to_string();
    let store = ChatStore::new(workspace_root.to_path_buf());
    let _ = store
        .append_message_with_run_id(
            &run.session_id,
            ChatRole::Meta,
            &content,
            Vec::new(),
            None,
            Some(run.run_id.clone()),
        )
        .await;
}

async fn append_agent_error_message(
    workspace_root: &Path,
    run: &AgentRunHandle,
    error_type: AgentErrorType,
    message: &str,
) {
    append_agent_message(
        workspace_root,
        run,
        &format!("Agent run failed ({error_type:?}): {message}"),
    )
    .await;
}

fn cadquery_ready_for_tool_result(
    cadquery_results: &Arc<Mutex<CadQueryResultCache>>,
    result: &str,
) -> Option<app_server_protocol::CadQueryResultReady> {
    let value = serde_json::from_str::<serde_json::Value>(result).ok()?;
    let result_id = value.get("result_id")?.as_str()?;
    cadquery_results
        .lock()
        .ok()
        .and_then(|cache| cache.get(result_id).as_ref().map(cadquery_result_ready))
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

fn push_agent_reasoning(push_sink: &ServerPushSink, run: &AgentRunHandle, text: &str) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentReasoning(AgentReasoningEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            text: text.to_owned(),
        }),
    });
}

fn push_llm_tool_start(push_sink: &ServerPushSink, run: &AgentRunHandle, call: &AgentToolCall) {
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
    call: &AgentToolCall,
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
    log::error!("[agent run={}] {:?}: {}", run.run_id, error_type, msg);
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
fn is_hard_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata.nlink() > 1
}

#[cfg(not(unix))]
fn is_hard_link(_metadata: &std::fs::Metadata) -> bool {
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
    let watched_path_for_events = watched_path.clone();
    let watcher = FileWatcher::new(move |message| match message {
        app_server_core::WatchMessage::Changed(changed_paths) => {
            let changed_handles = watch_changed_paths_to_handles(
                &watched_handle,
                &watched_path_for_events,
                &changed_paths,
            );
            (push_sink)(ServerPushEnvelope {
                event: ServerPushEvent::WatchChanged(WatchChangedEvent {
                    subscription_id: subscription_id.clone(),
                    changed_paths: changed_handles,
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

pub fn watch_changed_paths_to_handles(
    watched_handle: &PathHandle,
    watched_path: &Path,
    changed_paths: &[PathBuf],
) -> Vec<PathHandle> {
    let mut handles = Vec::new();
    for changed_path in changed_paths {
        if let Some(handle) =
            watch_changed_path_to_handle(watched_handle, watched_path, changed_path)
        {
            if !handles.contains(&handle) {
                handles.push(handle);
            }
        }
    }
    if handles.is_empty() {
        handles.push(watched_handle.clone());
    }
    handles
}

fn watch_changed_path_to_handle(
    watched_handle: &PathHandle,
    watched_path: &Path,
    changed_path: &Path,
) -> Option<PathHandle> {
    let relative = changed_path.strip_prefix(watched_path).ok()?;
    let mut segments = watched_handle.path_segments().to_vec();
    for component in relative.components() {
        match component {
            Component::Normal(value) => segments.push(value.to_str()?.to_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    PathHandle::new(watched_handle.workspace_id().clone(), segments).ok()
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

fn server_capabilities(agent_provider: Option<AgentProviderCapabilities>) -> ServerCapabilities {
    let llm_configured = agent_provider.is_some();
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
        llm_configured,
        agent_provider,
        agent_model_registry: None,
    }
}

async fn server_capabilities_for_request(
    requested_preview_kinds: Vec<PreviewRequestKind>,
) -> ServerCapabilities {
    let registry = app_server_core::llm::load_agent_provider_registry_with_discovery()
        .await
        .ok()
        .flatten();
    let agent_model_registry = registry.as_ref().map(|registry| {
        agent_model_registry_response(registry, &AgentModelRuntimeState::default())
    });
    let agent_provider = agent_model_registry
        .as_ref()
        .and_then(agent_provider_capability_from_registry);
    let mut capabilities = server_capabilities(agent_provider);
    capabilities.agent_model_registry = agent_model_registry;
    capabilities.supported_preview_kinds = requested_preview_kinds
        .into_iter()
        .filter(|kind| matches!(kind, PreviewRequestKind::GeometryArtifact))
        .collect();
    if capabilities.supported_preview_kinds.is_empty() {
        capabilities.supported_preview_kinds = vec![PreviewRequestKind::GeometryArtifact];
    }
    capabilities
}

fn agent_provider_capability_from_registry(
    registry: &AgentModelRegistryResponse,
) -> Option<AgentProviderCapabilities> {
    let provider = registry
        .providers
        .iter()
        .find(|provider| provider.id == registry.active_provider_id)?;
    let model = provider
        .models
        .iter()
        .find(|model| model.id == registry.active_model_id)?;
    Some(AgentProviderCapabilities {
        provider: provider.kind.clone(),
        model: Some(model.id.clone()),
        native_web_search_enabled: model.native_web_search_applied,
        search_sources_supported: model.search_sources_supported,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_server_core::AgentToolObserver as _;
    use app_server_protocol::{
        CadQueryFeatureFaces, CadQueryPartMesh, ChatSessionId, EdgeGroup, FaceGroup, PreviewUnit,
        VertexPoint,
    };

    #[tokio::test]
    async fn agent_tool_recorder_flushes_history_in_event_order() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace_root = temp_dir.path().to_path_buf();
        let store = ChatStore::new(workspace_root.clone());
        let created = store
            .create("agent history", None, Vec::new())
            .await
            .expect("chat session should be created");
        let run = AgentRunHandle {
            session_id: created.session_id.clone(),
            run_id: "agent-1".into(),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let cadquery_results = Arc::new(Mutex::new(CadQueryResultCache::new(
            CADQUERY_RESULT_CACHE_LIMIT,
        )));
        cadquery_results
            .lock()
            .expect("cache lock")
            .insert_cached(CadQueryToolCachedResult {
                mesh: sample_mesh("cq_1"),
                exports: vec!["outputs/top_lid.step".into()],
                warnings: Vec::new(),
            });
        let recorder = AgentToolEventRecorder {
            workspace_root: workspace_root.clone(),
            cadquery_results,
            push_sink: Arc::new(|_| {}),
            run,
            history_writes: Arc::new(Mutex::new(Vec::new())),
        };
        let call = AgentToolCall {
            id: "call-1".into(),
            function_name: "read_file".into(),
            arguments: r#"{"path":"parts/top_lid.py"}"#.into(),
        };

        recorder.tool_start(&call);
        recorder.tool_result(&call, r#"{"status":"ok","result_id":"cq_1"}"#);
        recorder.flush_history_writes().await;

        let history = store
            .history(&created.session_id, None)
            .await
            .expect("history should be readable")
            .messages;
        assert_eq!(history.len(), 3);
        assert_eq!(history[1].role, ChatRole::Assistant);
        assert_eq!(history[1].tool_calls[0].tool_call_id, "call-1");
        assert_eq!(history[1].run_id.as_deref(), Some("agent-1"));
        assert_eq!(history[2].role, ChatRole::Tool);
        assert_eq!(
            history[2]
                .tool_result
                .as_ref()
                .expect("tool result")
                .tool_call_id,
            "call-1"
        );
        assert_eq!(history[2].run_id.as_deref(), Some("agent-1"));
        assert_eq!(
            history[2]
                .mesh_result
                .as_ref()
                .expect("mesh result should be persisted")
                .result_id,
            "cq_1"
        );
    }

    #[tokio::test]
    async fn agent_capability_meta_records_native_web_search_state() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace_root = temp_dir.path().to_path_buf();
        let store = ChatStore::new(workspace_root.clone());
        let created = store
            .create("agent capability", None, Vec::new())
            .await
            .expect("chat session should be created");
        let run = AgentRunHandle {
            session_id: created.session_id.clone(),
            run_id: "agent-1".into(),
            cancelled: Arc::new(AtomicBool::new(false)),
        };

        append_agent_capability_meta(&workspace_root, &run, "anthropic_messages", true).await;

        let history = store
            .history(&created.session_id, None)
            .await
            .expect("history should be readable")
            .messages;
        let (meta, value) = history
            .iter()
            .filter(|message| message.role == ChatRole::Meta)
            .filter_map(|message| {
                let value: serde_json::Value = serde_json::from_str(&message.content).ok()?;
                (value["type"] == "agent_run_capabilities").then_some((message, value))
            })
            .next()
            .expect("capability meta should be present");
        assert_eq!(value["type"], "agent_run_capabilities");
        assert_eq!(value["provider"], "anthropic_messages");
        assert_eq!(value["native_web_search_enabled"], true);
        assert_eq!(meta.run_id.as_deref(), Some("agent-1"));
    }

    #[test]
    fn rig_agent_errors_map_timeout_separately_from_provider_errors() {
        assert_eq!(
            agent_error_type_for_rig_message("Rig Agent request timed out"),
            AgentErrorType::Timeout
        );
        assert_eq!(
            agent_error_type_for_rig_message("Hosted tool 'web_search' is not supported"),
            AgentErrorType::LlmError
        );
        assert_eq!(
            agent_error_type_for_rig_message("rate limit exceeded"),
            AgentErrorType::LlmError
        );
        assert_eq!(
            agent_error_type_for_rig_message("authentication failed"),
            AgentErrorType::LlmError
        );
    }

    #[test]
    fn agent_invoke_model_state_does_not_inherit_params_across_models() {
        let current = AgentModelRuntimeState {
            provider_id: Some("openai".into()),
            model_id: Some("gpt-5.2".into()),
            reasoning_effort: Some("high".into()),
            service_label: Some("flex".into()),
        };

        let same_model = agent_model_state_for_request(
            &current,
            &agent_invoke_request("openai", "gpt-5.2", None, None),
        );
        assert_eq!(same_model.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(same_model.service_label.as_deref(), Some("flex"));

        let different_model = agent_model_state_for_request(
            &current,
            &agent_invoke_request("anthropic", "claude-sonnet", None, None),
        );
        assert_eq!(different_model.provider_id.as_deref(), Some("anthropic"));
        assert_eq!(different_model.model_id.as_deref(), Some("claude-sonnet"));
        assert!(different_model.reasoning_effort.is_none());
        assert!(different_model.service_label.is_none());
    }

    fn agent_invoke_request(
        provider_id: &str,
        model_id: &str,
        reasoning_effort: Option<&str>,
        service_label: Option<&str>,
    ) -> AgentInvokeRequest {
        AgentInvokeRequest {
            session_id: ChatSessionId("main".into()),
            prompt: "inspect".into(),
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
            provider_id: Some(provider_id.into()),
            model_id: Some(model_id.into()),
            reasoning_effort: reasoning_effort.map(str::to_owned),
            service_label: service_label.map(str::to_owned),
        }
    }

    fn sample_mesh(result_id: &str) -> CadQueryMeshPayload {
        CadQueryMeshPayload {
            result_id: result_id.into(),
            build_id: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            unit: PreviewUnit::Millimeter,
            root_ref_text: "@part[lid]".into(),
            root_object_kind: CadQueryObjectKind::Part,
            artifact_relation: None,
            parts: vec![CadQueryPartMesh {
                name: "lid".into(),
                object_kind: CadQueryObjectKind::Part,
                ref_text: "@part[lid]".into(),
                instance_path: None,
                transform: None,
                faces: vec![FaceGroup {
                    face_idx: 0,
                    positions: vec![0.0, 0.0, 0.0],
                    normals: vec![0.0, 0.0, 1.0],
                    features: vec!["lid_alignment_surface".into()],
                    ambiguous: false,
                }],
                edges: vec![EdgeGroup {
                    edge_idx: 0,
                    polyline: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    adjacent_faces: vec![0],
                }],
                vertices: vec![VertexPoint {
                    vertex_idx: 0,
                    position: [0.0, 0.0, 0.0],
                    adjacent_edges: vec![0],
                }],
                feature_map: vec![CadQueryFeatureFaces {
                    feature: "lid_alignment_surface".into(),
                    face_indices: vec![0],
                }],
            }],
        }
    }
}
