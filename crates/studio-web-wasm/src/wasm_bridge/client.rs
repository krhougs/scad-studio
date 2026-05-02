//! `ClientHandle` wasm_bindgen 包装与所有 `client_*` 导出。
//!
//! 所有业务状态机归 `studio_common::ManagedClient<NullTransport>`；本文件
//! 仅负责参数反序列化、错误转换和结果序列化。重载荷预览 artifact 使用
//! request_id side buffer 暂存，避免 `serde_wasm_bindgen` 把字节数组展开为 JS
//! number 数组。

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;

use app_server_protocol::{
    AgentCancelRequest, AgentInvokeRequest, AgentModelParamsUpdateRequest, AgentModelSelectRequest,
    AgentPlanConfirmRequest, AgentPlanRejectRequest, AgentStartTurnRequest, CadQueryExecuteRequest,
    CadQueryMeshPayload, CadQueryPreviewRequest, CadQueryResultGetRequest, CadQueryResultReady,
    CapabilityHandshakeRequest, ChatArchiveRequest, ChatCreateRequest, ChatHistoryRequest,
    ChatListRequest, ChatSendRequest, ChatSessionId, CommandSuccess, ConfigSaveRequest,
    ExportRunRequest, FileReadRequest, FileWriteTextRequest, PathHandle, PreviewArtifact,
    PreviewRequest, RequestId, SelectionUpdateRequest, SlicerListRequest, WorkspaceListRequest,
};
use scad_scene::{MeshData, mesh::load_stl_from_reader, three_mf::load_3mf_from_reader};
use studio_common::{
    ClientError, ClientTimeouts, ManagedClient, TransportCloseReason, WatchParams,
};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use super::{cadquery_mesh::CadQueryMeshHandle, mesh::MeshHandle, transport::NullTransport};

const PREVIEW_SIDE_BUFFER_CAPACITY: usize = 8;
const CADQUERY_SIDE_BUFFER_CAPACITY: usize = 8;

#[wasm_bindgen]
pub struct ClientHandle {
    inner: Option<ManagedClient<NullTransport>>,
    preview_artifacts: PreviewSideBuffer,
    preview_targets: HashMap<RequestId, PathHandle>,
    cadquery_meshes: CadQuerySideBuffer,
}

impl ClientHandle {
    fn borrow_mut(&mut self) -> Result<&mut ManagedClient<NullTransport>, JsValue> {
        self.inner.as_mut().ok_or_else(invalid_handle_js)
    }

    fn borrow(&self) -> Result<&ManagedClient<NullTransport>, JsValue> {
        self.inner.as_ref().ok_or_else(invalid_handle_js)
    }
}

#[wasm_bindgen]
pub fn client_create() -> ClientHandle {
    ClientHandle {
        inner: Some(ManagedClient::new(NullTransport)),
        preview_artifacts: PreviewSideBuffer::default(),
        preview_targets: HashMap::new(),
        cadquery_meshes: CadQuerySideBuffer::default(),
    }
}

#[wasm_bindgen]
pub fn client_create_with_timeouts(timeouts: JsValue) -> Result<ClientHandle, JsValue> {
    let parsed: ClientTimeouts = serde_wasm_bindgen::from_value(timeouts)
        .map_err(|err| JsValue::from_str(&format!("invalid timeouts: {err}")))?;
    Ok(ClientHandle {
        inner: Some(ManagedClient::with_timeouts(NullTransport, parsed)),
        preview_artifacts: PreviewSideBuffer::default(),
        preview_targets: HashMap::new(),
        cadquery_meshes: CadQuerySideBuffer::default(),
    })
}

#[wasm_bindgen]
pub fn client_begin_handshake(handle: &mut ClientHandle, params: JsValue) -> Result<(), JsValue> {
    let parsed: CapabilityHandshakeRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid handshake params: {err}")))?;
    handle
        .borrow_mut()?
        .begin_handshake(parsed)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_next_outbound(handle: &mut ClientHandle) -> Result<Option<Vec<u8>>, JsValue> {
    Ok(handle.borrow_mut()?.next_outbound())
}

#[wasm_bindgen]
pub fn client_receive_inbound(handle: &mut ClientHandle, bytes: &[u8]) -> Result<(), JsValue> {
    handle
        .borrow_mut()?
        .receive_inbound(bytes)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_mark_transport_closed(
    handle: &mut ClientHandle,
    reason: JsValue,
) -> Result<(), JsValue> {
    let parsed: TransportCloseReason = serde_wasm_bindgen::from_value(reason)
        .map_err(|err| JsValue::from_str(&format!("invalid close reason: {err}")))?;
    handle.preview_artifacts.clear();
    handle.preview_targets.clear();
    handle.cadquery_meshes.clear();
    handle.borrow_mut()?.mark_transport_closed(parsed);
    Ok(())
}

#[wasm_bindgen]
pub fn client_cancel(handle: &mut ClientHandle, request_id: u64) -> Result<u64, JsValue> {
    handle
        .borrow_mut()?
        .cancel(RequestId(request_id))
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_tick(handle: &mut ClientHandle, now_ms: u64) -> Result<(), JsValue> {
    handle.borrow_mut()?.tick(now_ms);
    Ok(())
}

#[wasm_bindgen]
pub fn client_destroy(handle: &mut ClientHandle) {
    handle.preview_artifacts.clear();
    handle.preview_targets.clear();
    handle.cadquery_meshes.clear();
    handle.inner.take();
}

#[wasm_bindgen]
pub fn client_drain_events(handle: &mut ClientHandle) -> Result<JsValue, JsValue> {
    let mut events = handle.borrow_mut()?.drain_events();
    for event in &mut events {
        match event {
            studio_common::ClientEvent::RequestSucceeded {
                payload: success @ CommandSuccess::CadQueryMesh(_),
                ..
            } => {
                handle.cadquery_meshes.insert_from_success(success);
            }
            studio_common::ClientEvent::RequestSucceeded {
                request_id,
                payload: CommandSuccess::PreviewReady(ready),
            } => {
                let target = handle.preview_targets.remove(request_id).or_else(|| {
                    handle
                        .borrow()
                        .ok()
                        .and_then(|client| {
                            client
                                .snapshot()
                                .preview_tasks
                                .into_iter()
                                .find(|task| task.request_id == *request_id)
                        })
                        .map(|task| task.target)
                });
                let Some(buffered) = BufferedPreview::take_from_artifact(&mut ready.artifact)
                else {
                    continue;
                };
                handle
                    .preview_artifacts
                    .insert(*request_id, target, buffered);
            }
            studio_common::ClientEvent::RequestSucceeded { request_id, .. }
            | studio_common::ClientEvent::RequestFailed { request_id, .. }
            | studio_common::ClientEvent::RequestTimedOut { request_id } => {
                handle.preview_targets.remove(request_id);
            }
            _ => {}
        }
    }
    serde_wasm_bindgen::to_value(&events)
        .map_err(|err| JsValue::from_str(&format!("drain_events serialize: {err}")))
}

#[wasm_bindgen]
pub fn client_take_cadquery_mesh(
    handle: &mut ClientHandle,
    result_id: &str,
) -> Result<Option<CadQueryMeshHandle>, JsValue> {
    handle.borrow()?;
    Ok(handle
        .cadquery_meshes
        .take(result_id)
        .map(|payload| CadQueryMeshHandle { payload }))
}

#[wasm_bindgen]
pub fn client_take_preview_mesh(
    handle: &mut ClientHandle,
    request_id: u64,
) -> Result<Option<MeshHandle>, JsValue> {
    handle.borrow()?;
    let request_id = RequestId(request_id);
    let Some(buffered) = handle.preview_artifacts.take(request_id) else {
        return Ok(None);
    };
    let decoded = buffered.decode().map_err(|message| {
        if let Ok(client) = handle.borrow_mut() {
            client.fail_preview_decode(request_id, message.clone());
        }
        JsValue::from_str(&message)
    })?;
    Ok(Some(MeshHandle { data: decoded }))
}

#[wasm_bindgen]
pub fn client_snapshot(handle: &ClientHandle) -> Result<JsValue, JsValue> {
    let snapshot = handle.borrow()?.snapshot();
    serde_wasm_bindgen::to_value(&snapshot)
        .map_err(|err| JsValue::from_str(&format!("snapshot serialize: {err}")))
}

#[wasm_bindgen]
pub fn client_dispatch_workspace_current(handle: &mut ClientHandle) -> Result<u64, JsValue> {
    handle
        .borrow_mut()?
        .dispatch_workspace_current()
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_workspace_list(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: WorkspaceListRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid workspace_list params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_workspace_list(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_preview_request(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: PreviewRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid preview_request params: {err}")))?;
    let target = parsed.source.clone();
    handle
        .borrow_mut()?
        .dispatch_preview_request(parsed)
        .map(|id| {
            handle.preview_targets.insert(id, target);
            id.0
        })
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_cadquery_execute(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: CadQueryExecuteRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid cadquery_execute params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_cadquery_execute(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_cadquery_preview(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: CadQueryPreviewRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid cadquery_preview params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_cadquery_preview(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_cadquery_result_get(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: CadQueryResultGetRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid cadquery_result_get params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_cadquery_result_get(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_chat_create(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ChatCreateRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid chat_create params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_chat_create(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_chat_list(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ChatListRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid chat_list params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_chat_list(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_chat_send(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ChatSendRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid chat_send params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_chat_send(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_chat_history(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ChatHistoryRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid chat_history params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_chat_history(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_chat_select(
    handle: &mut ClientHandle,
    session_id: &str,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ChatHistoryRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid chat_select params: {err}")))?;
    let sid = ChatSessionId(session_id.to_owned());
    handle
        .borrow_mut()?
        .dispatch_chat_select(sid, parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_chat_archive(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ChatArchiveRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid chat_archive params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_chat_archive(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_invoke(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentInvokeRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid agent_invoke params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_agent_invoke(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_cancel(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentCancelRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid agent_cancel params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_agent_cancel(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_start_turn(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentStartTurnRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid agent_start_turn params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_agent_start_turn(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_model_registry(handle: &mut ClientHandle) -> Result<u64, JsValue> {
    handle
        .borrow_mut()?
        .dispatch_agent_model_registry()
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_model_select(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentModelSelectRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid agent.model.select params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_agent_model_select(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_model_params_update(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentModelParamsUpdateRequest =
        serde_wasm_bindgen::from_value(params).map_err(|err| {
            JsValue::from_str(&format!("invalid agent.model.params.update params: {err}"))
        })?;
    handle
        .borrow_mut()?
        .dispatch_agent_model_params_update(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_plan_confirm(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentPlanConfirmRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid agent_plan_confirm params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_agent_plan_confirm(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_agent_plan_reject(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: AgentPlanRejectRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid agent_plan_reject params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_agent_plan_reject(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_selection_update(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: SelectionUpdateRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid selection_update params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_selection_update(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_file_read(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: FileReadRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid file_read params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_file_read(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_file_write_text(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: FileWriteTextRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid file_write_text params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_file_write_text(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_config_load(handle: &mut ClientHandle) -> Result<u64, JsValue> {
    handle
        .borrow_mut()?
        .dispatch_config_load()
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_config_save(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ConfigSaveRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid config_save params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_config_save(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_slicer_list(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: SlicerListRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid slicer_list params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_slicer_list(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_dispatch_export_run(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: ExportRunRequest = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid export_run params: {err}")))?;
    handle
        .borrow_mut()?
        .dispatch_export_run(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

#[wasm_bindgen]
pub fn client_subscribe_directory_watch(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<u64, JsValue> {
    let parsed: WatchParams = serde_wasm_bindgen::from_value(params)
        .map_err(|err| JsValue::from_str(&format!("invalid watch params: {err}")))?;
    handle
        .borrow_mut()?
        .subscribe_directory_watch(parsed)
        .map(|id| id.0)
        .map_err(client_error_to_js)
}

fn client_error_to_js(err: ClientError) -> JsValue {
    serde_wasm_bindgen::to_value(&err)
        .unwrap_or_else(|_| JsValue::from_str("client error serialize failed"))
}

fn invalid_handle_js() -> JsValue {
    serde_wasm_bindgen::to_value(&ClientError::InvalidHandle)
        .unwrap_or_else(|_| JsValue::from_str("invalid handle"))
}

#[derive(Default)]
struct CadQuerySideBuffer {
    entries: HashMap<String, CadQueryMeshPayload>,
    order: VecDeque<String>,
}

impl CadQuerySideBuffer {
    fn insert_from_success(&mut self, success: &mut CommandSuccess) {
        let CommandSuccess::CadQueryMesh(payload) = success else {
            return;
        };
        let ready = cadquery_ready_from_payload(payload);
        let buffered = std::mem::replace(payload, empty_cadquery_payload(&ready));
        self.insert(buffered);
        *success = CommandSuccess::CadQueryResultReady(ready);
    }

    fn insert(&mut self, payload: CadQueryMeshPayload) {
        let result_id = payload.result_id.clone();
        self.order.retain(|value| value != &result_id);
        self.order.push_back(result_id.clone());
        self.entries.insert(result_id, payload);
        while self.entries.len() > CADQUERY_SIDE_BUFFER_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn take(&mut self, result_id: &str) -> Option<CadQueryMeshPayload> {
        let value = self.entries.remove(result_id);
        if value.is_some() {
            self.order.retain(|id| id != result_id);
        }
        value
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

fn cadquery_ready_from_payload(payload: &CadQueryMeshPayload) -> CadQueryResultReady {
    CadQueryResultReady {
        result_id: payload.result_id.clone(),
        build_id: payload.build_id.clone(),
        part_count: payload.parts.len() as u32,
        face_count: payload
            .parts
            .iter()
            .map(|part| part.faces.len() as u32)
            .sum(),
        edge_count: payload
            .parts
            .iter()
            .map(|part| part.edges.len() as u32)
            .sum(),
        vertex_count: payload
            .parts
            .iter()
            .map(|part| part.vertices.len() as u32)
            .sum(),
        artifact_relation: payload.artifact_relation.clone(),
    }
}

fn empty_cadquery_payload(ready: &CadQueryResultReady) -> CadQueryMeshPayload {
    CadQueryMeshPayload {
        result_id: ready.result_id.clone(),
        build_id: ready.build_id.clone(),
        unit: app_server_protocol::PreviewUnit::Millimeter,
        root_ref_text: String::new(),
        root_object_kind: app_server_protocol::CadQueryObjectKind::Part,
        artifact_relation: ready.artifact_relation.clone(),
        parts: Vec::new(),
    }
}

#[derive(Default)]
struct PreviewSideBuffer {
    entries: HashMap<RequestId, BufferedPreview>,
    order: VecDeque<RequestId>,
    targets: HashMap<RequestId, PathHandle>,
}

impl PreviewSideBuffer {
    fn insert(
        &mut self,
        request_id: RequestId,
        target: Option<PathHandle>,
        artifact: BufferedPreview,
    ) {
        if let Some(target) = target {
            let stale_newer = self.targets.iter().find_map(|(id, existing)| {
                (id.0 > request_id.0 && *existing == target).then_some(*id)
            });
            if stale_newer.is_some() {
                return;
            }
            let stale_older = self.targets.iter().find_map(|(id, existing)| {
                (id.0 < request_id.0 && *existing == target).then_some(*id)
            });
            if let Some(stale) = stale_older {
                self.remove(stale);
            }
            self.targets.insert(request_id, target);
        }
        if !self.entries.contains_key(&request_id) {
            self.order.push_back(request_id);
        }
        self.entries.insert(request_id, artifact);
        while self.entries.len() > PREVIEW_SIDE_BUFFER_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
                self.targets.remove(&oldest);
            }
        }
    }

    fn take(&mut self, request_id: RequestId) -> Option<BufferedPreview> {
        let value = self.entries.remove(&request_id);
        if value.is_some() {
            self.targets.remove(&request_id);
            self.order.retain(|id| *id != request_id);
        }
        value
    }

    fn remove(&mut self, request_id: RequestId) {
        self.entries.remove(&request_id);
        self.targets.remove(&request_id);
        self.order.retain(|id| *id != request_id);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.targets.clear();
    }
}

enum BufferedPreview {
    ThreeMf { bytes: Vec<u8>, media_type: String },
    Stl { bytes: Vec<u8>, media_type: String },
}

impl BufferedPreview {
    fn take_from_artifact(artifact: &mut PreviewArtifact) -> Option<Self> {
        match artifact {
            PreviewArtifact::ThreeMf(payload) => Some(Self::ThreeMf {
                bytes: std::mem::take(&mut payload.bytes),
                media_type: payload.media_type.clone(),
            }),
            PreviewArtifact::Stl(payload) => Some(Self::Stl {
                bytes: std::mem::take(&mut payload.bytes),
                media_type: payload.media_type.clone(),
            }),
            _ => None,
        }
    }

    fn decode(self) -> Result<MeshData, String> {
        match self {
            Self::ThreeMf { bytes, media_type } => {
                let _ = media_type;
                let mut cursor = Cursor::new(bytes);
                load_3mf_from_reader(&mut cursor)
                    .map_err(|error| format!("3mf decode failed: {error}"))
            }
            Self::Stl { bytes, media_type } => {
                let _ = media_type;
                let mut cursor = Cursor::new(bytes);
                load_stl_from_reader(&mut cursor)
                    .map_err(|error| format!("stl decode failed: {error}"))
            }
        }
    }
}
