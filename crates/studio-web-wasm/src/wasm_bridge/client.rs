//! `ClientHandle` wasm_bindgen 包装与所有 `client_*` 导出。
//!
//! 所有业务状态机归 `studio_common::ManagedClient<NullTransport>`；本文件
//! 仅负责参数反序列化、错误转换和结果序列化。禁止在这里引入协议状态机。

use app_server_protocol::{
    CapabilityHandshakeRequest, ConfigSaveRequest, ExportRunRequest, FileReadRequest,
    FileWriteTextRequest, PreviewRequest, RequestId, SlicerListRequest, WorkspaceListRequest,
};
use studio_common::{
    ClientError, ClientTimeouts, ManagedClient, TransportCloseReason, WatchParams,
};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use super::transport::NullTransport;

#[wasm_bindgen]
pub struct ClientHandle {
    inner: Option<ManagedClient<NullTransport>>,
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
    }
}

#[wasm_bindgen]
pub fn client_create_with_timeouts(timeouts: JsValue) -> Result<ClientHandle, JsValue> {
    let parsed: ClientTimeouts = serde_wasm_bindgen::from_value(timeouts)
        .map_err(|err| JsValue::from_str(&format!("invalid timeouts: {err}")))?;
    Ok(ClientHandle {
        inner: Some(ManagedClient::with_timeouts(NullTransport, parsed)),
    })
}

#[wasm_bindgen]
pub fn client_begin_handshake(
    handle: &mut ClientHandle,
    params: JsValue,
) -> Result<(), JsValue> {
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
    handle.inner.take();
}

#[wasm_bindgen]
pub fn client_drain_events(handle: &mut ClientHandle) -> Result<JsValue, JsValue> {
    let events = handle.borrow_mut()?.drain_events();
    serde_wasm_bindgen::to_value(&events)
        .map_err(|err| JsValue::from_str(&format!("drain_events serialize: {err}")))
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
    handle
        .borrow_mut()?
        .dispatch_preview_request(parsed)
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
