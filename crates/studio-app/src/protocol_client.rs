use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_server_host::{ClientTransport, InProcessHost, spawn_in_process_mpsc_host};
use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, CommandSuccess, ConfigLoadResponse, ConfigSaveRequest, DisplayUnitDto,
    ExportRunRequest, ExportRunResponse, FileReadContents, FileReadRequest, FileWriteTextRequest,
    HostLocalPath, PathHandle, PreviewArtifact, PreviewRequest, PreviewRequestKind, ProtocolError,
    ProtocolVersionRange, RequestId, SlicerConfigDto, SlicerInstallRecord, SlicerListRequest,
    SubscriptionId, WatchChangedEvent, WatchErrorEvent, WatchSubscribeRequest,
    WatchSubscriptionAck, WatchUnsubscribeRequest,
};
use app_server_transport::ServerEnvelope;
use scad_scene::{MeshData, mesh::load_stl_from_reader, three_mf::load_3mf_from_reader};
use scad_ui::file_tree::{FileTreeEntry, FileTreeEntryKind};
use scad_viewer::app::SlicerInstall;
use studio_common::{AppConfig, DisplayUnit, ExportFormat, PresetFile, SlicerConfig};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone)]
pub struct DesktopProtocolClient {
    inner: Arc<DesktopProtocolClientInner>,
}

struct DesktopProtocolClientInner {
    transport: Arc<Mutex<app_server_host::MpscTransportAdapter>>,
    host: Mutex<InProcessHost>,
    next_request_id: AtomicU64,
    pending: Mutex<HashMap<RequestId, SyncSender<Result<CommandSuccess, ProtocolError>>>>,
    handshake_waiter: Mutex<Option<SyncSender<Result<(), String>>>>,
    watch_handlers: Mutex<HashMap<String, WatchHandler>>,
    workspace_root: Mutex<Option<PathBuf>>,
    workspace_id: Mutex<Option<app_server_protocol::WorkspaceId>>,
    closed: AtomicBool,
}

struct WatchHandler {
    on_changed: Box<dyn Fn(PathBuf) + Send + 'static>,
    on_error: Box<dyn Fn(String) + Send + 'static>,
}

pub struct WatchSubscriptionHandle {
    client: DesktopProtocolClient,
    subscription_id: Option<SubscriptionId>,
}

#[derive(Debug, Clone)]
pub struct PreviewSuccess {
    pub source_path: PathBuf,
    pub mesh: MeshData,
}

impl DesktopProtocolClient {
    pub fn connect() -> Result<Self, String> {
        let (host, transport) = spawn_in_process_mpsc_host()?;
        let client = Self {
            inner: Arc::new(DesktopProtocolClientInner {
                transport: Arc::new(Mutex::new(transport)),
                host: Mutex::new(host),
                next_request_id: AtomicU64::new(1),
                pending: Mutex::new(HashMap::new()),
                handshake_waiter: Mutex::new(None),
                watch_handlers: Mutex::new(HashMap::new()),
                workspace_root: Mutex::new(None),
                workspace_id: Mutex::new(None),
                closed: AtomicBool::new(false),
            }),
        };
        client.start_reader_thread();
        client.handshake()?;
        Ok(client)
    }

    pub fn load_config(&self) -> Result<AppConfig, String> {
        let response = self.send_command(ClientCommand::ConfigLoad)?;
        let CommandSuccess::ConfigLoaded(ConfigLoadResponse { config }) = response else {
            return Err("config.load 返回了意外响应".into());
        };
        app_config_from_dto(config)
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<(), String> {
        let response = self.send_command(ClientCommand::ConfigSave(ConfigSaveRequest {
            config: app_config_to_dto(config)?,
        }))?;
        match response {
            CommandSuccess::ConfigSaved => Ok(()),
            other => Err(format!("config.save 返回了意外响应: {other:?}")),
        }
    }

    pub fn rebind_workspace(&self, path: PathBuf) -> Result<(), String> {
        {
            let mut host = self
                .inner
                .host
                .lock()
                .map_err(|_| "desktop host 锁已损坏".to_string())?;
            host.rebind_workspace(path.clone());
        }
        let response = self.send_command(ClientCommand::WorkspaceCurrent)?;
        let CommandSuccess::WorkspaceCurrent(current) = response else {
            return Err("workspace.current 返回了意外响应".into());
        };
        *self
            .inner
            .workspace_root
            .lock()
            .map_err(|_| "workspace root 锁已损坏".to_string())? = Some(path);
        *self
            .inner
            .workspace_id
            .lock()
            .map_err(|_| "workspace id 锁已损坏".to_string())? = Some(current.workspace_id);
        Ok(())
    }

    pub fn workspace_root(&self) -> Option<PathBuf> {
        self.inner.workspace_root.lock().ok()?.clone()
    }

    pub fn list_directory(&self, path: &Path) -> Result<Vec<FileTreeEntry>, String> {
        let handle = self.path_handle(path)?;
        let response = self.send_command(ClientCommand::WorkspaceList(
            app_server_protocol::WorkspaceListRequest {
                directory: Some(handle),
            },
        ))?;
        let CommandSuccess::WorkspaceList(list) = response else {
            return Err("workspace.list 返回了意外响应".into());
        };
        Ok(list
            .entries
            .into_iter()
            .map(|entry| FileTreeEntry {
                name: entry.name.clone(),
                path: entry
                    .path
                    .as_ref()
                    .map(|path| self.path_from_handle(path))
                    .unwrap_or_else(|| {
                        let mut path = self.workspace_root().unwrap_or_default();
                        path.push(&entry.name);
                        path
                    }),
                kind: match entry.kind {
                    app_server_protocol::WorkspaceEntryKind::Directory => {
                        FileTreeEntryKind::Directory
                    }
                    app_server_protocol::WorkspaceEntryKind::File => FileTreeEntryKind::File,
                },
            })
            .collect())
    }

    pub fn read_text_file(&self, path: &Path, context: &str) -> Result<String, String> {
        let response = self.send_command(ClientCommand::FileRead(FileReadRequest {
            path: self.path_handle(path)?,
        }))?;
        let CommandSuccess::FileRead(file) = response else {
            return Err(format!("读取{context}时收到了意外响应"));
        };
        match file.contents {
            FileReadContents::Utf8Text(text) => Ok(text),
            FileReadContents::Binary(_) => Err(format!("读取{context}失败: 结果不是 UTF-8 文本")),
        }
    }

    pub fn read_binary_file(&self, path: &Path, context: &str) -> Result<Vec<u8>, String> {
        let response = self.send_command(ClientCommand::FileRead(FileReadRequest {
            path: self.path_handle(path)?,
        }))?;
        let CommandSuccess::FileRead(file) = response else {
            return Err(format!("读取{context}时收到了意外响应"));
        };
        match file.contents {
            FileReadContents::Binary(bytes) => Ok(bytes),
            FileReadContents::Utf8Text(text) => Ok(text.into_bytes()),
        }
    }

    pub fn read_presets(&self, path: &Path) -> Result<PresetFile, String> {
        match self.send_command(ClientCommand::FileRead(FileReadRequest {
            path: self.path_handle(path)?,
        })) {
            Ok(CommandSuccess::FileRead(file)) => match file.contents {
                FileReadContents::Utf8Text(text) => serde_json::from_str(&text)
                    .map_err(|error| format!("解析预设文件失败: {error}")),
                FileReadContents::Binary(_) => Err("读取预设文件失败: 结果不是 UTF-8 文本".into()),
            },
            Err(message) if message.contains("NotFound") => Ok(PresetFile::default()),
            Ok(other) => Err(format!("读取预设文件时收到了意外响应: {other:?}")),
            Err(error) => Err(error),
        }
    }

    pub fn write_text_file(&self, path: &Path, contents: String) -> Result<(), String> {
        let response = self.send_command(ClientCommand::FileWriteText(FileWriteTextRequest {
            path: self.path_handle(path)?,
            contents,
        }))?;
        match response {
            CommandSuccess::FileWritten(_) => Ok(()),
            other => Err(format!("file.write_text 返回了意外响应: {other:?}")),
        }
    }

    pub fn list_slicers(&self, configured: &[SlicerConfig]) -> Result<Vec<SlicerInstall>, String> {
        let configured = configured
            .iter()
            .map(|item| {
                Ok(SlicerInstallRecord {
                    name: item.name.clone(),
                    path: path_to_host_local(&item.path)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let response =
            self.send_command(ClientCommand::SlicerList(SlicerListRequest { configured }))?;
        let CommandSuccess::SlicerListed(list) = response else {
            return Err("slicer.list 返回了意外响应".into());
        };
        Ok(list
            .slicers
            .into_iter()
            .map(|item| SlicerInstall {
                name: item.name,
                path: item.path.to_path_buf(),
            })
            .collect())
    }

    pub fn export_model(
        &self,
        config: &AppConfig,
        source_path: &Path,
        defines: &[String],
        output_path: PathBuf,
        format: ExportFormat,
        slicer_name: Option<String>,
    ) -> Result<PathBuf, String> {
        let configured_slicers = config
            .slicers
            .iter()
            .map(|item| {
                Ok(SlicerInstallRecord {
                    name: item.name.clone(),
                    path: path_to_host_local(&item.path)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let response = self.send_command(ClientCommand::ExportRun(ExportRunRequest {
            configured_openscad_path: optional_path_to_host_local(config.openscad_path.as_ref())?,
            configured_slicers,
            source: self.path_handle(source_path)?,
            defines: defines.to_vec(),
            output_path: self.path_handle(&output_path)?,
            format,
            slicer_name,
        }))?;
        let CommandSuccess::ExportRun(ExportRunResponse { output_path }) = response else {
            return Err("export.run 返回了意外响应".into());
        };
        Ok(self.path_from_handle(&output_path))
    }

    pub fn subscribe_path<F, E>(
        &self,
        path: &Path,
        on_changed: F,
        on_error: E,
    ) -> Result<WatchSubscriptionHandle, String>
    where
        F: Fn(PathBuf) + Send + 'static,
        E: Fn(String) + Send + 'static,
    {
        let response = self.send_command(ClientCommand::WatchSubscribe(WatchSubscribeRequest {
            directory: Some(self.path_handle(path)?),
        }))?;
        let CommandSuccess::WatchSubscribed(WatchSubscriptionAck { subscription_id }) = response
        else {
            return Err("watch.subscribe 返回了意外响应".into());
        };
        self.inner
            .watch_handlers
            .lock()
            .map_err(|_| "watch handler 锁已损坏".to_string())?
            .insert(
                subscription_id.0.clone(),
                WatchHandler {
                    on_changed: Box::new(on_changed),
                    on_error: Box::new(on_error),
                },
            );
        Ok(WatchSubscriptionHandle {
            client: self.clone(),
            subscription_id: Some(subscription_id),
        })
    }

    pub fn request_preview_async<F>(
        &self,
        path: PathBuf,
        configured_openscad_path: Option<PathBuf>,
        defines: Vec<String>,
        on_result: F,
    ) where
        F: FnOnce(Result<PreviewSuccess, String>) + Send + 'static,
    {
        let client = self.clone();
        std::thread::spawn(move || {
            let result = client.request_preview(&path, configured_openscad_path, &defines);
            on_result(result);
        });
    }

    pub fn run_smoke_check(workspace: PathBuf) -> Result<(), String> {
        let client = Self::connect()?;
        client.rebind_workspace(workspace.clone())?;
        let _ = client.list_directory(&workspace)?;
        Ok(())
    }

    fn request_preview(
        &self,
        path: &Path,
        configured_openscad_path: Option<PathBuf>,
        defines: &[String],
    ) -> Result<PreviewSuccess, String> {
        let response = self.send_command(ClientCommand::PreviewRequest(PreviewRequest {
            source: self.path_handle(path)?,
            defines: defines.to_vec(),
            kind: PreviewRequestKind::GeometryArtifact,
            configured_openscad_path: optional_path_to_host_local(
                configured_openscad_path.as_ref(),
            )?,
        }))?;
        let CommandSuccess::PreviewReady(ready) = response else {
            return Err("preview.request 返回了意外响应".into());
        };
        match ready.artifact {
            PreviewArtifact::Mesh(mesh) => Ok(PreviewSuccess {
                source_path: path.to_path_buf(),
                mesh: mesh_from_preview_payload(mesh),
            }),
            PreviewArtifact::ThreeMf(artifact) => Ok(PreviewSuccess {
                source_path: path.to_path_buf(),
                mesh: mesh_from_3mf_bytes(&artifact.bytes)?,
            }),
            PreviewArtifact::Stl(artifact) => Ok(PreviewSuccess {
                source_path: path.to_path_buf(),
                mesh: mesh_from_stl_bytes(&artifact.bytes)?,
            }),
            other => Err(format!("桌面端暂不支持的预览结果: {other:?}")),
        }
    }

    fn handshake(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        *self
            .inner
            .handshake_waiter
            .lock()
            .map_err(|_| "handshake waiter 锁已损坏".to_string())? = Some(tx);
        self.inner
            .transport
            .lock()
            .map_err(|_| "transport 锁已损坏".to_string())?
            .handshake(CapabilityHandshakeRequest {
                capabilities: ClientCapabilities {
                    client_name: "studio-app".into(),
                    platform: ClientPlatform::Desktop,
                    protocol_version: ProtocolVersionRange::new(2, 2),
                    file_read: app_server_protocol::FileReadCapability {
                        denied_extensions: Vec::new(),
                    },
                    supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
                },
            })
            .map_err(|error| error.to_string())?;
        rx.recv_timeout(REQUEST_TIMEOUT)
            .map_err(|_| "等待 handshake 响应超时".to_string())??;
        Ok(())
    }

    fn send_command(&self, command: ClientCommand) -> Result<CommandSuccess, String> {
        let request_id = RequestId(self.inner.next_request_id.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = mpsc::sync_channel(1);
        self.inner
            .pending
            .lock()
            .map_err(|_| "pending request 锁已损坏".to_string())?
            .insert(request_id, tx);
        self.inner
            .transport
            .lock()
            .map_err(|_| "transport 锁已损坏".to_string())?
            .request(ClientRequestEnvelope {
                request_id,
                command,
            })
            .map_err(|error| error.to_string())?;
        match rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(Ok(success)) => Ok(success),
            Ok(Err(error)) => Err(format!("{:?}: {}", error.code, error.message)),
            Err(_) => Err("等待协议响应超时".into()),
        }
    }

    fn start_reader_thread(&self) {
        let inner = self.inner.clone();
        std::thread::Builder::new()
            .name("studio-protocol-reader".into())
            .spawn(move || {
                while !inner.closed.load(Ordering::SeqCst) {
                    let message = match inner.transport.lock() {
                        Ok(mut transport) => transport.next_server_message(),
                        Err(_) => break,
                    };
                    match message {
                        Ok(Some(ServerEnvelope::HandshakeAck(_))) => {
                            if let Ok(mut waiter) = inner.handshake_waiter.lock()
                                && let Some(sender) = waiter.take()
                            {
                                let _ = sender.send(Ok(()));
                            }
                        }
                        Ok(Some(ServerEnvelope::Response(response))) => {
                            if let Ok(mut pending) = inner.pending.lock()
                                && let Some(sender) = pending.remove(&response.request_id)
                            {
                                let _ = sender.send(response.result);
                            }
                        }
                        Ok(Some(ServerEnvelope::Push(push))) => match push.event {
                            app_server_protocol::ServerPushEvent::WatchChanged(event) => {
                                dispatch_watch_changed(&inner, event)
                            }
                            app_server_protocol::ServerPushEvent::WatchError(event) => {
                                dispatch_watch_error(&inner, event)
                            }
                            app_server_protocol::ServerPushEvent::AgentToken(_)
                            | app_server_protocol::ServerPushEvent::AgentToolStart(_)
                            | app_server_protocol::ServerPushEvent::AgentToolResult(_)
                            | app_server_protocol::ServerPushEvent::AgentMeshReady(_)
                            | app_server_protocol::ServerPushEvent::AgentError(_)
                            | app_server_protocol::ServerPushEvent::AgentDone(_) => {}
                        },
                        Ok(Some(ServerEnvelope::TransportError(error))) => {
                            if let Ok(mut waiter) = inner.handshake_waiter.lock()
                                && let Some(sender) = waiter.take()
                            {
                                let _ = sender.send(Err(error.message.clone()));
                            }
                        }
                        Ok(Some(ServerEnvelope::Closed)) => break,
                        Ok(None) => std::thread::sleep(POLL_INTERVAL),
                        Err(_) => std::thread::sleep(POLL_INTERVAL),
                    }
                }
            })
            .expect("protocol reader thread should start");
    }

    fn path_handle(&self, path: &Path) -> Result<PathHandle, String> {
        let workspace_root = self
            .inner
            .workspace_root
            .lock()
            .map_err(|_| "workspace root 锁已损坏".to_string())?
            .clone()
            .ok_or_else(|| "当前还没有绑定 workspace".to_string())?;
        let workspace_id = self
            .inner
            .workspace_id
            .lock()
            .map_err(|_| "workspace id 锁已损坏".to_string())?
            .clone()
            .ok_or_else(|| "当前还没有拿到 workspace id".to_string())?;
        let relative = path.strip_prefix(&workspace_root).map_err(|_| {
            format!(
                "路径 {} 不在当前 workspace {} 内",
                path.display(),
                workspace_root.display()
            )
        })?;
        let segments = relative
            .components()
            .filter_map(|component| component.as_os_str().to_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();
        PathHandle::new(workspace_id, segments).map_err(|error| error.to_string())
    }

    fn path_from_handle(&self, handle: &PathHandle) -> PathBuf {
        let workspace_root = self.workspace_root().unwrap_or_default();
        let mut path = workspace_root;
        for segment in handle.path_segments() {
            path.push(segment);
        }
        path
    }

    fn unsubscribe(&self, subscription_id: SubscriptionId) {
        let _ = self.send_command(ClientCommand::WatchUnsubscribe(WatchUnsubscribeRequest {
            subscription_id: subscription_id.clone(),
        }));
        if let Ok(mut handlers) = self.inner.watch_handlers.lock() {
            handlers.remove(&subscription_id.0);
        }
    }
}

impl Drop for WatchSubscriptionHandle {
    fn drop(&mut self) {
        if let Some(subscription_id) = self.subscription_id.take() {
            self.client.unsubscribe(subscription_id);
        }
    }
}

fn dispatch_watch_changed(inner: &DesktopProtocolClientInner, event: WatchChangedEvent) {
    let Some(path) = event.changed_paths.first() else {
        return;
    };
    if let Ok(handlers) = inner.watch_handlers.lock()
        && let Some(handler) = handlers.get(&event.subscription_id.0)
    {
        (handler.on_changed)(path_to_pathbuf(inner, path));
    }
}

fn dispatch_watch_error(inner: &DesktopProtocolClientInner, event: WatchErrorEvent) {
    if let Ok(handlers) = inner.watch_handlers.lock()
        && let Some(handler) = handlers.get(&event.subscription_id.0)
    {
        (handler.on_error)(event.message);
    }
}

fn path_to_pathbuf(inner: &DesktopProtocolClientInner, handle: &PathHandle) -> PathBuf {
    let mut path = inner
        .workspace_root
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_default();
    for segment in handle.path_segments() {
        path.push(segment);
    }
    path
}

fn app_config_to_dto(config: &AppConfig) -> Result<app_server_protocol::AppConfigDto, String> {
    Ok(app_server_protocol::AppConfigDto {
        openscad_path: optional_path_to_host_local(config.openscad_path.as_ref())?,
        slicers: config
            .slicers
            .iter()
            .map(|item| {
                Ok(SlicerConfigDto {
                    name: item.name.clone(),
                    path: path_to_host_local(&item.path)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        recent_workspaces: config
            .recent_workspaces
            .iter()
            .map(path_to_host_local)
            .collect::<Result<Vec<_>, String>>()?,
        floating_panel_opacity: config.floating_panel_opacity,
        left_panel_width: config.left_panel_width,
        right_panel_width: config.right_panel_width,
        display_unit: display_unit_to_dto(config.display_unit),
        camera_overlay_pos: config.camera_overlay_pos,
        camera_overlay_size: config.camera_overlay_size,
        param_panel_pos: config.param_panel_pos,
        param_panel_size: config.param_panel_size,
        log_panel_pos: config.log_panel_pos,
        log_panel_size: config.log_panel_size,
    })
}

fn app_config_from_dto(config: app_server_protocol::AppConfigDto) -> Result<AppConfig, String> {
    Ok(AppConfig {
        openscad_path: config.openscad_path.map(|path| path.to_path_buf()),
        slicers: config
            .slicers
            .into_iter()
            .map(|item| SlicerConfig {
                name: item.name,
                path: item.path.to_path_buf(),
            })
            .collect(),
        recent_workspaces: config
            .recent_workspaces
            .into_iter()
            .map(|path| path.to_path_buf())
            .collect(),
        floating_panel_opacity: config.floating_panel_opacity,
        left_panel_width: config.left_panel_width,
        right_panel_width: config.right_panel_width,
        display_unit: display_unit_from_dto(config.display_unit),
        camera_overlay_pos: config.camera_overlay_pos,
        camera_overlay_size: config.camera_overlay_size,
        param_panel_pos: config.param_panel_pos,
        param_panel_size: config.param_panel_size,
        log_panel_pos: config.log_panel_pos,
        log_panel_size: config.log_panel_size,
    })
}

fn optional_path_to_host_local(path: Option<&PathBuf>) -> Result<Option<HostLocalPath>, String> {
    path.map(path_to_host_local).transpose()
}

fn path_to_host_local(path: &PathBuf) -> Result<HostLocalPath, String> {
    let value = path
        .to_str()
        .ok_or_else(|| "host-local path 必须是 UTF-8".to_string())?;
    HostLocalPath::new(value).map_err(|error| error.message)
}

fn display_unit_to_dto(unit: DisplayUnit) -> DisplayUnitDto {
    match unit {
        DisplayUnit::Millimeter => DisplayUnitDto::Millimeter,
        DisplayUnit::Centimeter => DisplayUnitDto::Centimeter,
        DisplayUnit::Inch => DisplayUnitDto::Inch,
    }
}

fn display_unit_from_dto(unit: DisplayUnitDto) -> DisplayUnit {
    match unit {
        DisplayUnitDto::Millimeter => DisplayUnit::Millimeter,
        DisplayUnitDto::Centimeter => DisplayUnit::Centimeter,
        DisplayUnitDto::Inch => DisplayUnit::Inch,
    }
}

fn mesh_from_preview_payload(payload: app_server_protocol::PreviewMeshPayload) -> MeshData {
    let app_server_protocol::PreviewMeshPayload {
        positions,
        normals,
        vertex_colors,
        indices,
        ..
    } = payload;
    MeshData::from_indexed_buffers(positions, normals, vertex_colors, indices)
        .expect("preview mesh payload should convert into renderable mesh")
}

fn mesh_from_3mf_bytes(bytes: &[u8]) -> Result<MeshData, String> {
    let mut cursor = Cursor::new(bytes);
    load_3mf_from_reader(&mut cursor).map_err(|error| format!("解析 3MF 预览失败: {error}"))
}

fn mesh_from_stl_bytes(bytes: &[u8]) -> Result<MeshData, String> {
    let mut cursor = Cursor::new(bytes);
    load_stl_from_reader(&mut cursor).map_err(|error| format!("解析 STL 预览失败: {error}"))
}
