use app_server_core::{
    AGENT_ERROR_FACT_PREFIX, AgentToolCall, AgentToolRunContext, AgentTurnFinalFactKind,
    AgentTurnInput, CadQueryCommitScope, CadQueryContractConfig, CadQueryModelContract,
    CadQueryRunConfig, CadQueryRunResult, CadQueryRunnerError, CadQueryRunnerErrorKind,
    CadQueryToolCachedResult, CadQueryToolRunRequest, CadQueryToolRunResult, CadQueryToolRuntime,
    CadQueryToolRuntimeError, ChatIndexListenerRegistration, ChatStore, FileWatcher,
    HostedToolRequest, SlicerInstall, cadquery_result_ready, current_workspace_owned,
    detect_slicer_paths, export_model, list_workspace_entries_owned, load_config_dto,
    preview_ready_response, read_file_response_owned, register_chat_index_listener,
    resolve_workspace_path_owned, resolve_workspace_write_path_owned, run_cadquery_contract,
    run_cadquery_runner, run_cadquery_runner_with_cancel, run_rig_agent_turn_with_config,
    save_config_dto, send_to_slicer, stage_cadquery_project_owned,
};
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentCancelRequest, AgentCancelledResponse, AgentDoneEvent,
    AgentErrorEvent, AgentErrorType, AgentEventId, AgentEventPayload, AgentEventRecord,
    AgentHostedToolActivityEvent, AgentHostedToolActivityStatus, AgentId, AgentInvokeRequest,
    AgentMeshReadyEvent, AgentMode, AgentModelDiscoveryState, AgentModelDiscoveryStatus,
    AgentModelParamsUpdateRequest, AgentModelRegistryModel, AgentModelRegistryProvider,
    AgentModelRegistryResponse, AgentModelSelectRequest, AgentModelSource, AgentPlanConfirmRequest,
    AgentPlanProposedEvent, AgentPlanRejectRequest, AgentProviderCapabilities, AgentProviderType,
    AgentReasoningEvent, AgentRuntimeStatus, AgentSnapshotRequest, AgentSnapshotResponse,
    AgentStartTurnRequest, AgentStartedResponse, AgentSubscribeRequest, AgentSubscribeResponse,
    AgentTokenEvent, AgentToolResultEvent, AgentToolStartEvent, AgentTurnId, BoundAgentModel,
    CURRENT_PROTOCOL_VERSION, CadQueryExportFormat, CadQueryMeshPayload, CadQueryObjectKind,
    CapabilityHandshakeRequest, CapabilityHandshakeResponse, ChatListResponse, ChatRole,
    ChatSessionId, ChatToolCallRecord, ChatToolResultRecord, ClientCommand, ClientRequestEnvelope,
    CommandSuccess, ConfigLoadResponse, DEFAULT_SESSION_RECONNECT_WINDOW_MS, ExportRunResponse,
    FileWriteTextResponse, HostLocalPath, PathHandle, PreviewRequestKind, ProtocolError,
    ProtocolErrorCode, ProtocolVersionRange, SelectionUpdateRequest, SelectionUpdateResponse,
    ServerCapabilities, ServerPushEnvelope, ServerPushEvent, ServerResponseEnvelope,
    SessionReclaimedResponse, SessionToken, SubscriptionId, WatchChangedEvent, WatchErrorEvent,
    WatchSubscriptionAck, WorkspaceId, WorkspaceListResponse, negotiate_protocol_version,
};

use crate::cadquery_python_path;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Notify, futures::OwnedNotified, mpsc};

use crate::HostSession;

pub type ServerPushSink = Arc<dyn Fn(ServerPushEnvelope) + Send + Sync>;

const CADQUERY_RESULT_CACHE_LIMIT: usize = 8;
const CADQUERY_RUNNER_TIMEOUT: Duration = Duration::from_secs(180);
const CHAT_BOUND_MODEL_LOCK_REASON: &str = "chat_bound_model";
const AGENT_INITIAL_IDEMPOTENCY_WINDOW: Duration = Duration::from_millis(10);
static AGENT_RUNTIMES: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<WorkspaceAgentRuntime>>>>> =
    OnceLock::new();
static CHAT_PUSH_SUBSCRIBERS: OnceLock<Mutex<HashMap<PathBuf, HashMap<u64, ServerPushSink>>>> =
    OnceLock::new();
static NEXT_CHAT_PUSH_SUBSCRIBER_ID: AtomicU64 = AtomicU64::new(1);

struct ChatPushSubscription {
    workspace_path: PathBuf,
    id: u64,
}

impl Drop for ChatPushSubscription {
    fn drop(&mut self) {
        let Some(subscribers) = CHAT_PUSH_SUBSCRIBERS.get() else {
            return;
        };
        let Ok(mut subscribers) = subscribers.lock() else {
            return;
        };
        if let Some(workspace_subscribers) = subscribers.get_mut(&self.workspace_path) {
            workspace_subscribers.remove(&self.id);
            if workspace_subscribers.is_empty() {
                subscribers.remove(&self.workspace_path);
            }
        }
    }
}

fn register_chat_push_subscriber(
    workspace_path: Option<&Path>,
    push_sink: ServerPushSink,
) -> Option<ChatPushSubscription> {
    let workspace_path = workspace_path?.to_path_buf();
    let id = NEXT_CHAT_PUSH_SUBSCRIBER_ID.fetch_add(1, Ordering::SeqCst);
    let subscribers = CHAT_PUSH_SUBSCRIBERS.get_or_init(|| Mutex::new(HashMap::new()));
    subscribers
        .lock()
        .expect("Chat push subscriber map lock should not be poisoned")
        .entry(workspace_path.clone())
        .or_default()
        .insert(id, push_sink);
    Some(ChatPushSubscription { workspace_path, id })
}

fn broadcast_chat_list_changed(workspace_path: &Path, response: ChatListResponse) {
    let Some(subscribers) = CHAT_PUSH_SUBSCRIBERS.get() else {
        return;
    };
    let sinks = subscribers
        .lock()
        .expect("Chat push subscriber map lock should not be poisoned")
        .get(workspace_path)
        .map(|subscribers| subscribers.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for sink in sinks {
        sink(ServerPushEnvelope {
            event: ServerPushEvent::ChatListChanged(response.clone()),
        });
    }
}

fn register_host_chat_index_listener(
    workspace_path: Option<&Path>,
    push_sink: ServerPushSink,
) -> Option<ChatIndexListenerRegistration> {
    let workspace_path = workspace_path?.to_path_buf();
    let callback_workspace = workspace_path.clone();
    Some(register_chat_index_listener(
        &workspace_path,
        Arc::new(move || {
            let workspace = callback_workspace.clone();
            let push_sink = Arc::clone(&push_sink);
            tokio::spawn(async move {
                if let Ok(response) = ChatStore::new(workspace).list(false).await {
                    push_sink(ServerPushEnvelope {
                        event: ServerPushEvent::ChatListChanged(response),
                    });
                }
            });
        }),
    ))
}

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
    provider_type: Option<AgentProviderType>,
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
    agent_runtime: Arc<Mutex<WorkspaceAgentRuntime>>,
    agent_runtime_subscription: Option<AgentRuntimeSubscription>,
    agent_model_state: AgentModelRuntimeState,
    selection_snapshot: SelectionUpdateRequest,
    push_sink: ServerPushSink,
    chat_push_subscription: Option<ChatPushSubscription>,
    chat_index_listener: Option<ChatIndexListenerRegistration>,
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
        let agent_runtime = agent_runtime_for_workspace(workspace_path.as_deref());
        spawn_agent_startup_recovery(workspace_path.as_deref(), &agent_runtime);
        let agent_runtime_subscription =
            register_agent_runtime_subscriber(&agent_runtime, Arc::clone(&push_sink));
        let chat_push_subscription =
            register_chat_push_subscriber(workspace_path.as_deref(), Arc::clone(&push_sink));
        let chat_index_listener =
            register_host_chat_index_listener(workspace_path.as_deref(), Arc::clone(&push_sink));
        Self {
            workspace_id: WorkspaceId::new("workspace"),
            workspace_path,
            denied_extensions,
            next_subscription_id: 1,
            watchers: HashMap::new(),
            cadquery_results: Arc::new(Mutex::new(CadQueryResultCache::new(
                CADQUERY_RESULT_CACHE_LIMIT,
            ))),
            agent_runtime,
            agent_runtime_subscription,
            agent_model_state: AgentModelRuntimeState::default(),
            selection_snapshot: SelectionUpdateRequest {
                selections: Vec::new(),
                active_index: None,
            },
            push_sink,
            chat_push_subscription,
            chat_index_listener,
            session: HostSession::new(session_token, server_capabilities(None)),
        }
    }

    pub fn rebind_workspace(&mut self, workspace_path: PathBuf) {
        self.agent_runtime_subscription = None;
        self.agent_runtime = agent_runtime_for_workspace(Some(&workspace_path));
        spawn_agent_startup_recovery(Some(&workspace_path), &self.agent_runtime);
        self.agent_runtime_subscription =
            register_agent_runtime_subscriber(&self.agent_runtime, Arc::clone(&self.push_sink));
        self.chat_push_subscription =
            register_chat_push_subscriber(Some(&workspace_path), Arc::clone(&self.push_sink));
        self.chat_index_listener =
            register_host_chat_index_listener(Some(&workspace_path), Arc::clone(&self.push_sink));
        self.workspace_path = Some(workspace_path);
    }

    pub async fn handshake(
        &mut self,
        request: CapabilityHandshakeRequest,
    ) -> Result<CapabilityHandshakeResponse, ProtocolError> {
        let requested_preview_kinds = request.capabilities.supported_preview_kinds.clone();
        let negotiated_version = negotiate_protocol_version(
            request.capabilities.protocol_version,
            ProtocolVersionRange::new(CURRENT_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION),
        )?;
        let server_capabilities =
            server_capabilities_for_request(requested_preview_kinds, &self.agent_model_state)
                .await?;
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
        self.agent_runtime_subscription = None;
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
                let store = self.chat_store()?;
                let initial_turn = request.initial_turn.clone();
                let bound_model = request.requested_model.clone();
                if initial_turn.is_some() && bound_model.is_none() {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::InvalidCommand,
                        "chat.create initial_turn requires requested_model",
                    ));
                }
                let Some(initial_user_message) = request.initial_user_message else {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::InvalidCommand,
                        "chat.create requires initial_user_message",
                    ));
                };
                if initial_user_message.trim().is_empty() {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::InvalidCommand,
                        "chat.create initial_user_message must not be empty",
                    ));
                }
                let Some(client_request_id_value) = request.client_request_id.clone() else {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::InvalidCommand,
                        "chat.create requires client_request_id",
                    ));
                };
                if client_request_id_value.trim().is_empty() {
                    return Err(ProtocolError::new(
                        ProtocolErrorCode::InvalidCommand,
                        "chat.create client_request_id must not be empty",
                    ));
                }
                let client_request_id = Some(client_request_id_value.clone());
                let reserved_initial_turn = if initial_turn.is_some() {
                    self.reserve_initial_turn_for_chat_create(&store, &client_request_id_value)
                        .await?
                } else {
                    false
                };
                let persisted_run_id = if initial_turn.is_some() {
                    match workspace_persisted_run_id(&store).await {
                        Ok(run_id) => run_id,
                        Err(error) => {
                            if reserved_initial_turn {
                                self.release_initial_turn_reservation(&client_request_id_value)?;
                            }
                            return Err(error);
                        }
                    }
                } else {
                    None
                };
                let response = match store
                    .create_with_client_request_id_initial_message_and_model_outcome(
                        &client_request_id_value,
                        &request.title,
                        request.goal,
                        request.related_files,
                        initial_user_message.clone(),
                        request.requested_model,
                    )
                    .await
                {
                    Ok(response) => response,
                    Err(error) => {
                        if reserved_initial_turn {
                            self.release_initial_turn_reservation(&client_request_id_value)?;
                        }
                        return Err(error);
                    }
                };
                let (mut response, created_now) = response;
                if let Some(turn) = initial_turn
                    && created_now
                {
                    let started = match self.start_agent_run_with_reserved_initial_turn(
                        response.session_id.clone(),
                        response.agent_id.clone(),
                        client_request_id,
                        initial_user_message,
                        turn.mode,
                        turn.plan_ref,
                        turn.context_refs,
                        bound_model.clone(),
                        agent_model_state_for_bound_or_current(
                            bound_model.as_ref(),
                            &self.agent_model_state,
                        ),
                        None,
                        persisted_run_id,
                    ) {
                        Ok(started) => started,
                        Err(error) => {
                            if reserved_initial_turn {
                                self.release_initial_turn_reservation(&client_request_id_value)?;
                            }
                            return Err(error);
                        }
                    };
                    if let CommandSuccess::AgentStarted(started) = started {
                        response.initial_turn = Some(started);
                    }
                }
                if reserved_initial_turn && response.initial_turn.is_none() {
                    self.release_initial_turn_reservation(&client_request_id_value)?;
                }
                self.broadcast_chat_list_snapshot().await?;
                Ok(CommandSuccess::ChatCreated(response))
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
                        request.client_request_id,
                    )
                    .await
                    .map(CommandSuccess::ChatAck)
            }
            ClientCommand::ChatHistory(request) => {
                let store = self.chat_store()?;
                store.select(&request.session_id).await?;
                let response = store.history(&request.session_id, request.limit).await?;
                self.broadcast_chat_list_snapshot().await?;
                Ok(CommandSuccess::ChatHistory(response))
            }
            ClientCommand::ChatArchive(request) => {
                let response = self.chat_store()?.archive_owned(request.session_id).await?;
                self.broadcast_chat_list_snapshot().await?;
                Ok(CommandSuccess::ChatArchived(response))
            }
            ClientCommand::AgentInvoke(request) => self.start_agent_after_history(request).await,
            ClientCommand::AgentStartTurn(request) => self.start_agent_turn(request).await,
            ClientCommand::AgentCancel(request) => self.cancel_agent(request),
            ClientCommand::AgentSnapshot(request) => self.snapshot_agent(request).await,
            ClientCommand::AgentSubscribe(request) => self.subscribe_agent(request).await,
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

    async fn broadcast_chat_list_snapshot(&self) -> Result<(), ProtocolError> {
        let workspace_root = self.workspace_root()?.to_path_buf();
        let response = ChatStore::new(workspace_root.clone()).list(false).await?;
        broadcast_chat_list_changed(&workspace_root, response);
        Ok(())
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
        let selected_model = registry
            .provider(&request.provider_id)
            .and_then(|provider| {
                provider
                    .models
                    .iter()
                    .find(|model| model.id == request.model_id)
            });
        self.agent_model_state = AgentModelRuntimeState {
            provider_id: Some(request.provider_id),
            provider_type: None,
            model_id: Some(request.model_id),
            reasoning_effort: selected_model.and_then(|model| model.reasoning_effort.clone()),
            service_label: selected_model.and_then(|model| model.service_label.clone()),
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
        self.agent_model_state = AgentModelRuntimeState {
            provider_id: Some(request.provider_id),
            provider_type: None,
            model_id: Some(request.model_id),
            reasoning_effort: request.reasoning_effort,
            service_label: request.service_label,
        };
        Ok(agent_model_registry_response(
            &registry,
            &self.agent_model_state,
        ))
    }

    async fn reserve_initial_turn_for_chat_create(
        &self,
        store: &ChatStore,
        client_request_id: &str,
    ) -> Result<bool, ProtocolError> {
        loop {
            if store.has_create_request_id(client_request_id).await? {
                return Ok(false);
            }
            let outcome = self
                .agent_runtime
                .lock()
                .map_err(|_| internal_error("Agent registry lock poisoned"))?
                .registry
                .reserve_initial_turn(client_request_id)?;
            match outcome {
                InitialTurnReserveOutcome::Reserved => return Ok(true),
                InitialTurnReserveOutcome::DuplicateCommitted => {
                    tokio::task::yield_now().await;
                }
                InitialTurnReserveOutcome::DuplicateInProgress(notified) => {
                    notified.await;
                }
            }
        }
    }

    fn release_initial_turn_reservation(
        &self,
        client_request_id: &str,
    ) -> Result<(), ProtocolError> {
        self.agent_runtime
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .registry
            .release_initial_turn_reservation(client_request_id);
        Ok(())
    }

    async fn start_agent_after_history(
        &mut self,
        request: AgentInvokeRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        let store = self.chat_store()?;
        let agent_id = store.agent_id_for_session(&request.session_id).await?;
        let bound_model = store.bound_model_for_session(&request.session_id).await?;
        let persisted_events = recover_agent_persisted_events(
            &store,
            &self.agent_runtime,
            &request.session_id,
            &agent_id,
        )
        .await?;
        let persisted_event_id = max_agent_event_id(&persisted_events);
        let persisted_run_id =
            max_agent_run_id(&persisted_events).max(workspace_persisted_run_id(&store).await?);
        let model_state = agent_model_state_for_bound_or_request(
            bound_model.as_ref(),
            &self.agent_model_state,
            &request,
        );
        self.start_agent_run(
            request.session_id.clone(),
            agent_id,
            request.client_request_id.clone(),
            request.prompt,
            request.mode,
            request.plan_ref,
            request.context_refs,
            bound_model,
            model_state,
            persisted_event_id,
            persisted_run_id,
        )
    }

    async fn start_agent_turn(
        &mut self,
        request: AgentStartTurnRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        let store = self.chat_store()?;
        let session_id = store.session_id_for_agent(&request.agent_id).await?;
        let bound_model = store.bound_model_for_agent(&request.agent_id).await?;
        let persisted_events = recover_agent_persisted_events(
            &store,
            &self.agent_runtime,
            &session_id,
            &request.agent_id,
        )
        .await?;
        let persisted_event_id = max_agent_event_id(&persisted_events);
        let persisted_run_id =
            max_agent_run_id(&persisted_events).max(workspace_persisted_run_id(&store).await?);
        self.start_agent_run(
            session_id,
            request.agent_id,
            request.client_request_id,
            request.prompt,
            request.mode,
            request.plan_ref,
            request.context_refs,
            bound_model.clone(),
            agent_model_state_for_bound_or_current(bound_model.as_ref(), &self.agent_model_state),
            persisted_event_id,
            persisted_run_id,
        )
    }

    fn start_agent_run(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
        prompt: String,
        mode: AgentMode,
        plan_ref: Option<PathHandle>,
        context_refs: Vec<String>,
        bound_model: Option<BoundAgentModel>,
        model_state: AgentModelRuntimeState,
        persisted_event_id: Option<AgentEventId>,
        persisted_run_id: Option<u64>,
    ) -> Result<CommandSuccess, ProtocolError> {
        let run = self
            .agent_runtime
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .start_run(
                session_id,
                agent_id,
                client_request_id,
                bound_model,
                self.agent_subscriber_id(),
                persisted_event_id,
                persisted_run_id,
            )?;
        if !run.started_now {
            return Ok(CommandSuccess::AgentStarted(AgentStartedResponse {
                session_id: run.session_id,
                agent_id: run.agent_id,
                run_id: run.run_id,
                turn_id: run.turn_id,
            }));
        }
        let response = AgentStartedResponse {
            session_id: run.session_id.clone(),
            agent_id: run.agent_id.clone(),
            run_id: run.run_id.clone(),
            turn_id: run.turn_id.clone(),
        };
        let push_sink = agent_runtime_push_sink(&self.agent_runtime, &run);
        let worker = AgentWorker {
            run,
            prompt,
            mode,
            plan_ref,
            context_refs,
            model_state,
            selection_snapshot: self.selection_snapshot.clone(),
            workspace_root: self.workspace_root()?.to_path_buf(),
            python: cadquery_python_path(),
            cadquery_results: Arc::clone(&self.cadquery_results),
            agent_runtime: Arc::clone(&self.agent_runtime),
            push_sink,
        };
        tokio::spawn(run_agent_worker(worker));
        Ok(CommandSuccess::AgentStarted(response))
    }

    fn start_agent_run_with_reserved_initial_turn(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
        prompt: String,
        mode: AgentMode,
        plan_ref: Option<PathHandle>,
        context_refs: Vec<String>,
        bound_model: Option<BoundAgentModel>,
        model_state: AgentModelRuntimeState,
        persisted_event_id: Option<AgentEventId>,
        persisted_run_id: Option<u64>,
    ) -> Result<CommandSuccess, ProtocolError> {
        let run = self
            .agent_runtime
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .start_reserved_initial_turn(
                session_id,
                agent_id,
                client_request_id,
                bound_model,
                self.agent_subscriber_id(),
                persisted_event_id,
                persisted_run_id,
            )?;
        let response = AgentStartedResponse {
            session_id: run.session_id.clone(),
            agent_id: run.agent_id.clone(),
            run_id: run.run_id.clone(),
            turn_id: run.turn_id.clone(),
        };
        let push_sink = agent_runtime_push_sink(&self.agent_runtime, &run);
        let worker = AgentWorker {
            run,
            prompt,
            mode,
            plan_ref,
            context_refs,
            model_state,
            selection_snapshot: self.selection_snapshot.clone(),
            workspace_root: self.workspace_root()?.to_path_buf(),
            push_sink,
            cadquery_results: Arc::clone(&self.cadquery_results),
            agent_runtime: Arc::clone(&self.agent_runtime),
            python: cadquery_python_path(),
        };
        tokio::spawn(run_agent_worker(worker));
        Ok(CommandSuccess::AgentStarted(response))
    }

    fn cancel_agent(
        &mut self,
        request: AgentCancelRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        let cancelled = self
            .agent_runtime
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .cancel(&request.agent_id);
        if let Some(run) = cancelled {
            Ok(CommandSuccess::AgentCancelled(AgentCancelledResponse {
                agent_id: run.agent_id,
                cancelled: true,
            }))
        } else {
            Ok(CommandSuccess::AgentCancelled(AgentCancelledResponse {
                agent_id: request.agent_id,
                cancelled: false,
            }))
        }
    }

    async fn snapshot_agent(
        &mut self,
        request: AgentSnapshotRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        let store = self.chat_store()?;
        let chat_id = store.session_id_for_agent(&request.agent_id).await?;
        let bound_model = store.bound_model_for_agent(&request.agent_id).await?;
        let persisted_events = recover_agent_persisted_events(
            &store,
            &self.agent_runtime,
            &chat_id,
            &request.agent_id,
        )
        .await?;
        let snapshot = self
            .agent_runtime
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .snapshot(
                &request.agent_id,
                chat_id,
                bound_model,
                persisted_events,
                request.since_event_id,
            );
        Ok(CommandSuccess::AgentSnapshot(snapshot))
    }

    async fn subscribe_agent(
        &mut self,
        request: AgentSubscribeRequest,
    ) -> Result<CommandSuccess, ProtocolError> {
        self.chat_store()?
            .session_id_for_agent(&request.agent_id)
            .await?;
        let replays = self
            .agent_runtime
            .lock()
            .map_err(|_| internal_error("Agent registry lock poisoned"))?
            .subscribe_agent(
                self.agent_subscriber_id(),
                &request.agent_id,
                request.since_event_id,
            );
        for envelope in replays {
            (self.push_sink)(envelope);
        }
        loop {
            let pending = self
                .agent_runtime
                .lock()
                .map_err(|_| internal_error("Agent registry lock poisoned"))?
                .drain_or_activate_subscribe(self.agent_subscriber_id(), &request.agent_id);
            if pending.is_empty() {
                break;
            }
            for envelope in pending {
                (self.push_sink)(envelope);
            }
        }
        Ok(CommandSuccess::AgentSubscribed(AgentSubscribeResponse {
            agent_id: request.agent_id,
        }))
    }

    fn agent_subscriber_id(&self) -> Option<u64> {
        self.agent_runtime_subscription
            .as_ref()
            .map(|subscription| subscription.id)
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
struct WorkspaceAgentRuntime {
    workspace_root: Option<PathBuf>,
    registry: AgentRunRegistry,
    next_subscriber_id: u64,
    subscribers: HashMap<u64, AgentRuntimeSubscriber>,
    logs: HashMap<AgentId, AgentRuntimeLog>,
    next_event_id: u64,
    event_persist_sender: Option<mpsc::UnboundedSender<AgentEventRecord>>,
    pending_event_persists: Arc<Mutex<HashMap<AgentId, u64>>>,
}

struct AgentRuntimeSubscriber {
    push_sink: ServerPushSink,
    agents: HashSet<AgentId>,
    replaying_agents: HashSet<AgentId>,
    pending_events: HashMap<AgentId, VecDeque<ServerPushEnvelope>>,
}

struct AgentRuntimeSubscription {
    runtime: Arc<Mutex<WorkspaceAgentRuntime>>,
    id: u64,
}

impl Drop for AgentRuntimeSubscription {
    fn drop(&mut self) {
        let Ok(mut runtime) = self.runtime.lock() else {
            return;
        };
        runtime.unregister_subscriber(self.id);
    }
}

#[derive(Clone)]
struct AgentRuntimeLog {
    chat_id: app_server_protocol::ChatSessionId,
    bound_model: Option<BoundAgentModel>,
    state: AgentRuntimeStatus,
    active_turn_id: Option<AgentTurnId>,
    events: Vec<AgentEventRecord>,
    legacy_events: Vec<(AgentEventId, ServerPushEvent)>,
    current_text: String,
    current_reasoning: String,
    error: Option<String>,
}

fn agent_runtime_for_workspace(workspace_path: Option<&Path>) -> Arc<Mutex<WorkspaceAgentRuntime>> {
    let Some(workspace_path) = workspace_path else {
        return Arc::new(Mutex::new(WorkspaceAgentRuntime::default()));
    };
    let workspace_key =
        std::fs::canonicalize(workspace_path).unwrap_or_else(|_| workspace_path.to_path_buf());
    let runtimes = AGENT_RUNTIMES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut runtimes = runtimes
        .lock()
        .expect("Agent runtime map lock should not be poisoned");
    Arc::clone(runtimes.entry(workspace_key).or_insert_with(|| {
        Arc::new(Mutex::new(WorkspaceAgentRuntime {
            workspace_root: Some(workspace_path.to_path_buf()),
            ..WorkspaceAgentRuntime::default()
        }))
    }))
}

fn register_agent_runtime_subscriber(
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
    push_sink: ServerPushSink,
) -> Option<AgentRuntimeSubscription> {
    let mut runtime_lock = runtime.lock().ok()?;
    let id = runtime_lock.register_subscriber(push_sink);
    Some(AgentRuntimeSubscription {
        runtime: Arc::clone(runtime),
        id,
    })
}

fn spawn_agent_startup_recovery(
    workspace_path: Option<&Path>,
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
) {
    let Some(workspace_path) = workspace_path else {
        return;
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let store = ChatStore::new(workspace_path.to_path_buf());
    let runtime = Arc::clone(runtime);
    handle.spawn(async move {
        if let Err(error) = recover_workspace_agent_events(&store, &runtime).await {
            log::error!("[agent startup recovery] failed: {:?}", error);
        }
    });
}

fn agent_runtime_push_sink(
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
    run: &AgentRunHandle,
) -> ServerPushSink {
    let runtime = Arc::clone(runtime);
    let run = run.clone();
    Arc::new(move |envelope| {
        let sinks = runtime
            .lock()
            .map(|mut runtime| runtime.record_push_and_collect_sinks(&run, &envelope.event))
            .unwrap_or_default();
        for sink in sinks {
            (sink)(envelope.clone());
        }
    })
}

impl WorkspaceAgentRuntime {
    fn register_subscriber(&mut self, push_sink: ServerPushSink) -> u64 {
        self.next_subscriber_id = self.next_subscriber_id.saturating_add(1);
        let id = self.next_subscriber_id;
        self.subscribers.insert(
            id,
            AgentRuntimeSubscriber {
                push_sink,
                agents: HashSet::new(),
                replaying_agents: HashSet::new(),
                pending_events: HashMap::new(),
            },
        );
        id
    }

    fn unregister_subscriber(&mut self, id: u64) {
        self.subscribers.remove(&id);
        self.prune_idle_unobserved_logs();
    }

    fn subscribe_agent(
        &mut self,
        subscriber_id: Option<u64>,
        agent_id: &AgentId,
        since_event_id: Option<AgentEventId>,
    ) -> Vec<ServerPushEnvelope> {
        let Some(subscriber_id) = subscriber_id else {
            return Vec::new();
        };
        if let Some(subscriber) = self.subscribers.get_mut(&subscriber_id) {
            subscriber.agents.insert(agent_id.clone());
            subscriber.replaying_agents.insert(agent_id.clone());
        }
        self.replay_legacy_events(agent_id, since_event_id)
    }

    fn drain_or_activate_subscribe(
        &mut self,
        subscriber_id: Option<u64>,
        agent_id: &AgentId,
    ) -> Vec<ServerPushEnvelope> {
        let Some(subscriber_id) = subscriber_id else {
            return Vec::new();
        };
        let Some(subscriber) = self.subscribers.get_mut(&subscriber_id) else {
            return Vec::new();
        };
        let pending = subscriber
            .pending_events
            .remove(agent_id)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        if pending.is_empty() {
            subscriber.replaying_agents.remove(agent_id);
        }
        pending
    }

    fn start_run(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
        bound_model: Option<BoundAgentModel>,
        subscriber_id: Option<u64>,
        persisted_event_id: Option<AgentEventId>,
        persisted_run_id: Option<u64>,
    ) -> Result<AgentRunHandle, ProtocolError> {
        self.advance_event_cursor(persisted_event_id);
        self.advance_run_cursor(persisted_run_id);
        let run = self
            .registry
            .try_start(session_id, agent_id, client_request_id)?;
        self.record_run_started(&run, bound_model, subscriber_id);
        Ok(run)
    }

    fn start_reserved_initial_turn(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
        bound_model: Option<BoundAgentModel>,
        subscriber_id: Option<u64>,
        persisted_event_id: Option<AgentEventId>,
        persisted_run_id: Option<u64>,
    ) -> Result<AgentRunHandle, ProtocolError> {
        self.advance_event_cursor(persisted_event_id);
        self.advance_run_cursor(persisted_run_id);
        let run = self.registry.try_start_reserved_initial_turn(
            session_id,
            agent_id,
            client_request_id,
        )?;
        self.record_run_started(&run, bound_model, subscriber_id);
        Ok(run)
    }

    fn cancel(&mut self, agent_id: &AgentId) -> Option<AgentRunHandle> {
        self.registry.cancel(agent_id)
    }

    fn active_turn_id_for_agent(&self, agent_id: &AgentId) -> Option<AgentTurnId> {
        self.registry
            .running
            .as_ref()
            .filter(|run| &run.agent_id == agent_id)
            .map(|run| run.turn_id.clone())
    }

    #[cfg(test)]
    fn has_runtime_log_or_pending_persist_for_agent(&self, agent_id: &AgentId) -> bool {
        self.logs.contains_key(agent_id)
            || self
                .pending_event_persists
                .lock()
                .ok()
                .is_some_and(|pending| pending.contains_key(agent_id))
    }

    fn record_done_and_finish(
        &mut self,
        run: &AgentRunHandle,
        cancelled: bool,
    ) -> Option<(ServerPushEnvelope, Vec<ServerPushSink>)> {
        let is_current = self
            .registry
            .running
            .as_ref()
            .is_some_and(|active| active.run_id == run.run_id);
        if !is_current {
            return None;
        }
        let envelope = ServerPushEnvelope {
            event: ServerPushEvent::AgentDone(AgentDoneEvent {
                session_id: run.session_id.clone(),
                run_id: run.run_id.clone(),
                cancelled,
            }),
        };
        let sinks = self.record_push_and_collect_sinks(run, &envelope.event);
        self.registry.finish_if_current(&run.run_id);
        self.prune_idle_unobserved_logs();
        Some((envelope, sinks))
    }

    fn run_is_failed(&self, run: &AgentRunHandle) -> bool {
        self.logs
            .get(&run.agent_id)
            .is_some_and(|log| log.state == AgentRuntimeStatus::Failed)
    }

    fn finish_failed_without_done(&mut self, run: &AgentRunHandle) -> bool {
        let is_current = self
            .registry
            .running
            .as_ref()
            .is_some_and(|active| active.run_id == run.run_id);
        if !is_current {
            return false;
        }
        self.registry.finish_if_current(&run.run_id);
        self.prune_idle_unobserved_logs();
        true
    }

    fn snapshot(
        &self,
        agent_id: &AgentId,
        chat_id: app_server_protocol::ChatSessionId,
        bound_model: Option<BoundAgentModel>,
        persisted_events: Vec<AgentEventRecord>,
        since_event_id: Option<AgentEventId>,
    ) -> AgentSnapshotResponse {
        if let Some(log) = self.logs.get(agent_id) {
            let bound_model = log.bound_model.clone().or(bound_model);
            return AgentSnapshotResponse {
                agent_id: agent_id.clone(),
                chat_id: log.chat_id.clone(),
                model_lock_reason: model_lock_reason_for(bound_model.as_ref()),
                bound_model,
                state: log.state,
                active_turn_id: log.active_turn_id.clone(),
                since_event_id,
                events: merge_agent_events(&persisted_events, &log.events, since_event_id),
                current_text: log.current_text.clone(),
                current_reasoning: log.current_reasoning.clone(),
                error: log.error.clone(),
            };
        }
        if !persisted_events.is_empty() {
            let mut log = runtime_log_from_events(chat_id, bound_model.clone(), persisted_events);
            interrupt_restored_running_log(&mut log);
            return AgentSnapshotResponse {
                agent_id: agent_id.clone(),
                chat_id: log.chat_id,
                model_lock_reason: model_lock_reason_for(bound_model.as_ref()),
                bound_model,
                state: log.state,
                active_turn_id: log.active_turn_id,
                since_event_id,
                events: filter_agent_events(&log.events, since_event_id),
                current_text: log.current_text,
                current_reasoning: log.current_reasoning,
                error: log.error,
            };
        }
        idle_agent_snapshot(agent_id.clone(), chat_id, bound_model, since_event_id)
    }

    fn advance_event_cursor(&mut self, persisted_event_id: Option<AgentEventId>) {
        if let Some(event_id) = persisted_event_id {
            self.next_event_id = self.next_event_id.max(event_id.0);
        }
    }

    fn advance_run_cursor(&mut self, persisted_run_id: Option<u64>) {
        if let Some(run_id) = persisted_run_id {
            self.registry.next_run_id = self.registry.next_run_id.max(run_id);
        }
    }

    fn prune_idle_unobserved_logs(&mut self) {
        let observed_agents = self
            .subscribers
            .values()
            .flat_map(|subscriber| subscriber.agents.iter().cloned())
            .collect::<HashSet<_>>();
        let running_agent = self
            .registry
            .running
            .as_ref()
            .map(|run| run.agent_id.clone());
        self.logs.retain(|agent_id, log| {
            running_agent.as_ref() == Some(agent_id)
                || observed_agents.contains(agent_id)
                || log.state == AgentRuntimeStatus::Running
        });
    }

    fn record_run_started(
        &mut self,
        run: &AgentRunHandle,
        bound_model: Option<BoundAgentModel>,
        subscriber_id: Option<u64>,
    ) {
        if !run.started_now {
            return;
        }
        if let Some(subscriber_id) = subscriber_id
            && let Some(subscriber) = self.subscribers.get_mut(&subscriber_id)
        {
            subscriber.agents.insert(run.agent_id.clone());
        }
        self.ensure_log(run, bound_model);
        self.record_payload(
            run,
            AgentEventPayload::StateChanged {
                state: AgentRuntimeStatus::Running,
            },
            None,
        );
    }

    fn record_push_and_collect_sinks(
        &mut self,
        run: &AgentRunHandle,
        event: &ServerPushEvent,
    ) -> Vec<ServerPushSink> {
        if let Some(payload) = agent_payload_from_push(event) {
            self.record_payload(run, payload, Some(event.clone()));
        } else if is_agent_runtime_legacy_push(event) {
            self.record_legacy_push(run, event.clone());
        }
        let envelope = ServerPushEnvelope {
            event: event.clone(),
        };
        let mut sinks = Vec::new();
        for subscriber in self.subscribers.values_mut() {
            if !subscriber.agents.contains(&run.agent_id) {
                continue;
            }
            if subscriber.replaying_agents.contains(&run.agent_id) {
                subscriber
                    .pending_events
                    .entry(run.agent_id.clone())
                    .or_default()
                    .push_back(envelope.clone());
            } else {
                sinks.push(Arc::clone(&subscriber.push_sink));
            }
        }
        sinks
    }

    fn record_legacy_push(&mut self, run: &AgentRunHandle, event: ServerPushEvent) {
        let event_id = self.next_agent_event_id();
        let log = self.ensure_log(run, None);
        log.legacy_events.push((event_id, event));
    }

    fn record_payload(
        &mut self,
        run: &AgentRunHandle,
        payload: AgentEventPayload,
        legacy_event: Option<ServerPushEvent>,
    ) {
        let event_id = self.next_agent_event_id();
        let record = AgentEventRecord {
            event_id,
            agent_id: run.agent_id.clone(),
            turn_id: Some(run.turn_id.clone()),
            ts_ms: unix_now_ms(),
            payload,
        };
        let record_to_persist = record.clone();
        let log = self.ensure_log(run, None);
        update_runtime_log(log, &record.payload);
        if matches!(
            record.payload,
            AgentEventPayload::StateChanged {
                state: AgentRuntimeStatus::Running
            }
        ) {
            log.active_turn_id = Some(run.turn_id.clone());
        }
        log.events.push(record);
        if let Some(legacy_event) = legacy_event {
            log.legacy_events.push((event_id, legacy_event));
        }
        self.queue_event_record_persist(record_to_persist);
    }

    fn queue_event_record_persist(&mut self, record: AgentEventRecord) {
        let Some(workspace_root) = self.workspace_root.clone() else {
            return;
        };
        self.increment_pending_event_persist(&record.agent_id);
        if self.event_persist_sender.is_none() {
            let Ok(handle) = tokio::runtime::Handle::try_current() else {
                self.decrement_pending_event_persist(&record.agent_id);
                return;
            };
            let (sender, mut receiver) = mpsc::unbounded_channel::<AgentEventRecord>();
            let pending_event_persists = Arc::clone(&self.pending_event_persists);
            handle.spawn(async move {
                let store = ChatStore::new(workspace_root);
                while let Some(record) = receiver.recv().await {
                    if let Err(error) = store.append_agent_event(&record.agent_id, &record).await {
                        log::error!(
                            "[agent event persist agent={}] failed: {:?}",
                            record.agent_id.0,
                            error
                        );
                    }
                    decrement_pending_event_persist(&pending_event_persists, &record.agent_id);
                }
            });
            self.event_persist_sender = Some(sender);
        }
        if let Some(sender) = &self.event_persist_sender {
            if let Err(error) = sender.send(record) {
                self.decrement_pending_event_persist(&error.0.agent_id);
            }
        }
    }

    fn increment_pending_event_persist(&self, agent_id: &AgentId) {
        if let Ok(mut pending) = self.pending_event_persists.lock() {
            *pending.entry(agent_id.clone()).or_insert(0) += 1;
        }
    }

    fn decrement_pending_event_persist(&self, agent_id: &AgentId) {
        decrement_pending_event_persist(&self.pending_event_persists, agent_id);
    }

    fn ensure_log(
        &mut self,
        run: &AgentRunHandle,
        bound_model: Option<BoundAgentModel>,
    ) -> &mut AgentRuntimeLog {
        let log = self
            .logs
            .entry(run.agent_id.clone())
            .or_insert_with(|| AgentRuntimeLog {
                chat_id: run.session_id.clone(),
                bound_model: bound_model.clone(),
                state: AgentRuntimeStatus::Idle,
                active_turn_id: None,
                events: Vec::new(),
                legacy_events: Vec::new(),
                current_text: String::new(),
                current_reasoning: String::new(),
                error: None,
            });
        log.chat_id = run.session_id.clone();
        if bound_model.is_some() {
            log.bound_model = bound_model;
        }
        log
    }

    fn next_agent_event_id(&mut self) -> AgentEventId {
        self.next_event_id = self.next_event_id.saturating_add(1);
        AgentEventId(self.next_event_id)
    }

    fn replay_legacy_events(
        &self,
        agent_id: &AgentId,
        since_event_id: Option<AgentEventId>,
    ) -> Vec<ServerPushEnvelope> {
        self.logs
            .get(agent_id)
            .map(|log| {
                log.legacy_events
                    .iter()
                    .filter(|(event_id, _)| event_is_after(*event_id, since_event_id))
                    .map(|(_, event)| ServerPushEnvelope {
                        event: event.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn idle_agent_snapshot(
    agent_id: AgentId,
    chat_id: app_server_protocol::ChatSessionId,
    bound_model: Option<BoundAgentModel>,
    since_event_id: Option<AgentEventId>,
) -> AgentSnapshotResponse {
    AgentSnapshotResponse {
        agent_id,
        chat_id,
        model_lock_reason: model_lock_reason_for(bound_model.as_ref()),
        bound_model,
        state: AgentRuntimeStatus::Idle,
        active_turn_id: None,
        since_event_id,
        events: Vec::new(),
        current_text: String::new(),
        current_reasoning: String::new(),
        error: None,
    }
}

fn model_lock_reason_for(bound_model: Option<&BoundAgentModel>) -> Option<String> {
    bound_model.map(|_| CHAT_BOUND_MODEL_LOCK_REASON.to_owned())
}

fn filter_agent_events(
    events: &[AgentEventRecord],
    since_event_id: Option<AgentEventId>,
) -> Vec<AgentEventRecord> {
    events
        .iter()
        .filter(|event| event_is_after(event.event_id, since_event_id))
        .cloned()
        .collect()
}

fn merge_agent_events(
    persisted: &[AgentEventRecord],
    memory: &[AgentEventRecord],
    since_event_id: Option<AgentEventId>,
) -> Vec<AgentEventRecord> {
    let mut by_id = HashMap::new();
    for event in persisted.iter().chain(memory.iter()) {
        if event_is_after(event.event_id, since_event_id) {
            by_id.insert(event.event_id, event.clone());
        }
    }
    let mut events = by_id.into_values().collect::<Vec<_>>();
    events.sort_by_key(|event| event.event_id.0);
    events
}

fn runtime_log_from_events(
    chat_id: app_server_protocol::ChatSessionId,
    bound_model: Option<BoundAgentModel>,
    events: Vec<AgentEventRecord>,
) -> AgentRuntimeLog {
    let mut log = AgentRuntimeLog {
        chat_id,
        bound_model,
        state: AgentRuntimeStatus::Idle,
        active_turn_id: None,
        events: Vec::new(),
        legacy_events: Vec::new(),
        current_text: String::new(),
        current_reasoning: String::new(),
        error: None,
    };
    for event in events {
        update_runtime_log(&mut log, &event.payload);
        if matches!(
            event.payload,
            AgentEventPayload::StateChanged {
                state: AgentRuntimeStatus::Running
            }
        ) {
            log.active_turn_id = event.turn_id.clone();
        }
        log.events.push(event);
    }
    log
}

fn interrupt_restored_running_log(log: &mut AgentRuntimeLog) {
    if log.state == AgentRuntimeStatus::Running {
        log.state = AgentRuntimeStatus::Interrupted;
        log.active_turn_id = None;
    }
}

fn max_agent_event_id(events: &[AgentEventRecord]) -> Option<AgentEventId> {
    events
        .iter()
        .map(|event| event.event_id.0)
        .max()
        .map(AgentEventId)
}

fn max_agent_run_id(events: &[AgentEventRecord]) -> Option<u64> {
    events
        .iter()
        .filter_map(|event| event.turn_id.as_ref())
        .filter_map(agent_turn_run_id)
        .max()
}

fn agent_turn_run_id(turn_id: &AgentTurnId) -> Option<u64> {
    turn_id.0.strip_prefix("agent-")?.parse::<u64>().ok()
}

fn agent_turn_id_is_after(left: &AgentTurnId, right: &AgentTurnId) -> bool {
    match (agent_turn_run_id(left), agent_turn_run_id(right)) {
        (Some(left), Some(right)) => left > right,
        _ => false,
    }
}

async fn recover_agent_persisted_events(
    store: &ChatStore,
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
    session_id: &ChatSessionId,
    agent_id: &AgentId,
) -> Result<Vec<AgentEventRecord>, ProtocolError> {
    wait_for_pending_event_persist(runtime, agent_id).await?;
    let mut events = store.read_agent_events(agent_id, None).await?;
    let latest_final_fact = store
        .latest_agent_turn_final_fact(session_id, agent_id)
        .await?;
    let event_turn_id = last_agent_turn_id(&events);
    let (turn_id, final_fact) = match (event_turn_id, latest_final_fact) {
        (Some(event_turn_id), Some(final_fact))
            if agent_turn_id_is_after(&final_fact.turn_id, &event_turn_id) =>
        {
            if runtime_active_turn_id(runtime, agent_id)? == Some(final_fact.turn_id.clone()) {
                return Ok(events);
            }
            (final_fact.turn_id, Some(final_fact.kind))
        }
        (Some(turn_id), _) => {
            if runtime_active_turn_id(runtime, agent_id)? == Some(turn_id.clone()) {
                return Ok(events);
            }
            let final_fact = store
                .agent_turn_final_fact_kind(session_id, agent_id, &turn_id)
                .await?;
            (turn_id, final_fact)
        }
        (None, Some(final_fact)) => {
            if runtime_active_turn_id(runtime, agent_id)? == Some(final_fact.turn_id.clone()) {
                return Ok(events);
            }
            (final_fact.turn_id, Some(final_fact.kind))
        }
        (None, None) => return Ok(events),
    };
    let terminal_status = terminal_status_for_turn(&events, &turn_id);
    let recovery_payload = match (terminal_status, final_fact) {
        (None, None) => Some(AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::Interrupted,
        }),
        (None, Some(AgentTurnFinalFactKind::Success)) => {
            Some(AgentEventPayload::Done { cancelled: false })
        }
        (None, Some(AgentTurnFinalFactKind::Failure)) => Some(AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::Failed,
        }),
        (Some(AgentRuntimeStatus::Done), Some(AgentTurnFinalFactKind::Failure)) => {
            Some(AgentEventPayload::StateChanged {
                state: AgentRuntimeStatus::FailedNeedsRecovery,
            })
        }
        (Some(AgentRuntimeStatus::Failed), Some(AgentTurnFinalFactKind::Success))
        | (Some(AgentRuntimeStatus::Cancelled), Some(_)) => Some(AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::FailedNeedsRecovery,
        }),
        (Some(AgentRuntimeStatus::Done | AgentRuntimeStatus::Failed), None) => {
            Some(AgentEventPayload::StateChanged {
                state: AgentRuntimeStatus::FailedNeedsRecovery,
            })
        }
        _ => None,
    };
    if let Some(payload) = recovery_payload {
        if runtime_active_turn_id(runtime, agent_id)? == Some(turn_id.clone()) {
            return Ok(events);
        }
        events = store
            .recover_agent_event_if_current(agent_id, &turn_id, payload)
            .await?;
    }
    Ok(events)
}

async fn recover_workspace_agent_events(
    store: &ChatStore,
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
) -> Result<(), ProtocolError> {
    let sessions = store.agent_identities(true).await?;
    for session in sessions {
        if let Err(error) =
            recover_agent_persisted_events(store, runtime, &session.session_id, &session.agent_id)
                .await
        {
            log::error!(
                "[agent startup recovery session={} agent={}] failed: {:?}",
                session.session_id.0,
                session.agent_id.0,
                error
            );
        }
    }
    Ok(())
}

async fn workspace_persisted_run_id(store: &ChatStore) -> Result<Option<u64>, ProtocolError> {
    let mut max_run_id = None;
    for identity in store.agent_identities(true).await? {
        let events = match store.read_agent_events(&identity.agent_id, None).await {
            Ok(events) => events,
            Err(error) => {
                log::error!(
                    "[agent run cursor session={} agent={}] failed: {:?}",
                    identity.session_id.0,
                    identity.agent_id.0,
                    error
                );
                return Err(error);
            }
        };
        max_run_id = max_run_id.max(max_agent_run_id(&events));
        max_run_id = max_run_id.max(
            store
                .max_agent_turn_run_id(&identity.session_id, &identity.agent_id)
                .await?,
        );
    }
    Ok(max_run_id)
}

async fn wait_for_pending_event_persist(
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
    agent_id: &AgentId,
) -> Result<(), ProtocolError> {
    for _ in 0..200 {
        if !runtime_has_pending_event_persist(runtime, agent_id)? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Err(internal_error(format!(
        "Agent event persist 未完成，暂停恢复: {}",
        agent_id.0
    )))
}

fn decrement_pending_event_persist(
    pending_event_persists: &Arc<Mutex<HashMap<AgentId, u64>>>,
    agent_id: &AgentId,
) {
    if let Ok(mut pending) = pending_event_persists.lock()
        && let Some(count) = pending.get_mut(agent_id)
    {
        *count = count.saturating_sub(1);
        if *count == 0 {
            pending.remove(agent_id);
        }
    }
}

fn runtime_active_turn_id(
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
    agent_id: &AgentId,
) -> Result<Option<AgentTurnId>, ProtocolError> {
    runtime
        .lock()
        .map_err(|_| internal_error("Agent registry lock poisoned"))
        .map(|runtime| runtime.active_turn_id_for_agent(agent_id))
}

fn runtime_has_pending_event_persist(
    runtime: &Arc<Mutex<WorkspaceAgentRuntime>>,
    agent_id: &AgentId,
) -> Result<bool, ProtocolError> {
    runtime
        .lock()
        .map_err(|_| internal_error("Agent registry lock poisoned"))
        .map(|runtime| {
            runtime
                .pending_event_persists
                .lock()
                .ok()
                .is_some_and(|pending| pending.contains_key(agent_id))
        })
}

fn last_agent_turn_id(events: &[AgentEventRecord]) -> Option<AgentTurnId> {
    events.iter().rev().find_map(|event| event.turn_id.clone())
}

fn terminal_status_for_turn(
    events: &[AgentEventRecord],
    turn_id: &AgentTurnId,
) -> Option<AgentRuntimeStatus> {
    let mut status = None;
    for event in events
        .iter()
        .filter(|event| event.turn_id.as_ref() == Some(turn_id))
    {
        match &event.payload {
            AgentEventPayload::Done { cancelled } => {
                if *cancelled {
                    status = Some(AgentRuntimeStatus::Cancelled);
                } else if status != Some(AgentRuntimeStatus::Failed) {
                    status = Some(AgentRuntimeStatus::Done);
                }
            }
            AgentEventPayload::Error { .. } => status = Some(AgentRuntimeStatus::Failed),
            AgentEventPayload::StateChanged { state } if is_terminal_runtime_status(*state) => {
                status = Some(*state);
            }
            _ => {}
        }
    }
    status
}

fn is_terminal_runtime_status(status: AgentRuntimeStatus) -> bool {
    matches!(
        status,
        AgentRuntimeStatus::Done
            | AgentRuntimeStatus::Failed
            | AgentRuntimeStatus::Cancelled
            | AgentRuntimeStatus::Interrupted
            | AgentRuntimeStatus::FailedNeedsRecovery
    )
}

fn event_is_after(event_id: AgentEventId, since_event_id: Option<AgentEventId>) -> bool {
    since_event_id.is_none_or(|since| event_id.0 > since.0)
}

fn update_runtime_log(log: &mut AgentRuntimeLog, payload: &AgentEventPayload) {
    match payload {
        AgentEventPayload::StateChanged { state } => {
            log.state = *state;
            if *state == AgentRuntimeStatus::Running {
                log.current_text.clear();
                log.current_reasoning.clear();
                log.error = None;
            } else if is_terminal_runtime_status(*state) {
                log.active_turn_id = None;
            }
        }
        AgentEventPayload::Token { text } => log.current_text.push_str(text),
        AgentEventPayload::Reasoning { text } => log.current_reasoning.push_str(text),
        AgentEventPayload::Error { message, .. } => {
            log.state = AgentRuntimeStatus::Failed;
            log.error = Some(message.clone());
            log.active_turn_id = None;
        }
        AgentEventPayload::Done { cancelled } => {
            if *cancelled {
                log.state = AgentRuntimeStatus::Cancelled;
            } else if log.state != AgentRuntimeStatus::Failed {
                log.state = AgentRuntimeStatus::Done;
            }
            log.active_turn_id = None;
        }
        AgentEventPayload::ToolStart { .. }
        | AgentEventPayload::ToolResult { .. }
        | AgentEventPayload::HostedToolActivity { .. } => {}
    }
}

fn agent_payload_from_push(event: &ServerPushEvent) -> Option<AgentEventPayload> {
    match event {
        ServerPushEvent::AgentToken(event) => Some(AgentEventPayload::Token {
            text: event.text.clone(),
        }),
        ServerPushEvent::AgentReasoning(event) => Some(AgentEventPayload::Reasoning {
            text: event.text.clone(),
        }),
        ServerPushEvent::AgentToolStart(event) => Some(AgentEventPayload::ToolStart {
            tool_call_id: event.tool_call_id.clone(),
            tool_name: event.tool_name.clone(),
            args_json: event.args_json.clone(),
        }),
        ServerPushEvent::AgentToolResult(event) => Some(AgentEventPayload::ToolResult {
            tool_call_id: event.tool_call_id.clone(),
            tool_name: event.tool_name.clone(),
            result_json: event.result_json.clone(),
        }),
        ServerPushEvent::AgentHostedToolActivity(event) => {
            Some(AgentEventPayload::HostedToolActivity {
                provider_id: event.provider_id.clone(),
                provider_kind: event.provider_kind,
                tool_type: event.tool_type.clone(),
                status: event.status,
            })
        }
        ServerPushEvent::AgentError(event) => Some(AgentEventPayload::Error {
            error_type: event.error_type,
            message: event.message.clone(),
        }),
        ServerPushEvent::AgentDone(event) => Some(AgentEventPayload::Done {
            cancelled: event.cancelled,
        }),
        _ => None,
    }
}

fn is_agent_runtime_legacy_push(event: &ServerPushEvent) -> bool {
    matches!(
        event,
        ServerPushEvent::AgentMeshReady(_)
            | ServerPushEvent::AgentPlanProposed(_)
            | ServerPushEvent::AgentPlanSaved(_)
    )
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[derive(Default)]
struct AgentRunRegistry {
    next_run_id: u64,
    running: Option<AgentRunHandle>,
    initial_turn_reserved: Option<InitialTurnReservation>,
    running_initial_create_request_id: Option<String>,
    started_by_request: HashMap<AgentRunRequestKey, AgentRunHandle>,
}

#[derive(Clone)]
struct InitialTurnReservation {
    request_id: String,
    notify: Arc<Notify>,
}

enum InitialTurnReserveOutcome {
    Reserved,
    DuplicateCommitted,
    DuplicateInProgress(OwnedNotified),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AgentRunRequestKey {
    session_id: app_server_protocol::ChatSessionId,
    request_id: String,
}

impl AgentRunRegistry {
    fn reserve_initial_turn(
        &mut self,
        client_request_id: &str,
    ) -> Result<InitialTurnReserveOutcome, ProtocolError> {
        if self.running.is_some() {
            if self.running_initial_create_request_id.as_deref() == Some(client_request_id) {
                return Ok(InitialTurnReserveOutcome::DuplicateCommitted);
            }
            return Err(ProtocolError::new(
                ProtocolErrorCode::AgentBusy,
                "已有 Agent session 正在运行",
            ));
        }
        if let Some(reservation) = &self.initial_turn_reserved {
            if reservation.request_id == client_request_id {
                return Ok(InitialTurnReserveOutcome::DuplicateInProgress(
                    Arc::clone(&reservation.notify).notified_owned(),
                ));
            }
            return Err(ProtocolError::new(
                ProtocolErrorCode::AgentBusy,
                "已有 Agent session 正在运行",
            ));
        }
        self.initial_turn_reserved = Some(InitialTurnReservation {
            request_id: client_request_id.to_owned(),
            notify: Arc::new(Notify::new()),
        });
        Ok(InitialTurnReserveOutcome::Reserved)
    }

    fn release_initial_turn_reservation(&mut self, client_request_id: &str) {
        let Some(reservation) = &self.initial_turn_reserved else {
            return;
        };
        if reservation.request_id != client_request_id {
            return;
        }
        if let Some(reservation) = self.initial_turn_reserved.take() {
            reservation.notify.notify_waiters();
        }
    }

    fn try_start(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
    ) -> Result<AgentRunHandle, ProtocolError> {
        let request_key = client_request_id
            .as_ref()
            .map(|request_id| AgentRunRequestKey {
                session_id: session_id.clone(),
                request_id: request_id.clone(),
            });
        if let Some(request_key) = request_key.as_ref() {
            if let Some(run) = self.started_by_request.get(request_key) {
                return Ok(run.clone().as_existing());
            }
        }
        if self.running.is_some() || self.initial_turn_reserved.is_some() {
            return Err(ProtocolError::new(
                ProtocolErrorCode::AgentBusy,
                "已有 Agent session 正在运行",
            ));
        }
        self.next_run_id = self.next_run_id.saturating_add(1);
        let run_id = format!("agent-{}", self.next_run_id);
        let run = AgentRunHandle {
            session_id,
            agent_id,
            run_id: run_id.clone(),
            turn_id: AgentTurnId(run_id),
            cancelled: Arc::new(AtomicBool::new(false)),
            started_now: true,
            started_at: Instant::now(),
        };
        self.running = Some(run.clone());
        self.running_initial_create_request_id = None;
        if let Some(request_key) = request_key {
            self.started_by_request.insert(request_key, run.clone());
        }
        Ok(run)
    }

    fn try_start_reserved_initial_turn(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
    ) -> Result<AgentRunHandle, ProtocolError> {
        let notify = self.take_initial_turn_reservation(client_request_id.as_deref());
        if notify.is_none() {
            return self.try_start(session_id, agent_id, client_request_id);
        }
        let run = self.start_without_busy_check(session_id, agent_id, client_request_id);
        if let Some(notify) = notify {
            notify.notify_waiters();
        }
        run
    }

    fn take_initial_turn_reservation(
        &mut self,
        client_request_id: Option<&str>,
    ) -> Option<Arc<Notify>> {
        let reservation = self.initial_turn_reserved.as_ref()?;
        if client_request_id != Some(reservation.request_id.as_str()) {
            return None;
        }
        self.initial_turn_reserved
            .take()
            .map(|reservation| reservation.notify)
    }

    fn start_without_busy_check(
        &mut self,
        session_id: app_server_protocol::ChatSessionId,
        agent_id: AgentId,
        client_request_id: Option<String>,
    ) -> Result<AgentRunHandle, ProtocolError> {
        let initial_create_request_id = client_request_id.clone();
        let request_key = client_request_id
            .as_ref()
            .map(|request_id| AgentRunRequestKey {
                session_id: session_id.clone(),
                request_id: request_id.clone(),
            });
        if let Some(request_key) = request_key.as_ref()
            && let Some(run) = self.started_by_request.get(request_key)
        {
            return Ok(run.clone().as_existing());
        }
        self.next_run_id = self.next_run_id.saturating_add(1);
        let run_id = format!("agent-{}", self.next_run_id);
        let run = AgentRunHandle {
            session_id,
            agent_id,
            run_id: run_id.clone(),
            turn_id: AgentTurnId(run_id),
            cancelled: Arc::new(AtomicBool::new(false)),
            started_now: true,
            started_at: Instant::now(),
        };
        self.running = Some(run.clone());
        self.running_initial_create_request_id = initial_create_request_id;
        if let Some(request_key) = request_key {
            self.started_by_request.insert(request_key, run.clone());
        }
        Ok(run)
    }

    fn cancel(&mut self, agent_id: &AgentId) -> Option<AgentRunHandle> {
        let run = self.running.as_ref()?;
        if &run.agent_id == agent_id {
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
        let finished = is_current.then(|| self.running.take()).flatten();
        if finished.is_some() {
            self.running_initial_create_request_id = None;
            self.started_by_request
                .retain(|_, run| run.run_id != run_id);
        }
        finished
    }
}

#[derive(Clone)]
struct AgentRunHandle {
    session_id: app_server_protocol::ChatSessionId,
    agent_id: AgentId,
    run_id: String,
    turn_id: AgentTurnId,
    cancelled: Arc<AtomicBool>,
    started_now: bool,
    started_at: Instant,
}

impl AgentRunHandle {
    fn as_existing(mut self) -> Self {
        self.started_now = false;
        self
    }
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
    agent_runtime: Arc<Mutex<WorkspaceAgentRuntime>>,
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
                        .append_tool_call_with_agent_turn(
                            &self.run.session_id,
                            "agent tool started",
                            tool_call,
                            &self.run.agent_id,
                            &self.run.turn_id,
                            Some(self.run.run_id.clone()),
                        )
                        .await;
                }
                AgentToolHistoryWrite::ToolResult(tool_result, mesh_result) => {
                    let _ = store
                        .append_tool_result_with_agent_turn(
                            &self.run.session_id,
                            "agent tool completed",
                            tool_result,
                            mesh_result,
                            &self.run.agent_id,
                            &self.run.turn_id,
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
            finish_agent_worker(worker, false).await;
            return;
        }
    };
    if worker.run.cancelled.load(Ordering::SeqCst) {
        finish_agent_worker(worker, true).await;
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
        if let Err(error) =
            append_agent_message(&worker.workspace_root, &worker.run, &response_text).await
        {
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::PersistenceError,
                format!("持久化 Agent 最终消息失败: {}", error.message),
            );
        }
    }
    finish_agent_worker(worker, false).await;
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
    AgentModelRuntimeState {
        provider_id,
        provider_type: None,
        model_id,
        reasoning_effort: request.reasoning_effort.clone(),
        service_label: request.service_label.clone(),
    }
}

fn agent_model_state_for_bound_or_request(
    bound_model: Option<&BoundAgentModel>,
    current: &AgentModelRuntimeState,
    request: &AgentInvokeRequest,
) -> AgentModelRuntimeState {
    bound_model
        .map(agent_model_state_for_bound_model)
        .unwrap_or_else(|| agent_model_state_for_request(current, request))
}

fn agent_model_state_for_bound_or_current(
    bound_model: Option<&BoundAgentModel>,
    current: &AgentModelRuntimeState,
) -> AgentModelRuntimeState {
    bound_model
        .map(agent_model_state_for_bound_model)
        .unwrap_or_else(|| current.clone())
}

fn agent_model_state_for_bound_model(bound_model: &BoundAgentModel) -> AgentModelRuntimeState {
    AgentModelRuntimeState {
        provider_id: Some(bound_model.provider_id.clone()),
        provider_type: Some(bound_model.provider_type),
        model_id: Some(bound_model.model_id.clone()),
        reasoning_effort: bound_model.reasoning_effort.clone(),
        service_label: bound_model.service_label.clone(),
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
    let state_matches_active = state.provider_id.as_deref() == Some(active_provider_id.as_str())
        && state.model_id.as_deref() == Some(active_model_id.as_str());
    let active_reasoning_effort = if state_matches_active {
        state.reasoning_effort.clone()
    } else {
        active_model.and_then(|model| model.reasoning_effort.clone())
    };
    let active_service_label = if state_matches_active {
        state.service_label.clone()
    } else {
        active_model.and_then(|model| model.service_label.clone())
    };
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
        Some(app_server_core::llm::AgentProviderKind::Anthropic) => model.is_some_and(|model| {
            anthropic_thinking_budget_tokens(effort, model.max_tokens).is_some()
        }),
        Some(app_server_core::llm::AgentProviderKind::OpenAiCompletions) => false,
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
    ) && service_label.is_some()
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
            .and_then(|config| ensure_bound_provider_type(config, worker.model_state.provider_type))
    }) {
        Ok(config) => config,
        Err(error) if error.message == "Rig Agent is not configured" => {
            let message =
                "Rig Agent is not configured. Set BUDN_AGENT_CONFIG or a provider API key env.";
            append_agent_error_message(
                &worker.workspace_root,
                &worker.run,
                AgentErrorType::LlmError,
                message,
            )
            .await;
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::LlmError,
                message,
            );
            return None;
        }
        Err(error) => {
            let message = error.message;
            append_agent_error_message(
                &worker.workspace_root,
                &worker.run,
                AgentErrorType::LlmError,
                &message,
            )
            .await;
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::LlmError,
                message.clone(),
            );
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
            append_agent_error_message(
                &worker.workspace_root,
                &worker.run,
                AgentErrorType::PermissionDenied,
                &message,
            )
            .await;
            push_agent_error(
                &worker.push_sink,
                &worker.run,
                AgentErrorType::PermissionDenied,
                message.clone(),
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
    let hosted_tool_push_sink = Arc::clone(&worker.push_sink);
    let hosted_tool_run = worker.run.clone();
    let hosted_tool_cancelled = Arc::clone(&worker.run.cancelled);
    let hosted_tool_provider_id = config.provider_id.clone();
    let hosted_tool_provider_kind = agent_provider_type_from_kind(config.provider_kind);
    let hosted_tool_model = config.model.clone();
    let hosted_tool_requested = Arc::new(move |request: &HostedToolRequest| {
        if hosted_tool_cancelled.load(Ordering::SeqCst) {
            return false;
        }
        log::info!(
            "[agent run={}] hosted tool requested provider={} model={} tool_type={} status=requested",
            hosted_tool_run.run_id,
            hosted_tool_provider_id,
            hosted_tool_model,
            request.tool_type,
        );
        push_agent_hosted_tool_requested(
            &hosted_tool_push_sink,
            &hosted_tool_run,
            &hosted_tool_provider_id,
            hosted_tool_provider_kind,
            request,
        );
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
            on_hosted_tool_requested: hosted_tool_requested,
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
            append_agent_error_message(&worker.workspace_root, &worker.run, error_type, &message)
                .await;
            push_agent_error(&worker.push_sink, &worker.run, error_type, message.clone());
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

fn ensure_bound_provider_type(
    config: app_server_core::llm::RigAgentConfig,
    bound_type: Option<AgentProviderType>,
) -> Result<app_server_core::llm::RigAgentConfig, ProtocolError> {
    let Some(bound_type) = bound_type else {
        return Ok(config);
    };
    if provider_kind_matches_bound_type(config.provider_kind, bound_type) {
        return Ok(config);
    }
    Err(ProtocolError::new(
        ProtocolErrorCode::InvalidCommand,
        format!(
            "chat bound model provider type mismatch: expected {}, got {}",
            agent_provider_type_label(bound_type),
            config.provider_kind.as_str()
        ),
    ))
}

fn provider_kind_matches_bound_type(
    kind: app_server_core::llm::AgentProviderKind,
    bound_type: AgentProviderType,
) -> bool {
    matches!(
        (kind, bound_type),
        (
            app_server_core::llm::AgentProviderKind::OpenAiResponses,
            AgentProviderType::OpenAiResponses
        ) | (
            app_server_core::llm::AgentProviderKind::OpenAiCompletions,
            AgentProviderType::OpenAiCompletions
        ) | (
            app_server_core::llm::AgentProviderKind::Anthropic,
            AgentProviderType::Anthropic
        )
    )
}

fn agent_provider_type_label(provider_type: AgentProviderType) -> &'static str {
    match provider_type {
        AgentProviderType::Anthropic => "anthropic",
        AgentProviderType::OpenAiResponses => "openai_responses",
        AgentProviderType::OpenAiCompletions => "openai_completions",
    }
}

fn agent_provider_type_from_kind(
    kind: app_server_core::llm::AgentProviderKind,
) -> AgentProviderType {
    match kind {
        app_server_core::llm::AgentProviderKind::Anthropic => AgentProviderType::Anthropic,
        app_server_core::llm::AgentProviderKind::OpenAiResponses => {
            AgentProviderType::OpenAiResponses
        }
        app_server_core::llm::AgentProviderKind::OpenAiCompletions => {
            AgentProviderType::OpenAiCompletions
        }
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

async fn finish_agent_worker(worker: AgentWorker, cancelled: bool) {
    wait_for_initial_idempotency_window(worker.run.started_at).await;
    if !cancelled && run_failed(&worker) {
        let _ = worker
            .agent_runtime
            .lock()
            .ok()
            .map(|mut runtime| runtime.finish_failed_without_done(&worker.run));
        return;
    }
    let done = worker
        .agent_runtime
        .lock()
        .ok()
        .and_then(|mut runtime| runtime.record_done_and_finish(&worker.run, cancelled));
    if let Some((envelope, sinks)) = done {
        for sink in sinks {
            sink(envelope.clone());
        }
    }
}

fn run_failed(worker: &AgentWorker) -> bool {
    worker
        .agent_runtime
        .lock()
        .ok()
        .is_some_and(|runtime| runtime.run_is_failed(&worker.run))
}

async fn wait_for_initial_idempotency_window(started_at: Instant) {
    let elapsed = started_at.elapsed();
    if elapsed < AGENT_INITIAL_IDEMPOTENCY_WINDOW {
        tokio::time::sleep(AGENT_INITIAL_IDEMPOTENCY_WINDOW - elapsed).await;
    }
}

async fn append_agent_message(
    workspace_root: &Path,
    run: &AgentRunHandle,
    content: &str,
) -> Result<(), ProtocolError> {
    let store = ChatStore::new(workspace_root.to_path_buf());
    store
        .append_message_with_agent_turn(
            &run.session_id,
            ChatRole::Assistant,
            content,
            &run.agent_id,
            &run.turn_id,
            Some(run.run_id.clone()),
        )
        .await?;
    Ok(())
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
            None,
        )
        .await;
}

async fn append_agent_error_message(
    workspace_root: &Path,
    run: &AgentRunHandle,
    error_type: AgentErrorType,
    message: &str,
) {
    let _ = append_agent_message(
        workspace_root,
        run,
        &format!("{AGENT_ERROR_FACT_PREFIX} ({error_type:?}): {message}"),
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

fn push_agent_hosted_tool_requested(
    push_sink: &ServerPushSink,
    run: &AgentRunHandle,
    provider_id: &str,
    provider_kind: AgentProviderType,
    request: &HostedToolRequest,
) {
    (push_sink)(ServerPushEnvelope {
        event: ServerPushEvent::AgentHostedToolActivity(AgentHostedToolActivityEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            provider_id: provider_id.to_owned(),
            provider_kind,
            tool_type: request.tool_type.clone(),
            status: AgentHostedToolActivityStatus::Requested,
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
    agent_model_state: &AgentModelRuntimeState,
) -> Result<ServerCapabilities, ProtocolError> {
    let registry = load_agent_model_registry().await.map_err(|error| {
        log::error!(
            "[agent provider config] required model registry failed during handshake: {}",
            error.message
        );
        error
    })?;
    let agent_model_registry = Some(agent_model_registry_response(&registry, agent_model_state));
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
    Ok(capabilities)
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
        AgentProviderType, CadQueryFeatureFaces, CadQueryPartMesh, ChatSessionId, EdgeGroup,
        FaceGroup, PreviewUnit, VertexPoint,
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
            agent_id: created.agent_id.clone(),
            run_id: "agent-1".into(),
            turn_id: AgentTurnId("agent-1".into()),
            cancelled: Arc::new(AtomicBool::new(false)),
            started_now: true,
            started_at: Instant::now(),
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
            agent_id: created.agent_id.clone(),
            run_id: "agent-1".into(),
            turn_id: AgentTurnId("agent-1".into()),
            cancelled: Arc::new(AtomicBool::new(false)),
            started_now: true,
            started_at: Instant::now(),
        };

        append_agent_capability_meta(&workspace_root, &run, "anthropic", true).await;

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
        assert_eq!(value["provider"], "anthropic");
        assert_eq!(value["native_web_search_enabled"], true);
        assert_eq!(meta.run_id.as_deref(), Some("agent-1"));
    }

    #[test]
    fn hosted_tool_activity_push_maps_to_persisted_payload() {
        let payload = agent_payload_from_push(&ServerPushEvent::AgentHostedToolActivity(
            AgentHostedToolActivityEvent {
                session_id: ChatSessionId("chat-1".into()),
                run_id: "agent-1".into(),
                provider_id: "openai".into(),
                provider_kind: AgentProviderType::OpenAiResponses,
                tool_type: "web_search".into(),
                status: AgentHostedToolActivityStatus::Requested,
            },
        ))
        .expect("hosted tool activity should persist");

        assert_eq!(
            payload,
            AgentEventPayload::HostedToolActivity {
                provider_id: "openai".into(),
                provider_kind: AgentProviderType::OpenAiResponses,
                tool_type: "web_search".into(),
                status: AgentHostedToolActivityStatus::Requested,
            }
        );
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
    fn agent_invoke_model_state_uses_request_param_snapshot() {
        let current = AgentModelRuntimeState {
            provider_id: Some("openai".into()),
            provider_type: None,
            model_id: Some("gpt-5.2".into()),
            reasoning_effort: Some("high".into()),
            service_label: Some("flex".into()),
        };

        let same_model = agent_model_state_for_request(
            &current,
            &agent_invoke_request("openai", "gpt-5.2", None, None),
        );
        assert_eq!(same_model.provider_id.as_deref(), Some("openai"));
        assert_eq!(same_model.model_id.as_deref(), Some("gpt-5.2"));
        assert!(same_model.reasoning_effort.is_none());
        assert!(same_model.service_label.is_none());

        let different_model = agent_model_state_for_request(
            &current,
            &agent_invoke_request("anthropic", "claude-sonnet", None, None),
        );
        assert_eq!(different_model.provider_id.as_deref(), Some("anthropic"));
        assert_eq!(different_model.model_id.as_deref(), Some("claude-sonnet"));
        assert!(different_model.reasoning_effort.is_none());
        assert!(different_model.service_label.is_none());
    }

    #[test]
    fn agent_invoke_model_state_prefers_bound_model_over_request_params() {
        let current = AgentModelRuntimeState {
            provider_id: Some("openai".into()),
            provider_type: None,
            model_id: Some("gpt-5.2".into()),
            reasoning_effort: Some("high".into()),
            service_label: Some("flex".into()),
        };
        let bound_model = BoundAgentModel {
            provider_id: "openai_completions".into(),
            provider_type: AgentProviderType::OpenAiCompletions,
            model_id: "gpt-4o".into(),
            reasoning_effort: Some("low".into()),
            service_label: Some("default".into()),
        };

        let state = agent_model_state_for_bound_or_request(
            Some(&bound_model),
            &current,
            &agent_invoke_request("anthropic", "claude-sonnet", None, None),
        );

        assert_eq!(state.provider_id.as_deref(), Some("openai_completions"));
        assert_eq!(
            state.provider_type,
            Some(AgentProviderType::OpenAiCompletions)
        );
        assert_eq!(state.model_id.as_deref(), Some("gpt-4o"));
        assert_eq!(state.reasoning_effort.as_deref(), Some("low"));
        assert_eq!(state.service_label.as_deref(), Some("default"));
    }

    #[test]
    fn bound_provider_type_mismatch_rejects_config() {
        let config = app_server_core::llm::RigAgentConfig {
            provider_id: "openai".into(),
            provider_kind: app_server_core::llm::AgentProviderKind::OpenAiCompletions,
            api_key: "test".into(),
            model: "gpt-4o".into(),
            timeout_secs: 1,
            max_tokens: 1024,
            temperature: 0.0,
            reasoning_effort: None,
            service_label: None,
            native_web_search: false,
            anthropic_version: None,
            base_url: None,
        };

        let error =
            match ensure_bound_provider_type(config, Some(AgentProviderType::OpenAiResponses)) {
                Ok(_) => panic!("mismatch should reject"),
                Err(error) => error,
            };

        assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
        assert!(error.message.contains("provider type mismatch"));
    }

    #[test]
    fn runtime_subscribe_queues_live_events_until_replay_finishes() {
        let mut runtime = WorkspaceAgentRuntime::default();
        let delivered = Arc::new(Mutex::new(Vec::<ServerPushEnvelope>::new()));
        let sink: ServerPushSink = {
            let delivered = Arc::clone(&delivered);
            Arc::new(move |envelope| delivered.lock().expect("push lock").push(envelope))
        };
        let subscriber_id = runtime.register_subscriber(sink);
        let run = runtime
            .start_run(
                ChatSessionId("chat-1".into()),
                AgentId("agent-1".into()),
                None,
                None,
                None,
                None,
                None,
            )
            .expect("run starts");

        assert!(
            runtime
                .record_push_and_collect_sinks(&run, &agent_token_push(&run, "one"))
                .is_empty()
        );
        let replay = runtime.subscribe_agent(Some(subscriber_id), &run.agent_id, None);
        assert_eq!(agent_token_texts(&replay), vec!["one"]);

        assert!(
            runtime
                .record_push_and_collect_sinks(&run, &agent_token_push(&run, "two"))
                .is_empty()
        );
        assert!(delivered.lock().expect("push lock").is_empty());
        let pending = runtime.drain_or_activate_subscribe(Some(subscriber_id), &run.agent_id);
        assert_eq!(agent_token_texts(&pending), vec!["two"]);

        assert!(
            runtime
                .record_push_and_collect_sinks(&run, &agent_token_push(&run, "three"))
                .is_empty()
        );
        let pending = runtime.drain_or_activate_subscribe(Some(subscriber_id), &run.agent_id);
        assert_eq!(agent_token_texts(&pending), vec!["three"]);

        assert!(
            runtime
                .drain_or_activate_subscribe(Some(subscriber_id), &run.agent_id)
                .is_empty()
        );
        let sinks = runtime.record_push_and_collect_sinks(&run, &agent_token_push(&run, "four"));
        assert_eq!(sinks.len(), 1);
        for sink in sinks {
            sink(ServerPushEnvelope {
                event: agent_token_push(&run, "four"),
            });
        }
        assert_eq!(
            agent_token_texts(&delivered.lock().expect("push lock")),
            vec!["four"]
        );
    }

    #[test]
    fn runtime_drops_terminal_log_without_subscribers() {
        let mut runtime = WorkspaceAgentRuntime::default();
        let run = runtime
            .start_run(
                ChatSessionId("chat-1".into()),
                AgentId("agent-1".into()),
                None,
                None,
                None,
                None,
                None,
            )
            .expect("run starts");

        let done = runtime.record_done_and_finish(&run, false);

        assert!(done.is_some());
        assert!(runtime.registry.running.is_none());
        assert!(!runtime.logs.contains_key(&run.agent_id));
    }

    #[test]
    fn runtime_keeps_terminal_log_for_observing_subscriber() {
        let mut runtime = WorkspaceAgentRuntime::default();
        let delivered = Arc::new(Mutex::new(Vec::<ServerPushEnvelope>::new()));
        let sink: ServerPushSink = {
            let delivered = Arc::clone(&delivered);
            Arc::new(move |envelope| delivered.lock().expect("push lock").push(envelope))
        };
        let subscriber_id = runtime.register_subscriber(sink);
        let run = runtime
            .start_run(
                ChatSessionId("chat-1".into()),
                AgentId("agent-1".into()),
                None,
                None,
                Some(subscriber_id),
                None,
                None,
            )
            .expect("run starts");

        let done = runtime.record_done_and_finish(&run, false);

        assert!(done.is_some());
        assert!(runtime.logs.contains_key(&run.agent_id));
        runtime.unregister_subscriber(subscriber_id);
        assert!(!runtime.logs.contains_key(&run.agent_id));
    }

    #[test]
    fn runtime_pending_persist_blocks_recovery_without_retaining_idle_log() {
        let mut runtime = WorkspaceAgentRuntime::default();
        let run = runtime
            .start_run(
                ChatSessionId("chat-1".into()),
                AgentId("agent-1".into()),
                None,
                None,
                None,
                None,
                None,
            )
            .expect("run starts");
        runtime
            .pending_event_persists
            .lock()
            .expect("pending lock")
            .insert(run.agent_id.clone(), 1);

        let done = runtime.record_done_and_finish(&run, false);

        assert!(done.is_some());
        assert!(runtime.registry.running.is_none());
        assert!(!runtime.logs.contains_key(&run.agent_id));
        assert!(runtime.has_runtime_log_or_pending_persist_for_agent(&run.agent_id));

        runtime.decrement_pending_event_persist(&run.agent_id);

        assert!(!runtime.has_runtime_log_or_pending_persist_for_agent(&run.agent_id));
    }

    #[tokio::test]
    async fn recovery_waits_for_pending_terminal_persist_before_snapshot() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace_root = temp_dir.path().to_path_buf();
        let store = ChatStore::new(workspace_root);
        let created = store
            .create("pending terminal recovery", None, Vec::new())
            .await
            .expect("chat session should be created");
        let turn_id = AgentTurnId("agent-1".into());
        store
            .append_agent_event(
                &created.agent_id,
                &AgentEventRecord {
                    event_id: AgentEventId(1),
                    agent_id: created.agent_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    ts_ms: 100,
                    payload: AgentEventPayload::StateChanged {
                        state: AgentRuntimeStatus::Running,
                    },
                },
            )
            .await
            .expect("seed running event");
        store
            .append_message_with_agent_turn(
                &created.session_id,
                ChatRole::Assistant,
                "final answer",
                &created.agent_id,
                &turn_id,
                Some(turn_id.0.clone()),
            )
            .await
            .expect("seed final assistant fact");
        let runtime = Arc::new(Mutex::new(WorkspaceAgentRuntime::default()));
        runtime
            .lock()
            .expect("runtime lock")
            .pending_event_persists
            .lock()
            .expect("pending lock")
            .insert(created.agent_id.clone(), 1);
        let completion_store = store.clone();
        let completion_runtime = Arc::clone(&runtime);
        let completion_agent_id = created.agent_id.clone();
        let completion_turn_id = turn_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            completion_store
                .append_agent_event(
                    &completion_agent_id,
                    &AgentEventRecord {
                        event_id: AgentEventId(2),
                        agent_id: completion_agent_id.clone(),
                        turn_id: Some(completion_turn_id),
                        ts_ms: 101,
                        payload: AgentEventPayload::Done { cancelled: false },
                    },
                )
                .await
                .expect("append pending terminal event");
            completion_runtime
                .lock()
                .expect("runtime lock")
                .decrement_pending_event_persist(&completion_agent_id);
        });

        let recovered = recover_agent_persisted_events(
            &store,
            &runtime,
            &created.session_id,
            &created.agent_id,
        )
        .await
        .expect("recover waits for pending terminal");
        let snapshot = runtime.lock().expect("runtime lock").snapshot(
            &created.agent_id,
            created.session_id,
            None,
            recovered,
            None,
        );

        assert_eq!(snapshot.state, AgentRuntimeStatus::Done);
    }

    #[tokio::test]
    async fn recovery_returns_error_when_pending_persist_does_not_complete() {
        let temp_dir = tempfile::tempdir().expect("temp workspace");
        let workspace_root = temp_dir.path().to_path_buf();
        let store = ChatStore::new(workspace_root);
        let created = store
            .create("stuck pending recovery", None, Vec::new())
            .await
            .expect("chat session should be created");
        let turn_id = AgentTurnId("agent-1".into());
        store
            .append_agent_event(
                &created.agent_id,
                &AgentEventRecord {
                    event_id: AgentEventId(1),
                    agent_id: created.agent_id.clone(),
                    turn_id: Some(turn_id),
                    ts_ms: 100,
                    payload: AgentEventPayload::StateChanged {
                        state: AgentRuntimeStatus::Running,
                    },
                },
            )
            .await
            .expect("seed running event");
        let runtime = Arc::new(Mutex::new(WorkspaceAgentRuntime::default()));
        runtime
            .lock()
            .expect("runtime lock")
            .pending_event_persists
            .lock()
            .expect("pending lock")
            .insert(created.agent_id.clone(), 1);

        let error = recover_agent_persisted_events(
            &store,
            &runtime,
            &created.session_id,
            &created.agent_id,
        )
        .await
        .expect_err("stuck pending persist should block recovery");
        let records = store
            .read_agent_events(&created.agent_id, None)
            .await
            .expect("read event log");

        assert_eq!(error.code, ProtocolErrorCode::Internal);
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn runtime_snapshot_rebuilds_idle_state_from_persisted_events() {
        let runtime = WorkspaceAgentRuntime::default();
        let agent_id = AgentId("agent-1".into());
        let turn_id = AgentTurnId("turn-1".into());
        let snapshot = runtime.snapshot(
            &agent_id,
            ChatSessionId("chat-1".into()),
            None,
            vec![
                AgentEventRecord {
                    event_id: AgentEventId(1),
                    agent_id: agent_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    ts_ms: 100,
                    payload: AgentEventPayload::StateChanged {
                        state: AgentRuntimeStatus::Running,
                    },
                },
                AgentEventRecord {
                    event_id: AgentEventId(2),
                    agent_id: agent_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    ts_ms: 101,
                    payload: AgentEventPayload::Token {
                        text: "hello".into(),
                    },
                },
                AgentEventRecord {
                    event_id: AgentEventId(3),
                    agent_id: agent_id.clone(),
                    turn_id: Some(turn_id),
                    ts_ms: 102,
                    payload: AgentEventPayload::Done { cancelled: false },
                },
            ],
            Some(AgentEventId(1)),
        );

        assert_eq!(snapshot.agent_id, agent_id);
        assert_eq!(snapshot.state, AgentRuntimeStatus::Done);
        assert_eq!(snapshot.current_text, "hello");
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.events[0].event_id, AgentEventId(2));
    }

    #[test]
    fn runtime_snapshot_marks_persisted_running_event_as_interrupted_without_worker() {
        let runtime = WorkspaceAgentRuntime::default();
        let agent_id = AgentId("agent-1".into());
        let turn_id = AgentTurnId("turn-1".into());
        let snapshot = runtime.snapshot(
            &agent_id,
            ChatSessionId("chat-1".into()),
            None,
            vec![AgentEventRecord {
                event_id: AgentEventId(1),
                agent_id: agent_id.clone(),
                turn_id: Some(turn_id),
                ts_ms: 100,
                payload: AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            }],
            None,
        );

        assert_eq!(snapshot.agent_id, agent_id);
        assert_eq!(snapshot.state, AgentRuntimeStatus::Interrupted);
        assert_eq!(snapshot.active_turn_id, None);
    }

    #[tokio::test]
    async fn initial_turn_reservation_duplicate_waiter_cannot_miss_release() {
        let mut registry = AgentRunRegistry::default();
        match registry
            .reserve_initial_turn("request-1")
            .expect("reservation succeeds")
        {
            InitialTurnReserveOutcome::Reserved => {}
            _ => panic!("first reservation should reserve"),
        }
        let waiter = match registry
            .reserve_initial_turn("request-1")
            .expect("duplicate reservation should wait")
        {
            InitialTurnReserveOutcome::DuplicateInProgress(waiter) => waiter,
            _ => panic!("duplicate reservation should return waiter"),
        };

        registry.release_initial_turn_reservation("request-1");

        tokio::time::timeout(std::time::Duration::from_millis(20), waiter)
            .await
            .expect("waiter should observe release");
    }

    #[test]
    fn initial_turn_reservation_same_request_running_is_committed_duplicate() {
        let mut registry = AgentRunRegistry::default();
        match registry
            .reserve_initial_turn("request-1")
            .expect("reservation succeeds")
        {
            InitialTurnReserveOutcome::Reserved => {}
            _ => panic!("first reservation should reserve"),
        }
        let run = registry
            .try_start_reserved_initial_turn(
                ChatSessionId("chat-1".into()),
                AgentId("agent-1".into()),
                Some("request-1".into()),
            )
            .expect("reserved turn starts");
        assert!(run.started_now);

        match registry
            .reserve_initial_turn("request-1")
            .expect("same request should deduplicate")
        {
            InitialTurnReserveOutcome::DuplicateCommitted => {}
            _ => panic!("same request while running should re-read committed chat"),
        }
        let error = match registry.reserve_initial_turn("request-2") {
            Ok(_) => panic!("different request should stay busy"),
            Err(error) => error,
        };
        assert_eq!(error.code, ProtocolErrorCode::AgentBusy);
    }

    fn agent_invoke_request(
        provider_id: &str,
        model_id: &str,
        reasoning_effort: Option<&str>,
        service_label: Option<&str>,
    ) -> AgentInvokeRequest {
        AgentInvokeRequest {
            session_id: ChatSessionId("main".into()),
            client_request_id: None,
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

    fn agent_token_push(run: &AgentRunHandle, text: &str) -> ServerPushEvent {
        ServerPushEvent::AgentToken(AgentTokenEvent {
            session_id: run.session_id.clone(),
            run_id: run.run_id.clone(),
            text: text.into(),
        })
    }

    fn agent_token_texts(envelopes: &[ServerPushEnvelope]) -> Vec<&str> {
        envelopes
            .iter()
            .filter_map(|envelope| match &envelope.event {
                ServerPushEvent::AgentToken(event) => Some(event.text.as_str()),
                _ => None,
            })
            .collect()
    }
}
