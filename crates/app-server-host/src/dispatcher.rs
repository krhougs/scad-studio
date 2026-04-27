use app_server_core::{
    CadQueryExecuteConfig, CadQueryRunConfig, FileWatcher, SlicerInstall, current_workspace,
    detect_slicer_paths, execute_cadquery_with_staging, export_model, list_workspace_entries,
    load_config_dto, preview_ready_response, read_file_response, resolve_workspace_path,
    resolve_workspace_write_path, run_cadquery_runner, save_config_dto, send_to_slicer,
    stage_cadquery_project,
};
use app_server_protocol::{
    CadQueryMeshPayload, CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCommand,
    ClientRequestEnvelope, CommandSuccess, ConfigLoadResponse, DEFAULT_SESSION_RECONNECT_WINDOW_MS,
    ExportRunResponse, FileWriteTextResponse, HostLocalPath, PathHandle, PreviewRequestKind,
    ProtocolError, ProtocolErrorCode, ProtocolVersionRange, ServerCapabilities, ServerPushEnvelope,
    ServerPushEvent, ServerResponseEnvelope, SessionReclaimedResponse, SessionToken,
    SubscriptionId, WatchChangedEvent, WatchErrorEvent, WatchSubscriptionAck, WorkspaceId,
    WorkspaceListResponse,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::HostSession;

pub type ServerPushSink = Arc<dyn Fn(ServerPushEnvelope) + Send + Sync>;

pub struct HostRequestDispatcher {
    workspace_id: WorkspaceId,
    workspace_path: Option<PathBuf>,
    denied_extensions: Vec<String>,
    next_subscription_id: u64,
    watchers: HashMap<String, FileWatcher>,
    cadquery_results: HashMap<String, CadQueryMeshPayload>,
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
            cadquery_results: HashMap::new(),
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
                staged.commit_outputs().map_err(internal_error)?;
                self.cadquery_results
                    .insert(result.ready.result_id.clone(), result.mesh);
                Ok(CommandSuccess::CadQueryResultReady(result.ready))
            }
            ClientCommand::CadQueryExecute(request) => {
                let workspace_path = self.workspace_root()?.to_path_buf();
                let _target_path =
                    resolve_workspace_write_path(&workspace_path, &request.target_path)?;
                let target = path_handle_to_relative_path(&request.target_path);
                self.session.issue_handle(request.target_path);
                let result = execute_cadquery_with_staging(&CadQueryExecuteConfig {
                    python: cadquery_python_path(),
                    workspace_root: workspace_path,
                    target_relative_path: target,
                    code: request.code,
                    export_formats: request.export_formats,
                    params_json: request.params_json,
                    timeout: Duration::from_secs(60),
                })
                .map_err(internal_error)?;
                self.cadquery_results
                    .insert(result.ready.result_id.clone(), result.mesh);
                Ok(CommandSuccess::CadQueryResultReady(result.ready))
            }
            ClientCommand::CadQueryResultGet(request) => {
                let payload = self
                    .cadquery_results
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
        agent: false,
        selection_sync: false,
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
